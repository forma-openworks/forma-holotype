//! Сокращения в духе Emmet: короткая запись разворачивается по Tab в готовую разметку.
//!
//! `-*3` — три пункта списка, `t3x4` — таблица, `!tip` — плашка, `code:rust` — блок кода,
//! `h2{План}+[]*3` — заголовок и три задачи. Операторы как в Emmet: `>` — вложить,
//! `+` — рядом, `*N` — повторить, `{текст}` — содержимое, `$` — номер повтора, `( )` — группа.
//! После вставки Tab прыгает по местам для ввода.

use std::ops::Range;

use crate::complete::{self, Block};
use crate::theme;
use forma_i18n::t;

/// Границы места для ввода внутри собранного текста (потом превращаются в `stops`).
const OPEN: char = '\u{E000}';
const CLOSE: char = '\u{E001}';

pub struct Expansion {
    pub text: String,
    /// Места для ввода в символах от начала `text`, в порядке обхода по Tab.
    pub stops: Vec<Range<usize>>,
    /// Короткое описание для подсказки: «Таблица 3×2».
    pub title: String,
}

/// Сокращение, найденное у курсора.
pub struct Found {
    /// Байтовый диапазон текста, который заменяется разворотом.
    pub range: Range<usize>,
    pub expansion: Expansion,
    /// Сокращение занимает всю строку, а не одно слово в тексте.
    pub whole_line: bool,
    /// Простое сокращение вроде `h2` или `date`, которому подсказка не нужна.
    pub quiet: bool,
}

/// Ищет сокращение перед курсором (`cursor` — байтовое смещение). Сначала пробуем всю строку
/// целиком, потом последнее слово — для строчных сокращений вроде `img` или `date`.
pub fn at_cursor(text: &str, cursor: usize) -> Option<Found> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let ls = text[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let le = text[cursor..].find('\n').map_or(text.len(), |i| cursor + i);
    if complete::block_at(text, ls) != Block::Normal {
        return None;
    }
    let line = &text[ls..cursor];
    let after = &text[cursor..le];
    let prefix_len = line_prefix_len(line);
    let token = &line[prefix_len..];
    if token.is_empty() || token.ends_with(char::is_whitespace) {
        return None;
    }

    if after.trim().is_empty()
        && let Some(nodes) = parse(token)
        && !is_trivial(&nodes)
    {
        let quiet = is_quiet(&nodes);
        let expansion = build(&nodes, &line[..prefix_len]);
        return Some(Found { range: ls + prefix_len..cursor, expansion, whole_line: true, quiet });
    }

    // Одно слово посреди текста: только строчные сокращения.
    if !after.chars().next().is_none_or(|c| c.is_whitespace() || matches!(c, '|' | ')' | ']' | ',' | '.' | ';')) {
        return None;
    }
    let start = word_start(token);
    let nodes = parse(&token[start..])?;
    if !all_inline(&nodes) {
        return None;
    }
    let expansion = build(&nodes, "");
    Some(Found { range: ls + prefix_len + start..cursor, expansion, whole_line: false, quiet: true })
}

/// Длина «префикса» строки: отступ и маркеры цитаты `> `. Он повторяется в начале каждой новой строки.
fn line_prefix_len(line: &str) -> usize {
    let mut i = line.len() - line.trim_start().len();
    let b = line.as_bytes();
    while i < b.len() && b[i] == b'>' {
        i += 1;
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
    }
    i
}

/// Начало последнего слова: пробелы внутри `{…}` слово не разрывают.
fn word_start(token: &str) -> usize {
    let mut depth = 0i32;
    for (i, c) in token.char_indices().rev() {
        match c {
            '}' => depth += 1,
            '{' => depth -= 1,
            c if c.is_whitespace() && depth <= 0 => return i + c.len_utf8(),
            _ => {}
        }
    }
    0
}

// ---------------------------------------------------------------------------------------------
// Разбор
// ---------------------------------------------------------------------------------------------

struct Node {
    item: Item,
    mult: usize,
    children: Vec<Node>,
}

enum Item {
    Atom(Atom, Option<String>),
    Group(Vec<Node>),
}

#[derive(Clone, PartialEq, Debug)]
enum Atom {
    Heading(u8),
    Bullet,
    Ordered(u64),
    Task(bool),
    Quote,
    /// Тип плашки и знак сворачивания (`-`, `+` или пусто).
    Callout(String, &'static str),
    Paragraph,
    Table { cols: Option<usize>, rows: Option<usize> },
    Code(String),
    Math,
    Diagram(&'static Diagram),
    Front,
    Rule,
    Lorem(usize),
    // Строчные
    Image,
    Link,
    Kbd,
    Badge,
    Date,
    Time,
}

impl Atom {
    fn is_inline(&self) -> bool {
        matches!(self, Atom::Image | Atom::Link | Atom::Kbd | Atom::Badge | Atom::Date | Atom::Time)
    }

    fn is_list(&self) -> bool {
        matches!(self, Atom::Bullet | Atom::Ordered(_) | Atom::Task(_))
    }
}

/// Предел размера разворота в строках: `-*100>-*100>-*100` не строим (разбор идёт на каждое нажатие).
const MAX_LINES: usize = 1000;

/// Разбирает сокращение целиком. None — это не сокращение.
fn parse(src: &str) -> Option<Vec<Node>> {
    let mut p = Parser { c: src.chars().collect(), i: 0 };
    let nodes = p.seq(0)?;
    (p.i == p.c.len() && weight(&nodes) <= MAX_LINES).then_some(nodes)
}

/// Сколько строк даст разворот — по дереву, без сборки текста. Множители перемножаются (с насыщением).
fn weight(nodes: &[Node]) -> usize {
    nodes.iter().fold(0usize, |sum, n| {
        let own = match &n.item {
            Item::Group(inner) => weight(inner),
            Item::Atom(Atom::Table { rows, .. }, _) => rows.unwrap_or(2) + 2,
            Item::Atom(..) => 1,
        };
        sum.saturating_add(n.mult.saturating_mul(own.saturating_add(weight(&n.children))))
    })
}

struct Parser {
    c: Vec<char>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn eat(&mut self, ch: char) -> bool {
        if self.peek() == Some(ch) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn eat_str(&mut self, s: &str) -> bool {
        let n = s.chars().count();
        if self.c.len() >= self.i + n && self.c[self.i..self.i + n].iter().copied().eq(s.chars()) {
            self.i += n;
            true
        } else {
            false
        }
    }

    fn take(&mut self, f: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&f) {
            self.i += 1;
        }
    }

    fn number(&mut self) -> Option<usize> {
        let start = self.i;
        self.take(|c| c.is_ascii_digit());
        self.c[start..self.i].iter().collect::<String>().parse().ok()
    }

    /// Соседи через `+`; `>` забирает всё, что идёт дальше, в дети (как в Emmet).
    fn seq(&mut self, depth: usize) -> Option<Vec<Node>> {
        if depth > 12 {
            return None;
        }
        let mut out = Vec::new();
        loop {
            let mut node = self.factor(depth)?;
            if self.eat('>') {
                node.children = self.seq(depth + 1)?;
                out.push(node);
                break;
            }
            out.push(node);
            if !self.eat('+') {
                break;
            }
        }
        Some(out)
    }

    fn factor(&mut self, depth: usize) -> Option<Node> {
        let item = if self.eat('(') {
            let inner = self.seq(depth + 1)?;
            if !self.eat(')') {
                return None;
            }
            Item::Group(inner)
        } else {
            let name = self.name()?;
            let text = if self.peek() == Some('{') { Some(self.text()?) } else { None };
            // Число строк таблицы после заголовков: `t{Имя|Возраст}x5`.
            let mut rows = None;
            if text.is_some()
                && matches!(self.peek(), Some('x' | 'х' | '×'))
                && self.c.get(self.i + 1).is_some_and(char::is_ascii_digit)
            {
                self.i += 1;
                rows = Some(self.number().filter(|n| (1..=100).contains(n))?);
            }
            Item::Atom(resolve(&name, text.is_some(), rows)?, text)
        };
        let mult = if self.eat('*') { self.number().filter(|n| (1..=100).contains(n))? } else { 1 };
        Some(Node { item, mult, children: Vec::new() })
    }

    fn name(&mut self) -> Option<String> {
        let start = self.i;
        match self.peek()? {
            '!' => {
                self.i += 1;
                self.take(char::is_alphanumeric);
                // `!note-` — свёрнутая плашка; `+` считаем знаком, только если за ним не следующий элемент.
                if self.eat('-') {
                } else if self.peek() == Some('+')
                    && self.c.get(self.i + 1).is_none_or(|c| matches!(*c, '{' | '*' | '>' | ')'))
                {
                    self.i += 1;
                }
            }
            '`' => {
                if !self.eat_str("```") {
                    return None;
                }
                self.take(|c| c.is_alphanumeric() || matches!(c, '+' | '-' | '#' | '_'));
            }
            '$' => {
                if !self.eat_str("$$") {
                    return None;
                }
            }
            '[' => {
                if !(self.eat_str("[]") || self.eat_str("[ ]") || self.eat_str("[x]") || self.eat_str("[X]")) {
                    return None;
                }
            }
            '-' => self.i += 1,
            c if c.is_ascii_digit() => {
                self.take(|c| c.is_ascii_digit());
                if !self.eat('.') {
                    return None;
                }
            }
            c if is_word(c) => self.take(is_word),
            _ => return None,
        }
        Some(self.c[start..self.i].iter().collect())
    }

    fn text(&mut self) -> Option<String> {
        self.eat('{');
        let start = self.i;
        let mut depth = 1;
        while let Some(c) = self.peek() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let s = self.c[start..self.i].iter().collect();
                        self.i += 1;
                        return Some(s);
                    }
                }
                _ => {}
            }
            self.i += 1;
        }
        None
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ':' | '_' | '#' | '×')
}

/// Имя элемента → что разворачивать.
fn resolve(name: &str, has_text: bool, rows: Option<usize>) -> Option<Atom> {
    let lower = name.to_lowercase();
    let n = lower.as_str();
    if let Some(kind) = n.strip_prefix('!') {
        let (kind, fold) = match (kind.strip_suffix('-'), kind.strip_suffix('+')) {
            (Some(k), _) => (k, "-"),
            (_, Some(k)) => (k, "+"),
            _ => (kind, ""),
        };
        return theme::is_callout_kind(kind).then(|| Atom::Callout(kind.to_owned(), fold));
    }
    if let Some(lang) = n.strip_prefix("```").or_else(|| n.strip_prefix("code:")).or_else(|| n.strip_prefix("cb:")) {
        return Some(Atom::Code(lang.to_owned()));
    }
    if let Some(d) = n.strip_suffix('.') {
        return d.parse().ok().map(Atom::Ordered);
    }
    if let Some(l) = n.strip_prefix('h')
        && let Ok(level @ 1..=6) = l.parse::<u8>()
    {
        return Some(Atom::Heading(level));
    }
    if let Some(count) = n.strip_prefix("lorem") {
        return if count.is_empty() { Some(Atom::Lorem(20)) } else { count.parse().ok().filter(|c| (1..=500).contains(c)).map(Atom::Lorem) };
    }
    if let Some((cols, r)) = table_spec(n, has_text) {
        return Some(Atom::Table { cols, rows: rows.or(r) });
    }
    Some(match n {
        "-" | "ul" | "li" => Atom::Bullet,
        "ol" => Atom::Ordered(1),
        "[]" | "[ ]" | "todo" | "task" => Atom::Task(false),
        "[x]" | "done" => Atom::Task(true),
        "q" | "quote" | "bq" => Atom::Quote,
        "p" => Atom::Paragraph,
        "code" | "cb" => Atom::Code(String::new()),
        "$$" | "math" => Atom::Math,
        "fm" | "front" | "meta" | "props" => Atom::Front,
        "hr" => Atom::Rule,
        "img" | "image" | "pic" => Atom::Image,
        "link" | "ln" => Atom::Link,
        "kbd" | "key" => Atom::Kbd,
        "badge" => Atom::Badge,
        "date" | "today" => Atom::Date,
        "time" | "now" => Atom::Time,
        _ => return DIAGRAMS.iter().find(|d| d.names.contains(&n)).map(Atom::Diagram),
    })
}

/// Таблица: `t3x4`, `table3`, `tbl`, `t{Имя|Возраст}`. Возвращает (колонки, строки).
fn table_spec(name: &str, has_text: bool) -> Option<(Option<usize>, Option<usize>)> {
    let (rest, bare_ok) = if let Some(r) = name.strip_prefix("table") {
        (r, true)
    } else if let Some(r) = name.strip_prefix("tbl") {
        (r, true)
    } else {
        (name.strip_prefix('t')?, false)
    };
    if rest.is_empty() {
        return (bare_ok || has_text).then_some((None, None));
    }
    let (c, r) = match rest.split_once(['x', 'х', '×']) {
        Some((c, r)) => (c, Some(r)),
        None => (rest, None),
    };
    let cols = if c.is_empty() { None } else { Some(c.parse::<usize>().ok().filter(|n| (1..=20).contains(n))?) };
    let rows = match r {
        Some(r) => Some(r.parse::<usize>().ok().filter(|n| (1..=100).contains(n))?),
        None => None,
    };
    (cols.is_some() || rows.is_some()).then_some((cols, rows))
}

/// Одиночный элемент, который разворачивать незачем: `-`, `1.`, `q`, `p`, `hr`.
fn is_trivial(nodes: &[Node]) -> bool {
    matches!(nodes, [Node { item: Item::Atom(a, None), mult: 1, children }]
        if children.is_empty() && matches!(a, Atom::Bullet | Atom::Ordered(_) | Atom::Quote | Atom::Paragraph | Atom::Rule))
}

/// Простые сокращения разворачиваются, но подсказку для них не показываем.
fn is_quiet(nodes: &[Node]) -> bool {
    matches!(nodes, [Node { item: Item::Atom(a, _), mult: 1, children }]
        if children.is_empty() && matches!(a, Atom::Heading(_) | Atom::Task(_)))
}

fn all_inline(nodes: &[Node]) -> bool {
    nodes.iter().all(|n| matches!(&n.item, Item::Atom(a, _) if a.is_inline()) && n.children.is_empty())
}

// ---------------------------------------------------------------------------------------------
// Сборка текста
// ---------------------------------------------------------------------------------------------

/// Кусок результата: пункты списков идут подряд, остальные блоки разделяются пустой строкой.
struct Piece {
    list: bool,
    text: String,
}

fn build(nodes: &[Node], prefix: &str) -> Expansion {
    let mut pieces = Vec::new();
    render_nodes(nodes, 1, &mut pieces);
    let body = join(&pieces);
    // Первая строка уже стоит после префикса, остальным его добавляем.
    let mut out = String::new();
    for (i, line) in body.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(if line.is_empty() { prefix.trim_end() } else { prefix });
        }
        out.push_str(line);
    }
    let (text, stops) = finalize(&out);
    let title = describe(nodes, &text);
    Expansion { text, stops, title }
}

fn render_nodes(nodes: &[Node], num: usize, out: &mut Vec<Piece>) {
    for node in nodes {
        for rep in 0..node.mult {
            let n = if node.mult > 1 { rep + 1 } else { num };
            match &node.item {
                Item::Group(inner) => {
                    render_nodes(inner, n, out);
                    render_nodes(&node.children, n, out);
                }
                Item::Atom(atom, text) => render_atom(atom, text.as_deref(), n, rep, &node.children, out),
            }
        }
    }
}

fn render_atom(atom: &Atom, text: Option<&str>, num: usize, rep: usize, children: &[Node], out: &mut Vec<Piece>) {
    let text = text.map(|t| numbered(t, num));
    // Строчные дети становятся содержимым строки: `-*3>link` — три пункта со ссылками.
    let inline_kids = !children.is_empty() && all_inline(children);
    let kids_inline = if inline_kids { Some(inline_join(children, num)) } else { None };
    let content = {
        let mut s = text.clone().unwrap_or_default();
        if let Some(k) = &kids_inline {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(k);
        }
        if s.is_empty() { stop("") } else { s }
    };
    let nested = || -> Option<String> {
        if inline_kids || children.is_empty() {
            return None;
        }
        let mut pieces = Vec::new();
        render_nodes(children, num, &mut pieces);
        Some(join(&pieces))
    };
    let block = |out: &mut Vec<Piece>, text: String| out.push(Piece { list: false, text });

    match atom {
        Atom::Bullet | Atom::Task(_) | Atom::Ordered(_) => {
            let (marker, width) = match atom {
                Atom::Task(false) => ("- [ ] ".to_owned(), 2),
                Atom::Task(true) => ("- [x] ".to_owned(), 2),
                Atom::Ordered(start) => {
                    let m = format!("{}. ", start + rep as u64);
                    let w = m.len();
                    (m, w)
                }
                _ => ("- ".to_owned(), 2),
            };
            let mut s = format!("{marker}{content}");
            if let Some(n) = nested() {
                s.push('\n');
                s.push_str(&indent(&n, &" ".repeat(width)));
            }
            out.push(Piece { list: true, text: s });
        }
        Atom::Quote => {
            let s = match (nested(), &text) {
                (Some(n), None) => indent(&n, "> "),
                (Some(n), Some(_)) => format!("> {content}\n{}", indent(&n, "> ")),
                (None, _) => format!("> {content}"),
            };
            block(out, s);
        }
        Atom::Callout(kind, fold) => {
            let title = text.clone().unwrap_or_else(|| stop(""));
            let body = nested().or(kids_inline).unwrap_or_else(|| stop(""));
            block(out, format!("> [!{kind}]{fold} {title}\n{}", indent(&body, "> ")));
        }
        Atom::Heading(level) => {
            block(out, format!("{} {content}", "#".repeat(*level as usize)));
            if !inline_kids {
                render_nodes(children, num, out);
            }
        }
        Atom::Paragraph => {
            block(out, content);
            if !inline_kids {
                render_nodes(children, num, out);
            }
        }
        Atom::Table { cols, rows } => {
            let headers: Vec<String> =
                text.as_deref().map(|t| t.split(['|', ',']).map(|s| s.trim().to_owned()).collect()).unwrap_or_default();
            let cols = cols.unwrap_or(if headers.is_empty() { 3 } else { headers.len() });
            let rows = rows.unwrap_or(2);
            let mut s = String::from("|");
            for c in 0..cols {
                let h = headers.get(c).filter(|h| !h.is_empty()).cloned().unwrap_or_else(|| stop(&t!("emmet-column", n = c + 1)));
                s.push_str(&format!(" {h} |"));
            }
            s.push_str("\n|");
            s.push_str(&" --- |".repeat(cols));
            for _ in 0..rows {
                s.push_str("\n|");
                for _ in 0..cols {
                    s.push_str(&format!(" {} |", stop("")));
                }
            }
            block(out, s);
            render_nodes(children, num, out);
        }
        Atom::Code(lang) => {
            let title = text.as_deref().map(|t| format!(" title=\"{t}\"")).unwrap_or_default();
            block(out, format!("```{lang}{title}\n{}\n```", stop("")));
            render_nodes(children, num, out);
        }
        Atom::Math => {
            block(out, format!("$$\n{}\n$$", stop("")));
            render_nodes(children, num, out);
        }
        Atom::Diagram(d) => {
            let body = translate_placeholders(d.template).replace('⟨', &OPEN.to_string()).replace('⟩', &CLOSE.to_string());
            block(out, format!("```{}\n{body}\n```", d.lang));
            render_nodes(children, num, out);
        }
        Atom::Front => {
            block(out, format!("---\ntags: [{}]\n---", stop("")));
            render_nodes(children, num, out);
        }
        Atom::Rule => {
            block(out, "---".to_owned());
            render_nodes(children, num, out);
        }
        Atom::Lorem(n) => {
            block(out, lorem(*n));
            render_nodes(children, num, out);
        }
        _ => {
            block(out, inline_text(atom, text.as_deref()));
            render_nodes(children, num, out);
        }
    }
}

fn inline_text(atom: &Atom, text: Option<&str>) -> String {
    let or_stop = |t: Option<&str>| t.map_or_else(|| stop(""), str::to_owned);
    match atom {
        Atom::Image => format!("![[{}]]", or_stop(text)),
        Atom::Link => format!("[[{}]]", or_stop(text)),
        Atom::Kbd => format!("{{{{{}}}}}", or_stop(text)),
        Atom::Badge => format!("`{}`{{badge}}", text.map_or_else(|| stop(&t!("emmet-badge")), str::to_owned)),
        Atom::Date => chrono::Local::now().format("%Y-%m-%d").to_string(),
        Atom::Time => chrono::Local::now().format("%H:%M").to_string(),
        _ => String::new(),
    }
}

fn inline_join(nodes: &[Node], num: usize) -> String {
    let mut parts = Vec::new();
    for node in nodes {
        for rep in 0..node.mult {
            let n = if node.mult > 1 { rep + 1 } else { num };
            if let Item::Atom(atom, text) = &node.item {
                parts.push(inline_text(atom, text.as_deref().map(|t| numbered(t, n)).as_deref()));
            }
        }
    }
    parts.join(" ")
}

fn join(pieces: &[Piece]) -> String {
    let mut s = String::new();
    for (i, p) in pieces.iter().enumerate() {
        if i > 0 {
            s.push_str(if p.list && pieces[i - 1].list { "\n" } else { "\n\n" });
        }
        s.push_str(&p.text);
    }
    s
}

/// Сдвигает строки вправо (для вложенных списков) или добавляет `> ` (для цитат).
fn indent(s: &str, prefix: &str) -> String {
    s.split('\n')
        .map(|l| if l.is_empty() { prefix.trim_end().to_owned() } else { format!("{prefix}{l}") })
        .collect::<Vec<_>>()
        .join("\n")
}

fn stop(placeholder: &str) -> String {
    format!("{OPEN}{placeholder}{CLOSE}")
}

/// `$` → номер повтора, `$$` → с ведущим нулём, `$@3` — счёт с трёх, `\$` — сам знак доллара.
fn numbered(t: &str, num: usize) -> String {
    let c: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\\' && c.get(i + 1) == Some(&'$') {
            out.push('$');
            i += 2;
        } else if c[i] == '$' {
            let mut width = 0;
            while c.get(i) == Some(&'$') {
                width += 1;
                i += 1;
            }
            let mut value = num;
            if c.get(i) == Some(&'@') && c.get(i + 1).is_some_and(char::is_ascii_digit) {
                i += 1;
                let mut base = 0usize;
                // Длинное число не должно переполнить usize (в debug это паника).
                while let Some(d) = c.get(i).and_then(|c| c.to_digit(10)) {
                    base = base.saturating_mul(10).saturating_add(d as usize);
                    i += 1;
                }
                value = base.saturating_add(num.saturating_sub(1));
            }
            out.push_str(&format!("{value:0width$}"));
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

/// Убирает метки мест для ввода и запоминает их позиции в символах.
fn finalize(marked: &str) -> (String, Vec<Range<usize>>) {
    let mut text = String::with_capacity(marked.len());
    let mut stops = Vec::new();
    let mut n = 0;
    let mut open = None;
    for ch in marked.chars() {
        match ch {
            OPEN => open = Some(n),
            CLOSE => {
                if let Some(s) = open.take() {
                    stops.push(s..n);
                }
            }
            _ => {
                text.push(ch);
                n += 1;
            }
        }
    }
    (text, stops)
}

fn lorem(n: usize) -> String {
    const WORDS: &[&str] = &[
        "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed", "do", "eiusmod", "tempor",
        "incididunt", "ut", "labore", "et", "dolore", "magna", "aliqua", "enim", "ad", "minim", "veniam", "quis", "nostrud",
        "exercitation", "ullamco", "laboris", "nisi", "aliquip", "ex", "ea", "commodo", "consequat",
    ];
    let mut s: String = (0..n).map(|i| WORDS[i % WORDS.len()]).collect::<Vec<_>>().join(" ");
    if let Some(first) = s.get(..1) {
        let upper = first.to_uppercase();
        s.replace_range(..1, &upper);
    }
    s.push('.');
    s
}

fn describe(nodes: &[Node], text: &str) -> String {
    if let [Node { item: Item::Atom(atom, label), mult, children }] = nodes {
        if children.is_empty() && *mult > 1 && atom.is_list() {
            let kind = match atom {
                Atom::Task(_) => t!("emmet-desc-task-list"),
                Atom::Ordered(_) => t!("emmet-desc-ordered-list"),
                _ => t!("emmet-desc-list"),
            };
            return t!("emmet-desc-items", kind = kind, count = *mult);
        }
        if children.is_empty() {
            let base = match atom {
                Atom::Heading(l) => t!("emmet-desc-heading", level = *l),
                Atom::Bullet => t!("emmet-desc-bullet"),
                Atom::Ordered(_) => t!("emmet-desc-ordered"),
                Atom::Task(_) => t!("emmet-desc-task"),
                Atom::Quote => t!("emmet-desc-quote"),
                Atom::Callout(kind, _) => t!("emmet-desc-callout", kind = theme::callout(kind).title),
                Atom::Paragraph => t!("emmet-desc-paragraph"),
                Atom::Table { .. } => {
                    let first = text.lines().next().unwrap_or("");
                    let cols = first.matches('|').count().saturating_sub(1);
                    let rows = text.lines().count().saturating_sub(2);
                    t!("emmet-desc-table", cols = cols, rows = rows)
                }
                Atom::Code(lang) if lang.is_empty() => t!("emmet-desc-code"),
                Atom::Code(lang) => t!("emmet-desc-code-lang", lang = lang.clone()),
                Atom::Math => t!("emmet-desc-math"),
                Atom::Diagram(d) => forma_i18n::tr(d.title),
                Atom::Front => t!("emmet-desc-front"),
                Atom::Rule => t!("emmet-desc-rule"),
                Atom::Lorem(n) => t!("emmet-desc-lorem", count = *n),
                Atom::Image => match label.as_deref() {
                    Some(name) => t!("emmet-desc-image-named", name = name.to_owned()),
                    None => t!("emmet-desc-image"),
                },
                Atom::Link => t!("emmet-desc-link"),
                Atom::Kbd => t!("emmet-desc-kbd"),
                Atom::Badge => t!("emmet-desc-badge"),
                Atom::Date => t!("emmet-desc-date"),
                Atom::Time => t!("emmet-desc-time"),
            };
            return if *mult > 1 { format!("{base} ×{mult}") } else { base };
        }
    }
    let lines = text.lines().count();
    let has_list = text.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with("- ") || t.split_once(". ").is_some_and(|(d, _)| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
    });
    let what = if has_list && nodes.iter().all(|n| matches!(&n.item, Item::Atom(a, _) if a.is_list())) { t!("emmet-desc-list") } else { t!("emmet-desc-markup") };
    t!("emmet-desc-lines", kind = what, count = lines)
}


// ---------------------------------------------------------------------------------------------
// Заготовки диаграмм
// ---------------------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub struct Diagram {
    names: &'static [&'static str],
    lang: &'static str,
    /// Ключ перевода названия.
    title: &'static str,
    /// Текст с местами для ввода в скобках ⟨…⟩; ⦃имя⦄ — подпись `emmet-ph-<имя>` на языке интерфейса.
    template: &'static str,
}

/// Подставляет переведённые подписи ⦃имя⦄ в заготовку диаграммы.
fn translate_placeholders(template: &str) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('⦃') {
        out.push_str(&rest[..start]);
        let after = &rest[start + '⦃'.len_utf8()..];
        let Some(end) = after.find('⦄') else { break };
        out.push_str(&forma_i18n::tr(&format!("emmet-ph-{}", &after[..end])));
        rest = &after[end + '⦄'.len_utf8()..];
    }
    out.push_str(rest);
    out
}

static DIAGRAMS: &[Diagram] = &[
    Diagram {
        names: &["mermaid", "flow", "flowchart", "mermaid:flow"],
        lang: "mermaid",
        title: "emmet-diagram-flow",
        template: "flowchart TD\n    A[⟨⦃start⦄⟩] --> B{⟨⦃condition⦄⟩}\n    B -- ⦃yes⦄ --> C[⟨⦃action⦄⟩]\n    B -- ⦃no⦄ --> D[⟨⦃other⦄⟩]",
    },
    Diagram {
        names: &["seq", "sequence", "mermaid:seq"],
        lang: "mermaid",
        title: "emmet-diagram-seq",
        template: "sequenceDiagram\n    participant A as ⟨⦃client⦄⟩\n    participant B as ⟨⦃server⦄⟩\n    A->>B: ⟨⦃request⦄⟩\n    B-->>A: ⟨⦃response⦄⟩",
    },
    Diagram {
        names: &["classd", "mermaid:class"],
        lang: "mermaid",
        title: "emmet-diagram-class",
        template: "classDiagram\n    class ⟨Note⟩ {\n        +String ⟨title⟩\n        +⟨open()⟩\n    }",
    },
    Diagram {
        names: &["stated", "mermaid:state"],
        lang: "mermaid",
        title: "emmet-diagram-state",
        template: "stateDiagram-v2\n    state \"⟨⦃idle⦄⟩\" as S1\n    state \"⟨⦃working⦄⟩\" as S2\n    [*] --> S1\n    S1 --> S2 : ⟨⦃go⦄⟩\n    S2 --> [*]",
    },
    Diagram {
        names: &["pie", "mermaid:pie"],
        lang: "mermaid",
        title: "emmet-diagram-pie",
        template: "pie title ⟨⦃title⦄⟩\n    \"⟨⦃first⦄⟩\" : 50\n    \"⟨⦃second⦄⟩\" : 30\n    \"⟨⦃third⦄⟩\" : 20",
    },
    Diagram {
        names: &["dot", "graphviz", "digraph"],
        lang: "dot",
        title: "emmet-diagram-dot",
        template: "digraph G {\n    a [label=\"⟨⦃start⦄⟩\"];\n    b [label=\"⟨⦃end⦄⟩\"];\n    a -> b;\n}",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(abbr: &str) -> Expansion {
        let found = at_cursor(abbr, abbr.len()).unwrap_or_else(|| panic!("не развернулось: {abbr}"));
        found.expansion
    }

    /// Текст с местами для ввода, отмеченными как `[…]`.
    fn marked(e: &Expansion) -> String {
        let chars: Vec<char> = e.text.chars().collect();
        let mut out = String::new();
        for (i, ch) in chars.iter().enumerate() {
            for s in &e.stops {
                if s.start == i {
                    out.push('[');
                }
                if s.end == i {
                    out.push(']');
                }
            }
            out.push(*ch);
        }
        for s in &e.stops {
            if s.start == chars.len() {
                out.push('[');
            }
            if s.end == chars.len() {
                out.push(']');
            }
        }
        out
    }

    #[test]
    fn lists_and_nesting() {
        assert_eq!(marked(&expand("-*3")), "- []\n- []\n- []");
        assert_eq!(marked(&expand("ol*2>[]*2")), "1. []\n   - [ ] []\n   - [ ] []\n2. []\n   - [ ] []\n   - [ ] []");
        assert_eq!(expand("-{Шаг $}*3").text, "- Шаг 1\n- Шаг 2\n- Шаг 3");
        assert_eq!(expand("-{$$@9}*2").text, "- 09\n- 10");
    }

    #[test]
    fn siblings_groups_and_headings() {
        assert_eq!(expand("h2{План}+[]{Купить хлеб}+[x]{Сделать}").text, "## План\n\n- [ ] Купить хлеб\n- [x] Сделать");
        assert_eq!(expand("(h3{Раздел $}+p{Текст})*2").text, "### Раздел 1\n\nТекст\n\n### Раздел 2\n\nТекст");
        // Строчные дети становятся содержимым пункта.
        assert_eq!(marked(&expand("-*2>link")), "- [[[]]]\n- [[[]]]");
    }

    #[test]
    fn tables() {
        crate::i18n::init();
        let e = expand("t2x1");
        assert_eq!(marked(&e), "| [Колонка 1] | [Колонка 2] |\n| --- | --- |\n| [] | [] |");
        assert_eq!(e.title, "Таблица 2×1");
        assert_eq!(expand("t{Имя|Возраст}х2").text, "| Имя | Возраст |\n| --- | --- |\n|  |  |\n|  |  |");
        assert!(at_cursor("t", 1).is_none(), "одиночная t — не таблица");
        assert!(at_cursor("text", 4).is_none());
    }

    #[test]
    fn blocks() {
        assert_eq!(marked(&expand("!tip{Совет}")), "> [!tip] Совет\n> []");
        assert_eq!(marked(&expand("!question-")), "> [!question]- []\n> []");
        assert_eq!(marked(&expand("code:rust")), "```rust\n[]\n```");
        assert_eq!(marked(&expand("```js")), "```js\n[]\n```");
        assert!(at_cursor("!nonsense", 9).is_none());
        let e = expand("flow");
        assert!(e.text.starts_with("```mermaid\nflowchart TD"));
        assert_eq!(e.stops.len(), 4);
    }

    #[test]
    fn prefix_and_inline() {
        // Отступ и маркеры цитаты повторяются на новых строках.
        let src = "  > -*2";
        let f = at_cursor(src, src.len()).unwrap();
        assert_eq!(f.range, 4..7);
        assert_eq!(f.expansion.text, "- \n  > - ");
        // Слово посреди текста разворачивается, только если сокращение строчное.
        let src = "смотри img";
        let f = at_cursor(src, src.len()).unwrap();
        assert!(!f.whole_line);
        assert_eq!(f.expansion.text, "![[]]");
        assert!(at_cursor("смотри -*3", "смотри -*3".len()).is_none());
        assert!(at_cursor("kbd{Ctrl S}", 11).is_some_and(|f| f.expansion.text == "{{Ctrl S}}"));
    }

    #[test]
    fn not_abbreviations() {
        for s in ["-", "1.", "p", "hello", "h7", "- ", "Привет", "a+b"] {
            assert!(at_cursor(s, s.len()).is_none(), "{s}");
        }
        // Внутри блока кода ничего не разворачиваем.
        let src = "```\nt3x3";
        assert!(at_cursor(src, src.len()).is_none());
    }

    #[test]
    fn size_limits() {
        // Строки после заголовков таблицы ограничены так же, как в `t3x4`.
        assert_eq!(expand("t{A|B}x100").text.lines().count(), 102);
        for s in ["t{A|B}x101", "t{A|B}x0", "t{A|B}x99999999999999999999"] {
            assert!(at_cursor(s, s.len()).is_none(), "{s}");
        }
        // Множители вложенных элементов перемножаются; больше MAX_LINES строк не строим вовсе.
        assert_eq!(expand("-*100>-*9").text.lines().count(), MAX_LINES);
        let deep = "-*100>".repeat(11) + "-*100";
        for s in ["-*100>-*10", "-*100>-*100>-*100", "(-*100)*100", "t3x100*10", deep.as_str()] {
            assert!(at_cursor(s, s.len()).is_none(), "{s}");
        }
    }

    #[test]
    fn huge_numbering_base_saturates() {
        // `$@` с огромным числом не переполняет usize, а упирается в предел.
        assert_eq!(numbered("$@99999999999999999999999", 2), usize::MAX.to_string());
        assert_eq!(numbered("$@5", 3), "7");
    }
}
