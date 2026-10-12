//! Небольшой движок вёрстки формул LaTeX.
//!
//! Поддерживает основное подмножество TeX-математики: индексы и степени, дроби, корни,
//! большие операторы с пределами, `\left…\right`, матрицы и `cases`, `aligned`, акценты,
//! шрифты (`\mathbb`, `\mathbf`, `\mathcal`…), `\text{…}` (в том числе по-русски), цвета.
//! Результат — набор примитивов (текст, линии, заливки), которые рисуются через egui,
//! а значит, в итоге через Vulkan.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use eframe::egui::{self, Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

use crate::fonts;

// ============================================================================================
// Публичный интерфейс
// ============================================================================================

/// Сверстанная формула. Начало координат — левый край на базовой линии, ось Y вверх.
#[derive(Debug, Clone, Default)]
pub struct MathBox {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    items: Vec<Item>,
}

#[derive(Debug, Clone)]
enum Item {
    Text { x: f32, y: f32, text: String, font: FontId, color: Option<Color32> },
    Rect { x0: f32, y0: f32, x1: f32, y1: f32, color: Option<Color32> },
    Line { pts: Vec<Vec2>, width: f32, color: Option<Color32> },
    /// Полоса треугольников между двумя кривыми (скобки переменной толщины).
    Strip { outer: Vec<Vec2>, inner: Vec<Vec2>, color: Option<Color32> },
    /// Залитый выпуклый многоугольник (наконечники стрелок).
    Poly { pts: Vec<Vec2>, color: Option<Color32> },
    Circle { c: Vec2, r: f32, color: Option<Color32> },
    Frame { x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Option<Color32> },
}

thread_local! {
    static CACHE: RefCell<HashMap<u64, Arc<MathBox>>> = RefCell::new(HashMap::new());
}

/// Вёрстка с кэшированием: повторные вызовы с той же формулой почти бесплатны.
pub fn layout(ctx: &egui::Context, tex: &str, size: f32, display: bool) -> Arc<MathBox> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    tex.hash(&mut h);
    size.to_bits().hash(&mut h);
    display.hash(&mut h);
    fonts::has_math().hash(&mut h);
    let key = h.finish();
    if let Some(found) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return found;
    }
    let result = ctx.fonts_mut(|f| {
        let mut measure = |s: &str, font: &FontId| -> f32 {
            f.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE).size().x
        };
        let nodes = Parser::new(tex).parse_top();
        let mut l = Layouter { measure: &mut measure, base: size };
        let b = l.row(&nodes, size, display, None);
        Arc::new(MathBox { width: b.w, ascent: b.asc, descent: b.desc, items: b.items })
    });
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 2000 {
            c.clear();
        }
        c.insert(key, result.clone());
    });
    result
}

impl MathBox {
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }

    /// Рисует формулу: `origin` — точка на базовой линии у левого края.
    pub fn paint(&self, painter: &Painter, origin: Pos2, color: Color32) {
        let p = |v: Vec2| pos2(origin.x + v.x, origin.y - v.y);
        for item in &self.items {
            match item {
                Item::Text { x, y, text, font, color: c } => {
                    let c = c.unwrap_or(color);
                    let galley = painter.layout_no_wrap(text.clone(), font.clone(), c);
                    let baseline = galley
                        .rows
                        .first()
                        .and_then(|r| r.row.glyphs.first().map(|g| r.pos.y + g.pos.y))
                        .unwrap_or(font.size * 0.8);
                    painter.galley(pos2(origin.x + x, origin.y - y - baseline), galley, c);
                }
                Item::Rect { x0, y0, x1, y1, color: c } => {
                    // Тонкие линейки (дроби, корни) выравниваем по пиксельной сетке, чтобы не мылились.
                    let ppp = painter.pixels_per_point();
                    let snap = |v: f32| (v * ppp).round() / ppp;
                    let mut r = Rect::from_min_max(p(vec2(*x0, *y1)), p(vec2(*x1, *y0)));
                    let h = (r.height() * ppp).round().max(1.0) / ppp;
                    r.min.y = snap(r.min.y);
                    r.max.y = r.min.y + h;
                    painter.rect_filled(r, 0.0, c.unwrap_or(color));
                }
                Item::Line { pts, width, color: c } => {
                    let pts: Vec<Pos2> = pts.iter().map(|v| p(*v)).collect();
                    painter.add(Shape::line(pts, Stroke::new(width.max(1.05), c.unwrap_or(color))));
                }
                Item::Poly { pts, color: c } => {
                    let pts: Vec<Pos2> = pts.iter().map(|v| p(*v)).collect();
                    painter.add(Shape::convex_polygon(pts, c.unwrap_or(color), Stroke::NONE));
                }
                Item::Strip { outer, inner, color: c } => {
                    // Скобка переменной толщины: средняя линия с меняющейся шириной.
                    // Каждый отрезок и стык сглаживаются egui (в отличие от «сырой» сетки треугольников).
                    let c = c.unwrap_or(color);
                    let centers: Vec<(Pos2, f32)> =
                        outer.iter().zip(inner).map(|(o, i)| (p((*o + *i) * 0.5), (*o - *i).length().max(1.0))).collect();
                    for w in centers.windows(2) {
                        let (a, wa) = w[0];
                        let (b, wb) = w[1];
                        painter.line_segment([a, b], Stroke::new((wa + wb) * 0.5, c));
                    }
                    for &(pt, w) in centers.iter().skip(1).take(centers.len().saturating_sub(2)) {
                        painter.circle_filled(pt, w * 0.5, c);
                    }
                }
                Item::Circle { c: center, r, color: c } => {
                    painter.circle_filled(p(*center), *r, c.unwrap_or(color));
                }
                Item::Frame { x0, y0, x1, y1, width, color: c } => {
                    let r = Rect::from_min_max(p(vec2(*x0, *y1)), p(vec2(*x1, *y0)));
                    painter.rect_stroke(r, 2.0, Stroke::new(*width, c.unwrap_or(color)), egui::StrokeKind::Middle);
                }
            }
        }
    }
}

/// Упрощённая текстовая запись формулы для коротких фрагментов (обратные ссылки, подсказки):
/// `\sum_{i=1}^{n} x^2` → `∑ᵢ₌₁ⁿ x²`.
pub fn to_plain(tex: &str) -> String {
    let sup = |c: char| -> Option<char> {
        Some(match c {
            '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴', '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸',
            '9' => '⁹', '+' => '⁺', '-' | '−' => '⁻', '=' => '⁼', '(' => '⁽', ')' => '⁾', 'n' => 'ⁿ', 'i' => 'ⁱ', 'x' => 'ˣ',
            'k' => 'ᵏ', 'm' => 'ᵐ', 'T' => 'ᵀ', '′' => '′',
            _ => return None,
        })
    };
    let sub = |c: char| -> Option<char> {
        Some(match c {
            '0' => '₀', '1' => '₁', '2' => '₂', '3' => '₃', '4' => '₄', '5' => '₅', '6' => '₆', '7' => '₇', '8' => '₈',
            '9' => '₉', '+' => '₊', '-' | '−' => '₋', '=' => '₌', '(' => '₍', ')' => '₎', 'i' => 'ᵢ', 'j' => 'ⱼ', 'k' => 'ₖ',
            'n' => 'ₙ', 'x' => 'ₓ', 'm' => 'ₘ', 't' => 'ₜ', 'a' => 'ₐ', 'e' => 'ₑ',
            _ => return None,
        })
    };
    let toks = tokenize(tex);
    let mut out = String::new();
    let mut i = 0;
    // Текст аргумента команды или группы, уже переведённый в простую запись.
    fn group_text(toks: &[Tok], i: &mut usize) -> String {
        let mut depth = 0;
        let start = *i;
        while *i < toks.len() {
            match toks[*i] {
                Tok::Open => depth += 1,
                Tok::Close => {
                    depth -= 1;
                    if depth == 0 {
                        *i += 1;
                        break;
                    }
                }
                _ if depth == 0 => {
                    *i += 1;
                    break;
                }
                _ => {}
            }
            *i += 1;
        }
        let inner: Vec<Tok> = toks[start..*i].iter().filter(|t| !matches!(t, Tok::Open | Tok::Close)).cloned().collect();
        plain_tokens(&inner)
    }
    fn plain_tokens(toks: &[Tok]) -> String {
        let mut s = String::new();
        for t in toks {
            match t {
                Tok::Char(c) => s.push(*c),
                Tok::Space => s.push(' '),
                Tok::Prime => s.push('′'),
                Tok::Cmd(name) => {
                    if let Some((sym, _)) = symbol(name) {
                        s.push_str(sym);
                    } else if let Some((sym, _)) = big_operator(name) {
                        s.push_str(sym);
                    } else if let Some((op, _)) = named_operator(name) {
                        s.push_str(op);
                    } else if name == "," || name == ";" || name == "quad" {
                        s.push(' ');
                    }
                }
                _ => {}
            }
        }
        s
    }
    while i < toks.len() {
        match &toks[i] {
            Tok::Sup | Tok::Sub => {
                let is_sup = toks[i] == Tok::Sup;
                i += 1;
                let text = group_text(&toks, &mut i);
                let map = if is_sup { &sup as &dyn Fn(char) -> Option<char> } else { &sub };
                match text.chars().map(map).collect::<Option<String>>() {
                    Some(mapped) => out.push_str(&mapped),
                    None => {
                        out.push(if is_sup { '^' } else { '_' });
                        out.push_str(&text);
                    }
                }
            }
            Tok::Cmd(name) if name == "frac" || name == "dfrac" || name == "tfrac" => {
                i += 1;
                let num = group_text(&toks, &mut i);
                let den = group_text(&toks, &mut i);
                out.push_str(&format!("{num}/{den}"));
            }
            Tok::Cmd(name) if name == "sqrt" => {
                i += 1;
                let body = group_text(&toks, &mut i);
                out.push_str(&format!("√{body}"));
            }
            Tok::Open | Tok::Close => i += 1,
            other => {
                out.push_str(&plain_tokens(std::slice::from_ref(other)));
                i += 1;
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ============================================================================================
// Лексер и парсер
// ============================================================================================

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Cmd(String),
    Char(char),
    Space,
    Open,
    Close,
    Sup,
    Sub,
    Amp,
    NewRow,
    Prime,
}

fn tokenize(s: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    break;
                }
                if chars[i].is_ascii_alphabetic() {
                    let start = i;
                    while i < chars.len() && chars[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let name: String = chars[start..i].iter().collect();
                    // Звёздочка у команд вроде \operatorname* — часть имени.
                    if i < chars.len() && chars[i] == '*' {
                        out.push(Tok::Cmd(format!("{name}*")));
                        i += 1;
                    } else {
                        out.push(Tok::Cmd(name));
                    }
                    continue;
                }
                if chars[i] == '\\' {
                    out.push(Tok::NewRow);
                } else {
                    out.push(Tok::Cmd(chars[i].to_string()));
                }
            }
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '^' => out.push(Tok::Sup),
            '_' => out.push(Tok::Sub),
            '&' => out.push(Tok::Amp),
            '\'' => out.push(Tok::Prime),
            '~' => out.push(Tok::Cmd("nbsp".into())),
            '%' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            c if c.is_whitespace() => {
                if out.last() != Some(&Tok::Space) {
                    out.push(Tok::Space);
                }
            }
            c => out.push(Tok::Char(c)),
        }
        i += 1;
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MFont {
    Italic,
    Upright,
    Text,
    Bold,
    BoldText,
    ItalicText,
    BB,
    Cal,
    Frak,
    Sans,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Limits {
    Auto,
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Env {
    Matrix,
    PMatrix,
    BMatrix,
    BbMatrix,
    VMatrix,
    VvMatrix,
    Cases,
    Aligned,
    Gathered,
    Array,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccentKind {
    Hat,
    WideHat,
    Bar,
    Overline,
    Underline,
    Vec,
    Dot,
    DDot,
    Tilde,
    WideTilde,
    OverBrace,
    UnderBrace,
    Not,
}

#[derive(Debug, Clone)]
enum Node {
    Atom { text: String, kind: Kind, font: MFont },
    Row(Vec<Node>),
    Scripts { base: Box<Node>, sup: Option<Box<Node>>, sub: Option<Box<Node>> },
    Frac { num: Box<Node>, den: Box<Node>, bar: bool, display: Option<bool> },
    Binom { top: Box<Node>, bottom: Box<Node>, display: Option<bool> },
    Sqrt { index: Option<Box<Node>>, body: Box<Node> },
    Op { text: String, limits: Limits, big: bool },
    Delim { left: String, right: String, parts: Vec<Vec<Node>>, middles: Vec<String> },
    BigDelim { delim: String, scale: f32, kind: Kind },
    Accent { kind: AccentKind, body: Box<Node> },
    OverUnder { base: Box<Node>, script: Box<Node>, over: bool, kind: Kind },
    Matrix { rows: Vec<Vec<Node>>, env: Env },
    Space(f32),
    Color(Color32, Box<Node>),
    Boxed(Box<Node>),
    Phantom { body: Box<Node>, h: bool, v: bool },
    StyleSwitch(bool),
    Error(String),
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Stop {
    Eof,
    Close,
    Right,
    /// Внутри матрицы: строка/ячейка заканчиваются на `&`, `\\` или `\end`.
    Cell,
}

impl Parser {
    fn new(s: &str) -> Self {
        Self { toks: tokenize(s), i: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn skip_spaces(&mut self) {
        while self.peek() == Some(&Tok::Space) {
            self.i += 1;
        }
    }

    /// Разбор всей формулы. Если на верхнем уровне есть `\\` или `&`, это многострочная формула.
    fn parse_top(&mut self) -> Vec<Node> {
        let has_rows = self.toks.iter().any(|t| matches!(t, Tok::NewRow | Tok::Amp));
        if has_rows {
            let has_amp = self.toks.iter().any(|t| *t == Tok::Amp);
            let rows = self.parse_cells(MFont::Italic);
            let env = if has_amp { Env::Aligned } else { Env::Gathered };
            return vec![Node::Matrix { rows, env }];
        }
        self.parse_row(Stop::Eof, MFont::Italic)
    }

    fn parse_cells(&mut self, font: MFont) -> Vec<Vec<Node>> {
        let mut rows: Vec<Vec<Node>> = Vec::new();
        let mut row: Vec<Node> = Vec::new();
        loop {
            let cell = self.parse_row(Stop::Cell, font);
            row.push(Node::Row(cell));
            match self.peek() {
                Some(Tok::Amp) => {
                    self.i += 1;
                }
                Some(Tok::NewRow) => {
                    self.i += 1;
                    rows.push(std::mem::take(&mut row));
                }
                Some(Tok::Cmd(c)) if c == "end" => {
                    self.i += 1;
                    self.read_group_text();
                    break;
                }
                _ => break,
            }
        }
        // Пустая последняя строка после завершающего `\\` не нужна.
        let empty = row.iter().all(|c| matches!(c, Node::Row(v) if v.is_empty()));
        if !(empty && !rows.is_empty()) {
            rows.push(row);
        }
        rows
    }

    fn parse_row(&mut self, stop: Stop, font: MFont) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        loop {
            let Some(tok) = self.peek().cloned() else { break };
            match (&tok, stop) {
                (Tok::Close, Stop::Close) => break,
                (Tok::Close, _) => {
                    self.i += 1;
                    continue;
                }
                (Tok::Amp | Tok::NewRow, Stop::Cell) => break,
                (Tok::Amp | Tok::NewRow, _) => {
                    self.i += 1;
                    continue;
                }
                (Tok::Cmd(c), Stop::Right) if c == "right" || c == "middle" => break,
                (Tok::Cmd(c), Stop::Cell) if c == "end" => break,
                _ => {}
            }
            if tok == Tok::Space {
                self.i += 1;
                continue;
            }
            if let Tok::Cmd(c) = &tok {
                // \color{красный} без группы действует до конца текущей группы.
                if c == "color" {
                    self.i += 1;
                    let color = parse_color(&self.read_group_text());
                    self.skip_spaces();
                    if self.peek() == Some(&Tok::Open) {
                        let body = self.parse_arg(font);
                        out.push(Node::Color(color, Box::new(body)));
                    } else {
                        let rest = self.parse_row(stop, font);
                        out.push(Node::Color(color, Box::new(Node::Row(rest))));
                        break;
                    }
                    continue;
                }
                if c == "limits" || c == "nolimits" {
                    self.i += 1;
                    if let Some(Node::Op { limits, .. }) = out.last_mut() {
                        *limits = if c == "limits" { Limits::Yes } else { Limits::No };
                    }
                    continue;
                }
                if c == "over" || c == "choose" {
                    // Старый синтаксис {a \over b}.
                    self.i += 1;
                    let num = Node::Row(std::mem::take(&mut out));
                    let den = Node::Row(self.parse_row(stop, font));
                    let bin = c == "choose";
                    out.push(if bin {
                        Node::Binom { top: Box::new(num), bottom: Box::new(den), display: None }
                    } else {
                        Node::Frac { num: Box::new(num), den: Box::new(den), bar: true, display: None }
                    });
                    break;
                }
            }
            let Some(atom) = self.parse_atom(font) else { continue };
            let atom = self.parse_scripts(atom, font);
            out.push(atom);
        }
        out
    }

    fn parse_scripts(&mut self, mut base: Node, font: MFont) -> Node {
        let mut sup: Option<Node> = None;
        let mut sub: Option<Node> = None;
        let mut primes = 0;
        loop {
            self.skip_spaces();
            match self.peek() {
                Some(Tok::Sup) => {
                    self.i += 1;
                    let s = self.parse_arg(font);
                    sup = Some(match (sup.take(), primes) {
                        (Some(prev), _) => Node::Row(vec![prev, s]),
                        (None, 0) => s,
                        (None, n) => Node::Row(vec![prime_atoms(n), s]),
                    });
                    primes = 0;
                }
                Some(Tok::Sub) => {
                    self.i += 1;
                    sub = Some(self.parse_arg(font));
                }
                Some(Tok::Prime) => {
                    self.i += 1;
                    primes += 1;
                }
                Some(Tok::Cmd(c)) if (c == "limits" || c == "nolimits") && sup.is_none() && sub.is_none() => {
                    let yes = c == "limits";
                    self.i += 1;
                    if let Node::Op { limits, .. } = &mut base {
                        *limits = if yes { Limits::Yes } else { Limits::No };
                    }
                }
                _ => break,
            }
        }
        if primes > 0 {
            sup = Some(match sup.take() {
                Some(s) => Node::Row(vec![prime_atoms(primes), s]),
                None => prime_atoms(primes),
            });
        }
        if sup.is_none() && sub.is_none() {
            return base;
        }
        Node::Scripts { base: Box::new(base), sup: sup.map(Box::new), sub: sub.map(Box::new) }
    }

    /// Обязательный аргумент: группа `{…}` или одиночный элемент.
    fn parse_arg(&mut self, font: MFont) -> Node {
        self.skip_spaces();
        match self.peek() {
            Some(Tok::Open) => {
                self.i += 1;
                let row = self.parse_row(Stop::Close, font);
                if self.peek() == Some(&Tok::Close) {
                    self.i += 1;
                }
                Node::Row(row)
            }
            Some(_) => self.parse_atom(font).unwrap_or(Node::Row(Vec::new())),
            None => Node::Row(Vec::new()),
        }
    }

    /// Необязательный аргумент `[…]`.
    fn parse_optional(&mut self, font: MFont) -> Option<Node> {
        self.skip_spaces();
        if self.peek() != Some(&Tok::Char('[')) {
            return None;
        }
        self.i += 1;
        let mut out = Vec::new();
        while let Some(t) = self.peek() {
            if *t == Tok::Char(']') {
                self.i += 1;
                break;
            }
            if *t == Tok::Space {
                self.i += 1;
                continue;
            }
            if let Some(a) = self.parse_atom(font) {
                let a = self.parse_scripts(a, font);
                out.push(a);
            }
        }
        Some(Node::Row(out))
    }

    /// Сырой текст группы `{…}` (для \text, имён окружений, цветов).
    fn read_group_text(&mut self) -> String {
        self.skip_spaces();
        if self.peek() != Some(&Tok::Open) {
            // Одиночный символ без скобок.
            return match self.next() {
                Some(Tok::Char(c)) => c.to_string(),
                Some(Tok::Cmd(c)) => format!("\\{c}"),
                _ => String::new(),
            };
        }
        self.i += 1;
        let mut depth = 1;
        let mut s = String::new();
        while let Some(t) = self.next() {
            match t {
                Tok::Open => {
                    depth += 1;
                    s.push('{');
                }
                Tok::Close => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    s.push('}');
                }
                Tok::Char(c) => s.push(c),
                Tok::Space => s.push(' '),
                Tok::Cmd(c) => match c.as_str() {
                    "{" | "}" | "%" | "$" | "&" | "#" | "_" => s.push_str(&c),
                    " " | "," | "nbsp" => s.push(' '),
                    other => {
                        if let Some((sym, _)) = symbol(other) {
                            s.push_str(sym);
                        } else {
                            s.push('\\');
                            s.push_str(other);
                        }
                    }
                },
                Tok::Sup => s.push('^'),
                Tok::Sub => s.push('_'),
                Tok::Amp => s.push('&'),
                Tok::Prime => s.push('\''),
                Tok::NewRow => s.push(' '),
            }
        }
        s
    }

    fn parse_atom(&mut self, font: MFont) -> Option<Node> {
        let tok = self.next()?;
        Some(match tok {
            Tok::Open => {
                let row = self.parse_row(Stop::Close, font);
                if self.peek() == Some(&Tok::Close) {
                    self.i += 1;
                }
                Node::Row(row)
            }
            Tok::Char(c) => char_atom(c, font),
            Tok::Prime => prime_atoms(1),
            Tok::Cmd(name) => self.command(&name, font),
            Tok::Space | Tok::Close | Tok::Amp | Tok::NewRow => return None,
            Tok::Sup | Tok::Sub => {
                // Степень без основания: x^ в начале — основание пустое.
                self.i -= 1;
                Node::Row(Vec::new())
            }
        })
    }

    fn command(&mut self, name: &str, font: MFont) -> Node {
        match name {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let num = self.parse_arg(font);
                let den = self.parse_arg(font);
                let display = match name {
                    "dfrac" | "cfrac" => Some(true),
                    "tfrac" => Some(false),
                    _ => None,
                };
                Node::Frac { num: Box::new(num), den: Box::new(den), bar: true, display }
            }
            "binom" | "dbinom" | "tbinom" => {
                let top = self.parse_arg(font);
                let bottom = self.parse_arg(font);
                let display = match name {
                    "dbinom" => Some(true),
                    "tbinom" => Some(false),
                    _ => None,
                };
                Node::Binom { top: Box::new(top), bottom: Box::new(bottom), display }
            }
            "sqrt" => {
                let index = self.parse_optional(font).map(Box::new);
                let body = self.parse_arg(font);
                Node::Sqrt { index, body: Box::new(body) }
            }
            "left" => {
                let left = self.read_delim();
                let mut parts = vec![self.parse_row(Stop::Right, font)];
                let mut middles = Vec::new();
                let mut right = ".".to_owned();
                while let Some(Tok::Cmd(c)) = self.peek().cloned() {
                    self.i += 1;
                    if c == "middle" {
                        middles.push(self.read_delim());
                        parts.push(self.parse_row(Stop::Right, font));
                    } else {
                        right = self.read_delim();
                        break;
                    }
                }
                Node::Delim { left, right, parts, middles }
            }
            "right" | "middle" => {
                self.read_delim();
                Node::Row(Vec::new())
            }
            "big" | "Big" | "bigg" | "Bigg" | "bigl" | "Bigl" | "biggl" | "Biggl" | "bigr" | "Bigr" | "biggr"
            | "Biggr" | "bigm" | "Bigm" | "biggm" | "Biggm" => {
                let delim = self.read_delim();
                let scale = if name.starts_with("Bigg") {
                    3.0
                } else if name.starts_with("bigg") {
                    2.4
                } else if name.starts_with("Big") {
                    1.8
                } else {
                    1.2
                };
                let kind = if name.ends_with('l') {
                    Kind::Open
                } else if name.ends_with('r') {
                    Kind::Close
                } else if name.ends_with('m') {
                    Kind::Rel
                } else {
                    delim_kind(&delim)
                };
                Node::BigDelim { delim, scale, kind }
            }
            "begin" => {
                let env_name = self.read_group_text();
                let env = match env_name.trim_end_matches('*') {
                    "pmatrix" => Env::PMatrix,
                    "bmatrix" => Env::BMatrix,
                    "Bmatrix" => Env::BbMatrix,
                    "vmatrix" => Env::VMatrix,
                    "Vmatrix" => Env::VvMatrix,
                    "cases" | "dcases" => Env::Cases,
                    "aligned" | "align" | "alignat" | "split" | "eqnarray" => Env::Aligned,
                    "gathered" | "gather" | "multline" | "equation" => Env::Gathered,
                    "array" | "tabular" => {
                        // Спецификацию колонок {cc|c} пропускаем.
                        self.read_group_text();
                        Env::Array
                    }
                    _ => Env::Matrix,
                };
                let rows = self.parse_cells(font);
                Node::Matrix { rows, env }
            }
            "end" => {
                self.read_group_text();
                Node::Row(Vec::new())
            }
            "text" | "textrm" | "textnormal" | "mbox" | "hbox" | "textup" => {
                Node::Atom { text: self.read_group_text(), kind: Kind::Ord, font: MFont::Text }
            }
            "textbf" => Node::Atom { text: self.read_group_text(), kind: Kind::Ord, font: MFont::BoldText },
            "textit" | "emph" => Node::Atom { text: self.read_group_text(), kind: Kind::Ord, font: MFont::ItalicText },
            "texttt" => Node::Atom { text: self.read_group_text(), kind: Kind::Ord, font: MFont::Mono },
            "operatorname" | "operatorname*" => {
                let text = self.read_group_text();
                let limits = if name.ends_with('*') { Limits::Auto } else { Limits::No };
                Node::Op { text, limits, big: false }
            }
            "mathrm" | "rm" | "mathup" => self.parse_arg(MFont::Upright),
            "mathit" | "it" => self.parse_arg(MFont::Italic),
            "mathbf" | "bf" | "boldsymbol" | "bm" | "mathbfit" => self.parse_arg(MFont::Bold),
            "mathbb" | "Bbb" => self.parse_arg(MFont::BB),
            "mathcal" | "mathscr" => self.parse_arg(MFont::Cal),
            "mathfrak" => self.parse_arg(MFont::Frak),
            "mathsf" => self.parse_arg(MFont::Sans),
            "mathtt" => self.parse_arg(MFont::Mono),
            "displaystyle" => Node::StyleSwitch(true),
            "textstyle" | "scriptstyle" | "scriptscriptstyle" => Node::StyleSwitch(false),
            "textcolor" | "colorbox" => {
                let color = parse_color(&self.read_group_text());
                let body = self.parse_arg(font);
                Node::Color(color, Box::new(body))
            }
            "boxed" | "fbox" => Node::Boxed(Box::new(self.parse_arg(font))),
            "phantom" => Node::Phantom { body: Box::new(self.parse_arg(font)), h: true, v: true },
            "hphantom" => Node::Phantom { body: Box::new(self.parse_arg(font)), h: true, v: false },
            "vphantom" => Node::Phantom { body: Box::new(self.parse_arg(font)), h: false, v: true },
            "overset" | "stackrel" | "underset" => {
                let script = self.parse_arg(font);
                let base = self.parse_arg(font);
                let kind = if name == "stackrel" { Kind::Rel } else { node_kind(&base) };
                Node::OverUnder { base: Box::new(base), script: Box::new(script), over: name != "underset", kind }
            }
            "hat" | "widehat" | "bar" | "overline" | "underline" | "vec" | "overrightarrow" | "dot" | "ddot"
            | "tilde" | "widetilde" | "overbrace" | "underbrace" | "check" | "breve" | "acute" | "grave" => {
                let kind = match name {
                    "hat" | "check" | "breve" | "acute" | "grave" => AccentKind::Hat,
                    "widehat" => AccentKind::WideHat,
                    "bar" => AccentKind::Bar,
                    "overline" => AccentKind::Overline,
                    "underline" => AccentKind::Underline,
                    "vec" | "overrightarrow" => AccentKind::Vec,
                    "dot" => AccentKind::Dot,
                    "ddot" => AccentKind::DDot,
                    "tilde" => AccentKind::Tilde,
                    "widetilde" => AccentKind::WideTilde,
                    "overbrace" => AccentKind::OverBrace,
                    _ => AccentKind::UnderBrace,
                };
                let body = self.parse_arg(font);
                Node::Accent { kind, body: Box::new(body) }
            }
            "not" => {
                let body = self.parse_arg(font);
                if let Node::Atom { text, .. } = &body {
                    let negated = match text.as_str() {
                        "=" => Some("≠"),
                        "∈" => Some("∉"),
                        "⊂" => Some("⊄"),
                        "⊆" => Some("⊈"),
                        "≡" => Some("≢"),
                        "∼" => Some("≁"),
                        "<" => Some("≮"),
                        ">" => Some("≯"),
                        _ => None,
                    };
                    if let Some(n) = negated {
                        return Node::Atom { text: n.into(), kind: Kind::Rel, font: MFont::Upright };
                    }
                }
                Node::Accent { kind: AccentKind::Not, body: Box::new(body) }
            }
            "bmod" | "mod" => Node::Atom { text: "mod".into(), kind: Kind::Bin, font: MFont::Upright },
            "pmod" => {
                let arg = self.parse_arg(font);
                Node::Row(vec![
                    Node::Space(0.6),
                    Node::Atom { text: "(mod".into(), kind: Kind::Ord, font: MFont::Upright },
                    Node::Space(0.3),
                    arg,
                    Node::Atom { text: ")".into(), kind: Kind::Ord, font: MFont::Upright },
                ])
            }
            "," | "thinspace" => Node::Space(3.0 / 18.0),
            ":" | ">" | "medspace" => Node::Space(4.0 / 18.0),
            ";" | "thickspace" => Node::Space(5.0 / 18.0),
            "!" | "negthinspace" => Node::Space(-3.0 / 18.0),
            " " | "nbsp" => Node::Space(0.33),
            "quad" => Node::Space(1.0),
            "qquad" => Node::Space(2.0),
            "hspace" | "kern" | "mkern" | "mspace" => {
                let t = self.read_group_text();
                Node::Space(parse_length(&t))
            }
            "{" => Node::Atom { text: "{".into(), kind: Kind::Open, font: MFont::Upright },
            "}" => Node::Atom { text: "}".into(), kind: Kind::Close, font: MFont::Upright },
            "|" => Node::Atom { text: "‖".into(), kind: Kind::Ord, font: MFont::Upright },
            "%" | "$" | "#" | "&" | "_" => Node::Atom { text: name.into(), kind: Kind::Ord, font: MFont::Upright },
            other => {
                if let Some(op) = named_operator(other) {
                    return Node::Op { text: op.0.into(), limits: if op.1 { Limits::Auto } else { Limits::No }, big: false };
                }
                if let Some((sym, limits)) = big_operator(other) {
                    return Node::Op { text: sym.into(), limits, big: true };
                }
                if let Some((sym, kind)) = symbol(other) {
                    let f = if is_lower_greek(sym) && font != MFont::Upright && font != MFont::Bold {
                        MFont::Italic
                    } else {
                        MFont::Upright
                    };
                    return Node::Atom { text: sym.into(), kind, font: f };
                }
                Node::Error(format!("\\{other}"))
            }
        }
    }

    /// Читает разделитель после \left, \right, \big и т.п.
    fn read_delim(&mut self) -> String {
        self.skip_spaces();
        match self.next() {
            Some(Tok::Char(c)) => match c {
                '<' => "⟨".into(),
                '>' => "⟩".into(),
                c => c.to_string(),
            },
            Some(Tok::Cmd(c)) => match c.as_str() {
                "{" | "lbrace" => "{".into(),
                "}" | "rbrace" => "}".into(),
                "|" | "Vert" | "lVert" | "rVert" => "‖".into(),
                "vert" | "lvert" | "rvert" | "mid" => "|".into(),
                "langle" => "⟨".into(),
                "rangle" => "⟩".into(),
                "lfloor" => "⌊".into(),
                "rfloor" => "⌋".into(),
                "lceil" => "⌈".into(),
                "rceil" => "⌉".into(),
                "backslash" => "\\".into(),
                "uparrow" => "↑".into(),
                "downarrow" => "↓".into(),
                _ => ".".into(),
            },
            _ => ".".into(),
        }
    }
}

fn prime_atoms(n: usize) -> Node {
    let text = match n {
        1 => "′",
        2 => "″",
        3 => "‴",
        _ => "⁗",
    };
    Node::Atom { text: text.into(), kind: Kind::Ord, font: MFont::Upright }
}

fn is_lower_greek(s: &str) -> bool {
    s.chars().next().is_some_and(|c| ('α'..='ω').contains(&c) || "ϵϑϕϱϖϰ".contains(c))
}

fn char_atom(c: char, font: MFont) -> Node {
    let (text, kind, f): (String, Kind, MFont) = match c {
        'a'..='z' | 'A'..='Z' => (c.to_string(), Kind::Ord, font),
        '0'..='9' | '.' => {
            let f = if matches!(font, MFont::Bold | MFont::Sans | MFont::Mono | MFont::BB) { font } else { MFont::Upright };
            (c.to_string(), Kind::Ord, f)
        }
        '+' => ("+".into(), Kind::Bin, MFont::Upright),
        '-' => ("−".into(), Kind::Bin, MFont::Upright),
        '*' => ("∗".into(), Kind::Bin, MFont::Upright),
        '/' => ("/".into(), Kind::Ord, MFont::Upright),
        '=' | '<' | '>' | ':' => (c.to_string(), Kind::Rel, MFont::Upright),
        ',' | ';' => (c.to_string(), Kind::Punct, MFont::Upright),
        '(' | '[' => (c.to_string(), Kind::Open, MFont::Upright),
        ')' | ']' => (c.to_string(), Kind::Close, MFont::Upright),
        '!' | '?' => (c.to_string(), Kind::Close, MFont::Upright),
        '|' => ("|".into(), Kind::Ord, MFont::Upright),
        c if ('α'..='ω').contains(&c) => (c.to_string(), Kind::Ord, MFont::Italic),
        // Кириллицу и прочий текст печатаем прямым текстовым шрифтом.
        c if c.is_alphabetic() => (c.to_string(), Kind::Ord, MFont::Text),
        c => {
            let kind = match c {
                '≤' | '≥' | '≠' | '≈' | '≡' | '→' | '←' | '⇒' | '⇔' | '∈' | '⊂' | '⊆' | '∼' => Kind::Rel,
                '±' | '×' | '÷' | '·' | '∪' | '∩' => Kind::Bin,
                _ => Kind::Ord,
            };
            (c.to_string(), kind, MFont::Upright)
        }
    };
    Node::Atom { text, kind, font: f }
}

fn delim_kind(d: &str) -> Kind {
    match d {
        "(" | "[" | "{" | "⟨" | "⌊" | "⌈" => Kind::Open,
        ")" | "]" | "}" | "⟩" | "⌋" | "⌉" => Kind::Close,
        _ => Kind::Ord,
    }
}

fn node_kind(n: &Node) -> Kind {
    match n {
        Node::Atom { kind, .. } => *kind,
        Node::Op { .. } => Kind::Op,
        Node::Scripts { base, .. } => node_kind(base),
        Node::Row(v) if v.len() == 1 => node_kind(&v[0]),
        Node::OverUnder { kind, .. } => *kind,
        Node::Accent { kind: AccentKind::Not, body } => node_kind(body),
        Node::Delim { .. } => Kind::Inner,
        Node::BigDelim { kind, .. } => *kind,
        Node::Frac { .. } | Node::Binom { .. } => Kind::Inner,
        Node::Color(_, b) => node_kind(b),
        _ => Kind::Ord,
    }
}

fn parse_length(s: &str) -> f32 {
    let s = s.trim();
    let num: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    let v: f32 = num.parse().unwrap_or(0.0);
    let unit = s[num.len()..].trim();
    match unit {
        "mu" => v / 18.0,
        "pt" => v / 10.0,
        "ex" => v * 0.43,
        _ => v,
    }
}

fn parse_color(name: &str) -> Color32 {
    let n = name.trim();
    if let Some(hex) = n.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(v) = u32::from_str_radix(hex, 16) {
                return Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
            }
        }
    }
    match n.to_lowercase().as_str() {
        "red" => Color32::from_rgb(239, 68, 68),
        "green" => Color32::from_rgb(34, 197, 94),
        "blue" => Color32::from_rgb(59, 130, 246),
        "orange" => Color32::from_rgb(249, 115, 22),
        "purple" | "violet" => Color32::from_rgb(168, 85, 247),
        "magenta" => Color32::from_rgb(217, 70, 239),
        "pink" => Color32::from_rgb(236, 72, 153),
        "cyan" => Color32::from_rgb(6, 182, 212),
        "teal" => Color32::from_rgb(20, 184, 166),
        "yellow" => Color32::from_rgb(234, 179, 8),
        "brown" => Color32::from_rgb(161, 98, 7),
        "gray" | "grey" => Color32::from_rgb(148, 163, 184),
        "black" => Color32::from_rgb(20, 20, 20),
        "white" => Color32::from_rgb(250, 250, 250),
        _ => Color32::from_rgb(239, 68, 68),
    }
}

/// Имена операторов вида \sin. Второе значение — ставятся ли пределы снизу в выключной формуле.
fn named_operator(name: &str) -> Option<(&'static str, bool)> {
    const OPS: &[(&str, bool)] = &[
        ("sin", false), ("cos", false), ("tan", false), ("cot", false), ("sec", false), ("csc", false),
        ("arcsin", false), ("arccos", false), ("arctan", false), ("sinh", false), ("cosh", false),
        ("tanh", false), ("coth", false), ("log", false), ("ln", false), ("lg", false), ("exp", false),
        ("dim", false), ("ker", false), ("deg", false), ("hom", false), ("arg", false), ("tg", false),
        ("ctg", false), ("arctg", false), ("sh", false), ("ch", false), ("th", false),
        ("det", true), ("gcd", true), ("lim", true), ("liminf", true), ("limsup", true), ("max", true),
        ("min", true), ("sup", true), ("inf", true), ("Pr", true), ("argmax", true), ("argmin", true),
    ];
    OPS.iter().find(|(n, _)| *n == name).map(|(n, l)| {
        let text: &'static str = match *n {
            "liminf" => "lim inf",
            "limsup" => "lim sup",
            "argmax" => "arg max",
            "argmin" => "arg min",
            other => other,
        };
        (text, *l)
    })
}

fn big_operator(name: &str) -> Option<(&'static str, Limits)> {
    Some(match name {
        "sum" => ("∑", Limits::Auto),
        "prod" => ("∏", Limits::Auto),
        "coprod" => ("∐", Limits::Auto),
        "bigcup" => ("⋃", Limits::Auto),
        "bigcap" => ("⋂", Limits::Auto),
        "bigvee" => ("⋁", Limits::Auto),
        "bigwedge" => ("⋀", Limits::Auto),
        "bigoplus" => ("⨁", Limits::Auto),
        "bigotimes" => ("⨂", Limits::Auto),
        "bigodot" => ("⨀", Limits::Auto),
        "biguplus" => ("⨄", Limits::Auto),
        "int" => ("∫", Limits::No),
        "iint" => ("∬", Limits::No),
        "iiint" => ("∭", Limits::No),
        "oint" => ("∮", Limits::No),
        "oiint" => ("∯", Limits::No),
        _ => return None,
    })
}

/// Таблица символов: имя команды → (символ, класс).
fn symbol(name: &str) -> Option<(&'static str, Kind)> {
    static TABLE: OnceLock<HashMap<&'static str, (&'static str, Kind)>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        use Kind::*;
        let list: &[(&str, &str, Kind)] = &[
            // Греческие строчные
            ("alpha", "α", Ord), ("beta", "β", Ord), ("gamma", "γ", Ord), ("delta", "δ", Ord),
            ("epsilon", "ϵ", Ord), ("varepsilon", "ε", Ord), ("zeta", "ζ", Ord), ("eta", "η", Ord),
            ("theta", "θ", Ord), ("vartheta", "ϑ", Ord), ("iota", "ι", Ord), ("kappa", "κ", Ord),
            ("lambda", "λ", Ord), ("mu", "μ", Ord), ("nu", "ν", Ord), ("xi", "ξ", Ord), ("omicron", "ο", Ord),
            ("pi", "π", Ord), ("varpi", "ϖ", Ord), ("rho", "ρ", Ord), ("varrho", "ϱ", Ord),
            ("sigma", "σ", Ord), ("varsigma", "ς", Ord), ("tau", "τ", Ord), ("upsilon", "υ", Ord),
            ("phi", "ϕ", Ord), ("varphi", "φ", Ord), ("chi", "χ", Ord), ("psi", "ψ", Ord), ("omega", "ω", Ord),
            // Греческие прописные
            ("Gamma", "Γ", Ord), ("Delta", "Δ", Ord), ("Theta", "Θ", Ord), ("Lambda", "Λ", Ord),
            ("Xi", "Ξ", Ord), ("Pi", "Π", Ord), ("Sigma", "Σ", Ord), ("Upsilon", "Υ", Ord),
            ("Phi", "Φ", Ord), ("Psi", "Ψ", Ord), ("Omega", "Ω", Ord),
            // Бинарные операции
            ("pm", "±", Bin), ("mp", "∓", Bin), ("times", "×", Bin), ("div", "÷", Bin), ("cdot", "⋅", Bin),
            ("ast", "∗", Bin), ("star", "⋆", Bin), ("circ", "∘", Bin), ("bullet", "∙", Bin),
            ("cap", "∩", Bin), ("cup", "∪", Bin), ("wedge", "∧", Bin), ("land", "∧", Bin),
            ("vee", "∨", Bin), ("lor", "∨", Bin), ("oplus", "⊕", Bin), ("ominus", "⊖", Bin),
            ("otimes", "⊗", Bin), ("odot", "⊙", Bin), ("setminus", "∖", Bin), ("uplus", "⊎", Bin),
            ("sqcup", "⊔", Bin), ("sqcap", "⊓", Bin), ("dagger", "†", Bin), ("amalg", "⨿", Bin),
            // Отношения
            ("le", "≤", Rel), ("leq", "≤", Rel), ("ge", "≥", Rel), ("geq", "≥", Rel), ("ne", "≠", Rel),
            ("neq", "≠", Rel), ("leqslant", "⩽", Rel), ("geqslant", "⩾", Rel), ("approx", "≈", Rel),
            ("equiv", "≡", Rel), ("sim", "∼", Rel), ("simeq", "≃", Rel), ("cong", "≅", Rel),
            ("propto", "∝", Rel), ("in", "∈", Rel), ("notin", "∉", Rel), ("ni", "∋", Rel),
            ("subset", "⊂", Rel), ("supset", "⊃", Rel), ("subseteq", "⊆", Rel), ("supseteq", "⊇", Rel),
            ("subsetneq", "⊊", Rel), ("to", "→", Rel), ("rightarrow", "→", Rel), ("leftarrow", "←", Rel),
            ("gets", "←", Rel), ("Rightarrow", "⇒", Rel), ("Leftarrow", "⇐", Rel),
            ("Leftrightarrow", "⇔", Rel), ("leftrightarrow", "↔", Rel), ("iff", "⟺", Rel),
            ("implies", "⟹", Rel), ("impliedby", "⟸", Rel), ("mapsto", "↦", Rel),
            ("longrightarrow", "⟶", Rel), ("longleftarrow", "⟵", Rel), ("Longrightarrow", "⟹", Rel),
            ("longmapsto", "⟼", Rel), ("uparrow", "↑", Rel), ("downarrow", "↓", Rel),
            ("nearrow", "↗", Rel), ("searrow", "↘", Rel), ("hookrightarrow", "↪", Rel),
            ("perp", "⊥", Rel), ("parallel", "∥", Rel), ("mid", "∣", Rel), ("nmid", "∤", Rel),
            ("ll", "≪", Rel), ("gg", "≫", Rel), ("models", "⊨", Rel), ("vdash", "⊢", Rel),
            ("dashv", "⊣", Rel), ("asymp", "≍", Rel), ("doteq", "≐", Rel), ("prec", "≺", Rel),
            ("succ", "≻", Rel), ("coloneqq", "≔", Rel), ("triangleq", "≜", Rel), ("lt", "<", Rel),
            ("gt", ">", Rel),
            // Обычные символы
            ("infty", "∞", Ord), ("partial", "∂", Ord), ("nabla", "∇", Ord), ("forall", "∀", Ord),
            ("exists", "∃", Ord), ("nexists", "∄", Ord), ("emptyset", "∅", Ord), ("varnothing", "∅", Ord),
            ("hbar", "ℏ", Ord), ("ell", "ℓ", Ord), ("Re", "ℜ", Ord), ("Im", "ℑ", Ord), ("aleph", "ℵ", Ord),
            ("wp", "℘", Ord), ("angle", "∠", Ord), ("triangle", "△", Ord), ("square", "□", Ord),
            ("neg", "¬", Ord), ("lnot", "¬", Ord), ("prime", "′", Ord), ("degree", "°", Ord),
            ("circ", "∘", Bin), ("top", "⊤", Ord), ("bot", "⊥", Ord), ("checkmark", "✓", Ord),
            ("S", "§", Ord), ("P", "¶", Ord), ("clubsuit", "♣", Ord), ("heartsuit", "♡", Ord),
            ("spadesuit", "♠", Ord), ("diamondsuit", "♢", Ord), ("flat", "♭", Ord), ("sharp", "♯", Ord),
            ("ldots", "…", Inner), ("dots", "…", Inner), ("cdots", "⋯", Inner), ("vdots", "⋮", Ord),
            ("ddots", "⋱", Inner), ("dotsc", "…", Inner), ("dotsb", "⋯", Inner),
            ("imath", "ı", Ord), ("jmath", "ȷ", Ord), ("Box", "□", Ord), ("blacksquare", "■", Ord),
            ("lbrace", "{", Open), ("rbrace", "}", Close), ("langle", "⟨", Open), ("rangle", "⟩", Close),
            ("lfloor", "⌊", Open), ("rfloor", "⌋", Close), ("lceil", "⌈", Open), ("rceil", "⌉", Close),
            ("vert", "|", Ord), ("Vert", "‖", Ord), ("lvert", "|", Open), ("rvert", "|", Close),
            ("lVert", "‖", Open), ("rVert", "‖", Close), ("backslash", "∖", Ord),
            ("colon", ":", Punct), ("cdotp", "⋅", Punct),
        ];
        list.iter().map(|(n, s, k)| (*n, (*s, *k))).collect()
    });
    table.get(name).copied()
}

// ============================================================================================
// Вёрстка
// ============================================================================================

#[derive(Debug, Clone, Default)]
struct Bx {
    w: f32,
    asc: f32,
    desc: f32,
    items: Vec<Item>,
}

impl Bx {
    fn empty() -> Self {
        Self::default()
    }

    /// Добавляет содержимое `other`, сдвинутое на (dx, dy).
    fn put(&mut self, other: Bx, dx: f32, dy: f32) {
        for mut item in other.items {
            shift_item(&mut item, dx, dy);
            self.items.push(item);
        }
    }

    fn shift_v(&mut self, dy: f32) {
        for item in &mut self.items {
            shift_item(item, 0.0, dy);
        }
        self.asc += dy;
        self.desc -= dy;
    }

    fn colorize(&mut self, color: Color32) {
        for item in &mut self.items {
            let c = match item {
                Item::Text { color, .. }
                | Item::Rect { color, .. }
                | Item::Line { color, .. }
                | Item::Strip { color, .. }
                | Item::Poly { color, .. }
                | Item::Circle { color, .. }
                | Item::Frame { color, .. } => color,
            };
            if c.is_none() {
                *c = Some(color);
            }
        }
    }
}

fn shift_item(item: &mut Item, dx: f32, dy: f32) {
    match item {
        Item::Text { x, y, .. } => {
            *x += dx;
            *y += dy;
        }
        Item::Rect { x0, y0, x1, y1, .. } | Item::Frame { x0, y0, x1, y1, .. } => {
            *x0 += dx;
            *x1 += dx;
            *y0 += dy;
            *y1 += dy;
        }
        Item::Line { pts, .. } => {
            for p in pts {
                *p += vec2(dx, dy);
            }
        }
        Item::Strip { outer, inner, .. } => {
            for p in outer.iter_mut().chain(inner.iter_mut()) {
                *p += vec2(dx, dy);
            }
        }
        Item::Poly { pts, .. } => {
            for p in pts {
                *p += vec2(dx, dy);
            }
        }
        Item::Circle { c, .. } => *c += vec2(dx, dy),
    }
}

/// Примерные высота над базовой линией и глубина под ней (в долях кегля) для символа.
fn char_metrics(c: char) -> (f32, f32) {
    match c {
        'a' | 'c' | 'e' | 'm' | 'n' | 'o' | 'r' | 's' | 'u' | 'v' | 'w' | 'x' | 'z' | 'ı' => (0.47, 0.0),
        'g' | 'p' | 'q' | 'y' | 'ȷ' => (0.47, 0.22),
        'j' => (0.68, 0.22),
        'f' => (0.72, 0.2),
        'α' | 'ε' | 'ϵ' | 'ι' | 'κ' | 'ν' | 'ο' | 'π' | 'σ' | 'τ' | 'υ' | 'ω' | 'ϖ' => (0.47, 0.0),
        'γ' | 'η' | 'μ' | 'ρ' | 'χ' | 'ς' | 'ϱ' => (0.47, 0.22),
        'β' | 'ζ' | 'ξ' | 'φ' | 'ψ' | 'ϕ' => (0.72, 0.22),
        'δ' | 'θ' | 'λ' | 'ϑ' | 'ℓ' | '∂' => (0.72, 0.0),
        '(' | ')' | '[' | ']' | '{' | '}' | '|' | '‖' | '⟨' | '⟩' | '⌊' | '⌋' | '⌈' | '⌉' | '/' | '∖' => (0.75, 0.25),
        '+' | '−' | '±' | '∓' | '×' | '÷' | '=' | '≠' | '≈' | '≡' | '∼' | '≃' | '≅' | '→' | '←' | '↔' | '⇒'
        | '⇐' | '⇔' | '⟺' | '⟹' | '↦' | '⟶' => (0.56, 0.06),
        '<' | '>' | '≤' | '≥' | '⩽' | '⩾' | '∈' | '∉' | '⊂' | '⊃' | '⊆' | '⊇' | '∪' | '∩' => (0.6, 0.1),
        '⋅' | '·' | '∘' | '∙' | '∗' | '⋆' => (0.4, 0.0),
        ',' | ';' => (0.12, 0.2),
        '.' | '…' | '⋯' => (0.12, 0.0),
        '′' | '″' | '‴' | '°' => (0.75, 0.0),
        '∞' => (0.47, 0.0),
        _ => (0.7, 0.0),
    }
}

fn text_metrics(s: &str, size: f32, font: MFont) -> (f32, f32) {
    if matches!(font, MFont::Text | MFont::BoldText | MFont::ItalicText | MFont::Mono) {
        let desc = if s.chars().any(|c| "gjpqyруфдцщ".contains(c)) { 0.22 } else { 0.05 };
        return (0.72 * size, desc * size);
    }
    let (mut a, mut d) = (0.0f32, 0.0f32);
    for c in s.chars() {
        let (ca, cd) = if c.is_ascii_uppercase() || c.is_ascii_digit() {
            (0.7, 0.0)
        } else if c.is_alphabetic() && !c.is_ascii() && !('α'..='ω').contains(&c) {
            (0.72, 0.1)
        } else {
            char_metrics(c)
        };
        a = a.max(ca);
        d = d.max(cd);
    }
    (a * size, d * size)
}

/// Перевод буквы в математический алфавит Unicode (𝔸, 𝐀, 𝒜, 𝔄…).
fn map_alphabet(c: char, font: MFont) -> char {
    if !fonts::has_math() {
        return c;
    }
    let exceptions = |c: char| -> Option<char> {
        Some(match (font, c) {
            (MFont::BB, 'C') => 'ℂ',
            (MFont::BB, 'H') => 'ℍ',
            (MFont::BB, 'N') => 'ℕ',
            (MFont::BB, 'P') => 'ℙ',
            (MFont::BB, 'Q') => 'ℚ',
            (MFont::BB, 'R') => 'ℝ',
            (MFont::BB, 'Z') => 'ℤ',
            (MFont::Cal, 'B') => 'ℬ',
            (MFont::Cal, 'E') => 'ℰ',
            (MFont::Cal, 'F') => 'ℱ',
            (MFont::Cal, 'H') => 'ℋ',
            (MFont::Cal, 'I') => 'ℐ',
            (MFont::Cal, 'L') => 'ℒ',
            (MFont::Cal, 'M') => 'ℳ',
            (MFont::Cal, 'R') => 'ℛ',
            (MFont::Cal, 'e') => 'ℯ',
            (MFont::Cal, 'g') => 'ℊ',
            (MFont::Cal, 'o') => 'ℴ',
            (MFont::Frak, 'C') => 'ℭ',
            (MFont::Frak, 'H') => 'ℌ',
            (MFont::Frak, 'I') => 'ℑ',
            (MFont::Frak, 'R') => 'ℜ',
            (MFont::Frak, 'Z') => 'ℨ',
            _ => return None,
        })
    };
    if let Some(e) = exceptions(c) {
        return e;
    }
    let (upper, lower, digit) = match font {
        MFont::Bold => (0x1D400, 0x1D41A, Some(0x1D7CE)),
        MFont::BB => (0x1D538, 0x1D552, Some(0x1D7D8)),
        MFont::Cal => (0x1D49C, 0x1D4B6, None),
        MFont::Frak => (0x1D504, 0x1D51E, None),
        MFont::Sans => (0x1D5A0, 0x1D5BA, Some(0x1D7E2)),
        MFont::Mono => (0x1D670, 0x1D68A, Some(0x1D7F6)),
        _ => return c,
    };
    let code = if c.is_ascii_uppercase() {
        upper + (c as u32 - 'A' as u32)
    } else if c.is_ascii_lowercase() {
        lower + (c as u32 - 'a' as u32)
    } else if c.is_ascii_digit() {
        match digit {
            Some(d) => d + (c as u32 - '0' as u32),
            None => return c,
        }
    } else {
        return c;
    };
    char::from_u32(code).unwrap_or(c)
}

fn space_between(a: Kind, b: Kind, script: bool) -> f32 {
    use Kind::*;
    let thin = 3.0 / 18.0;
    let med = 4.0 / 18.0;
    let thick = 5.0 / 18.0;
    let s = |v: f32| if script { 0.0 } else { v };
    match (a, b) {
        (Ord, Op) | (Op, Ord) | (Op, Op) | (Close, Op) | (Inner, Op) => thin,
        (Ord | Close | Inner, Bin) | (Bin, Ord | Op | Open | Inner) => s(med),
        (Ord | Op | Close | Inner, Rel) | (Rel, Ord | Op | Open | Inner) => s(thick),
        (Punct, Ord | Op | Rel | Open | Punct | Inner) => s(thin),
        (Inner, Ord | Open | Punct | Inner) | (Ord | Close, Inner) | (Punct, Close) => s(thin),
        _ => 0.0,
    }
}

struct Layouter<'a> {
    measure: &'a mut dyn FnMut(&str, &FontId) -> f32,
    base: f32,
}

impl Layouter<'_> {
    fn script_size(&self, size: f32) -> f32 {
        (size * 0.72).max(self.base * 0.45)
    }

    fn rule(&self, size: f32) -> f32 {
        (size * 0.06).max(1.15)
    }

    fn axis(&self, size: f32) -> f32 {
        size * 0.25
    }

    fn font_id(&self, font: MFont, size: f32) -> FontId {
        let family = match font {
            MFont::Italic => fonts::math_italic(),
            MFont::Text => FontFamily::Proportional,
            MFont::BoldText => fonts::bold(),
            MFont::ItalicText => fonts::italic(),
            MFont::Mono if !fonts::has_math() => FontFamily::Monospace,
            MFont::Bold if !fonts::has_math() => fonts::bold(),
            _ => fonts::math(),
        };
        FontId::new(size, family)
    }

    fn text(&mut self, s: &str, font: MFont, size: f32) -> Bx {
        let mapped: String = match font {
            MFont::Bold | MFont::BB | MFont::Cal | MFont::Frak | MFont::Sans | MFont::Mono => {
                s.chars().map(|c| map_alphabet(c, font)).collect()
            }
            _ => s.to_owned(),
        };
        let fid = self.font_id(font, size);
        let mut w = (self.measure)(&mapped, &fid);
        // Курсивным буквам немного места справа, чтобы наклон не налезал на соседей.
        if font == MFont::Italic {
            w += size * 0.03;
        }
        let (asc, desc) = text_metrics(s, size, font);
        Bx { w, asc, desc, items: vec![Item::Text { x: 0.0, y: 0.0, text: mapped, font: fid, color: None }] }
    }

    fn row(&mut self, nodes: &[Node], size: f32, display: bool, _color: Option<Color32>) -> Bx {
        let script = size < self.base * 0.95;
        let mut display = display;
        let mut out = Bx::empty();
        let mut prev: Option<Kind> = None;
        let mut first = true;
        // Сначала определяем классы, чтобы корректно обработать унарный минус.
        let kinds: Vec<Kind> = nodes.iter().map(node_kind).collect();
        for (i, node) in nodes.iter().enumerate() {
            if let Node::StyleSwitch(d) = node {
                display = *d;
                continue;
            }
            let mut kind = kinds[i];
            if kind == Kind::Bin {
                let after = kinds.get(i + 1).copied();
                let bad_prev = matches!(prev, None | Some(Kind::Bin | Kind::Op | Kind::Rel | Kind::Open | Kind::Punct));
                let bad_next = matches!(after, None | Some(Kind::Rel | Kind::Close | Kind::Punct));
                if bad_prev || bad_next {
                    kind = Kind::Ord;
                }
            }
            if let Node::Space(em) = node {
                out.w += em * size;
                continue;
            }
            let b = self.node(node, size, display);
            if let Some(p) = prev {
                out.w += space_between(p, kind, script) * size;
            }
            let x = out.w;
            out.asc = out.asc.max(b.asc);
            out.desc = out.desc.max(b.desc);
            out.w += b.w;
            out.put(b, x, 0.0);
            prev = Some(kind);
            first = false;
        }
        if first {
            out.asc = 0.0;
            out.desc = 0.0;
        }
        out
    }

    fn node(&mut self, node: &Node, size: f32, display: bool) -> Bx {
        match node {
            Node::Atom { text, font, .. } => self.text(text, *font, size),
            Node::Row(nodes) => self.row(nodes, size, display, None),
            Node::Error(s) => {
                let mut b = self.text(s, MFont::Mono, size * 0.9);
                b.colorize(Color32::from_rgb(239, 68, 68));
                b
            }
            Node::Space(em) => Bx { w: em * size, ..Bx::empty() },
            Node::StyleSwitch(_) => Bx::empty(),
            Node::Color(c, body) => {
                let mut b = self.node(body, size, display);
                b.colorize(*c);
                b
            }
            Node::Scripts { base, sup, sub } => self.scripts(base, sup.as_deref(), sub.as_deref(), size, display),
            Node::Op { .. } => self.op(node, size, display),
            Node::Frac { num, den, bar, display: d } => {
                let disp = d.unwrap_or(display);
                self.frac(num, den, *bar, size, disp)
            }
            Node::Binom { top, bottom, display: d } => {
                let disp = d.unwrap_or(display);
                let inner = self.frac(top, bottom, false, size, disp);
                self.wrap_delims(inner, "(", ")", size)
            }
            Node::Sqrt { index, body } => self.sqrt(index.as_deref(), body, size, display),
            Node::Delim { left, right, parts, middles } => self.left_right(left, right, parts, middles, size, display),
            Node::BigDelim { delim, scale, .. } => {
                let h = scale * size;
                let axis = self.axis(size);
                self.delimiter(delim, axis + h / 2.0, h / 2.0 - axis, size)
            }
            Node::Accent { kind, body } => self.accent(*kind, body, size, display),
            Node::OverUnder { base, script, over, .. } => {
                let b = self.node(base, size, display);
                let s_size = self.script_size(size);
                let s = self.node(script, s_size, false);
                let (sup, sub) = if *over { (Some(s), None) } else { (None, Some(s)) };
                self.stack_limits(b, sup, sub, size)
            }
            Node::Matrix { rows, env } => self.matrix(rows, *env, size, display),
            Node::Boxed(body) => {
                let b = self.node(body, size, display);
                let pad = size * 0.22;
                let mut out = Bx { w: b.w + 2.0 * pad, asc: b.asc + pad, desc: b.desc + pad, items: Vec::new() };
                out.items.push(Item::Frame {
                    x0: 0.0,
                    y0: -b.desc - pad,
                    x1: b.w + 2.0 * pad,
                    y1: b.asc + pad,
                    width: self.rule(size),
                    color: None,
                });
                out.put(b, pad, 0.0);
                out
            }
            Node::Phantom { body, h, v } => {
                let b = self.node(body, size, display);
                Bx {
                    w: if *h { b.w } else { 0.0 },
                    asc: if *v { b.asc } else { 0.0 },
                    desc: if *v { b.desc } else { 0.0 },
                    items: Vec::new(),
                }
            }
        }
    }

    fn op(&mut self, node: &Node, size: f32, display: bool) -> Bx {
        let Node::Op { text, big, .. } = node else { return Bx::empty() };
        if !*big {
            return self.text(text, MFont::Upright, size);
        }
        let integral = text.chars().any(|c| "∫∬∭∮∯".contains(c));
        let scale = match (display, integral) {
            (true, true) => 1.9,
            (true, false) => 1.55,
            (false, true) => 1.3,
            (false, false) => 1.15,
        };
        let s = size * scale;
        let mut b = self.text(text, MFont::Upright, s);
        // Центрируем символ по математической оси.
        let (asc, desc) = if integral { (0.86 * s, 0.32 * s) } else { (0.76 * s, 0.24 * s) };
        b.asc = asc;
        b.desc = desc;
        let center = (asc - desc) / 2.0;
        b.shift_v(self.axis(size) - center);
        if integral {
            b.w += size * 0.08;
        }
        b
    }

    fn uses_limits(&self, base: &Node, display: bool) -> bool {
        match base {
            Node::Op { limits: Limits::Yes, .. } => true,
            Node::Op { limits: Limits::No, .. } => false,
            Node::Op { limits: Limits::Auto, .. } => display,
            Node::Accent { kind: AccentKind::OverBrace | AccentKind::UnderBrace, .. } => true,
            _ => false,
        }
    }

    fn scripts(&mut self, base: &Node, sup: Option<&Node>, sub: Option<&Node>, size: f32, display: bool) -> Bx {
        let b = self.node(base, size, display);
        let s_size = self.script_size(size);
        if self.uses_limits(base, display) {
            let sup_b = sup.map(|n| self.node(n, s_size, false));
            let sub_b = sub.map(|n| self.node(n, s_size, false));
            return self.stack_limits(b, sup_b, sub_b, size);
        }
        // Курсивная коррекция: у наклонных букв (особенно f) степень сдвигается правее.
        let italic_corr = match base {
            Node::Atom { font: MFont::Italic, text, .. } => match text.chars().last() {
                Some('f') => size * 0.16,
                Some('b' | 'd' | 'h' | 'k' | 'l' | 't' | 'β' | 'δ' | 'ζ' | 'ξ') => size * 0.07,
                Some(c) if c.is_uppercase() => size * 0.08,
                _ => size * 0.04,
            },
            _ => 0.0,
        };
        let integral = matches!(base, Node::Op { big: true, text, .. } if text.contains('∫') || text.contains('∮'));
        let mut out = Bx { w: b.w, asc: b.asc, desc: b.desc, items: Vec::new() };
        let base_w = b.w;
        let base_asc = b.asc;
        let base_desc = b.desc;
        out.put(b, 0.0, 0.0);

        let sup_b = sup.map(|n| self.node(n, s_size, false));
        let sub_b = sub.map(|n| self.node(n, s_size, false));
        let simple_base = base_asc <= size * 0.75;
        let mut u = if simple_base { size * 0.38 } else { (base_asc - s_size * 0.38).max(size * 0.38) };
        let mut d = if simple_base { size * 0.16 } else { (base_desc + s_size * 0.1).max(size * 0.16) };
        if let Some(s) = &sup_b {
            u = u.max(s.desc + size * 0.12);
        }
        if let Some(s) = &sub_b {
            d = d.max(s.asc - size * 0.4);
            if sup_b.is_some() {
                d = d.max(size * 0.24);
            }
        }
        if let (Some(sp), Some(sb)) = (&sup_b, &sub_b) {
            let gap = (u - sp.desc) - (sb.asc - d);
            let min_gap = size * 0.16;
            if gap < min_gap {
                let extra = min_gap - gap;
                u += extra * 0.5;
                d += extra * 0.5;
            }
        }
        let sup_x = base_w + italic_corr + if integral { size * 0.1 } else { 0.0 };
        let sub_x = base_w - if integral { size * 0.15 } else { 0.0 };
        let mut right = base_w;
        if let Some(s) = sup_b {
            out.asc = out.asc.max(u + s.asc);
            right = right.max(sup_x + s.w);
            out.put(s, sup_x, u);
        }
        if let Some(s) = sub_b {
            out.desc = out.desc.max(d + s.desc);
            right = right.max(sub_x + s.w);
            out.put(s, sub_x, -d);
        }
        out.w = right + size * 0.05;
        out
    }

    /// Пределы над и под элементом (суммы в выключных формулах, \overset, \underbrace…).
    fn stack_limits(&mut self, b: Bx, sup: Option<Bx>, sub: Option<Bx>, size: f32) -> Bx {
        let gap = size * 0.14;
        let w = b.w.max(sup.as_ref().map_or(0.0, |s| s.w)).max(sub.as_ref().map_or(0.0, |s| s.w));
        let mut out = Bx { w, asc: b.asc, desc: b.desc, items: Vec::new() };
        let b_asc = b.asc;
        let b_desc = b.desc;
        let bx = (w - b.w) / 2.0;
        out.put(b, bx, 0.0);
        if let Some(s) = sup {
            let y = b_asc + gap + s.desc;
            out.asc = y + s.asc;
            let x = (w - s.w) / 2.0;
            out.put(s, x, y);
        }
        if let Some(s) = sub {
            let y = -(b_desc + gap + s.asc);
            out.desc = -y + s.desc;
            let x = (w - s.w) / 2.0;
            out.put(s, x, y);
        }
        out
    }

    fn frac(&mut self, num: &Node, den: &Node, bar: bool, size: f32, display: bool) -> Bx {
        let part_size = if display { size } else { self.script_size(size).max(size * 0.8) };
        let n = self.node(num, part_size, false);
        let d = self.node(den, part_size, false);
        let t = if bar { self.rule(size) } else { 0.0 };
        let axis = self.axis(size);
        let gap = if display { size * 0.16 } else { size * 0.1 };
        let pad = size * 0.12;
        let w = n.w.max(d.w) + 2.0 * pad;
        let num_y = axis + t / 2.0 + gap + n.desc;
        let den_y = axis - t / 2.0 - gap - d.asc;
        let mut out = Bx { w, asc: num_y + n.asc, desc: -den_y + d.desc, items: Vec::new() };
        let nx = (w - n.w) / 2.0;
        let dx = (w - d.w) / 2.0;
        out.put(n, nx, num_y);
        out.put(d, dx, den_y);
        if bar {
            out.items.push(Item::Rect { x0: pad * 0.5, y0: axis - t / 2.0, x1: w - pad * 0.5, y1: axis + t / 2.0, color: None });
        }
        out
    }

    fn sqrt(&mut self, index: Option<&Node>, body: &Node, size: f32, display: bool) -> Bx {
        let b = self.node(body, size, display);
        let t = self.rule(size);
        let gap = size * 0.12;
        let asc = b.asc.max(size * 0.62);
        let desc = b.desc.max(size * 0.1);
        let top = asc + gap + t / 2.0;
        let bottom = -desc - size * 0.04;
        let h = top - bottom;
        let surd_w = (size * 0.5 + h * 0.08).min(size * 0.9);

        let idx = index.map(|n| self.node(n, self.base.min(size) * 0.5, false));
        let tick_y = bottom + h * 0.42;
        let mut offset = 0.0;
        if let Some(i) = &idx {
            offset = (i.w - surd_w * 0.45).max(0.0);
        }

        let mut out = Bx::empty();
        let p0 = vec2(offset, tick_y);
        let p1 = vec2(offset + surd_w * 0.22, tick_y + size * 0.06);
        let p2 = vec2(offset + surd_w * 0.5, bottom);
        let p3 = vec2(offset + surd_w, top);
        let body_x = offset + surd_w + size * 0.06;
        let p4 = vec2(body_x + b.w + size * 0.06, top);
        out.items.push(Item::Line { pts: vec![p0, p1], width: t * 0.9, color: None });
        out.items.push(Item::Line { pts: vec![p1, p2], width: t * 1.9, color: None });
        out.items.push(Item::Line { pts: vec![p2, p3, p4], width: t, color: None });
        out.w = p4.x + size * 0.04;
        out.asc = top + t / 2.0 + size * 0.04;
        out.desc = -bottom;
        out.put(b, body_x, 0.0);
        if let Some(i) = idx {
            let ix = offset + surd_w * 0.45 - i.w;
            let iy = tick_y + size * 0.12 + i.desc;
            out.asc = out.asc.max(iy + i.asc);
            out.put(i, ix.max(0.0), iy);
        }
        out
    }

    fn left_right(&mut self, left: &str, right: &str, parts: &[Vec<Node>], middles: &[String], size: f32, display: bool) -> Bx {
        let boxes: Vec<Bx> = parts.iter().map(|p| self.row(p, size, display, None)).collect();
        let asc = boxes.iter().map(|b| b.asc).fold(size * 0.7, f32::max);
        let desc = boxes.iter().map(|b| b.desc).fold(size * 0.25, f32::max);
        let mut out = Bx::empty();
        let l = self.delimiter(left, asc, desc, size);
        out.asc = out.asc.max(l.asc);
        out.desc = out.desc.max(l.desc);
        let lw = l.w;
        out.put(l, 0.0, 0.0);
        out.w = lw + size * 0.05;
        for (i, b) in boxes.into_iter().enumerate() {
            out.asc = out.asc.max(b.asc);
            out.desc = out.desc.max(b.desc);
            let x = out.w;
            out.w += b.w;
            out.put(b, x, 0.0);
            if let Some(m) = middles.get(i) {
                out.w += size * 0.15;
                let mb = self.delimiter(m, asc, desc, size);
                let x = out.w;
                out.w += mb.w + size * 0.15;
                out.put(mb, x, 0.0);
            }
        }
        out.w += size * 0.05;
        let r = self.delimiter(right, asc, desc, size);
        out.asc = out.asc.max(r.asc);
        out.desc = out.desc.max(r.desc);
        let x = out.w;
        out.w += r.w;
        out.put(r, x, 0.0);
        out
    }

    fn wrap_delims(&mut self, inner: Bx, left: &str, right: &str, size: f32) -> Bx {
        let (asc, desc) = (inner.asc, inner.desc);
        let mut out = Bx::empty();
        let l = self.delimiter(left, asc, desc, size);
        let r = self.delimiter(right, asc, desc, size);
        out.asc = asc.max(l.asc);
        out.desc = desc.max(l.desc);
        let lw = l.w;
        out.put(l, 0.0, 0.0);
        let iw = inner.w;
        out.put(inner, lw, 0.0);
        let rw = r.w;
        out.put(r, lw + iw, 0.0);
        out.w = lw + iw + rw;
        out
    }

    /// Разделитель, охватывающий содержимое с заданными высотой и глубиной.
    fn delimiter(&mut self, d: &str, asc: f32, desc: f32, size: f32) -> Bx {
        if d == "." || d.is_empty() {
            return Bx { w: size * 0.12, ..Bx::empty() };
        }
        let axis = self.axis(size);
        let half = (asc - axis).max(desc + axis).max(size * 0.5) * 1.06;
        let h = 2.0 * half;
        // Небольшие разделители берём из шрифта — так они выглядят аккуратнее.
        if h <= size * 1.25 {
            let mut b = self.text(d, MFont::Upright, size);
            b.w += size * 0.02;
            return b;
        }
        let top = axis + half;
        let bottom = axis - half;
        let mid = axis;
        let lw = (size * 0.065).max(1.2);
        let mut b = Bx { w: 0.0, asc: top, desc: -bottom, items: Vec::new() };
        let bez = |p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, n: usize| -> Vec<Vec2> {
            (0..=n)
                .map(|i| {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
                })
                .collect()
        };
        match d {
            "(" | ")" => {
                let w = (size * 0.32 + h * 0.06).min(size * 0.75);
                let thick = lw * 1.6;
                let flip = d == ")";
                let fx = |x: f32| if flip { w - x } else { x };
                let outer = bez(
                    vec2(fx(w * 0.85), top),
                    vec2(fx(w * 0.05), top - h * 0.22),
                    vec2(fx(w * 0.05), bottom + h * 0.22),
                    vec2(fx(w * 0.85), bottom),
                    32,
                );
                let inner = bez(
                    vec2(fx(w * 0.85), top),
                    vec2(fx(w * 0.05 + thick * 1.5), top - h * 0.22),
                    vec2(fx(w * 0.05 + thick * 1.5), bottom + h * 0.22),
                    vec2(fx(w * 0.85), bottom),
                    32,
                );
                b.items.push(Item::Strip { outer, inner, color: None });
                b.w = w;
            }
            "[" | "]" | "⌊" | "⌋" | "⌈" | "⌉" => {
                let w = size * 0.38;
                let flip = matches!(d, "]" | "⌋" | "⌉");
                let fx = |x: f32| if flip { w - x } else { x };
                let x_bar = fx(w * 0.3);
                let x_end = fx(w * 0.85);
                let top_serif = !matches!(d, "⌊" | "⌋");
                let bottom_serif = !matches!(d, "⌈" | "⌉");
                let mut pts = Vec::new();
                if top_serif {
                    pts.push(vec2(x_end, top));
                }
                pts.push(vec2(x_bar, top));
                pts.push(vec2(x_bar, bottom));
                if bottom_serif {
                    pts.push(vec2(x_end, bottom));
                }
                b.items.push(Item::Line { pts, width: lw * 1.2, color: None });
                b.w = w;
            }
            "{" | "}" => {
                let w = (size * 0.4 + h * 0.04).min(size * 0.8);
                let flip = d == "}";
                let fx = |x: f32| if flip { w - x } else { x };
                let xm = w * 0.5;
                let mut pts = bez(vec2(fx(w * 0.9), top), vec2(fx(xm), top), vec2(fx(xm), top), vec2(fx(xm), top - h * 0.1), 10);
                pts.extend(bez(
                    vec2(fx(xm), mid + h * 0.08),
                    vec2(fx(xm), mid + h * 0.02),
                    vec2(fx(w * 0.3), mid),
                    vec2(fx(w * 0.08), mid),
                    10,
                ));
                pts.extend(bez(
                    vec2(fx(w * 0.08), mid),
                    vec2(fx(w * 0.3), mid),
                    vec2(fx(xm), mid - h * 0.02),
                    vec2(fx(xm), mid - h * 0.08),
                    10,
                ));
                pts.extend(bez(vec2(fx(xm), bottom + h * 0.1), vec2(fx(xm), bottom), vec2(fx(xm), bottom), vec2(fx(w * 0.9), bottom), 10));
                b.items.push(Item::Line { pts, width: lw * 1.15, color: None });
                b.w = w;
            }
            "|" | "‖" => {
                let double = d == "‖";
                let w = if double { size * 0.42 } else { size * 0.26 };
                if double {
                    for x in [w * 0.32, w * 0.68] {
                        b.items.push(Item::Line { pts: vec![vec2(x, top), vec2(x, bottom)], width: lw, color: None });
                    }
                } else {
                    b.items.push(Item::Line { pts: vec![vec2(w / 2.0, top), vec2(w / 2.0, bottom)], width: lw, color: None });
                }
                b.w = w;
            }
            "⟨" | "⟩" => {
                let w = (size * 0.35 + h * 0.05).min(size * 0.7);
                let flip = d == "⟩";
                let fx = |x: f32| if flip { w - x } else { x };
                b.items.push(Item::Line {
                    pts: vec![vec2(fx(w * 0.85), top), vec2(fx(w * 0.15), mid), vec2(fx(w * 0.85), bottom)],
                    width: lw,
                    color: None,
                });
                b.w = w;
            }
            "/" | "\\" => {
                let w = size * 0.2 + h * 0.25;
                let (a, z) = if d == "/" { (vec2(0.0, bottom), vec2(w, top)) } else { (vec2(0.0, top), vec2(w, bottom)) };
                b.items.push(Item::Line { pts: vec![a, z], width: lw, color: None });
                b.w = w;
            }
            _ => {
                // Неизвестный разделитель — растягиваем символ шрифта.
                let mut t = self.text(d, MFont::Upright, h * 0.8);
                t.shift_v(axis - (t.asc - t.desc) / 2.0);
                return t;
            }
        }
        b
    }

    fn accent(&mut self, kind: AccentKind, body: &Node, size: f32, display: bool) -> Bx {
        let b = self.node(body, size, display);
        let t = self.rule(size);
        let mut out = Bx { w: b.w, asc: b.asc, desc: b.desc, items: Vec::new() };
        let (bw, basc, bdesc) = (b.w, b.asc.max(size * 0.47), b.desc);
        // Курсивную букву слегка сдвигаем: акцент над ней должен стоять по центру наклона.
        let skew = if matches!(body, Node::Atom { font: MFont::Italic, .. }) { size * 0.06 } else { 0.0 };
        out.put(b, 0.0, 0.0);
        let cx = bw / 2.0 + skew;
        let y = basc + size * 0.1;
        match kind {
            AccentKind::Hat | AccentKind::WideHat => {
                let w = if kind == AccentKind::WideHat { bw * 0.9 } else { (size * 0.36).min(bw.max(size * 0.3)) };
                let hgt = size * 0.13;
                out.items.push(Item::Line {
                    pts: vec![vec2(cx - w / 2.0, y), vec2(cx, y + hgt), vec2(cx + w / 2.0, y)],
                    width: t,
                    color: None,
                });
                out.asc = y + hgt + t;
            }
            AccentKind::Bar | AccentKind::Overline => {
                let (x0, x1) = if kind == AccentKind::Bar {
                    let w = (size * 0.42).max(bw * 0.8);
                    (cx - w / 2.0, cx + w / 2.0)
                } else {
                    (0.0, bw)
                };
                out.items.push(Item::Rect { x0, y0: y, x1, y1: y + t, color: None });
                out.asc = y + t * 2.0;
            }
            AccentKind::Underline => {
                let y = -bdesc - size * 0.1;
                out.items.push(Item::Rect { x0: 0.0, y0: y - t, x1: bw, y1: y, color: None });
                out.desc = -y + t * 2.0;
            }
            AccentKind::Vec => {
                let w = bw.max(size * 0.45);
                let x0 = cx - w / 2.0;
                let x1 = cx + w / 2.0;
                let yy = y + size * 0.07;
                let head = size * 0.16;
                // Стрелка: стержень и залитый наконечник — так она чёткая даже на мелком кегле.
                out.items.push(Item::Line { pts: vec![vec2(x0, yy), vec2(x1 - head * 0.6, yy)], width: t * 1.1, color: None });
                out.items.push(Item::Poly {
                    pts: vec![vec2(x1 - head, yy + head * 0.45), vec2(x1, yy), vec2(x1 - head, yy - head * 0.45)],
                    color: None,
                });
                out.asc = yy + head * 0.6;
                out.w = out.w.max(x1 - 0.0);
            }
            AccentKind::Dot | AccentKind::DDot => {
                let r = size * 0.05;
                let yy = y + r * 1.5;
                if kind == AccentKind::Dot {
                    out.items.push(Item::Circle { c: vec2(cx, yy), r, color: None });
                } else {
                    out.items.push(Item::Circle { c: vec2(cx - r * 2.0, yy), r, color: None });
                    out.items.push(Item::Circle { c: vec2(cx + r * 2.0, yy), r, color: None });
                }
                out.asc = yy + r * 2.0;
            }
            AccentKind::Tilde | AccentKind::WideTilde => {
                let w = if kind == AccentKind::WideTilde { bw * 0.9 } else { (size * 0.4).max(bw * 0.5).min(bw.max(size * 0.4)) };
                let amp = size * 0.05;
                let yy = y + size * 0.07;
                let pts = (0..=16)
                    .map(|i| {
                        let f = i as f32 / 16.0;
                        vec2(cx - w / 2.0 + w * f, yy + amp * (f * std::f32::consts::TAU).sin())
                    })
                    .collect();
                out.items.push(Item::Line { pts, width: t, color: None });
                out.asc = yy + amp * 2.0;
            }
            AccentKind::OverBrace | AccentKind::UnderBrace => {
                let over = kind == AccentKind::OverBrace;
                let hb = size * 0.22;
                let y0 = if over { basc + size * 0.08 } else { -bdesc - size * 0.08 };
                let dir = if over { 1.0 } else { -1.0 };
                let m = bw / 2.0;
                let pts = vec![
                    vec2(0.0, y0),
                    vec2(size * 0.08, y0 + dir * hb * 0.5),
                    vec2(m - size * 0.12, y0 + dir * hb * 0.5),
                    vec2(m, y0 + dir * hb),
                    vec2(m + size * 0.12, y0 + dir * hb * 0.5),
                    vec2(bw - size * 0.08, y0 + dir * hb * 0.5),
                    vec2(bw, y0),
                ];
                out.items.push(Item::Line { pts, width: t, color: None });
                if over {
                    out.asc = y0 + hb + t;
                } else {
                    out.desc = -(y0 - hb) + t;
                }
            }
            AccentKind::Not => {
                let h = basc.max(size * 0.6);
                out.items.push(Item::Line {
                    pts: vec![vec2(cx - size * 0.15, -size * 0.1), vec2(cx + size * 0.15, h)],
                    width: t,
                    color: None,
                });
            }
        }
        out
    }

    fn matrix(&mut self, rows: &[Vec<Node>], env: Env, size: f32, display: bool) -> Bx {
        let cell_display = matches!(env, Env::Aligned | Env::Gathered) && display;
        let cells: Vec<Vec<Bx>> = rows
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .map(|(j, c)| {
                        // В aligned правая колонка обычно начинается с «=»: как в TeX, ставим перед ним
                        // пустой элемент, чтобы у отношения появился отступ слева.
                        if env == Env::Aligned && j % 2 == 1 {
                            let empty = Node::Atom { text: String::new(), kind: Kind::Ord, font: MFont::Upright };
                            let mut nodes = vec![empty];
                            match c {
                                Node::Row(inner) => nodes.extend(inner.iter().cloned()),
                                other => nodes.push(other.clone()),
                            }
                            self.node(&Node::Row(nodes), size, cell_display)
                        } else {
                            self.node(c, size, cell_display)
                        }
                    })
                    .collect()
            })
            .collect();
        let ncols = cells.iter().map(Vec::len).max().unwrap_or(0);
        let mut col_w = vec![0.0f32; ncols];
        for r in &cells {
            for (j, c) in r.iter().enumerate() {
                col_w[j] = col_w[j].max(c.w);
            }
        }
        let row_gap = match env {
            Env::Aligned | Env::Gathered => size * 0.45,
            Env::Cases => size * 0.3,
            _ => size * 0.35,
        };
        let row_metrics: Vec<(f32, f32)> = cells
            .iter()
            .map(|r| {
                let a = r.iter().map(|c| c.asc).fold(size * 0.7, f32::max);
                let d = r.iter().map(|c| c.desc).fold(size * 0.25, f32::max);
                (a, d)
            })
            .collect();
        let total_h: f32 = row_metrics.iter().map(|(a, d)| a + d).sum::<f32>()
            + row_gap * (row_metrics.len().saturating_sub(1)) as f32;

        // Положения колонок и выравнивание.
        let col_gap = |j: usize| -> f32 {
            match env {
                Env::Aligned => {
                    if j % 2 == 1 { size * 1.2 } else { 0.0 }
                }
                Env::Cases => size * 1.0,
                _ => size * 0.9,
            }
        };
        let mut col_x = vec![0.0f32; ncols];
        let mut x = 0.0;
        for j in 0..ncols {
            col_x[j] = x;
            x += col_w[j];
            if j + 1 < ncols {
                x += col_gap(j);
            }
        }
        let inner_w = x;

        let axis = self.axis(size);
        let mut inner = Bx { w: inner_w, asc: axis + total_h / 2.0, desc: total_h / 2.0 - axis, items: Vec::new() };
        let mut y = axis + total_h / 2.0;
        for (i, r) in cells.into_iter().enumerate() {
            let (a, d) = row_metrics[i];
            let baseline = y - a;
            for (j, c) in r.into_iter().enumerate() {
                let free = col_w[j] - c.w;
                let dx = match env {
                    Env::Aligned => {
                        if j % 2 == 0 { free } else { 0.0 }
                    }
                    Env::Cases => 0.0,
                    _ => free / 2.0,
                };
                inner.put(c, col_x[j] + dx, baseline);
            }
            y = baseline - d - row_gap;
        }

        let (l, r) = match env {
            Env::PMatrix => ("(", ")"),
            Env::BMatrix => ("[", "]"),
            Env::BbMatrix => ("{", "}"),
            Env::VMatrix => ("|", "|"),
            Env::VvMatrix => ("‖", "‖"),
            Env::Cases => ("{", "."),
            _ => (".", "."),
        };
        if l == "." && r == "." {
            return inner;
        }
        let pad = size * 0.12;
        inner.w += pad;
        let mut shifted = Bx { w: inner.w + pad, asc: inner.asc, desc: inner.desc, items: Vec::new() };
        shifted.put(inner, pad, 0.0);
        self.wrap_delims(shifted, l, r, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_test(tex: &str) -> Bx {
        let mut measure = |s: &str, f: &FontId| s.chars().count() as f32 * f.size * 0.5;
        let nodes = Parser::new(tex).parse_top();
        let mut l = Layouter { measure: &mut measure, base: 16.0 };
        l.row(&nodes, 16.0, true, None)
    }

    #[test]
    fn parses_common_constructs() {
        for tex in [
            r"E = mc^2",
            r"\frac{a+b}{c}",
            r"\sqrt[3]{x^2 + y^2}",
            r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}",
            r"\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}",
            r"\left( \frac{1}{2} \right)^2",
            r"\begin{pmatrix} 1 & 0 \\ 0 & 1 \end{pmatrix}",
            r"f(x) = \begin{cases} x & x \ge 0 \\ -x & x < 0 \end{cases}",
            r"\vec{v} + \hat{n} \cdot \bar{x}",
            r"\mathbb{R}^n \to \mathcal{L}",
            r"\text{скорость } v = \frac{s}{t}",
            r"\lim_{x \to 0} \frac{\sin x}{x} = 1",
            r"\color{red}{x} + \boxed{y}",
            r"a \\ b",
            r"\unknown + x'",
        ] {
            let b = layout_test(tex);
            assert!(b.w > 0.0, "пустая вёрстка для {tex}");
            assert!(b.asc > 0.0, "нулевая высота для {tex}");
        }
    }

    #[test]
    fn plain_text_of_formulas() {
        assert_eq!(to_plain("E = mc^2"), "E = mc²");
        assert_eq!(to_plain(r"\sum_{i=1}^{n} i"), "∑ᵢ₌₁ⁿ i");
        assert_eq!(to_plain(r"\frac{a}{b} + \sqrt{x}"), "a/b + √x");
        assert_eq!(to_plain(r"\alpha \to \infty"), "α → ∞");
    }

    #[test]
    fn aligned_relation_gets_space() {
        // В aligned «x &= y» должно получиться той же ширины, что и «x = y»: отступы с обеих сторон «=».
        let aligned = layout_test(r"x &= y \\ x &= y");
        let plain = layout_test(r"x=y");
        assert!((aligned.w - plain.w).abs() < 1.0, "перед «=» нет отступа: {} vs {}", aligned.w, plain.w);
    }

    #[test]
    fn fraction_is_taller_than_letter() {
        let x = layout_test("x");
        let f = layout_test(r"\frac{x}{y}");
        assert!(f.asc + f.desc > x.asc + x.desc);
    }
}
