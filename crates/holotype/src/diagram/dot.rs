//! Graphviz DOT (основное подмножество): `digraph G { a -> b [label="…"]; }`.

use std::collections::HashMap;

use super::graph::{Dir, Graph, Head, Line, Shape};
use super::text::{label_lines, parse_color};
use forma_i18n::t;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Id(String),
    Arrow,
    Dash,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Eq,
    Semi,
    Comma,
    Colon,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line_start = true;
    while i < c.len() {
        let ch = c[i];
        if ch == '\n' {
            line_start = true;
            i += 1;
            continue;
        }
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if line_start && ch == '#' {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        line_start = false;
        if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        match ch {
            '{' => out.push(Tok::LBrace),
            '}' => out.push(Tok::RBrace),
            '[' => out.push(Tok::LBracket),
            ']' => out.push(Tok::RBracket),
            '=' => out.push(Tok::Eq),
            ';' => out.push(Tok::Semi),
            ',' => out.push(Tok::Comma),
            ':' => out.push(Tok::Colon),
            '-' if c.get(i + 1) == Some(&'>') => {
                out.push(Tok::Arrow);
                i += 1;
            }
            '-' if c.get(i + 1) == Some(&'-') => {
                out.push(Tok::Dash);
                i += 1;
            }
            '"' => {
                let mut s = String::new();
                i += 1;
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && i + 1 < c.len() {
                        match c[i + 1] {
                            'n' | 'l' | 'r' => s.push('\n'),
                            '"' => s.push('"'),
                            '\\' => s.push('\\'),
                            other => {
                                s.push('\\');
                                s.push(other);
                            }
                        }
                        i += 2;
                        continue;
                    }
                    s.push(c[i]);
                    i += 1;
                }
                out.push(Tok::Id(s));
            }
            '<' => {
                // HTML-подпись: берём текст без тегов.
                let mut depth = 0;
                let mut s = String::new();
                while i < c.len() {
                    match c[i] {
                        '<' => depth += 1,
                        '>' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        ch if depth == 1 => s.push(ch),
                        _ => {}
                    }
                    i += 1;
                }
                out.push(Tok::Id(s));
            }
            c0 if c0.is_alphanumeric() || c0 == '_' || c0 == '.' || c0 == '-' => {
                let start = i;
                while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '.' || (c[i] == '-' && i == start)) {
                    i += 1;
                }
                out.push(Tok::Id(c[start..i].iter().collect()));
                continue;
            }
            other => return Err(t!("diagram-error-unexpected-char", char = other.to_string())),
        }
        i += 1;
    }
    Ok(out)
}

type Attrs = HashMap<String, String>;

struct P {
    t: Vec<Tok>,
    i: usize,
    g: Graph,
    directed: bool,
    node_defaults: Vec<Attrs>,
    edge_defaults: Vec<Attrs>,
}

pub fn parse(src: &str) -> Result<Graph, String> {
    let t = tokenize(src)?;
    let mut p = P { t, i: 0, g: Graph::new(Dir::TB), directed: true, node_defaults: vec![Attrs::new()], edge_defaults: vec![Attrs::new()] };
    if p.id_is("strict") {
        p.i += 1;
    }
    if p.id_is("digraph") {
        p.directed = true;
    } else if p.id_is("graph") {
        p.directed = false;
    } else {
        return Err(t!("diagram-error-expected-graph"));
    }
    p.i += 1;
    if matches!(p.peek(), Some(Tok::Id(_))) {
        p.i += 1;
    }
    p.expect(Tok::LBrace)?;
    p.stmts(None)?;
    if p.g.nodes.is_empty() {
        return Err(t!("diagram-error-no-nodes"));
    }
    Ok(p.g)
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn id_is(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Id(s)) if s.eq_ignore_ascii_case(kw))
    }
    fn expect(&mut self, t: Tok) -> Result<(), String> {
        if self.peek() == Some(&t) {
            self.i += 1;
            Ok(())
        } else {
            Err(t!("diagram-error-expected", what = format!("{t:?}")))
        }
    }

    fn attrs(&mut self) -> Result<Attrs, String> {
        let mut a = Attrs::new();
        while self.peek() == Some(&Tok::LBracket) {
            self.i += 1;
            loop {
                match self.peek() {
                    Some(Tok::RBracket) => {
                        self.i += 1;
                        break;
                    }
                    Some(Tok::Comma | Tok::Semi) => self.i += 1,
                    Some(Tok::Id(k)) => {
                        let k = k.to_lowercase();
                        self.i += 1;
                        if self.peek() == Some(&Tok::Eq) {
                            self.i += 1;
                            if let Some(Tok::Id(v)) = self.peek() {
                                a.insert(k, v.clone());
                                self.i += 1;
                            }
                        }
                    }
                    _ => return Err(t!("diagram-error-attributes")),
                }
            }
        }
        Ok(a)
    }

    /// Операторы до `}`; возвращает узлы, упомянутые в блоке.
    fn stmts(&mut self, cluster: Option<usize>) -> Result<Vec<usize>, String> {
        let mut members = Vec::new();
        loop {
            match self.peek() {
                None => return Ok(members),
                Some(Tok::RBrace) => {
                    self.i += 1;
                    return Ok(members);
                }
                Some(Tok::Semi | Tok::Comma) => {
                    self.i += 1;
                }
                _ => {
                    let got = self.stmt(cluster)?;
                    members.extend(got);
                }
            }
        }
    }

    fn stmt(&mut self, cluster: Option<usize>) -> Result<Vec<usize>, String> {
        if self.id_is("node") || self.id_is("edge") || (self.id_is("graph") && self.t.get(self.i + 1) == Some(&Tok::LBracket)) {
            let kind = match self.peek() {
                Some(Tok::Id(s)) => s.to_lowercase(),
                _ => String::new(),
            };
            self.i += 1;
            let a = self.attrs()?;
            match kind.as_str() {
                "node" => self.node_defaults.last_mut().unwrap().extend(a),
                "edge" => self.edge_defaults.last_mut().unwrap().extend(a),
                _ => self.graph_attrs(&a, cluster),
            }
            return Ok(Vec::new());
        }
        // Атрибут графа `rankdir=LR`.
        if let (Some(Tok::Id(k)), Some(Tok::Eq), Some(Tok::Id(v))) = (self.t.get(self.i), self.t.get(self.i + 1), self.t.get(self.i + 2)) {
            let mut a = Attrs::new();
            a.insert(k.to_lowercase(), v.clone());
            self.i += 3;
            self.graph_attrs(&a, cluster);
            return Ok(Vec::new());
        }
        let first = self.operand(cluster)?;
        let mut groups = vec![first];
        while matches!(self.peek(), Some(Tok::Arrow | Tok::Dash)) {
            self.i += 1;
            groups.push(self.operand(cluster)?);
        }
        let a = self.attrs()?;
        let mut all = Vec::new();
        if groups.len() == 1 {
            for &n in &groups[0] {
                self.apply_node(n, &a);
            }
        } else {
            let mut ea = self.edge_defaults.iter().fold(Attrs::new(), |mut acc, d| {
                acc.extend(d.clone());
                acc
            });
            ea.extend(a);
            for w in groups.windows(2) {
                for &x in &w[0] {
                    for &y in &w[1] {
                        self.add_edge(x, y, &ea);
                    }
                }
            }
        }
        for gr in groups {
            all.extend(gr);
        }
        Ok(all)
    }

    /// Узел или подграф (`subgraph cluster_x { … }`, `{ a b }`).
    fn operand(&mut self, cluster: Option<usize>) -> Result<Vec<usize>, String> {
        if self.id_is("subgraph") || self.peek() == Some(&Tok::LBrace) {
            let mut name = String::new();
            if self.id_is("subgraph") {
                self.i += 1;
                if let Some(Tok::Id(s)) = self.peek() {
                    name = s.clone();
                    self.i += 1;
                }
            }
            self.expect(Tok::LBrace)?;
            let c = if name.starts_with("cluster") { Some(self.g.cluster(&name, "", cluster)) } else { cluster };
            self.node_defaults.push(Attrs::new());
            self.edge_defaults.push(Attrs::new());
            let members = self.stmts(c)?;
            self.node_defaults.pop();
            self.edge_defaults.pop();
            return Ok(members);
        }
        let Some(Tok::Id(id)) = self.peek().cloned() else {
            return Err(t!("diagram-error-expected-node", found = format!("{:?}", self.peek())));
        };
        self.i += 1;
        // Порт `узел:порт` не поддерживаем — пропускаем.
        while self.peek() == Some(&Tok::Colon) {
            self.i += 2;
        }
        let existed = self.g.find(&id).is_some();
        let n = self.g.node(&id);
        if !existed {
            self.g.nodes[n].shape = Shape::Ellipse;
            self.g.nodes[n].cluster = cluster;
            let defaults = self.node_defaults.iter().fold(Attrs::new(), |mut acc, d| {
                acc.extend(d.clone());
                acc
            });
            self.apply_node(n, &defaults);
        }
        Ok(vec![n])
    }

    fn graph_attrs(&mut self, a: &Attrs, cluster: Option<usize>) {
        if let Some(d) = a.get("rankdir") {
            if cluster.is_none() {
                self.g.dir = Dir::parse(d).unwrap_or(self.g.dir);
            }
        }
        if let (Some(c), Some(l)) = (cluster, a.get("label")) {
            self.g.clusters[c].title = l.replace('\n', " ");
        }
    }

    fn apply_node(&mut self, n: usize, a: &Attrs) {
        let node = &mut self.g.nodes[n];
        if let Some(l) = a.get("label") {
            node.label = label_lines(&l.replace('\n', "<br>"));
            node.explicit = true;
        }
        if let Some(s) = a.get("shape") {
            node.shape = match s.to_lowercase().as_str() {
                "box" | "rect" | "rectangle" | "square" | "record" | "note" | "tab" | "folder" | "component" => Shape::Rect,
                "mrecord" => Shape::Round,
                "circle" => Shape::Circle,
                "doublecircle" => Shape::DoubleCircle,
                "diamond" => Shape::Rhombus,
                "hexagon" => Shape::Hexagon,
                "parallelogram" => Shape::Lean(true),
                "trapezium" => Shape::Trapezoid(false),
                "invtrapezium" => Shape::Trapezoid(true),
                "cylinder" => Shape::Cylinder,
                "plaintext" | "plain" | "none" => Shape::Plain,
                "point" => Shape::Point,
                "cds" => Shape::Flag,
                _ => Shape::Ellipse,
            };
        }
        let style = a.get("style").map(|s| s.to_lowercase()).unwrap_or_default();
        if style.contains("rounded") && node.shape == Shape::Rect {
            node.shape = Shape::Round;
        }
        if style.contains("dashed") || style.contains("dotted") {
            node.style.dashed = true;
        }
        if let Some(c) = a.get("fillcolor").and_then(|c| parse_color(c)) {
            node.style.fill = Some(c);
        } else if style.contains("filled") {
            node.style.fill = a.get("color").and_then(|c| parse_color(c));
        }
        if let Some(c) = a.get("color").and_then(|c| parse_color(c)) {
            node.style.stroke = Some(c);
        }
        if let Some(c) = a.get("fontcolor").and_then(|c| parse_color(c)) {
            node.style.text = Some(c);
        }
    }

    fn add_edge(&mut self, x: usize, y: usize, a: &Attrs) {
        let directed = self.directed;
        let e = self.g.edge(x, y);
        let arrow = |name: Option<&String>, default: Head| match name.map(|s| s.to_lowercase()).as_deref() {
            None => default,
            Some("none") => Head::None,
            Some("empty" | "onormal") => Head::Triangle,
            Some("vee" | "open") => Head::Vee,
            Some("diamond") => Head::DiamondFilled,
            Some("odiamond") => Head::Diamond,
            Some("dot" | "odot") => Head::Circle,
            Some(_) => Head::Arrow,
        };
        let base = if directed { Head::Arrow } else { Head::None };
        let (mut hf, mut ht) = (Head::None, arrow(a.get("arrowhead"), base));
        match a.get("dir").map(|s| s.to_lowercase()).as_deref() {
            Some("back") => {
                hf = arrow(a.get("arrowtail"), Head::Arrow);
                ht = Head::None;
            }
            Some("both") => hf = arrow(a.get("arrowtail"), Head::Arrow),
            Some("none") => ht = Head::None,
            _ => {}
        }
        e.head_from = hf;
        e.head_to = ht;
        if let Some(l) = a.get("label").or_else(|| a.get("xlabel")) {
            e.label = label_lines(&l.replace('\n', "<br>"));
        }
        e.line = match a.get("style").map(|s| s.to_lowercase()).as_deref() {
            Some(s) if s.contains("invis") => Line::Invisible,
            Some(s) if s.contains("dash") || s.contains("dot") => Line::Dotted,
            Some(s) if s.contains("bold") => Line::Thick,
            _ => Line::Solid,
        };
        e.color = a.get("color").and_then(|c| parse_color(c));
        e.tail_text = a.get("taillabel").cloned();
        e.head_text = a.get("headlabel").cloned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dot() {
        let g = parse(
            "digraph G {\n  rankdir=LR;\n  node [shape=box];\n  // комментарий\n  a [label=\"Начало\"];\n  a -> b -> c [label=\"шаг\", style=dashed];\n  subgraph cluster_x { label=\"Группа\"; d; e }\n  c -> {d e}\n  f [shape=circle]\n}",
        )
        .unwrap();
        assert_eq!(g.dir, Dir::LR);
        assert_eq!(g.nodes[g.find("a").unwrap()].label, vec!["Начало"]);
        assert_eq!(g.nodes[g.find("b").unwrap()].shape, Shape::Rect);
        assert_eq!(g.nodes[g.find("f").unwrap()].shape, Shape::Circle);
        assert_eq!(g.clusters[0].title, "Группа");
        assert_eq!(g.edges.len(), 4);
        assert_eq!(g.edges[0].line, Line::Dotted);
        let u = parse("graph { a -- b }").unwrap();
        assert_eq!(u.edges[0].head_to, Head::None);
    }
}
