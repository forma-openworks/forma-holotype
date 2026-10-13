//! Диаграммы последовательности mermaid: `sequenceDiagram`.

use std::collections::HashMap;

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

use super::graph::{Head, head};
use super::scene::{Anchor, Dash, Font, Paint, Prim, Role, Scene};
use super::text::{FONT, LINE, Measure, label_lines};
use forma_i18n::t;

const MSG_FONT: f32 = FONT * 0.92;
const BOX_H: f32 = 40.0;

struct Participant {
    label: Vec<String>,
    actor: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum NotePos {
    Left,
    Right,
    Over,
}

enum Ev {
    Msg { from: usize, to: usize, text: Vec<String>, dashed: bool, head: Head, both: bool, act: Option<bool> },
    Note { pos: NotePos, a: usize, b: usize, text: Vec<String> },
    Start { kind: String, label: String },
    Else { label: String },
    End,
    Activate(usize),
    Deactivate(usize),
}

struct Parsed {
    parts: Vec<Participant>,
    events: Vec<Ev>,
    autonumber: bool,
    title: Option<String>,
}

const ARROWS: [(&str, bool, Head, bool); 10] = [
    ("<<-->>", true, Head::Arrow, true),
    ("<<->>", false, Head::Arrow, true),
    ("-->>", true, Head::Arrow, false),
    ("->>", false, Head::Arrow, false),
    ("--x", true, Head::Cross, false),
    ("-x", false, Head::Cross, false),
    ("--)", true, Head::Vee, false),
    ("-)", false, Head::Vee, false),
    ("-->", true, Head::None, false),
    ("->", false, Head::None, false),
];

fn parse(src: &str) -> Result<Parsed, String> {
    let mut p = Parsed { parts: Vec::new(), events: Vec::new(), autonumber: false, title: None };
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut block_kinds: Vec<String> = Vec::new();
    let mut lines = src.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("%%"));
    lines.next();

    fn part(p: &mut Parsed, ids: &mut HashMap<String, usize>, id: &str) -> usize {
        let id = id.trim();
        if let Some(&i) = ids.get(id) {
            return i;
        }
        p.parts.push(Participant { label: label_lines(id), actor: false });
        ids.insert(id.to_owned(), p.parts.len() - 1);
        p.parts.len() - 1
    }

    for line in lines {
        // ASCII-регистр: длина `word` в байтах — та же, что в `line` (срезы ниже).
        let lower = line.to_ascii_lowercase();
        let word = lower.split_whitespace().next().unwrap_or("");
        let rest_after = |n: usize| line[n..].trim();
        match word {
            "participant" | "actor" | "create" => {
                let mut rest = line;
                if word == "create" {
                    rest = rest[6..].trim();
                }
                let lower_rest = rest.to_lowercase();
                let actor = lower_rest.starts_with("actor");
                let body = rest.split_once(char::is_whitespace).map_or("", |(_, b)| b).trim();
                let body = body.split("@{").next().unwrap_or(body);
                let (id, alias) = match body.find(" as ") {
                    Some(k) => (&body[..k], Some(&body[k + 4..])),
                    None => (body, None),
                };
                let i = part(&mut p, &mut ids, id);
                p.parts[i].actor = actor;
                if let Some(a) = alias {
                    p.parts[i].label = label_lines(a);
                }
                continue;
            }
            "destroy" => continue,
            "autonumber" => {
                p.autonumber = true;
                continue;
            }
            "title" => {
                p.title = Some(rest_after(5).trim_start_matches(':').trim().to_owned());
                continue;
            }
            "activate" => {
                let i = part(&mut p, &mut ids, rest_after(8));
                p.events.push(Ev::Activate(i));
                continue;
            }
            "deactivate" => {
                let i = part(&mut p, &mut ids, rest_after(10));
                p.events.push(Ev::Deactivate(i));
                continue;
            }
            "loop" | "alt" | "opt" | "par" | "critical" | "break" | "rect" | "box" => {
                block_kinds.push(word.to_owned());
                if word != "box" {
                    p.events.push(Ev::Start { kind: word.to_owned(), label: rest_after(word.len()).to_owned() });
                }
                continue;
            }
            "else" | "and" | "option" => {
                p.events.push(Ev::Else { label: rest_after(word.len()).to_owned() });
                continue;
            }
            "end" => {
                if block_kinds.pop().is_some_and(|k| k != "box") {
                    p.events.push(Ev::End);
                }
                continue;
            }
            "note" => {
                let (head_part, text) = line.split_once(':').ok_or_else(|| t!("diagram-error-note-text"))?;
                let h = head_part.to_ascii_lowercase();
                let after = |kw: &str| h.find(kw).map(|k| head_part[k + kw.len()..].trim().to_owned());
                let (pos, who) = if let Some(w) = after("right of") {
                    (NotePos::Right, w)
                } else if let Some(w) = after("left of") {
                    (NotePos::Left, w)
                } else if let Some(w) = after("over") {
                    (NotePos::Over, w)
                } else {
                    return Err(t!("diagram-error-note-place"));
                };
                let mut names = who.split(',');
                let a = part(&mut p, &mut ids, names.next().unwrap_or(""));
                let b = names.next().map_or(a, |n| part(&mut p, &mut ids, n));
                p.events.push(Ev::Note { pos, a: a.min(b), b: a.max(b), text: label_lines(text) });
                continue;
            }
            _ => {}
        }
        // Сообщение `A->>B: текст`.
        let head_part = line.split_once(':').map_or(line, |(h, _)| h);
        let found = ARROWS.iter().filter_map(|a| head_part.find(a.0).map(|k| (k, a))).min_by(|x, y| x.0.cmp(&y.0).then(y.1.0.len().cmp(&x.1.0.len())));
        let Some((k, &(arrow, dashed, head, both))) = found else {
            return Err(t!("diagram-error-unclear", text = line.to_owned()));
        };
        let from = head_part[..k].trim();
        let mut to = head_part[k + arrow.len()..].trim();
        let mut act = None;
        if let Some(t) = to.strip_prefix('+') {
            act = Some(true);
            to = t.trim();
        } else if let Some(t) = to.strip_prefix('-') {
            act = Some(false);
            to = t.trim();
        }
        if from.is_empty() || to.is_empty() {
            return Err(t!("diagram-error-message", line = line.to_owned()));
        }
        let a = part(&mut p, &mut ids, from);
        let b = part(&mut p, &mut ids, to);
        let text = line.split_once(':').map_or(Vec::new(), |(_, t)| label_lines(t));
        p.events.push(Ev::Msg { from: a, to: b, text, dashed, head, both, act });
    }
    if p.parts.is_empty() {
        return Err(t!("diagram-error-no-participants"));
    }
    Ok(p)
}

struct Frame {
    kind: String,
    label: String,
    top: f32,
    elses: Vec<(f32, String)>,
    /// Горизонтальные границы всего, что нарисовано внутри рамки.
    x0: f32,
    x1: f32,
}

pub fn layout(src: &str, m: &dyn Measure) -> Result<Scene, String> {
    let p = parse(src)?;
    let n = p.parts.len();
    let mf = Font::new(MSG_FONT);
    let lh = LINE * MSG_FONT / FONT;
    let tw = |lines: &[String]| lines.iter().map(|l| m.width(l, mf)).fold(0.0, f32::max);

    // Колонки участников.
    let widths: Vec<f32> =
        p.parts.iter().map(|pt| (pt.label.iter().map(|l| m.width(l, Font::new(FONT))).fold(0.0, f32::max) + 30.0).max(96.0)).collect();
    let mut x = vec![0.0f32; n];
    for i in 1..n {
        x[i] = x[i - 1] + (widths[i - 1] + widths[i]) / 2.0 + 36.0;
    }
    let mut left_extra = 0.0f32;
    let mut right_extra = 0.0f32;
    let mut cons: Vec<(usize, usize, f32)> = Vec::new();
    for ev in &p.events {
        match ev {
            Ev::Msg { from, to, text, .. } if from != to => {
                let num = if p.autonumber { 22.0 } else { 0.0 };
                cons.push(((*from).min(*to), (*from).max(*to), tw(text) + 28.0 + num));
            }
            Ev::Msg { from, text, .. } => {
                let need = tw(text) + 56.0;
                if *from + 1 < n { cons.push((*from, from + 1, need)) } else { right_extra = right_extra.max(need) }
            }
            Ev::Note { pos, a, b, text } => {
                let w = tw(text) + 24.0;
                match pos {
                    NotePos::Right => {
                        if *a + 1 < n { cons.push((*a, a + 1, w + 16.0)) } else { right_extra = right_extra.max(w + 10.0) }
                    }
                    NotePos::Left => {
                        if *a > 0 { cons.push((a - 1, *a, w + 16.0)) } else { left_extra = left_extra.max(w + 10.0) }
                    }
                    NotePos::Over if a != b => cons.push((*a, *b, w - 20.0)),
                    NotePos::Over => {}
                }
            }
            _ => {}
        }
    }
    cons.sort_by_key(|c| c.1 - c.0);
    for (i, j, d) in cons {
        let cur = x[j] - x[i];
        if cur < d {
            for xx in x.iter_mut().skip(j) {
                *xx += d - cur;
            }
        }
    }
    let _ = (left_extra, right_extra); // учитываются через границы сцены

    let mut scene = Scene::default();
    // Рамки блоков: фон — под всем остальным, ярлыки и подписи условий — поверх линий.
    let mut frames_back: Vec<Prim> = Vec::new();
    let mut frames_front: Vec<Prim> = Vec::new();
    let mut body: Scene = Scene::default();
    let mut y = 0.0f32;
    if let Some(t) = &p.title {
        let f = Font::new(FONT * 1.1).bold();
        let cx = (x[0] + x[n - 1]) / 2.0;
        scene.text(pos2(cx, y), t.clone(), f, Anchor::Middle, Role::Text, m.width(t, f));
        y += LINE + 12.0;
    }
    let head_top = y;
    let head_h = p.parts.iter().map(|pt| part_height(pt)).fold(BOX_H, f32::max);
    y += head_h + 18.0;

    let mut frames: Vec<Frame> = Vec::new();
    let mut active: Vec<Vec<f32>> = vec![Vec::new(); n];
    let mut bars: Vec<(usize, f32, f32, usize)> = Vec::new();
    let mut last_arrow = y;
    let mut number = 0usize;
    let touch = |frames: &mut Vec<Frame>, a: f32, b: f32| {
        for f in frames.iter_mut() {
            f.x0 = f.x0.min(a);
            f.x1 = f.x1.max(b);
        }
    };
    let off = |active: &Vec<Vec<f32>>, i: usize| if active[i].is_empty() { 0.0 } else { 5.0 + (active[i].len() - 1) as f32 * 4.0 };

    for ev in &p.events {
        match ev {
            Ev::Msg { from, to, text, dashed, head: hd, both, act } => {
                let (a, b) = (*from, *to);
                if a == b {
                    touch(&mut frames, x[a] - 12.0, x[a] + 46.0 + tw(text));
                } else {
                    touch(&mut frames, x[a].min(x[b]) - 12.0, x[a].max(x[b]) + 12.0);
                }
                let text_h = text.len() as f32 * lh;
                y += text_h + 4.0;
                let color = Paint::Role(Role::Edge);
                let dash = if *dashed { Dash::Dashed } else { Dash::Solid };
                if a == b {
                    let x0 = x[a] + off(&active, a);
                    let loop_h = 22.0;
                    let pts = vec![pos2(x0, y), pos2(x0 + 34.0, y), pos2(x0 + 34.0, y + loop_h), pos2(x0 + 2.0, y + loop_h)];
                    body.push(Prim::Poly { pts, closed: false, fill: Paint::None, stroke: color, width: 1.4, dash });
                    head(&mut body, pos2(x0, y + loop_h), vec2(-1.0, 0.0), *hd, color, 1.4);
                    for (k, l) in text.iter().enumerate() {
                        let ty = y - text_h - 2.0 + lh * (k as f32 + 0.5);
                        body.text(pos2(x0 + 6.0, ty), l.clone(), mf, Anchor::Start, Role::Text, m.width(l, mf));
                    }
                    last_arrow = y;
                    y += loop_h + 14.0;
                } else {
                    if *act == Some(true) {
                        active[b].push(y);
                    }
                    let sign = if b > a { 1.0 } else { -1.0 };
                    let xa = x[a] + sign * off(&active, a);
                    let xb = x[b] - sign * off(&active, b);
                    body.push(Prim::Poly { pts: vec![pos2(xa, y), pos2(xb, y)], closed: false, fill: Paint::None, stroke: color, width: 1.4, dash });
                    head(&mut body, pos2(xb, y), vec2(sign, 0.0), *hd, color, 1.4);
                    if *both {
                        head(&mut body, pos2(xa, y), vec2(-sign, 0.0), *hd, color, 1.4);
                    }
                    let cx = (xa + xb) / 2.0;
                    for (k, l) in text.iter().enumerate() {
                        let ty = y - text_h - 3.0 + lh * (k as f32 + 0.5);
                        body.text(pos2(cx, ty), l.clone(), mf, Anchor::Middle, Role::Text, m.width(l, mf));
                    }
                    if p.autonumber {
                        number += 1;
                        let c = pos2(xa + sign * 2.0, y);
                        body.push(Prim::Ellipse { center: c, radius: Vec2::splat(8.5), fill: Role::Accent.into(), stroke: Paint::None, width: 0.0 });
                        let f = Font::new(FONT * 0.72).bold();
                        let s = number.to_string();
                        body.text(c, s.clone(), f, Anchor::Middle, Role::Bg, m.width(&s, f));
                    }
                    if *act == Some(false) {
                        if let Some(start) = active[a].pop() {
                            bars.push((a, start, y, active[a].len()));
                        }
                    }
                    last_arrow = y;
                    y += 16.0;
                }
            }
            Ev::Note { pos, a, b, text } => {
                let w = tw(text) + 24.0;
                let h = text.len() as f32 * lh + 14.0;
                y += 4.0;
                let r = match pos {
                    NotePos::Right => Rect::from_min_size(pos2(x[*a] + 12.0, y), vec2(w, h)),
                    NotePos::Left => Rect::from_min_size(pos2(x[*a] - 12.0 - w, y), vec2(w, h)),
                    NotePos::Over => {
                        let (l, r) = (x[*a], x[*b]);
                        let w = w.max(r - l + 40.0);
                        Rect::from_min_size(pos2((l + r) / 2.0 - w / 2.0, y), vec2(w, h))
                    }
                };
                touch(&mut frames, r.left(), r.right());
                body.push(Prim::Rect { rect: r, radius: 4.0, fill: Role::NoteFill.into(), stroke: Role::NoteStroke.into(), width: 1.2, dash: Dash::Solid });
                for (k, l) in text.iter().enumerate() {
                    let ty = r.top() + 7.0 + lh * (k as f32 + 0.5);
                    body.text(pos2(r.center().x, ty), l.clone(), mf, Anchor::Middle, Role::Text, m.width(l, mf));
                }
                y += h + 12.0;
            }
            Ev::Start { kind, label } => {
                y += 6.0;
                frames.push(Frame { kind: kind.clone(), label: label.clone(), top: y, elses: Vec::new(), x0: f32::MAX, x1: f32::MIN });
                y += LINE + 14.0;
            }
            Ev::Else { label } => {
                if let Some(f) = frames.last_mut() {
                    y += 4.0;
                    f.elses.push((y, label.clone()));
                    y += LINE + 10.0;
                }
            }
            Ev::End => {
                let Some(f) = frames.pop() else { continue };
                y += 6.0;
                let (x0, x1) = if f.x0 > f.x1 { (x[0] - widths[0] / 2.0, x[n - 1] + widths[n - 1] / 2.0) } else { (f.x0, f.x1) };
                let mut r = Rect::from_min_max(pos2(x0 - 14.0, f.top), pos2(x1 + 14.0, y));
                // Ярлык и подписи условий должны помещаться в рамку.
                let need = frame_label_width(&f, m) + 24.0;
                if r.width() < need {
                    r.max.x = r.min.x + need;
                }
                // Внешняя рамка охватывает вложенную с запасом.
                touch(&mut frames, r.left() - 8.0, r.right() + 8.0);
                draw_frame(&mut frames_back, &mut frames_front, &f, r, m);
                y += 10.0;
            }
            Ev::Activate(i) => active[*i].push(last_arrow),
            Ev::Deactivate(i) => {
                if let Some(start) = active[*i].pop() {
                    bars.push((*i, start, last_arrow, active[*i].len()));
                }
            }
        }
    }
    let bottom = y + 8.0;
    for (i, stack) in active.iter().enumerate() {
        for (lvl, start) in stack.iter().enumerate() {
            bars.push((i, *start, bottom - 8.0, lvl));
        }
    }

    // Порядок слоёв: рамки, линии жизни, активации, сообщения, подписи рамок, участники.
    scene.prims.extend(frames_back);
    for i in 0..n {
        scene.push(Prim::Poly {
            pts: vec![pos2(x[i], head_top + head_h), pos2(x[i], bottom)],
            closed: false,
            fill: Paint::None,
            stroke: Role::ClusterStroke.into(),
            width: 1.2,
            dash: Dash::Dashed,
        });
    }
    for (i, y0, y1, lvl) in bars {
        let cx = x[i] + lvl as f32 * 4.0;
        scene.push(Prim::Rect {
            rect: Rect::from_min_max(pos2(cx - 5.0, y0 - 4.0), pos2(cx + 5.0, y1.max(y0 + 8.0) + 4.0)),
            radius: 2.0,
            fill: Role::ActorFill.into(),
            stroke: Role::NodeStroke.into(),
            width: 1.0,
            dash: Dash::Solid,
        });
    }
    scene.prims.extend(body.prims);
    scene.prims.extend(frames_front);
    for (i, pt) in p.parts.iter().enumerate() {
        draw_participant(&mut scene, pt, pos2(x[i], head_top), widths[i], head_h, m);
        draw_participant(&mut scene, pt, pos2(x[i], bottom), widths[i], head_h, m);
    }
    scene.finish(10.0);
    Ok(scene)
}

fn part_height(pt: &Participant) -> f32 {
    if pt.actor { 34.0 + pt.label.len() as f32 * LINE } else { BOX_H.max(pt.label.len() as f32 * LINE + 18.0) }
}

/// Участник: прямоугольник или человечек (`actor`). `top` — верх области высотой `h`.
fn draw_participant(scene: &mut Scene, pt: &Participant, top: Pos2, w: f32, h: f32, m: &dyn Measure) {
    let f = Font::new(FONT);
    if pt.actor {
        let c = top.x;
        let stroke = Paint::Role(Role::NodeStroke);
        let y0 = top.y + 2.0;
        scene.push(Prim::Ellipse { center: pos2(c, y0 + 6.0), radius: Vec2::splat(6.0), fill: Role::ActorFill.into(), stroke, width: 1.4 });
        let line = |scene: &mut Scene, a: Pos2, b: Pos2| {
            scene.push(Prim::Poly { pts: vec![a, b], closed: false, fill: Paint::None, stroke, width: 1.4, dash: Dash::Solid });
        };
        line(scene, pos2(c, y0 + 12.0), pos2(c, y0 + 22.0));
        line(scene, pos2(c - 9.0, y0 + 15.0), pos2(c + 9.0, y0 + 15.0));
        line(scene, pos2(c, y0 + 22.0), pos2(c - 7.0, y0 + 31.0));
        line(scene, pos2(c, y0 + 22.0), pos2(c + 7.0, y0 + 31.0));
        for (k, l) in pt.label.iter().enumerate() {
            let ty = y0 + 34.0 + LINE * (k as f32 + 0.5);
            scene.text(pos2(c, ty), l.clone(), f, Anchor::Middle, Role::Text, m.width(l, f));
        }
        return;
    }
    let r = Rect::from_min_size(pos2(top.x - w / 2.0, top.y), vec2(w, h));
    scene.push(Prim::Rect { rect: r, radius: 6.0, fill: Role::NodeFill.into(), stroke: Role::NodeStroke.into(), width: 1.4, dash: Dash::Solid });
    let y0 = r.center().y - (pt.label.len() as f32 - 1.0) * LINE / 2.0;
    for (k, l) in pt.label.iter().enumerate() {
        scene.text(pos2(r.center().x, y0 + k as f32 * LINE), l.clone(), f, Anchor::Middle, Role::Text, m.width(l, f));
    }
}

const FRAME_KIND: f32 = FONT * 0.8;
const FRAME_LABEL: f32 = FONT * 0.85;

/// Ширина, нужная рамке под ярлык и самую длинную подпись условия.
fn frame_label_width(f: &Frame, m: &dyn Measure) -> f32 {
    let lf = Font::new(FRAME_LABEL).italic();
    let tab = if f.kind == "rect" { 0.0 } else { m.width(&f.kind, Font::new(FRAME_KIND).bold()) + 28.0 };
    let label = |l: &str| if l.is_empty() { 0.0 } else { m.width(&format!("[{l}]"), lf) + 12.0 };
    let elses = f.elses.iter().map(|(_, l)| label(l)).fold(0.0, f32::max);
    (tab + label(&f.label)).max(elses)
}

/// Рамка блока (`loop`, `alt` …): фон и линии `else` — в `back`, ярлык и подписи — в `front`,
/// поверх линий жизни. Подписи стоят на подложке цвета фона, чтобы линии их не перечёркивали.
fn draw_frame(back: &mut Vec<Prim>, front: &mut Vec<Prim>, f: &Frame, r: Rect, m: &dyn Measure) {
    let rect_fill = if f.kind == "rect" { Role::ClusterFill } else { Role::Bg };
    back.push(Prim::Rect { rect: r, radius: 6.0, fill: Paint::Role(rect_fill), stroke: Role::ClusterStroke.into(), width: 1.2, dash: Dash::Solid });
    let mut s = Scene::default();
    let lf = Font::new(FRAME_LABEL).italic();
    let label = |s: &mut Scene, x: f32, y: f32, text: String| {
        let w = m.width(&text, lf);
        s.push(Prim::Rect {
            rect: Rect::from_min_max(pos2(x - 4.0, y - LINE * 0.42), pos2(x + w + 4.0, y + LINE * 0.42)),
            radius: 4.0,
            fill: Role::Bg.into(),
            stroke: Paint::None,
            width: 0.0,
            dash: Dash::Solid,
        });
        s.text(pos2(x, y), text, lf, Anchor::Start, Role::Muted, w);
    };
    if f.kind != "rect" {
        let kf = Font::new(FRAME_KIND).bold();
        let kw = m.width(&f.kind, kf);
        let tab = Rect::from_min_size(r.min, vec2(kw + 18.0, LINE + 4.0));
        s.push(Prim::Poly {
            pts: vec![tab.left_top(), tab.right_top(), pos2(tab.right(), tab.bottom() - 6.0), pos2(tab.right() - 6.0, tab.bottom()), tab.left_bottom()],
            closed: true,
            fill: Role::ClusterStroke.into(),
            stroke: Paint::None,
            width: 0.0,
            dash: Dash::Solid,
        });
        s.text(pos2(tab.left() + 9.0, tab.center().y), f.kind.clone(), kf, Anchor::Start, Role::Text, kw);
        if !f.label.is_empty() {
            label(&mut s, tab.right() + 10.0, tab.center().y, format!("[{}]", f.label));
        }
    } else if !f.label.is_empty() && !f.label.starts_with("rgb") {
        label(&mut s, r.left() + 10.0, r.top() + LINE / 2.0 + 4.0, f.label.clone());
    }
    for (yy, text) in &f.elses {
        back.push(Prim::Poly {
            pts: vec![pos2(r.left(), *yy), pos2(r.right(), *yy)],
            closed: false,
            fill: Paint::None,
            stroke: Role::ClusterStroke.into(),
            width: 1.2,
            dash: Dash::Dashed,
        });
        if !text.is_empty() {
            label(&mut s, r.left() + 10.0, yy + LINE / 2.0 + 4.0, format!("[{text}]"));
        }
    }
    front.extend(s.prims);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::text::Approx;

    #[test]
    fn parses_and_lays_out() {
        let src = "sequenceDiagram\n  autonumber\n  actor U as Пользователь\n  participant S as Сервер\n  U->>+S: Запрос\n  loop Каждую секунду\n    S-->>S: проверка\n  end\n  alt всё хорошо\n    S-->>-U: 200 OK\n  else ошибка\n    S--xU: 500\n  end\n  Note over U,S: конец\n  Note right of S: заметка";
        let p = parse(src).unwrap();
        assert_eq!(p.parts.len(), 2);
        assert!(p.parts[0].actor);
        assert_eq!(p.parts[1].label, vec!["Сервер"]);
        assert!(p.autonumber);
        let msgs = p.events.iter().filter(|e| matches!(e, Ev::Msg { .. })).count();
        assert_eq!(msgs, 4);
        let s = layout(src, &Approx).unwrap();
        assert!(s.size.x > 200.0 && s.size.y > 200.0);
    }

    #[test]
    fn non_ascii_before_keywords() {
        // 'İ' в нижнем регистре длиннее на байт, знак Кельвина 'K' становится 'k':
        // позиции из строчной копии не должны резать исходную строку посреди символа.
        let p = parse("sequenceDiagram\n  Note İİ over Жанна: привет").unwrap();
        assert_eq!(p.parts[0].label, vec!["Жанна"]);
        let _ = parse("sequenceDiagram\n  brea\u{212A} цикл\n  end");
    }
}
