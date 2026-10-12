//! Диаграммы состояний mermaid: `stateDiagram-v2`.

use std::collections::HashMap;

use super::graph::{Dir, Graph, NodeStyle, Shape, parse_style};
use super::text::label_lines;
use forma_i18n::t;

pub fn parse(src: &str) -> Result<Graph, String> {
    let mut g = Graph::new(Dir::TB);
    let mut stack: Vec<usize> = Vec::new();
    let mut classes: HashMap<String, NodeStyle> = HashMap::new();
    let mut node_classes: Vec<(usize, String)> = Vec::new();
    let mut in_note = false;
    let mut lines = src.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("%%"));
    lines.next();

    // Узел состояния; новые узлы попадают в текущее составное состояние.
    fn state(g: &mut Graph, stack: &[usize], id: &str) -> usize {
        let existed = g.find(id).is_some();
        let n = g.node(id);
        if !existed {
            g.nodes[n].shape = Shape::Round;
            g.nodes[n].cluster = stack.last().copied();
        }
        n
    }
    // Точки `[*]`: свои в каждом составном состоянии; как источник — начало, как цель — конец.
    fn star(g: &mut Graph, stack: &[usize], is_target: bool) -> usize {
        let scope = stack.last().map_or("root".to_owned(), |c| c.to_string());
        let id = format!("[*]{}@{scope}", if is_target { "end" } else { "start" });
        let existed = g.find(&id).is_some();
        let n = g.node(&id);
        if !existed {
            g.nodes[n].shape = if is_target { Shape::End } else { Shape::Start };
            g.nodes[n].label = vec![String::new()];
            g.nodes[n].cluster = stack.last().copied();
        }
        n
    }

    for line in lines {
        if in_note {
            if line.eq_ignore_ascii_case("end note") {
                in_note = false;
            }
            continue;
        }
        let lower = line.to_lowercase();
        if lower.starts_with("note ") {
            in_note = !line.contains(':');
            continue;
        }
        if let Some(d) = line.strip_prefix("direction ") {
            if stack.is_empty() {
                g.dir = Dir::parse(d).unwrap_or(g.dir);
            }
            continue;
        }
        if line == "}" {
            stack.pop();
            continue;
        }
        if line == "--" || line.starts_with("accTitle") || line.starts_with("accDescr") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("classDef ") {
            let (name, props) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            classes.insert(name.trim().to_owned(), parse_style(props));
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            if let Some((ids, class)) = rest.rsplit_once(char::is_whitespace) {
                for id in ids.split(',') {
                    let n = state(&mut g, &stack, id.trim());
                    node_classes.push((n, class.trim().to_owned()));
                }
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("state ") {
            let mut rest = rest.trim();
            let composite = rest.ends_with('{');
            if composite {
                rest = rest[..rest.len() - 1].trim();
            }
            let (id, title) = if rest.starts_with('"') {
                // state "Описание" as Id
                let end = rest[1..].find('"').map_or(rest.len(), |k| k + 1);
                let desc = rest[1..end].to_owned();
                let id = rest[end..].trim_start_matches('"').trim().trim_start_matches("as").trim().to_owned();
                (if id.is_empty() { desc.clone() } else { id }, Some(desc))
            } else {
                let mut parts = rest.split_whitespace();
                let id = parts.next().unwrap_or("").to_owned();
                (id, None)
            };
            if id.is_empty() {
                continue;
            }
            if composite {
                let c = g.cluster(&id, title.as_deref().unwrap_or(&id), stack.last().copied());
                stack.push(c);
                continue;
            }
            let n = state(&mut g, &stack, &id);
            if let Some(t) = title {
                g.nodes[n].label = label_lines(&t);
                g.nodes[n].explicit = true;
            }
            let lower = rest.to_lowercase();
            if lower.contains("<<choice>>") {
                g.nodes[n].shape = Shape::Rhombus;
                g.nodes[n].label = vec![String::new()];
            } else if lower.contains("<<fork>>") || lower.contains("<<join>>") {
                g.nodes[n].shape = Shape::Bar;
                g.nodes[n].label = vec![String::new()];
            }
            continue;
        }
        if let Some(arrow) = line.find("-->") {
            let left = line[..arrow].trim();
            let right = &line[arrow + 3..];
            let (target, label) = match right.split_once(':') {
                Some((t, l)) => (t.trim(), Some(l.trim())),
                None => (right.trim(), None),
            };
            let a = if left == "[*]" {
                star(&mut g, &stack, false)
            } else {
                let id = strip_class(left, &mut node_classes, &mut g, &stack);
                state(&mut g, &stack, id)
            };
            let b = if target == "[*]" {
                star(&mut g, &stack, true)
            } else {
                let id = strip_class(target, &mut node_classes, &mut g, &stack);
                state(&mut g, &stack, id)
            };
            let e = g.edge(a, b);
            if let Some(l) = label {
                e.label = label_lines(l);
            }
            continue;
        }
        // `Id : описание`
        if let Some((id, desc)) = line.split_once(':') {
            let n = state(&mut g, &stack, id.trim());
            let node = &mut g.nodes[n];
            if !node.explicit {
                node.label.clear();
                node.label.push(node.id.clone());
                node.explicit = true;
            }
            node.label.extend(label_lines(desc));
            continue;
        }
        // Просто имя состояния.
        if !line.contains(char::is_whitespace) {
            state(&mut g, &stack, line);
        }
    }
    for (n, class) in node_classes {
        if let Some(st) = classes.get(&class) {
            g.nodes[n].style.merge(st);
        }
    }
    g.redirect_cluster_links();
    if g.nodes.is_empty() {
        return Err(t!("diagram-error-no-states"));
    }
    Ok(g)
}

/// `Id:::класс` → `Id` (класс запоминается).
fn strip_class<'a>(s: &'a str, out: &mut Vec<(usize, String)>, g: &mut Graph, stack: &[usize]) -> &'a str {
    match s.split_once(":::") {
        Some((id, class)) => {
            let id = id.trim();
            let existed = g.find(id).is_some();
            let n = g.node(id);
            if !existed {
                g.nodes[n].shape = Shape::Round;
                g.nodes[n].cluster = stack.last().copied();
            }
            out.push((n, class.trim().to_owned()));
            id
        }
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_states() {
        let g = parse(
            "stateDiagram-v2\n  [*] --> Ожидание\n  Ожидание --> Работа : старт\n  Работа --> [*]\n  state Работа {\n    [*] --> Шаг1\n    Шаг1 --> Шаг2\n  }\n  state выбор <<choice>>\n  Шаг2 --> выбор\n  Ожидание : ждём команды",
        )
        .unwrap();
        assert_eq!(g.clusters.len(), 1);
        let idle = &g.nodes[g.find("Ожидание").unwrap()];
        assert_eq!(idle.label, vec!["Ожидание", "ждём команды"]);
        assert_eq!(g.nodes[g.find("выбор").unwrap()].shape, Shape::Rhombus);
        assert!(g.nodes.iter().filter(|n| n.shape == Shape::Start).count() == 2);
    }
}
