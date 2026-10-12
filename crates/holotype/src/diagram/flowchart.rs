//! Блок-схемы mermaid: `flowchart LR` / `graph TD`.

use std::collections::HashMap;

use super::graph::{Dir, Graph, Head, Line, NodeStyle, Shape, parse_style};
use super::text::label_lines;
use forma_i18n::t;

pub fn parse(src: &str) -> Result<Graph, String> {
    let mut lines = src.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("%%"));
    let header_line = lines.next().ok_or_else(|| t!("diagram-error-empty"))?;
    // Заголовок может продолжаться операторами: `graph TD; A-->B`.
    let mut header_parts = split_statements(header_line).into_iter();
    let header = header_parts.next().unwrap_or("");
    let mut words = header.split_whitespace();
    words.next();
    let dir = words.next().and_then(Dir::parse).unwrap_or(Dir::TB);
    let mut p = Parser { g: Graph::new(dir), stack: Vec::new(), classes: HashMap::new(), node_classes: Vec::new() };
    let first: Vec<&str> = header_parts.collect();
    let rest = std::iter::once(first).chain(lines.map(split_statements));
    for (no, stmts) in rest.enumerate() {
        for stmt in stmts {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            p.statement(stmt).map_err(|e| t!("diagram-error-line", line = no + 1, error = e))?;
        }
    }
    for (node, class) in std::mem::take(&mut p.node_classes) {
        if let Some(st) = p.classes.get(&class) {
            p.g.nodes[node].style.merge(st);
        }
    }
    if let Some(st) = p.classes.get("default").cloned() {
        for n in &mut p.g.nodes {
            let own = n.style.clone();
            n.style = st.clone();
            n.style.merge(&own);
        }
    }
    p.g.redirect_cluster_links();
    if p.g.nodes.is_empty() {
        return Err(t!("diagram-error-no-nodes"));
    }
    Ok(p.g)
}

/// Делит строку на операторы по `;` вне кавычек и скобок.
fn split_statements(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote = false;
    let mut start = 0;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quote = !quote,
            '[' | '(' | '{' if !quote => depth += 1,
            ']' | ')' | '}' if !quote => depth -= 1,
            ';' if !quote && depth <= 0 => {
                out.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&line[start..]);
    out
}

struct Parser {
    g: Graph,
    stack: Vec<usize>,
    classes: HashMap<String, NodeStyle>,
    node_classes: Vec<(usize, String)>,
}

fn keyword<'a>(s: &'a str, kw: &str) -> Option<&'a str> {
    let rest = s.strip_prefix(kw)?;
    if rest.is_empty() || rest.starts_with(char::is_whitespace) { Some(rest.trim()) } else { None }
}

impl Parser {
    fn statement(&mut self, s: &str) -> Result<(), String> {
        if let Some(rest) = keyword(s, "subgraph") {
            let (id, title) = subgraph_title(rest);
            let parent = self.stack.last().copied();
            let c = self.g.cluster(&id, &title, parent);
            self.stack.push(c);
            return Ok(());
        }
        if s == "end" {
            self.stack.pop();
            return Ok(());
        }
        if keyword(s, "direction").is_some()
            || keyword(s, "linkStyle").is_some()
            || keyword(s, "click").is_some()
            || s.starts_with("accTitle")
            || s.starts_with("accDescr")
        {
            return Ok(());
        }
        if let Some(rest) = keyword(s, "classDef") {
            let (names, props) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            let st = parse_style(props);
            for n in names.split(',') {
                self.classes.insert(n.trim().to_owned(), st.clone());
            }
            return Ok(());
        }
        if let Some(rest) = keyword(s, "class") {
            let (ids, class) = rest.rsplit_once(char::is_whitespace).ok_or_else(|| t!("diagram-error-expected-class"))?;
            for id in ids.split(',') {
                let n = self.g.node(id.trim());
                self.node_classes.push((n, class.trim().to_owned()));
            }
            return Ok(());
        }
        if let Some(rest) = keyword(s, "style") {
            let (id, props) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            let n = self.g.node(id.trim());
            let st = parse_style(props);
            self.g.nodes[n].style.merge(&st);
            return Ok(());
        }
        self.chain(s)
    }

    fn chain(&mut self, s: &str) -> Result<(), String> {
        let chars: Vec<char> = s.chars().collect();
        let mut sc = Scanner { s: &chars, i: 0 };
        let mut left = self.node_group(&mut sc)?;
        loop {
            sc.ws();
            if sc.done() {
                return Ok(());
            }
            let Some(link) = sc.link() else {
                return Err(t!("diagram-error-unclear", text = sc.rest().to_owned()));
            };
            let right = self.node_group(&mut sc)?;
            for &a in &left {
                for &b in &right {
                    let e = self.g.edge(a, b);
                    e.line = link.line;
                    e.head_from = link.head_from;
                    e.head_to = link.head_to;
                    if let Some(l) = &link.label {
                        e.label = label_lines(l);
                    }
                }
            }
            left = right;
        }
    }

    fn node_group(&mut self, sc: &mut Scanner) -> Result<Vec<usize>, String> {
        let mut out = Vec::new();
        loop {
            sc.ws();
            out.push(self.node(sc)?);
            sc.ws();
            if sc.peek() == Some('&') {
                sc.i += 1;
                continue;
            }
            return Ok(out);
        }
    }

    fn node(&mut self, sc: &mut Scanner) -> Result<usize, String> {
        let id = sc.ident();
        if id.is_empty() {
            return Err(t!("diagram-error-expected-node", found = sc.rest().to_owned()));
        }
        let n = self.g.node(&id);
        if let Some(c) = self.stack.last() {
            self.g.nodes[n].cluster = Some(*c);
        }
        if let Some((shape, text)) = sc.shape()? {
            let node = &mut self.g.nodes[n];
            node.shape = shape;
            node.label = label_lines(&text);
            node.explicit = true;
        }
        if sc.eat(":::") {
            let class = sc.ident();
            self.node_classes.push((n, class));
        }
        Ok(n)
    }
}

/// `id[Заголовок]`, `id [Заголовок]`, `"Заголовок"` или просто `Заголовок`.
fn subgraph_title(rest: &str) -> (String, String) {
    let rest = rest.trim();
    if let Some(open) = rest.find('[') {
        let id = rest[..open].trim();
        let title = rest[open + 1..].trim_end().trim_end_matches(']');
        let title = label_lines(title).join(" ");
        let id = if id.is_empty() { title.clone() } else { id.to_owned() };
        return (id, title);
    }
    let title = label_lines(rest).join(" ");
    let id = if rest.starts_with('"') { title.clone() } else { rest.split_whitespace().next().unwrap_or("").to_owned() };
    (id, title)
}

struct Link {
    line: Line,
    head_from: Head,
    head_to: Head,
    label: Option<String>,
}

struct Scanner<'a> {
    s: &'a [char],
    i: usize,
}

impl Scanner<'_> {
    fn done(&self) -> bool {
        self.i >= self.s.len()
    }
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn at(&self, k: usize) -> Option<char> {
        self.s.get(self.i + k).copied()
    }
    fn rest(&self) -> String {
        self.s[self.i.min(self.s.len())..].iter().collect()
    }
    fn ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }
    fn starts(&self, p: &str) -> bool {
        p.chars().enumerate().all(|(k, c)| self.at(k) == Some(c))
    }
    fn eat(&mut self, p: &str) -> bool {
        if self.starts(p) {
            self.i += p.chars().count();
            true
        } else {
            false
        }
    }
    fn find(&self, p: &str) -> Option<usize> {
        let pat: Vec<char> = p.chars().collect();
        (self.i..self.s.len()).find(|&k| self.s[k..].starts_with(&pat))
    }

    fn ident(&mut self) -> String {
        let start = self.i;
        while let Some(c) = self.peek() {
            let ok = c.is_alphanumeric() || c == '_' || (c == '-' && self.at(1).is_some_and(|n| n.is_alphanumeric()) && self.i > start);
            if !ok {
                break;
            }
            self.i += 1;
        }
        self.s[start..self.i].iter().collect()
    }

    /// Текст до закрывающей последовательности (с учётом кавычек).
    fn until(&mut self, close: &str) -> Result<String, String> {
        self.ws();
        if self.peek() == Some('"') {
            let start = self.i;
            self.i += 1;
            while let Some(c) = self.peek() {
                self.i += 1;
                if c == '"' {
                    break;
                }
            }
            let text: String = self.s[start..self.i].iter().collect();
            self.ws();
            if self.eat(close) {
                return Ok(text);
            }
            // Кавычки не охватывают весь текст — ищем закрывающую дальше.
            self.i = start;
        }
        let end = self.find(close).ok_or_else(|| t!("diagram-error-not-found", what = close.to_string()))?;
        let text: String = self.s[self.i..end].iter().collect();
        self.i = end + close.chars().count();
        Ok(text)
    }

    fn shape(&mut self) -> Result<Option<(Shape, String)>, String> {
        let table: [(&str, &str, Shape); 10] = [
            ("(((", ")))", Shape::DoubleCircle),
            ("((", "))", Shape::Circle),
            ("([", "])", Shape::Stadium),
            ("(", ")", Shape::Round),
            ("[[", "]]", Shape::Subroutine),
            ("[(", ")]", Shape::Cylinder),
            ("{{", "}}", Shape::Hexagon),
            ("{", "}", Shape::Rhombus),
            (">", "]", Shape::Flag),
            ("@{", "}", Shape::Rect),
        ];
        // Параллелограммы и трапеции: [/ … /], [\ … \], [/ … \], [\ … /].
        if self.starts("[/") || self.starts("[\\") {
            let open_slash = self.at(1) == Some('/');
            self.i += 2;
            let a = self.find("/]");
            let b = self.find("\\]");
            let (end, close_slash) = match (a, b) {
                (Some(a), Some(b)) => if a < b { (a, true) } else { (b, false) },
                (Some(a), None) => (a, true),
                (None, Some(b)) => (b, false),
                (None, None) => return Err(t!("diagram-error-unclosed-shape")),
            };
            let text: String = self.s[self.i..end].iter().collect();
            self.i = end + 2;
            let shape = match (open_slash, close_slash) {
                (true, true) => Shape::Lean(true),
                (false, false) => Shape::Lean(false),
                (true, false) => Shape::Trapezoid(false),
                (false, true) => Shape::Trapezoid(true),
            };
            return Ok(Some((shape, text)));
        }
        for (open, close, shape) in table {
            if self.starts(open) {
                self.i += open.chars().count();
                let text = self.until(close)?;
                if open == "@{" {
                    // Новый синтаксис `@{ shape: …, label: "…" }`: берём только подпись.
                    let label = text
                        .split(',')
                        .find_map(|p| p.trim().strip_prefix("label:").map(|l| l.trim().to_owned()))
                        .unwrap_or_default();
                    if label.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some((shape, label)));
                }
                return Ok(Some((shape, text)));
            }
        }
        if self.starts("[") {
            self.i += 1;
            let text = self.until("]")?;
            return Ok(Some((Shape::Rect, text)));
        }
        Ok(None)
    }

    /// Стрелка между узлами: `-->`, `---`, `-.->`, `==>`, `~~~`, `--o`, `<-->`,
    /// с подписью `-->|текст|` или `-- текст -->`.
    fn link(&mut self) -> Option<Link> {
        let is_link_char = |c: char| matches!(c, '-' | '=' | '.' | '~' | '<' | '>');
        let start = self.i;
        let mut tok = String::new();
        // Наконечник в начале: o--o, x--x.
        if matches!(self.peek(), Some('o' | 'x')) && matches!(self.at(1), Some('-' | '=' | '.')) {
            tok.push(self.peek().unwrap());
            self.i += 1;
        }
        while let Some(c) = self.peek() {
            if is_link_char(c) {
                tok.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        let take_end_head = |sc: &mut Scanner, tok: &mut String| {
            if matches!(sc.peek(), Some('o' | 'x')) && sc.at(1).is_none_or(|c| c.is_whitespace() || c == '|') {
                tok.push(sc.peek().unwrap());
                sc.i += 1;
            }
        };
        if tok.trim_start_matches(['o', 'x']).is_empty() || !tok.contains(['-', '=', '.', '~']) {
            self.i = start;
            return None;
        }
        let mut label = None;
        let open_form = matches!(tok.as_str(), "--" | "==" | "-." | "<--" | "<==" | "<-.");
        if open_form && self.peek().is_some_and(char::is_whitespace) {
            // `-- текст -->`: ищем закрывающую часть стрелки.
            let closers: &[&str] = match tok.trim_start_matches('<') {
                "--" => &["-->", "---", "--o", "--x"],
                "==" => &["==>", "===", "==o", "==x"],
                _ => &[".->", ".-"],
            };
            let found = closers.iter().filter_map(|c| self.find(c).map(|k| (k, *c))).min_by_key(|(k, _)| *k);
            if let Some((k, _)) = found {
                let text: String = self.s[self.i..k].iter().collect();
                label = Some(text.trim().to_owned());
                self.i = k;
                let mut close = String::new();
                while let Some(c) = self.peek() {
                    if is_link_char(c) {
                        close.push(c);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                take_end_head(self, &mut close);
                tok.push_str(&close);
            }
        } else {
            take_end_head(self, &mut tok);
        }
        self.ws();
        if self.peek() == Some('|') {
            self.i += 1;
            let end = self.find("|")?;
            let text: String = self.s[self.i..end].iter().collect();
            label = Some(text.trim().to_owned());
            self.i = end + 1;
        }
        let line = if tok.contains('~') {
            Line::Invisible
        } else if tok.contains('.') {
            Line::Dotted
        } else if tok.contains('=') {
            Line::Thick
        } else {
            Line::Solid
        };
        let head = |c: Option<char>| match c {
            Some('>') | Some('<') => Head::Arrow,
            Some('o') => Head::Circle,
            Some('x') => Head::Cross,
            _ => Head::None,
        };
        Some(Link { line, head_from: head(tok.chars().next()), head_to: head(tok.chars().last()), label })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shapes_links_and_labels() {
        let g = parse(
            "flowchart LR\n  A[Вершины] --> B(Шейдер) -.->|подпись| C{Решение}\n  C -- да --> D((Круг))\n  C == нет ==> E[(База)]\n  A & B --- F>Флаг]\n  E --o G[/Ввод/] --x H[\\Вывод/]",
        )
        .unwrap();
        assert_eq!(g.dir, Dir::LR);
        let get = |id: &str| &g.nodes[g.find(id).unwrap()];
        assert_eq!(get("A").label, vec!["Вершины"]);
        assert_eq!(get("C").shape, Shape::Rhombus);
        assert_eq!(get("D").shape, Shape::Circle);
        assert_eq!(get("E").shape, Shape::Cylinder);
        assert_eq!(get("F").shape, Shape::Flag);
        assert_eq!(get("G").shape, Shape::Lean(true));
        assert_eq!(get("H").shape, Shape::Trapezoid(true));
        let edge = |a: &str, b: &str| g.edges.iter().find(|e| e.from == g.find(a).unwrap() && e.to == g.find(b).unwrap()).unwrap();
        assert_eq!(edge("B", "C").line, Line::Dotted);
        assert_eq!(edge("B", "C").label, vec!["подпись"]);
        assert_eq!(edge("C", "D").label, vec!["да"]);
        assert_eq!(edge("C", "E").line, Line::Thick);
        assert_eq!(edge("C", "E").label, vec!["нет"]);
        assert_eq!(edge("A", "F").head_to, Head::None);
        assert_eq!(edge("E", "G").head_to, Head::Circle);
        assert_eq!(edge("G", "H").head_to, Head::Cross);
    }

    #[test]
    fn subgraphs_and_styles() {
        let g = parse(
            "graph TD\n  subgraph one [Первый]\n    a1-->a2\n  end\n  subgraph two\n    b1-->b2\n  end\n  one --> two\n  classDef hot fill:#f96,stroke:#333\n  class a1 hot\n  style b2 fill:#0f0",
        )
        .unwrap();
        assert_eq!(g.clusters.len(), 2);
        assert_eq!(g.clusters[0].title, "Первый");
        let a1 = g.find("a1").unwrap();
        assert!(g.nodes[a1].style.fill.is_some());
        assert!(g.find("one").is_none(), "ссылка на подграф должна перейти на его узел");
        assert_eq!(g.edges.len(), 3);
    }
}
