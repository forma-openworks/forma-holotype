//! Модель «узлы и рёбра» и послойная раскладка в духе dagre (её же использует mermaid):
//! 1) ранги узлов, 2) фиктивные узлы на длинных рёбрах и под подписями стрелок,
//! 3) порядок узлов в слоях методом барицентров, 4) координаты без наложений,
//! 5) рёбра — гладкие кривые через фиктивные узлы.

use std::collections::HashMap;

use eframe::egui::{Color32, Pos2, Rect, Vec2, pos2, vec2};

use super::scene::{Anchor, Dash, Font, Paint, Prim, Role, Scene};
use super::text::{FONT, LINE, Measure, parse_color};

/// Кегль подписей на стрелках.
const LABEL_FONT: f32 = FONT * 0.9;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Dir {
    #[default]
    TB,
    BT,
    LR,
    RL,
}

impl Dir {
    pub fn parse(s: &str) -> Option<Dir> {
        Some(match s.trim().to_uppercase().as_str() {
            "TB" | "TD" => Dir::TB,
            "BT" => Dir::BT,
            "LR" => Dir::LR,
            "RL" => Dir::RL,
            _ => return None,
        })
    }

    pub fn horizontal(self) -> bool {
        matches!(self, Dir::LR | Dir::RL)
    }

    /// Единичный вектор «вниз по рангам».
    fn main(self) -> Vec2 {
        match self {
            Dir::TB => vec2(0.0, 1.0),
            Dir::BT => vec2(0.0, -1.0),
            Dir::LR => vec2(1.0, 0.0),
            Dir::RL => vec2(-1.0, 0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ClassBox {
    pub annotation: Option<String>,
    pub attrs: Vec<String>,
    pub methods: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Rect,
    Round,
    Stadium,
    Subroutine,
    Cylinder,
    Circle,
    DoubleCircle,
    Ellipse,
    Flag,
    Rhombus,
    Hexagon,
    /// Параллелограмм; `true` — наклон вправо (`[/текст/]`).
    Lean(bool),
    /// Трапеция; `true` — перевёрнутая (`[\текст/]`).
    Trapezoid(bool),
    /// Начальное и конечное состояние.
    Start,
    End,
    /// Развилка/слияние (fork/join).
    Bar,
    Point,
    /// Только текст, без рамки.
    Plain,
    Note,
    Class(ClassBox),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeStyle {
    pub fill: Option<Color32>,
    pub stroke: Option<Color32>,
    pub text: Option<Color32>,
    pub width: Option<f32>,
    pub dashed: bool,
}

impl NodeStyle {
    /// Накладывает заданные свойства поверх текущих.
    pub fn merge(&mut self, o: &NodeStyle) {
        self.fill = o.fill.or(self.fill);
        self.stroke = o.stroke.or(self.stroke);
        self.text = o.text.or(self.text);
        self.width = o.width.or(self.width);
        self.dashed |= o.dashed;
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    pub label: Vec<String>,
    pub shape: Shape,
    pub style: NodeStyle,
    pub cluster: Option<usize>,
    /// Подпись задана явно (а не равна идентификатору).
    pub explicit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    Solid,
    Dotted,
    Thick,
    Invisible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    None,
    Arrow,
    /// Открытая стрелка «галочкой».
    Vee,
    Circle,
    Cross,
    /// Полый треугольник (наследование).
    Triangle,
    Diamond,
    DiamondFilled,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub label: Vec<String>,
    pub line: Line,
    pub head_from: Head,
    pub head_to: Head,
    /// Подписи у концов (кратность в диаграмме классов).
    pub tail_text: Option<String>,
    pub head_text: Option<String>,
    pub color: Option<Color32>,
}

#[derive(Clone, Debug)]
pub struct Cluster {
    pub id: String,
    pub title: String,
    pub parent: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub dir: Dir,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub clusters: Vec<Cluster>,
    index: HashMap<String, usize>,
}

impl Graph {
    pub fn new(dir: Dir) -> Self {
        Self { dir, ..Default::default() }
    }

    pub fn find(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    /// Узел по идентификатору (создаётся при первом упоминании).
    pub fn node(&mut self, id: &str) -> usize {
        if let Some(i) = self.find(id) {
            return i;
        }
        self.nodes.push(Node {
            id: id.to_owned(),
            label: vec![id.to_owned()],
            shape: Shape::Rect,
            style: NodeStyle::default(),
            cluster: None,
            explicit: false,
        });
        let i = self.nodes.len() - 1;
        self.index.insert(id.to_owned(), i);
        i
    }

    pub fn edge(&mut self, from: usize, to: usize) -> &mut Edge {
        self.edges.push(Edge {
            from,
            to,
            label: Vec::new(),
            line: Line::Solid,
            head_from: Head::None,
            head_to: Head::Arrow,
            tail_text: None,
            head_text: None,
            color: None,
        });
        self.edges.last_mut().unwrap()
    }

    pub fn cluster(&mut self, id: &str, title: &str, parent: Option<usize>) -> usize {
        self.clusters.push(Cluster { id: id.to_owned(), title: title.to_owned(), parent });
        self.clusters.len() - 1
    }

    /// Рёбра, ведущие к подграфу по его имени, перенаправляем на первый узел подграфа.
    pub fn redirect_cluster_links(&mut self) {
        let mut replace: HashMap<usize, usize> = HashMap::new();
        for (ci, c) in self.clusters.iter().enumerate() {
            let Some(ni) = self.find(&c.id) else { continue };
            if self.nodes[ni].explicit || self.nodes[ni].cluster.is_some() {
                continue;
            }
            if let Some(first) = self.nodes.iter().position(|n| n.cluster == Some(ci)) {
                replace.insert(ni, first);
            }
        }
        if replace.is_empty() {
            return;
        }
        for e in &mut self.edges {
            e.from = *replace.get(&e.from).unwrap_or(&e.from);
            e.to = *replace.get(&e.to).unwrap_or(&e.to);
        }
        // Удаляем узлы-заглушки и пересчитываем индексы.
        let mut map = vec![usize::MAX; self.nodes.len()];
        let mut kept = Vec::new();
        for (i, n) in std::mem::take(&mut self.nodes).into_iter().enumerate() {
            if !replace.contains_key(&i) {
                map[i] = kept.len();
                kept.push(n);
            }
        }
        self.nodes = kept;
        for e in &mut self.edges {
            e.from = map[e.from];
            e.to = map[e.to];
        }
        self.index = self.nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
    }
}

/// Делит строку по запятым верхнего уровня (запятые внутри скобок не считаются).
pub fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// Разбор стиля mermaid: `fill:#f9f,stroke:#333,stroke-width:2px,color:#fff`.
pub fn parse_style(props: &str) -> NodeStyle {
    let mut st = NodeStyle::default();
    for part in split_top(props.trim().trim_end_matches(';'), ',') {
        let Some((k, v)) = part.split_once(':') else { continue };
        let v = v.trim().trim_end_matches(';');
        match k.trim() {
            "fill" => st.fill = parse_color(v),
            "stroke" => st.stroke = parse_color(v),
            "color" => st.text = parse_color(v),
            "stroke-width" => st.width = v.trim_end_matches("px").trim().parse().ok(),
            "stroke-dasharray" => st.dashed = true,
            _ => {}
        }
    }
    st
}

// ============================================================================================
// Размеры узлов
// ============================================================================================

fn text_block(lines: &[String], m: &dyn Measure, font: Font) -> Vec2 {
    let w = lines.iter().map(|l| m.width(l, font)).fold(0.0, f32::max);
    vec2(w, lines.len().max(1) as f32 * LINE * font.size / FONT)
}

const CLASS_MEMBER: f32 = FONT * 0.92;

fn class_sections(n: &Node, cb: &ClassBox) -> (f32, f32, f32) {
    let head = 16.0 + n.label.len() as f32 * LINE + if cb.annotation.is_some() { LINE * 0.85 } else { 0.0 };
    let line = LINE * CLASS_MEMBER / FONT;
    let attrs = if cb.attrs.is_empty() { 10.0 } else { 12.0 + cb.attrs.len() as f32 * line };
    let methods = if cb.methods.is_empty() { 10.0 } else { 12.0 + cb.methods.len() as f32 * line };
    (head, attrs, methods)
}

fn node_size(n: &Node, m: &dyn Measure, dir: Dir) -> Vec2 {
    let t = text_block(&n.label, m, Font::new(FONT));
    let (tw, th) = (t.x, t.y);
    match &n.shape {
        Shape::Rect | Shape::Round | Shape::Note => vec2(tw + 32.0, th + 20.0),
        Shape::Subroutine => vec2(tw + 48.0, th + 20.0),
        Shape::Stadium => {
            let h = th + 20.0;
            vec2(tw + 32.0 + h * 0.3, h)
        }
        Shape::Circle => Vec2::splat((tw + 26.0).max(th + 26.0)),
        Shape::DoubleCircle => Vec2::splat((tw + 26.0).max(th + 26.0) + 10.0),
        Shape::Ellipse => vec2(tw * 1.18 + 36.0, th + 26.0),
        Shape::Rhombus => {
            let h = th + 20.0 + (tw * 0.3).min(48.0);
            let w = tw / (1.0 - th / h).max(0.3) + 12.0;
            vec2(w, h)
        }
        Shape::Hexagon => {
            let h = th + 20.0;
            vec2(tw + 32.0 + h * 0.5, h)
        }
        Shape::Lean(_) | Shape::Trapezoid(_) => {
            let h = th + 20.0;
            vec2(tw + 32.0 + h * 0.6, h)
        }
        Shape::Flag => {
            let h = th + 20.0;
            vec2(tw + 32.0 + h * 0.3, h)
        }
        Shape::Cylinder => {
            let w = tw + 32.0;
            let ry = (w * 0.07).clamp(4.0, 10.0);
            vec2(w, th + 20.0 + ry * 2.0)
        }
        Shape::Start => Vec2::splat(15.0),
        Shape::End => Vec2::splat(19.0),
        Shape::Bar => {
            if dir.horizontal() {
                vec2(8.0, 76.0)
            } else {
                vec2(76.0, 8.0)
            }
        }
        Shape::Point => Vec2::splat(9.0),
        Shape::Plain => vec2(tw + 8.0, th + 6.0),
        Shape::Class(cb) => {
            let name = text_block(&n.label, m, Font::new(FONT).bold()).x;
            let ann = cb.annotation.as_ref().map_or(0.0, |a| m.width(a, Font::new(FONT * 0.85).italic()));
            let mf = Font::new(CLASS_MEMBER);
            let members = cb.attrs.iter().chain(&cb.methods).map(|s| m.width(s, mf)).fold(0.0, f32::max);
            let (a, b, c) = class_sections(n, cb);
            vec2(name.max(ann).max(members) + 28.0, a + b + c).max(vec2(100.0, 0.0))
        }
    }
}

// ============================================================================================
// Раскладка
// ============================================================================================

struct LNode {
    real: Option<usize>,
    /// Размер поперёк рангов и вдоль них.
    cross: f32,
    main: f32,
    rank: usize,
    cluster: Option<usize>,
}

pub fn layout(g: &Graph, m: &dyn Measure) -> Scene {
    let n = g.nodes.len();
    let dir = g.dir;
    let horiz = dir.horizontal();
    let sizes: Vec<Vec2> = g.nodes.iter().map(|nd| node_size(nd, m, dir)).collect();
    let mut has_loop = vec![false; n];
    for e in &g.edges {
        if e.from == e.to {
            has_loop[e.from] = true;
        }
    }

    // Рёбра для раскладки (без петель).
    let ledges: Vec<(usize, usize, usize)> =
        g.edges.iter().enumerate().filter(|(_, e)| e.from != e.to).map(|(i, e)| (e.from, e.to, i)).collect();
    let reversed = break_cycles(n, &ledges);
    let oriented: Vec<(usize, usize, usize)> = ledges
        .iter()
        .zip(&reversed)
        .map(|(&(a, b, i), &r)| if r { (b, a, i) } else { (a, b, i) })
        .collect();

    // 1. Ранги.
    let mut rank = longest_path(n, &oriented);
    let has_labels = oriented.iter().any(|&(_, _, i)| has_label(&g.edges[i]));
    let factor = if has_labels { 2 } else { 1 };
    for r in &mut rank {
        *r *= factor;
    }

    // 2. Слоистый граф с фиктивными узлами.
    let label_font = Font::new(LABEL_FONT);
    let mut ln: Vec<LNode> = (0..n)
        .map(|i| {
            let s = sizes[i];
            let extra = if has_loop[i] { 92.0 } else { 0.0 };
            let (cross, main) = if horiz { (s.y + extra, s.x) } else { (s.x + extra, s.y) };
            LNode { real: Some(i), cross, main, rank: rank[i], cluster: g.nodes[i].cluster }
        })
        .collect();
    let mut chains: Vec<(Vec<usize>, usize, bool)> = Vec::new(); // (цепочка, ребро, развёрнуто)
    let mut label_at: HashMap<usize, usize> = HashMap::new(); // ребро → фиктивный узел подписи
    let mut label_sizes: HashMap<usize, Vec2> = HashMap::new();
    for (k, &(u, v, ei)) in oriented.iter().enumerate() {
        let e = &g.edges[ei];
        let span = rank[v].saturating_sub(rank[u]).max(1);
        let cluster = if g.nodes[u].cluster == g.nodes[v].cluster { g.nodes[u].cluster } else { None };
        let mut chain = vec![u];
        let label_k = span / 2;
        for step in 1..span {
            let (mut cross, mut main) = (0.0, 0.0);
            if step == label_k && has_label(e) {
                let t = text_block(&e.label, m, label_font) + vec2(12.0, 6.0);
                label_sizes.insert(ei, t);
                (cross, main) = if horiz { (t.y, t.x) } else { (t.x, t.y) };
                label_at.insert(ei, ln.len());
            }
            ln.push(LNode { real: None, cross, main, rank: rank[u] + step, cluster });
            chain.push(ln.len() - 1);
        }
        chain.push(v);
        chains.push((chain, ei, reversed[k]));
    }
    let total = ln.len();
    let mut up = vec![Vec::new(); total];
    let mut down = vec![Vec::new(); total];
    for (chain, _, _) in &chains {
        for w in chain.windows(2) {
            down[w[0]].push(w[1]);
            up[w[1]].push(w[0]);
        }
    }
    let ranks = ln.iter().map(|l| l.rank).max().unwrap_or(0) + 1;

    // 3. Порядок в слоях.
    let mut layers = initial_order(&ln, &down, ranks);
    order_layers(&mut layers, &up, &down, &ln, &g.clusters);

    // 4. Координаты поперёк рангов.
    let gap = |a: &LNode, b: &LNode| -> f32 {
        let base = match (a.real.is_some(), b.real.is_some()) {
            (true, true) => {
                if horiz { 26.0 } else { 36.0 }
            }
            (false, false) => 12.0,
            _ => 18.0,
        };
        let cl = if a.cluster != b.cluster && (a.cluster.is_some() || b.cluster.is_some()) { 30.0 } else { 0.0 };
        (a.cross + b.cross) / 2.0 + base + cl
    };
    let mut x = vec![0.0f32; total];
    for layer in &layers {
        let mut cur = 0.0;
        for (k, &v) in layer.iter().enumerate() {
            if k > 0 {
                cur += gap(&ln[layer[k - 1]], &ln[v]);
            }
            x[v] = cur;
        }
        let mid = cur / 2.0;
        for &v in layer {
            x[v] -= mid;
        }
    }
    let weight = |a: usize, b: usize| match (ln[a].real.is_some(), ln[b].real.is_some()) {
        (false, false) => 4.0,
        (true, true) => 1.0,
        _ => 2.0,
    };
    for it in 0..40 {
        let downward = it % 2 == 0;
        let order: Vec<usize> = if downward { (0..layers.len()).collect() } else { (0..layers.len()).rev().collect() };
        for l in order {
            let layer = &layers[l];
            if layer.is_empty() {
                continue;
            }
            let mut desired = Vec::with_capacity(layer.len());
            let mut weights = Vec::with_capacity(layer.len());
            for &v in layer {
                let nbs: Vec<usize> = if it < 10 {
                    if downward { up[v].clone() } else { down[v].clone() }
                } else {
                    up[v].iter().chain(&down[v]).copied().collect()
                };
                if nbs.is_empty() {
                    desired.push(x[v]);
                    weights.push(0.3);
                } else {
                    let (mut sw, mut s) = (0.0, 0.0);
                    for &u in &nbs {
                        let w = weight(u, v);
                        sw += w;
                        s += w * x[u];
                    }
                    desired.push(s / sw);
                    weights.push(if ln[v].real.is_some() { 1.0 } else { 2.0 });
                }
            }
            let mut seps = vec![0.0; layer.len()];
            for k in 1..layer.len() {
                seps[k] = gap(&ln[layer[k - 1]], &ln[layer[k]]);
            }
            let placed = isotonic(&desired, &weights, &seps);
            for (k, &v) in layer.iter().enumerate() {
                x[v] = placed[k];
            }
        }
    }
    // Координаты вдоль рангов.
    let has_end_texts = g.edges.iter().any(|e| e.tail_text.is_some() || e.head_text.is_some());
    let rank_gap = (if horiz { 56.0 } else { 46.0 }
        + if g.clusters.is_empty() { 0.0 } else { 14.0 }
        + if has_end_texts { 16.0 } else { 0.0 })
        / factor as f32;
    let mut max_main = vec![0.0f32; ranks];
    for l in &ln {
        max_main[l.rank] = max_main[l.rank].max(l.main);
    }
    let mut rank_pos = vec![0.0f32; ranks];
    for r in 1..ranks {
        rank_pos[r] = rank_pos[r - 1] + max_main[r - 1] / 2.0 + rank_gap + max_main[r] / 2.0;
    }

    // Длинные рёбра без лишних изгибов.
    let mut slot = vec![0usize; total];
    for layer in &layers {
        for (k, &v) in layer.iter().enumerate() {
            slot[v] = k;
        }
    }
    let rows = Rows { layers: &layers, slot: &slot, ln: &ln, rank_pos: &rank_pos, max_main: &max_main };
    straighten(&chains, &rows, &mut x, gap);
    // Где в своём слое может пройти фиктивный узел: не ближе `gap` к настоящим узлам
    // и подписям и не дальше середины до соседних рёбер, чтобы рёбра не перекрещивались.
    // Подпись стоит на своём фиктивном узле, и ребро проходит ровно через неё.
    let band = |v: usize| -> (f32, f32) {
        if ln[v].cross > 0.0 {
            return (x[v], x[v]);
        }
        let layer = &layers[ln[v].rank];
        let k = slot[v];
        let side = |u: usize| if solid(&ln[u]) { gap(&ln[u], &ln[v]) } else { (x[v] - x[u]).abs() / 2.0 };
        let lo = if k > 0 { x[layer[k - 1]] + side(layer[k - 1]) } else { f32::NEG_INFINITY };
        let hi = if k + 1 < layer.len() { x[layer[k + 1]] - side(layer[k + 1]) } else { f32::INFINITY };
        if lo <= hi { (lo, hi) } else { (x[v], x[v]) }
    };

    let pos_of = |v: usize| -> Pos2 {
        let (c, mm) = (x[v], rank_pos[ln[v].rank]);
        match dir {
            Dir::TB => pos2(c, mm),
            Dir::BT => pos2(c, -mm),
            Dir::LR => pos2(mm, c),
            Dir::RL => pos2(-mm, c),
        }
    };
    let rects: Vec<Rect> = (0..n).map(|i| Rect::from_center_size(pos_of(i), sizes[i])).collect();

    // 5. Рисование.
    let mut scene = Scene::default();
    draw_clusters(&mut scene, g, &rects, m);
    let d = dir.main();
    let c_axis = if horiz { vec2(0.0, 1.0) } else { vec2(1.0, 0.0) };
    let mut labels: Vec<(Pos2, &Edge, Vec2)> = Vec::new();
    for (chain, ei, rev) in &chains {
        let e = &g.edges[*ei];
        if e.line == Line::Invisible {
            continue;
        }
        let pts: Vec<Pos2> = chain.iter().map(|&v| pos_of(v)).collect();
        let (first, last) = (chain[0], *chain.last().unwrap());
        let mut pts = pts;
        let n0 = pts.len();
        let (next, prev) = (pts[1], pts[n0 - 2]);
        pts[0] = port(&g.nodes[first].shape, rects[first], d, next);
        pts[n0 - 1] = port(&g.nodes[last].shape, rects[last], -d, prev);
        // Ряд с узлами ребро пересекает внутри своей полосы, иначе заденет соседей.
        // Через эти «ворота» оно идёт натянутой нитью: изгибается только у углов узлов.
        let (p0, pn) = (pts[0], pts[n0 - 1]);
        let cross = |p: Pos2| p.to_vec2().dot(c_axis);
        let mut gates = Vec::new();
        for (k, &v) in chain.iter().enumerate().take(n0 - 1).skip(1) {
            let half = max_main[ln[v].rank] / 2.0;
            let (lo, hi) = band(v);
            let along = (pts[k] - p0).dot(d);
            if half < 1.0 {
                gates.push((along, lo, hi));
            } else {
                gates.extend([(along - half, lo, hi), (along + half, lo, hi)]);
            }
        }
        let mut path = taut((0.0, cross(p0)), ((pn - p0).dot(d), cross(pn)), &gates);
        // Центр подписи — тоже точка пути: сглаженная кривая пройдёт ровно через него.
        if let Some(&lv) = label_at.get(ei) {
            let along = (pos_of(lv) - p0).dot(d);
            let k = path.iter().position(|p| p.0 > along).unwrap_or(path.len() - 1).max(1);
            path.insert(k, (along, x[lv]));
        }
        let route: Vec<Pos2> = path.iter().map(|&(m, c)| p0 + d * m + c_axis * (c - cross(p0))).collect();
        let curve = smooth(&route, d);
        let (h_start, h_end) = if *rev { (e.head_to, e.head_from) } else { (e.head_from, e.head_to) };
        let (t_start, t_end) = if *rev { (&e.head_text, &e.tail_text) } else { (&e.tail_text, &e.head_text) };
        draw_edge(&mut scene, e, &curve, h_start, h_end, m);
        let color = e.color.map_or(Paint::Role(Role::Muted), Paint::Rgb);
        let n_perp = vec2(-d.y, d.x);
        for (text, p, dirv) in [(t_start, pts[0], d), (t_end, pts[n0 - 1], -d)] {
            if let Some(t) = text {
                let pos = p + dirv * 14.0 + n_perp * 12.0;
                let f = Font::new(FONT * 0.82);
                scene.text(pos, t.clone(), f, Anchor::Start, color, m.width(t, f));
            }
        }
        if let Some(&lv) = label_at.get(ei) {
            labels.push((pos_of(lv), e, label_sizes[ei]));
        }
    }
    // Петли (ребро из узла в него же).
    for e in g.edges.iter().filter(|e| e.from == e.to && e.line != Line::Invisible) {
        let r = rects[e.from];
        let nrm = if horiz { vec2(0.0, -1.0) } else { vec2(1.0, 0.0) };
        let half_c = if horiz { r.height() / 2.0 } else { r.width() / 2.0 };
        let half_m = if horiz { r.width() / 2.0 } else { r.height() / 2.0 };
        let c = r.center();
        let a = c + nrm * half_c - d * half_m * 0.45;
        let b = c + nrm * half_c + d * half_m * 0.45;
        let curve = vec![a, a + nrm * 38.0 - d * 8.0, b + nrm * 38.0 + d * 8.0, b];
        draw_edge(&mut scene, e, &curve, e.head_from, e.head_to, m);
        if has_label(e) {
            let size = text_block(&e.label, m, label_font) + vec2(12.0, 6.0);
            let p = c + nrm * (half_c + 34.0 + if horiz { size.y / 2.0 } else { size.x / 2.0 });
            labels.push((p, e, size));
        }
    }
    for (i, nd) in g.nodes.iter().enumerate() {
        draw_node(&mut scene, nd, rects[i], m);
    }
    for (p, e, size) in labels {
        let r = Rect::from_center_size(p, size);
        scene.push(Prim::Rect { rect: r, radius: 5.0, fill: Role::LabelBg.into(), stroke: Paint::None, width: 0.0, dash: Dash::Solid });
        text_lines(&mut scene, &e.label, r.center(), label_font, Role::Muted.into(), m);
    }
    scene.finish(8.0);
    scene
}

fn has_label(e: &Edge) -> bool {
    e.label.iter().any(|l| !l.is_empty())
}

/// Точка выхода ребра из узла в направлении `d` (на границе фигуры). Она сдвигается
/// вдоль края в сторону соседней точки `toward`, чтобы рёбра к разным соседям не сходились
/// в одну точку.
fn port(shape: &Shape, r: Rect, d: Vec2, toward: Pos2) -> Pos2 {
    let c = r.center();
    let n = vec2(-d.y, d.x);
    let (half_c, half_m) = if d.x != 0.0 { (r.height() / 2.0, r.width() / 2.0) } else { (r.width() / 2.0, r.height() / 2.0) };
    let shift = ((toward - c).dot(n) * 0.35).clamp(-half_c * 0.6, half_c * 0.6);
    let t = if half_c > 0.0 { (shift / half_c).abs() } else { 0.0 };
    // Насколько край фигуры отступает к центру при сдвиге от середины.
    let depth = match shape {
        Shape::Rhombus => half_m * t,
        Shape::Circle | Shape::DoubleCircle | Shape::Ellipse | Shape::Start | Shape::End | Shape::Point => {
            half_m * (1.0 - (1.0 - t * t).max(0.0).sqrt())
        }
        Shape::Hexagon if d.x == 0.0 => 0.0,
        Shape::Hexagon => r.height() * 0.25 * t,
        Shape::Stadium if d.x != 0.0 => half_m * (1.0 - (1.0 - t * t).max(0.0).sqrt()),
        _ => 0.0,
    };
    c + n * shift + d * (half_m - depth)
}

/// Узел, который ребро должно обходить: настоящий узел или подпись ребра.
fn solid(l: &LNode) -> bool {
    l.real.is_some() || l.cross > 0.0
}

/// Слои раскладки и их положение вдоль рангов.
struct Rows<'a> {
    layers: &'a [Vec<usize>],
    slot: &'a [usize],
    ln: &'a [LNode],
    rank_pos: &'a [f32],
    max_main: &'a [f32],
}

/// Раскладывает фиктивные узлы длинных рёбер вдоль натянутой нити между концами ребра.
/// Нить огибает настоящие узлы и подписи, а соседние рёбра только раздвигаются,
/// сохраняя порядок в слое. Без этого ребро петляет от слоя к слою.
fn straighten(chains: &[(Vec<usize>, usize, bool)], rows: &Rows, x: &mut [f32], gap: impl Fn(&LNode, &LNode) -> f32) {
    let Rows { layers, slot, ln, rank_pos, max_main } = *rows;
    let mut desired: Vec<Option<f32>> = vec![None; ln.len()];
    for (chain, _, _) in chains {
        if chain.len() < 3 {
            continue;
        }
        let (a, b) = (chain[0], chain[chain.len() - 1]);
        let start = (rank_pos[ln[a].rank] + ln[a].main / 2.0, x[a]);
        let end = (rank_pos[ln[b].rank] - ln[b].main / 2.0, x[b]);
        let mut gates = Vec::new();
        for &v in &chain[1..chain.len() - 1] {
            let r = ln[v].rank;
            let (layer, k) = (&layers[r], slot[v]);
            let (lo, hi) = if ln[v].cross > 0.0 {
                (x[v], x[v])
            } else {
                let lo = layer[..k].iter().rev().find(|&&u| solid(&ln[u])).map_or(f32::NEG_INFINITY, |&u| x[u] + gap(&ln[u], &ln[v]));
                let hi = layer[k + 1..].iter().find(|&&u| solid(&ln[u])).map_or(f32::INFINITY, |&u| x[u] - gap(&ln[v], &ln[u]));
                if lo <= hi { (lo, hi) } else { (x[v], x[v]) }
            };
            let half = max_main[r] / 2.0;
            if half < 1.0 {
                gates.push((rank_pos[r], lo, hi));
            } else {
                gates.extend([(rank_pos[r] - half, lo, hi), (rank_pos[r] + half, lo, hi)]);
            }
        }
        let path = taut(start, end, &gates);
        for &v in &chain[1..chain.len() - 1] {
            desired[v] = Some(at(&path, rank_pos[ln[v].rank]));
        }
    }
    for layer in layers {
        if layer.iter().all(|&v| desired[v].is_none()) {
            continue;
        }
        let want: Vec<f32> = layer.iter().map(|&v| desired[v].unwrap_or(x[v])).collect();
        let weights: Vec<f32> = layer.iter().map(|&v| if desired[v].is_some() { 1.0 } else { 1000.0 }).collect();
        let mut seps = vec![0.0; layer.len()];
        for k in 1..layer.len() {
            seps[k] = gap(&ln[layer[k - 1]], &ln[layer[k]]);
        }
        let placed = isotonic(&want, &weights, &seps);
        for (k, &v) in layer.iter().enumerate() {
            x[v] = placed[k];
        }
    }
}

/// Значение ломаной `(m, c)` на высоте `m`.
fn at(path: &[(f32, f32)], m: f32) -> f32 {
    for w in path.windows(2) {
        let ((m0, c0), (m1, c1)) = (w[0], w[1]);
        if m <= m1 {
            let t = if m1 > m0 { ((m - m0) / (m1 - m0)).clamp(0.0, 1.0) } else { 1.0 };
            return c0 + (c1 - c0) * t;
        }
    }
    path.last().map_or(0.0, |p| p.1)
}

/// Кратчайший путь от `a` до `b` через ворота. Точки — `(m, c)`: `m` вдоль рангов, `c` поперёк.
/// Ворота `(m, lo, hi)`: на высоте `m` путь проходит между `lo` и `hi`. Путь натягивается
/// как нить и изгибается только на краях ворот, которые иначе пришлось бы пересечь.
fn taut(a: (f32, f32), b: (f32, f32), gates: &[(f32, f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = vec![a];
    let mut start = 0;
    while let Some((k, corner)) = bend(out[out.len() - 1], b, &gates[start..]) {
        out.push(corner);
        start += k + 1;
    }
    out.push(b);
    out
}

/// Первый изгиб нити, натянутой из `apex` к `b` через ворота: номер ворот и край, на котором
/// нить перегибается. `None` — до `b` можно дотянуть прямо.
fn bend(apex: (f32, f32), b: (f32, f32), gates: &[(f32, f32, f32)]) -> Option<(usize, (f32, f32))> {
    // Допустимые наклоны из вершины и ворота, которые их ограничили.
    let (mut lo_s, mut hi_s) = (f32::NEG_INFINITY, f32::INFINITY);
    let (mut lo_k, mut hi_k) = (0, 0);
    for j in 0..=gates.len() {
        let (m, lo, hi) = if j < gates.len() { gates[j] } else { (b.0, b.1, b.1) };
        let dm = (m - apex.0).max(1e-3);
        let (sl, sh) = ((lo - apex.1) / dm, (hi - apex.1) / dm);
        if sh < lo_s {
            return Some((lo_k, (gates[lo_k].0, gates[lo_k].1)));
        }
        if sl > hi_s {
            return Some((hi_k, (gates[hi_k].0, gates[hi_k].2)));
        }
        if sl > lo_s {
            (lo_s, lo_k) = (sl, j);
        }
        if sh < hi_s {
            (hi_s, hi_k) = (sh, j);
        }
    }
    None
}

/// Гладкая кривая по ломаной ребра. Из узла и в узел ребро идёт вдоль рангов, а между
/// ними сглаживается B-сплайном (как `curveBasis` в d3): кривая не обязана проходить
/// через каждую точку, поэтому не виляет на каждом слое.
fn smooth(points: &[Pos2], d: Vec2) -> Vec<Pos2> {
    let n = points.len();
    let (a, b) = (points[0], points[n - 1]);
    let stub = |p: Pos2, q: Pos2| ((q - p).dot(d).abs() * 0.35).min(18.0);
    let mut ctrl = vec![a, a + d * stub(a, points[1])];
    ctrl.extend_from_slice(&points[1..n - 1]);
    ctrl.extend([b - d * stub(points[n - 2], b), b]);

    // Кубические сегменты Безье: p0, c1, c2, p1, …
    let mut out = vec![a];
    let line = |out: &mut Vec<Pos2>, q: Pos2| {
        let p = *out.last().unwrap();
        out.extend([p + (q - p) / 3.0, p + (q - p) * (2.0 / 3.0), q]);
    };
    line(&mut out, a + (ctrl[1] - a) / 6.0);
    for w in ctrl.windows(3) {
        let (p0, p1, p2) = (w[0].to_vec2(), w[1].to_vec2(), w[2].to_vec2());
        out.extend([
            ((p0 * 2.0 + p1) / 3.0).to_pos2(),
            ((p0 + p1 * 2.0) / 3.0).to_pos2(),
            ((p0 + p1 * 4.0 + p2) / 6.0).to_pos2(),
        ]);
    }
    let (p0, p1) = (ctrl[ctrl.len() - 2].to_vec2(), b.to_vec2());
    out.extend([((p0 * 2.0 + p1) / 3.0).to_pos2(), ((p0 + p1 * 2.0) / 3.0).to_pos2(), ((p0 + p1 * 5.0) / 6.0).to_pos2()]);
    line(&mut out, b);
    out
}

/// Разворачивает рёбра, замыкающие циклы (обход в глубину). Возвращает признак разворота.
fn break_cycles(n: usize, edges: &[(usize, usize, usize)]) -> Vec<bool> {
    let mut out_e: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, &(a, _, _)) in edges.iter().enumerate() {
        out_e[a].push(k);
    }
    let mut state = vec![0u8; n]; // 0 — не был, 1 — в стеке, 2 — готов
    let mut reversed = vec![false; edges.len()];
    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        state[start] = 1;
        while let Some(&mut (v, ref mut i)) = stack.last_mut() {
            if *i < out_e[v].len() {
                let k = out_e[v][*i];
                *i += 1;
                let w = edges[k].1;
                match state[w] {
                    0 => {
                        state[w] = 1;
                        stack.push((w, 0));
                    }
                    1 => reversed[k] = true,
                    _ => {}
                }
            } else {
                state[v] = 2;
                stack.pop();
            }
        }
    }
    reversed
}

/// Ранги по самому длинному пути; источники потом подтягиваются к своим потомкам.
fn longest_path(n: usize, edges: &[(usize, usize, usize)]) -> Vec<usize> {
    let mut indeg = vec![0usize; n];
    let mut out_n: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_n: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(a, b, _) in edges {
        indeg[b] += 1;
        out_n[a].push(b);
        in_n[b].push(a);
    }
    let mut rank = vec![0usize; n];
    let mut queue: Vec<usize> = (0..n).filter(|&v| indeg[v] == 0).collect();
    let mut topo = Vec::with_capacity(n);
    let mut qi = 0;
    while qi < queue.len() {
        let v = queue[qi];
        qi += 1;
        topo.push(v);
        for &w in &out_n[v] {
            rank[w] = rank[w].max(rank[v] + 1);
            indeg[w] -= 1;
            if indeg[w] == 0 {
                queue.push(w);
            }
        }
    }
    // Источники ставим прямо над ближайшим потомком, чтобы рёбра не тянулись через весь граф.
    for &v in topo.iter().rev() {
        if in_n[v].is_empty() && !out_n[v].is_empty() {
            let min_child = out_n[v].iter().map(|&w| rank[w]).min().unwrap_or(1);
            rank[v] = min_child.saturating_sub(1);
        }
    }
    rank
}

fn initial_order(ln: &[LNode], down: &[Vec<usize>], ranks: usize) -> Vec<Vec<usize>> {
    let mut layers = vec![Vec::new(); ranks];
    let mut seen = vec![false; ln.len()];
    // Обходим от узлов с меньшим рангом, сохраняя порядок объявления.
    let mut starts: Vec<usize> = (0..ln.len()).filter(|&v| ln[v].real.is_some()).collect();
    starts.sort_by_key(|&v| ln[v].rank);
    for s in starts {
        let mut stack = vec![s];
        while let Some(v) = stack.pop() {
            if seen[v] {
                continue;
            }
            seen[v] = true;
            layers[ln[v].rank].push(v);
            for &w in down[v].iter().rev() {
                if !seen[w] {
                    stack.push(w);
                }
            }
        }
    }
    for v in 0..ln.len() {
        if !seen[v] {
            layers[ln[v].rank].push(v);
        }
    }
    layers
}

fn positions(layers: &[Vec<usize>], total: usize) -> Vec<usize> {
    let mut pos = vec![0; total];
    for layer in layers {
        for (i, &v) in layer.iter().enumerate() {
            pos[v] = i;
        }
    }
    pos
}

fn crossings(layers: &[Vec<usize>], down: &[Vec<usize>], pos: &[usize]) -> usize {
    let mut total = 0;
    for layer in layers {
        let mut es: Vec<(usize, usize)> = Vec::new();
        for &v in layer {
            for &w in &down[v] {
                es.push((pos[v], pos[w]));
            }
        }
        for i in 0..es.len() {
            for j in i + 1..es.len() {
                let (a, b) = (es[i], es[j]);
                if (a.0 < b.0 && a.1 > b.1) || (a.0 > b.0 && a.1 < b.1) {
                    total += 1;
                }
            }
        }
    }
    total
}

/// Верхний подграф узла: по нему узлы одного подграфа держатся в слое рядом.
fn top_cluster(c: Option<usize>, clusters: &[Cluster]) -> Option<usize> {
    let mut c = c?;
    let mut guard = 0;
    while let Some(p) = clusters[c].parent {
        c = p;
        guard += 1;
        if guard > 64 {
            break;
        }
    }
    Some(c)
}

fn order_layers(layers: &mut [Vec<usize>], up: &[Vec<usize>], down: &[Vec<usize>], ln: &[LNode], clusters: &[Cluster]) {
    let total = ln.len();
    let groups: Vec<Option<usize>> = ln.iter().map(|l| top_cluster(l.cluster, clusters)).collect();
    let mut pos = positions(layers, total);
    let mut best = layers.to_vec();
    let mut best_c = crossings(layers, down, &pos);
    for it in 0..24 {
        if best_c == 0 {
            break;
        }
        let downward = it % 2 == 0;
        let idx: Vec<usize> = if downward { (1..layers.len()).collect() } else { (0..layers.len().saturating_sub(1)).rev().collect() };
        for l in idx {
            let nb = if downward { up } else { down };
            let bary: HashMap<usize, f32> = layers[l]
                .iter()
                .map(|&v| {
                    let b = if nb[v].is_empty() {
                        pos[v] as f32
                    } else {
                        nb[v].iter().map(|&u| pos[u] as f32).sum::<f32>() / nb[v].len() as f32
                    };
                    (v, b)
                })
                .collect();
            // Узлы одного подграфа сортируем по общему барицентру — так они остаются рядом.
            let mut gsum: HashMap<usize, (f32, usize)> = HashMap::new();
            for &v in &layers[l] {
                if let Some(gid) = groups[v] {
                    let e = gsum.entry(gid).or_insert((0.0, 0));
                    e.0 += bary[&v];
                    e.1 += 1;
                }
            }
            let key = |v: usize| -> (f32, f32) {
                let b = bary[&v];
                match groups[v] {
                    Some(gid) => {
                        let (s, c) = gsum[&gid];
                        (s / c as f32, b)
                    }
                    None => (b, b),
                }
            };
            layers[l].sort_by(|&a, &b| {
                let (ka, kb) = (key(a), key(b));
                ka.0.total_cmp(&kb.0).then(ka.1.total_cmp(&kb.1))
            });
            for (i, &v) in layers[l].iter().enumerate() {
                pos[v] = i;
            }
        }
        let c = crossings(layers, down, &pos);
        if c < best_c {
            best_c = c;
            best = layers.to_vec();
        }
    }
    layers.clone_from_slice(&best);
}

/// Изотоническая регрессия с минимальными зазорами: ближайшие к желаемым позиции,
/// при которых порядок сохраняется и соседи не наезжают друг на друга.
fn isotonic(desired: &[f32], weights: &[f32], seps: &[f32]) -> Vec<f32> {
    let n = desired.len();
    let mut s = vec![0.0f32; n];
    for k in 1..n {
        s[k] = s[k - 1] + seps[k];
    }
    // Блоки: (сумма весов, взвешенная сумма, длина).
    let mut blocks: Vec<(f32, f32, usize)> = Vec::with_capacity(n);
    for k in 0..n {
        blocks.push((weights[k], weights[k] * (desired[k] - s[k]), 1));
        while blocks.len() >= 2 {
            let (w1, t1, _) = blocks[blocks.len() - 2];
            let (w2, t2, _) = blocks[blocks.len() - 1];
            if t1 / w1 > t2 / w2 {
                let b = blocks.pop().unwrap();
                let last = blocks.last_mut().unwrap();
                last.0 += b.0;
                last.1 += b.1;
                last.2 += b.2;
            } else {
                break;
            }
        }
    }
    let mut out = Vec::with_capacity(n);
    let mut k = 0;
    for (w, t, len) in blocks {
        for _ in 0..len {
            out.push(t / w + s[k]);
            k += 1;
        }
    }
    out
}

// ============================================================================================
// Рисование
// ============================================================================================

fn text_lines(scene: &mut Scene, lines: &[String], center: Pos2, font: Font, color: Paint, m: &dyn Measure) {
    let lh = LINE * font.size / FONT;
    let y0 = center.y - (lines.len() as f32 - 1.0) * lh / 2.0;
    for (i, l) in lines.iter().enumerate() {
        scene.text(pos2(center.x, y0 + i as f32 * lh), l.clone(), font, Anchor::Middle, color, m.width(l, font));
    }
}

fn draw_clusters(scene: &mut Scene, g: &Graph, rects: &[Rect], m: &dyn Measure) {
    if g.clusters.is_empty() {
        return;
    }
    let depth = |mut c: usize| {
        let mut d = 0;
        while let Some(p) = g.clusters[c].parent {
            c = p;
            d += 1;
            if d > 64 {
                break;
            }
        }
        d
    };
    let mut order: Vec<usize> = (0..g.clusters.len()).collect();
    order.sort_by_key(|&c| std::cmp::Reverse(depth(c)));
    let mut boxes: Vec<Option<Rect>> = vec![None; g.clusters.len()];
    let title_font = Font::new(FONT * 0.9).bold();
    for &c in &order {
        let mut r: Option<Rect> = None;
        for (i, nd) in g.nodes.iter().enumerate() {
            if nd.cluster == Some(c) {
                r = Some(r.map_or(rects[i], |r| r.union(rects[i])));
            }
        }
        for (k, ch) in g.clusters.iter().enumerate() {
            if ch.parent == Some(c)
                && let Some(b) = boxes[k]
            {
                r = Some(r.map_or(b, |r| r.union(b)));
            }
        }
        let Some(r) = r else { continue };
        let title = &g.clusters[c].title;
        let tw = m.width(title, title_font);
        let mut b = r.expand(14.0);
        if !title.is_empty() {
            b.min.y -= LINE + 2.0;
            let need = tw + 28.0;
            if b.width() < need {
                let extra = (need - b.width()) / 2.0;
                b = b.expand2(vec2(extra, 0.0));
            }
        }
        boxes[c] = Some(b);
    }
    for &c in order.iter().rev() {
        let Some(b) = boxes[c] else { continue };
        scene.push(Prim::Rect {
            rect: b,
            radius: 10.0,
            fill: Role::ClusterFill.into(),
            stroke: Role::ClusterStroke.into(),
            width: 1.2,
            dash: Dash::Solid,
        });
        let title = &g.clusters[c].title;
        scene.text(
            pos2(b.left() + 12.0, b.top() + 8.0 + LINE * 0.45),
            title.clone(),
            title_font,
            Anchor::Start,
            Role::Muted,
            m.width(title, title_font),
        );
    }
}

fn draw_edge(scene: &mut Scene, e: &Edge, curve: &[Pos2], h_start: Head, h_end: Head, _m: &dyn Measure) {
    let color = e.color.map_or(Paint::Role(Role::Edge), Paint::Rgb);
    let (width, dash) = match e.line {
        Line::Solid => (1.5, Dash::Solid),
        Line::Dotted => (1.5, Dash::Dotted),
        Line::Thick => (3.0, Dash::Solid),
        Line::Invisible => return,
    };
    scene.push(Prim::Curve { pts: curve.to_vec(), stroke: color, width, dash });
    let n = curve.len();
    if n >= 4 {
        let end = curve[n - 1];
        let dir_end = (end - curve[n - 2]).normalized();
        head(scene, end, dir_end, h_end, color, width);
        let start = curve[0];
        let dir_start = (start - curve[1]).normalized();
        head(scene, start, dir_start, h_start, color, width);
    }
}

/// Наконечник ребра: остриё в `tip`, `u` — направление к острию.
pub fn head(scene: &mut Scene, tip: Pos2, u: Vec2, kind: Head, color: Paint, width: f32) {
    if !u.is_finite() || u == Vec2::ZERO {
        return;
    }
    let nrm = vec2(-u.y, u.x);
    let k = (width / 1.5).max(1.0).sqrt();
    match kind {
        Head::None => {}
        Head::Arrow => {
            let (l, w) = (10.0 * k, 4.2 * k);
            let base = tip - u * l;
            scene.push(Prim::Poly {
                pts: vec![tip, base + nrm * w, base - u * -1.5 * k, base - nrm * w],
                closed: true,
                fill: color,
                stroke: Paint::None,
                width: 0.0,
                dash: Dash::Solid,
            });
        }
        Head::Vee => {
            let base = tip - u * 10.0;
            scene.push(Prim::Poly {
                pts: vec![base + nrm * 5.5, tip, base - nrm * 5.5],
                closed: false,
                fill: Paint::None,
                stroke: color,
                width: 1.5,
                dash: Dash::Solid,
            });
        }
        Head::Circle => {
            scene.push(Prim::Ellipse { center: tip - u * 4.5, radius: Vec2::splat(4.0), fill: Role::Bg.into(), stroke: color, width: 1.5 });
        }
        Head::Cross => {
            let c = tip - u * 5.0;
            for s in [1.0, -1.0] {
                let a = c + (u + nrm * s) * 3.8;
                let b = c - (u + nrm * s) * 3.8;
                scene.push(Prim::Poly { pts: vec![a, b], closed: false, fill: Paint::None, stroke: color, width: 1.6, dash: Dash::Solid });
            }
        }
        Head::Triangle => {
            let base = tip - u * 13.0;
            scene.push(Prim::Poly {
                pts: vec![tip, base + nrm * 7.0, base - nrm * 7.0],
                closed: true,
                fill: Role::Bg.into(),
                stroke: color,
                width: 1.5,
                dash: Dash::Solid,
            });
        }
        Head::Diamond | Head::DiamondFilled => {
            let mid = tip - u * 8.0;
            let fill = if kind == Head::DiamondFilled { color } else { Role::Bg.into() };
            scene.push(Prim::Poly {
                pts: vec![tip, mid + nrm * 5.5, tip - u * 16.0, mid - nrm * 5.5],
                closed: true,
                fill,
                stroke: color,
                width: 1.5,
                dash: Dash::Solid,
            });
        }
    }
}

pub fn draw_node(scene: &mut Scene, nd: &Node, r: Rect, m: &dyn Measure) {
    let st = &nd.style;
    let note = nd.shape == Shape::Note;
    let fill = st.fill.map_or(Paint::Role(if note { Role::NoteFill } else { Role::NodeFill }), Paint::Rgb);
    let stroke = st.stroke.map_or(Paint::Role(if note { Role::NoteStroke } else { Role::NodeStroke }), Paint::Rgb);
    let text = st.text.map_or(Paint::Role(Role::Text), Paint::Rgb);
    let width = st.width.unwrap_or(1.4);
    let dash = if st.dashed { Dash::Dashed } else { Dash::Solid };
    let (l, t, rr, b) = (r.left(), r.top(), r.right(), r.bottom());
    let c = r.center();
    let poly = |scene: &mut Scene, pts: Vec<Pos2>| {
        scene.push(Prim::Poly { pts, closed: true, fill, stroke, width, dash });
    };
    let rect = |scene: &mut Scene, radius: f32| {
        scene.push(Prim::Rect { rect: r, radius, fill, stroke, width, dash });
    };
    match &nd.shape {
        Shape::Rect | Shape::Note => rect(scene, 5.0),
        Shape::Round => rect(scene, 12.0),
        Shape::Stadium => rect(scene, r.height() / 2.0),
        Shape::Subroutine => {
            rect(scene, 4.0);
            for xx in [l + 8.0, rr - 8.0] {
                scene.push(Prim::Poly { pts: vec![pos2(xx, t), pos2(xx, b)], closed: false, fill: Paint::None, stroke, width, dash });
            }
        }
        Shape::Cylinder => {
            let ry = (r.width() * 0.07).clamp(4.0, 10.0);
            let rx = r.width() / 2.0;
            scene.push(Prim::Ellipse { center: pos2(c.x, b - ry), radius: vec2(rx, ry), fill, stroke, width });
            scene.push(Prim::Rect {
                rect: Rect::from_min_max(pos2(l, t + ry), pos2(rr, b - ry)),
                radius: 0.0,
                fill,
                stroke: Paint::None,
                width: 0.0,
                dash,
            });
            for xx in [l, rr] {
                scene.push(Prim::Poly {
                    pts: vec![pos2(xx, t + ry), pos2(xx, b - ry)],
                    closed: false,
                    fill: Paint::None,
                    stroke,
                    width,
                    dash,
                });
            }
            scene.push(Prim::Ellipse { center: pos2(c.x, t + ry), radius: vec2(rx, ry), fill, stroke, width });
        }
        Shape::Circle => {
            scene.push(Prim::Ellipse { center: c, radius: Vec2::splat(r.width() / 2.0), fill, stroke, width });
        }
        Shape::DoubleCircle => {
            scene.push(Prim::Ellipse { center: c, radius: Vec2::splat(r.width() / 2.0), fill: Paint::None, stroke, width });
            scene.push(Prim::Ellipse { center: c, radius: Vec2::splat(r.width() / 2.0 - 5.0), fill, stroke, width });
        }
        Shape::Ellipse => {
            scene.push(Prim::Ellipse { center: c, radius: r.size() / 2.0, fill, stroke, width });
        }
        Shape::Flag => {
            let k = r.height() * 0.3;
            poly(scene, vec![pos2(l, t), pos2(rr, t), pos2(rr, b), pos2(l, b), pos2(l + k, c.y)]);
        }
        Shape::Rhombus => poly(scene, vec![pos2(c.x, t), pos2(rr, c.y), pos2(c.x, b), pos2(l, c.y)]),
        Shape::Hexagon => {
            let k = r.height() * 0.25;
            poly(scene, vec![pos2(l + k, t), pos2(rr - k, t), pos2(rr, c.y), pos2(rr - k, b), pos2(l + k, b), pos2(l, c.y)]);
        }
        Shape::Lean(right) => {
            let k = r.height() * 0.3;
            if *right {
                poly(scene, vec![pos2(l + k, t), pos2(rr, t), pos2(rr - k, b), pos2(l, b)]);
            } else {
                poly(scene, vec![pos2(l, t), pos2(rr - k, t), pos2(rr, b), pos2(l + k, b)]);
            }
        }
        Shape::Trapezoid(inv) => {
            let k = r.height() * 0.3;
            if *inv {
                poly(scene, vec![pos2(l, t), pos2(rr, t), pos2(rr - k, b), pos2(l + k, b)]);
            } else {
                poly(scene, vec![pos2(l + k, t), pos2(rr - k, t), pos2(rr, b), pos2(l, b)]);
            }
        }
        Shape::Start => {
            let f = st.fill.map_or(Paint::Role(Role::Strong), Paint::Rgb);
            scene.push(Prim::Ellipse { center: c, radius: Vec2::splat(r.width() / 2.0), fill: f, stroke: Paint::None, width: 0.0 });
            return;
        }
        Shape::End => {
            let f = st.fill.map_or(Paint::Role(Role::Strong), Paint::Rgb);
            scene.push(Prim::Ellipse { center: c, radius: Vec2::splat(r.width() / 2.0), fill: Role::Bg.into(), stroke: f, width: 1.5 });
            scene.push(Prim::Ellipse { center: c, radius: Vec2::splat(r.width() / 2.0 - 4.0), fill: f, stroke: Paint::None, width: 0.0 });
            return;
        }
        Shape::Bar => {
            let f = st.fill.map_or(Paint::Role(Role::Strong), Paint::Rgb);
            scene.push(Prim::Rect { rect: r, radius: 2.0, fill: f, stroke: Paint::None, width: 0.0, dash: Dash::Solid });
            return;
        }
        Shape::Point => {
            let f = st.fill.map_or(Paint::Role(Role::Strong), Paint::Rgb);
            scene.push(Prim::Ellipse { center: c, radius: Vec2::splat(r.width() / 2.0), fill: f, stroke: Paint::None, width: 0.0 });
            return;
        }
        Shape::Plain => {}
        Shape::Class(cb) => {
            rect(scene, 5.0);
            let (head_h, attrs_h, _) = class_sections(nd, cb);
            let mut y = t + 8.0;
            if let Some(a) = &cb.annotation {
                let f = Font::new(FONT * 0.85).italic();
                scene.text(pos2(c.x, y + LINE * 0.85 / 2.0), a.clone(), f, Anchor::Middle, Role::Muted, m.width(a, f));
                y += LINE * 0.85;
            }
            let bold = Font::new(FONT).bold();
            for name in &nd.label {
                scene.text(pos2(c.x, y + LINE / 2.0), name.clone(), bold, Anchor::Middle, text, m.width(name, bold));
                y += LINE;
            }
            let divider = |scene: &mut Scene, y: f32| {
                scene.push(Prim::Poly { pts: vec![pos2(l, y), pos2(rr, y)], closed: false, fill: Paint::None, stroke, width: 1.0, dash: Dash::Solid });
            };
            let y1 = t + head_h;
            divider(scene, y1);
            let y2 = y1 + attrs_h;
            divider(scene, y2);
            let mf = Font::new(CLASS_MEMBER);
            let lh = LINE * CLASS_MEMBER / FONT;
            for (start, list) in [(y1, &cb.attrs), (y2, &cb.methods)] {
                for (i, s) in list.iter().enumerate() {
                    let p = pos2(l + 12.0, start + 6.0 + lh * (i as f32 + 0.5));
                    scene.text(p, s.clone(), mf, Anchor::Start, text, m.width(s, mf));
                }
            }
            return;
        }
    }
    text_lines(scene, &nd.label, c, Font::new(FONT), text, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::scene::flatten;
    use crate::diagram::text::Approx;

    #[test]
    fn isotonic_keeps_order_and_gaps() {
        let x = isotonic(&[0.0, 0.0, 0.0], &[1.0, 1.0, 1.0], &[0.0, 10.0, 10.0]);
        assert!((x[1] - x[0] - 10.0).abs() < 1e-3 && (x[2] - x[1] - 10.0).abs() < 1e-3);
        assert!((x[1]).abs() < 1e-3, "блок должен центрироваться: {x:?}");
        let x = isotonic(&[0.0, 100.0], &[1.0, 1.0], &[0.0, 10.0]);
        assert_eq!(x, vec![0.0, 100.0]);
    }

    #[test]
    fn cycle_is_laid_out() {
        let mut g = Graph::new(Dir::TB);
        let a = g.node("A");
        let b = g.node("B");
        let c = g.node("C");
        g.edge(a, b);
        g.edge(b, c);
        g.edge(c, a).label = vec!["назад".into()];
        let s = layout(&g, &Approx);
        assert!(s.size.x > 0.0 && s.size.y > 0.0 && s.size.is_finite());
    }


    /// Схема со скриншота Холо: подпись на одном ребре даёт фиктивный узел каждому ребру,
    /// а два ребра идут через два слоя мимо других узлов.
    const LONG_EDGES: &str = "flowchart TD
    H[Holotype: чтение, Спросить Мьюри]
    R[RustLIDE: code, cargo, практика]
    L[learn CLI]
    E[learn-core: движок курса]
    A[forma-assistant: мост к Claude Code]
    C[claude с подпиской Холо]
    V[Хранилище RustLearn]
    H --> A
    R --> A
    R --> E
    L --> E
    L --> A
    A --> C
    E --> V
    H --> V
    C -->|читает| V";

    fn curves(s: &Scene) -> Vec<Vec<Pos2>> {
        s.prims.iter().filter_map(|p| if let Prim::Curve { pts, .. } = p { Some(flatten(pts)) } else { None }).collect()
    }

    #[test]
    fn edges_do_not_wiggle() {
        let (_, s) = crate::diagram::render("mermaid", LONG_EDGES, &Approx).unwrap();
        let all = curves(&s);
        assert_eq!(all.len(), 9);
        for c in all {
            // Поперёк рангов ребро движется в одну сторону: без лесенки и волн.
            let right = c.windows(2).any(|w| w[1].x > w[0].x + 0.5);
            let left = c.windows(2).any(|w| w[1].x < w[0].x - 0.5);
            assert!(!(right && left), "ребро виляет: {c:?}");
        }
    }

    #[test]
    fn edges_go_around_nodes() {
        let (_, s) = crate::diagram::render("mermaid", LONG_EDGES, &Approx).unwrap();
        let nodes: Vec<Rect> = s
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Rect { rect, fill, .. } if !matches!(fill, Paint::Role(Role::LabelBg)) => Some(rect.shrink(2.0)),
                _ => None,
            })
            .collect();
        for c in curves(&s) {
            for p in &c {
                assert!(nodes.iter().all(|r| !r.contains(*p)), "ребро задевает узел в {p:?}");
            }
        }
    }

    #[test]
    fn label_sits_on_its_edge() {
        let src = "flowchart LR\n A[Start] --> B{Есть ошибка?}\n B -->|да| C[Читать diagnostic]\n B -->|нет| D[cargo run]\n C --> E[Исправить]\n E --> A\n D --> F[Готово]\n A --> F";
        let (_, s) = crate::diagram::render("mermaid", src, &Approx).unwrap();
        let all = curves(&s);
        let labels: Vec<Pos2> = s
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Rect { rect, fill: Paint::Role(Role::LabelBg), .. } => Some(rect.center()),
                _ => None,
            })
            .collect();
        assert_eq!(labels.len(), 2);
        for l in labels {
            let near = all.iter().flatten().map(|p| p.distance(l)).fold(f32::INFINITY, f32::min);
            assert!(near < 1.5, "подпись в {l:?} не на ребре: ближайшая точка в {near}");
        }
    }
}
