//! Диаграммы классов mermaid: `classDiagram`.

use super::graph::{ClassBox, Dir, Graph, Head, Line, Shape};
use super::text::label_lines;
use forma_i18n::t;

/// Оператор связи: (запись, наконечник слева, наконечник справа, пунктир).
type Relation = (&'static str, Head, Head, bool);

/// Операторы связей.
const RELATIONS: [Relation; 16] = [
    ("<|--", Head::Triangle, Head::None, false),
    ("--|>", Head::None, Head::Triangle, false),
    ("<|..", Head::Triangle, Head::None, true),
    ("..|>", Head::None, Head::Triangle, true),
    ("*--", Head::DiamondFilled, Head::None, false),
    ("--*", Head::None, Head::DiamondFilled, false),
    ("o--", Head::Diamond, Head::None, false),
    ("--o", Head::None, Head::Diamond, false),
    ("<--", Head::Vee, Head::None, false),
    ("-->", Head::None, Head::Vee, false),
    ("<..", Head::Vee, Head::None, true),
    ("..>", Head::None, Head::Vee, true),
    ("<-->", Head::Vee, Head::Vee, false),
    ("--", Head::None, Head::None, false),
    ("..", Head::None, Head::None, true),
    ("-.-", Head::None, Head::None, true),
];

pub fn parse(src: &str) -> Result<Graph, String> {
    let mut g = Graph::new(Dir::TB);
    let mut lines = src.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("%%"));
    lines.next();
    let mut body: Option<usize> = None;
    for line in lines {
        if let Some(n) = body {
            if line == "}" {
                body = None;
            } else {
                add_member(&mut g, n, line);
            }
            continue;
        }
        if let Some(d) = line.strip_prefix("direction ") {
            g.dir = Dir::parse(d).unwrap_or(g.dir);
            continue;
        }
        if line.starts_with("note") || line.starts_with("style ") || line.starts_with("classDef ") || line.starts_with("cssClass ") || line.starts_with("click ") || line.starts_with("link ") || line.starts_with("callback ") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("<<") {
            // <<interface>> Имя
            if let Some((ann, name)) = rest.split_once(">>") {
                let n = class_node(&mut g, name.trim());
                set_annotation(&mut g, n, ann);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            let mut rest = rest.trim();
            let opens = rest.ends_with('{');
            if opens {
                rest = rest[..rest.len() - 1].trim();
            }
            let (name, label) = match rest.find('[') {
                Some(k) => (rest[..k].trim(), Some(rest[k + 1..].trim_end_matches(']').trim())),
                None => (rest.split(":::").next().unwrap_or(rest).trim(), None),
            };
            let n = class_node(&mut g, name);
            if let Some(l) = label {
                g.nodes[n].label = label_lines(l);
                g.nodes[n].explicit = true;
            }
            if opens {
                body = Some(n);
            }
            continue;
        }
        if relation(&mut g, line) {
            continue;
        }
        // `Имя : +поле`
        if let Some((name, member)) = line.split_once(':') {
            let name = name.trim();
            if !name.is_empty() && !name.contains(char::is_whitespace) {
                let n = class_node(&mut g, name);
                add_member(&mut g, n, member.trim());
            }
            continue;
        }
        if !line.contains(char::is_whitespace) {
            class_node(&mut g, line);
        }
    }
    if g.nodes.is_empty() {
        return Err(t!("diagram-error-no-classes"));
    }
    Ok(g)
}

/// `Имя~T~` → идентификатор `Имя` и подпись `Имя<T>`.
fn class_node(g: &mut Graph, raw: &str) -> usize {
    let raw = raw.trim().trim_matches('`');
    let id = raw.split('~').next().unwrap_or(raw).trim();
    let existed = g.find(id).is_some();
    let n = g.node(id);
    if !existed {
        g.nodes[n].shape = Shape::Class(ClassBox::default());
    }
    if raw.contains('~') && !g.nodes[n].explicit {
        g.nodes[n].label = vec![generics(raw)];
    }
    n
}

fn generics(s: &str) -> String {
    let mut out = String::new();
    let mut open = true;
    for c in s.chars() {
        if c == '~' {
            out.push(if open { '<' } else { '>' });
            open = !open;
        } else {
            out.push(c);
        }
    }
    out
}

fn set_annotation(g: &mut Graph, n: usize, ann: &str) {
    if let Shape::Class(cb) = &mut g.nodes[n].shape {
        cb.annotation = Some(format!("«{}»", ann.trim()));
    }
}

fn add_member(g: &mut Graph, n: usize, line: &str) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    if let Some(rest) = line.strip_prefix("<<") {
        let ann = rest.split(">>").next().unwrap_or(rest).to_owned();
        set_annotation(g, n, &ann);
        return;
    }
    let text = generics(line.trim_end_matches(['$', '*']).trim());
    if let Shape::Class(cb) = &mut g.nodes[n].shape {
        if line.contains('(') {
            cb.methods.push(text);
        } else {
            cb.attrs.push(text);
        }
    }
}

/// Связь `A "1" <|-- "*" B : подпись`. Возвращает false, если это не связь.
fn relation(g: &mut Graph, line: &str) -> bool {
    let (main, label) = match line.rfind(" : ").or_else(|| line.rfind(':')) {
        Some(k) if RELATIONS.iter().any(|(op, ..)| line[..k].contains(op)) => (&line[..k], Some(line[k..].trim_start_matches([' ', ':']).trim())),
        _ => (line, None),
    };
    // Ищем самый длинный оператор, окружённый пробелами или стоящий между именами.
    let mut best: Option<(usize, &Relation)> = None;
    for rel in &RELATIONS {
        let mut from = 0;
        while let Some(k) = main[from..].find(rel.0) {
            let pos = from + k;
            let before = main[..pos].chars().last();
            // «o» и «*» могут быть концом имени — для них нужен пробел перед оператором.
            let needs_space = rel.0.starts_with(['o', '*']);
            if !needs_space || before.is_none_or(char::is_whitespace) {
                let better = match best {
                    None => true,
                    Some((bp, br)) => pos < bp || (pos == bp && rel.0.len() > br.0.len()),
                };
                if better {
                    best = Some((pos, rel));
                }
                break;
            }
            from = pos + rel.0.len();
        }
    }
    let Some((pos, &(op, hl, hr, dotted))) = best else { return false };
    let left = main[..pos].trim();
    let right = main[pos + op.len()..].trim();
    let (left_id, tail) = split_card(left, true);
    let (right_id, head) = split_card(right, false);
    if left_id.is_empty() || right_id.is_empty() {
        return false;
    }
    let a = class_node(g, left_id);
    let b = class_node(g, right_id);
    let e = g.edge(a, b);
    e.head_from = hl;
    e.head_to = hr;
    e.line = if dotted { Line::Dotted } else { Line::Solid };
    e.tail_text = tail;
    e.head_text = head;
    if let Some(l) = label.filter(|l| !l.is_empty()) {
        e.label = label_lines(l);
    }
    true
}

/// Отделяет кратность в кавычках: `A "1"` → (`A`, 1) или `"*" B` → (`B`, *).
fn split_card(s: &str, card_after: bool) -> (&str, Option<String>) {
    let s = s.trim();
    if card_after {
        if s.ends_with('"')
            && let Some(k) = s[..s.len() - 1].rfind('"')
        {
            return (s[..k].trim(), Some(s[k + 1..s.len() - 1].to_owned()));
        }
    } else if let Some(rest) = s.strip_prefix('"')
        && let Some(k) = rest.find('"')
    {
        return (rest[k + 1..].trim(), Some(rest[..k].to_owned()));
    }
    (s, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_classes_and_relations() {
        let g = parse(
            "classDiagram\n  Animal <|-- Duck\n  Animal \"1\" *-- \"many\" Leg : имеет\n  Zoo o-- Animal\n  class Duck {\n    +String клюв\n    +плавать() void\n  }\n  <<interface>> Animal\n  Animal : +int возраст\n  class Box~T~",
        )
        .unwrap();
        let animal = &g.nodes[g.find("Animal").unwrap()];
        let Shape::Class(cb) = &animal.shape else { panic!() };
        assert_eq!(cb.annotation.as_deref(), Some("«interface»"));
        assert_eq!(cb.attrs, vec!["+int возраст"]);
        let duck = &g.nodes[g.find("Duck").unwrap()];
        let Shape::Class(cb) = &duck.shape else { panic!() };
        assert_eq!(cb.methods, vec!["+плавать() void"]);
        let leg = g.edges.iter().find(|e| g.nodes[e.to].id == "Leg").unwrap();
        assert_eq!(leg.head_from, Head::DiamondFilled);
        assert_eq!(leg.tail_text.as_deref(), Some("1"));
        assert_eq!(leg.head_text.as_deref(), Some("many"));
        assert_eq!(leg.label, vec!["имеет"]);
        let zoo = g.edges.iter().find(|e| g.nodes[e.from].id == "Zoo").unwrap();
        assert_eq!(zoo.head_from, Head::Diamond);
        assert_eq!(g.nodes[g.find("Box").unwrap()].label, vec!["Box<T>"]);
    }
}
