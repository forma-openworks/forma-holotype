//! Круговые диаграммы mermaid: `pie`.

use std::f32::consts::TAU;

use eframe::egui::{Rect, pos2, vec2};

use super::scene::{Anchor, Dash, Font, Paint, Prim, Role, Scene};
use super::text::{FONT, LINE, Measure, label_lines};
use forma_i18n::t;

pub fn layout(src: &str, m: &dyn Measure) -> Result<Scene, String> {
    let mut title: Option<String> = None;
    let mut show_data = false;
    let mut slices: Vec<(String, f32)> = Vec::new();
    for (i, line) in src.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("%%")).enumerate() {
        let mut line = line;
        if i == 0 {
            line = line.trim_start_matches("pie").trim();
            if let Some(rest) = line.strip_prefix("showData") {
                show_data = true;
                line = rest.trim();
            }
        }
        if let Some(t) = line.strip_prefix("title") {
            title = Some(label_lines(t).join(" "));
            continue;
        }
        if line == "showData" {
            show_data = true;
            continue;
        }
        if line.is_empty() || line.starts_with("accTitle") || line.starts_with("accDescr") {
            continue;
        }
        let (name, value) = line.rsplit_once(':').ok_or_else(|| t!("diagram-error-pie-line", line = line.to_owned()))?;
        let v: f32 = value.trim().replace(',', ".").parse().map_err(|_| t!("diagram-error-not-number", value = value.trim().to_owned()))?;
        if v > 0.0 {
            slices.push((label_lines(name).join(" "), v));
        }
    }
    if slices.is_empty() {
        return Err(t!("diagram-error-no-values"));
    }
    let total: f32 = slices.iter().map(|s| s.1).sum();

    let mut s = Scene::default();
    let r = 112.0;
    let mut top = 0.0;
    if let Some(t) = &title {
        let f = Font::new(FONT * 1.1).bold();
        let w = m.width(t, f);
        s.text(pos2(r, 0.0), t.clone(), f, Anchor::Middle, Role::Text, w);
        top = LINE + 14.0;
    }
    let c = pos2(r, top + r);
    let mut a = 0.0f32;
    let mut bounds = Vec::new();
    for (i, (_, v)) in slices.iter().enumerate() {
        let span = v / total * TAU;
        s.push(Prim::Wedge { center: c, radius: r, a0: a, a1: a + span, fill: Paint::Role(Role::Series(i as u8)) });
        bounds.push((a, span));
        a += span;
    }
    // Тонкие разделители между секторами цветом фона.
    if slices.len() > 1 {
        for (a, _) in &bounds {
            let p = c + vec2(a.sin(), -a.cos()) * r;
            s.push(Prim::Poly { pts: vec![c, p], closed: false, fill: Paint::None, stroke: Role::Bg.into(), width: 2.0, dash: Dash::Solid });
        }
    }
    // Проценты внутри крупных секторов.
    let pf = Font::new(FONT * 0.85).bold();
    for (i, (a, span)) in bounds.iter().enumerate() {
        if *span < 0.32 {
            continue;
        }
        let mid = a + span / 2.0;
        let p = c + vec2(mid.sin(), -mid.cos()) * r * 0.66;
        let pct = format!("{:.0}%", slices[i].1 / total * 100.0);
        s.text(p, pct.clone(), pf, Anchor::Middle, Role::Bg, m.width(&pct, pf));
    }
    // Легенда справа.
    let lf = Font::new(FONT * 0.95);
    let row = LINE + 8.0;
    let lx = c.x + r + 32.0;
    let ly0 = c.y - (slices.len() as f32 * row) / 2.0 + row / 2.0;
    for (i, (name, v)) in slices.iter().enumerate() {
        let y = ly0 + i as f32 * row;
        s.push(Prim::Rect {
            rect: Rect::from_center_size(pos2(lx + 7.0, y), vec2(14.0, 14.0)),
            radius: 4.0,
            fill: Paint::Role(Role::Series(i as u8)),
            stroke: Paint::None,
            width: 0.0,
            dash: Dash::Solid,
        });
        let mut text = name.clone();
        if show_data {
            text.push_str(&format!(" ({})", trim_num(*v)));
        }
        let w = m.width(&text, lf);
        s.text(pos2(lx + 22.0, y), text, lf, Anchor::Start, Role::Text, w);
        let pct = format!("{:.1}%", v / total * 100.0);
        let mf = Font::new(FONT * 0.85);
        s.text(pos2(lx + 30.0 + w, y), pct.clone(), mf, Anchor::Start, Role::Muted, m.width(&pct, mf));
    }
    s.finish(10.0);
    Ok(s)
}

fn trim_num(v: f32) -> String {
    if v.fract().abs() < 1e-4 { format!("{}", v as i64) } else { format!("{v:.2}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::text::Approx;

    #[test]
    fn pie_works() {
        let s = layout("pie showData title Питомцы\n  \"Кошки\" : 40\n  \"Собаки\" : 35.5\n  \"Хомяки\" : 5", &Approx).unwrap();
        assert_eq!(s.prims.iter().filter(|p| matches!(p, Prim::Wedge { .. })).count(), 3);
        assert!(layout("pie\n  \"x\" : abc", &Approx).is_err());
    }
}
