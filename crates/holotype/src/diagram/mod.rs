//! Диаграммы из текста: mermaid (блок-схемы, последовательности, классы, состояния,
//! круговые) и Graphviz DOT. Разбор и раскладка свои: результат — «сцена» из простых
//! фигур, которая рисуется через egui в приложении и выгружается в SVG при экспорте.

mod class;
mod dot;
mod flowchart;
mod graph;
mod pie;
pub mod scene;
mod sequence;
mod state;
mod text;

pub use scene::{Colors, Scene};
pub use text::{Approx, EguiMeasure, Measure};
use forma_i18n::t;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Flowchart,
    Sequence,
    Class,
    State,
    Pie,
    Dot,
}

impl Kind {
    /// Название вида диаграммы на языке интерфейса.
    pub fn title(self) -> String {
        forma_i18n::tr(match self {
            Kind::Flowchart => "diagram-kind-flowchart",
            Kind::Sequence => "diagram-kind-sequence",
            Kind::Class => "diagram-kind-class",
            Kind::State => "diagram-kind-state",
            Kind::Pie => "diagram-kind-pie",
            Kind::Dot => "diagram-kind-dot",
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Тип диаграммы пока не поддерживается (например, `gantt`).
    Unsupported(String),
    Parse(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(k) => write!(f, "{}", t!("diagram-unsupported", kind = k.clone())),
            Error::Parse(e) => write!(f, "{}", t!("diagram-failed", error = e.clone())),
        }
    }
}

/// Язык блока кода, который рисуется диаграммой.
pub fn is_diagram(lang: &str) -> bool {
    matches!(lang.trim().to_lowercase().as_str(), "mermaid" | "mmd" | "dot" | "graphviz" | "gv")
}

/// Убирает front-matter (`--- … ---`) и директивы `%%{init: …}%%`.
fn strip_preamble(code: &str) -> String {
    let mut lines = code.lines().peekable();
    let mut out = Vec::new();
    while lines.peek().is_some_and(|l| l.trim().is_empty()) {
        lines.next();
    }
    if lines.peek().is_some_and(|l| l.trim() == "---") {
        lines.next();
        for l in lines.by_ref() {
            if l.trim() == "---" {
                break;
            }
        }
    }
    for l in lines {
        let t = l.trim();
        if t.starts_with("%%{") {
            continue;
        }
        out.push(l);
    }
    out.join("\n")
}

/// Разбирает и раскладывает диаграмму.
pub fn render(lang: &str, code: &str, m: &dyn Measure) -> Result<(Kind, Scene), Error> {
    let lang = lang.trim().to_lowercase();
    if matches!(lang.as_str(), "dot" | "graphviz" | "gv") {
        let g = dot::parse(code).map_err(Error::Parse)?;
        return Ok((Kind::Dot, graph::layout(&g, m)));
    }
    let code = strip_preamble(code);
    let first = code
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))
        .ok_or_else(|| Error::Parse(t!("diagram-error-empty")))?;
    let word = first.split(|c: char| c.is_whitespace() || c == ';').next().unwrap_or("");
    match word {
        "flowchart" | "graph" | "flowchart-elk" => {
            let g = flowchart::parse(&code).map_err(Error::Parse)?;
            Ok((Kind::Flowchart, graph::layout(&g, m)))
        }
        "stateDiagram" | "stateDiagram-v2" => {
            let g = state::parse(&code).map_err(Error::Parse)?;
            Ok((Kind::State, graph::layout(&g, m)))
        }
        "classDiagram" | "classDiagram-v2" => {
            let g = class::parse(&code).map_err(Error::Parse)?;
            Ok((Kind::Class, graph::layout(&g, m)))
        }
        "sequenceDiagram" => Ok((Kind::Sequence, sequence::layout(&code, m).map_err(Error::Parse)?)),
        "pie" => Ok((Kind::Pie, pie::layout(&code, m).map_err(Error::Parse)?)),
        "" => Err(Error::Parse(t!("diagram-error-no-type"))),
        other => Err(Error::Unsupported(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kinds() {
        let ok = |src: &str, kind: Kind| {
            let (k, s) = render("mermaid", src, &Approx).unwrap();
            assert_eq!(k, kind);
            assert!(s.size.x > 0.0 && s.size.is_finite());
            assert!(s.to_svg().starts_with("<svg"));
        };
        ok("%%{init: {'theme':'dark'}}%%\nflowchart LR\n A --> B", Kind::Flowchart);
        ok("---\ntitle: x\n---\ngraph TD; A-->B; B-->C", Kind::Flowchart);
        ok("sequenceDiagram\n A->>B: hi", Kind::Sequence);
        ok("classDiagram\n A <|-- B", Kind::Class);
        ok("stateDiagram-v2\n [*] --> A", Kind::State);
        ok("pie\n \"a\" : 1", Kind::Pie);
        assert!(matches!(render("mermaid", "gantt\n title x", &Approx), Err(Error::Unsupported(k)) if k == "gantt"));
        assert!(matches!(render("dot", "digraph { a -> b }", &Approx), Ok((Kind::Dot, _))));
    }
}
