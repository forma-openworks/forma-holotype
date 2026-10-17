//! Сцена диаграммы — набор простых фигур. Её можно нарисовать через egui (в приложении)
//! или выгрузить в SVG (при экспорте). Цвета задаются ролями, поэтому смена темы
//! не требует заново раскладывать диаграмму.

use std::fmt::Write as _;

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2,
    epaint::{EllipseShape, Mesh, PathShape, PathStroke, WHITE_UV},
    pos2, vec2,
};

use crate::theme::Palette;

/// Роль цвета: превращается в конкретный цвет темы при рисовании.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    NodeFill,
    NodeStroke,
    Text,
    Muted,
    Edge,
    LabelBg,
    ClusterFill,
    ClusterStroke,
    NoteFill,
    NoteStroke,
    Accent,
    Bg,
    /// Самый контрастный цвет (точки начала и конца в диаграммах состояний).
    Strong,
    ActorFill,
    /// Цвета рядов круговой диаграммы.
    Series(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint {
    None,
    Role(Role),
    Rgb(Color32),
}

impl From<Role> for Paint {
    fn from(r: Role) -> Self {
        Paint::Role(r)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dash {
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    Start,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Font {
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
}

impl Font {
    pub fn new(size: f32) -> Self {
        Self { size, ..Default::default() }
    }
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
}

#[derive(Clone, Debug)]
pub enum Prim {
    Rect { rect: Rect, radius: f32, fill: Paint, stroke: Paint, width: f32, dash: Dash },
    Ellipse { center: Pos2, radius: Vec2, fill: Paint, stroke: Paint, width: f32 },
    Poly { pts: Vec<Pos2>, closed: bool, fill: Paint, stroke: Paint, width: f32, dash: Dash },
    /// Цепочка кубических кривых: p0, c1, c2, p1, c1, c2, p2, …
    Curve { pts: Vec<Pos2>, stroke: Paint, width: f32, dash: Dash },
    /// Сектор круга. Углы в радианах по часовой стрелке от направления «вверх».
    Wedge { center: Pos2, radius: f32, a0: f32, a1: f32, fill: Paint },
    /// Строка текста. `pos.y` — середина строки, `pos.x` — точка привязки.
    Text { pos: Pos2, text: String, font: Font, anchor: Anchor, color: Paint, width: f32 },
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub size: Vec2,
    pub prims: Vec<Prim>,
}

/// Конкретные цвета для рисования в приложении.
#[derive(Clone, Debug)]
pub struct Colors {
    pub node_fill: Color32,
    pub node_stroke: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub edge: Color32,
    pub label_bg: Color32,
    pub cluster_fill: Color32,
    pub cluster_stroke: Color32,
    pub note_fill: Color32,
    pub note_stroke: Color32,
    pub accent: Color32,
    pub bg: Color32,
    pub strong: Color32,
    pub actor_fill: Color32,
    pub dark: bool,
}

/// Цвета рядов (круговая диаграмма): мягкие и различимые в обеих темах.
pub const SERIES: [(u8, u8, u8); 10] = [
    (139, 92, 246),
    (56, 189, 248),
    (52, 211, 153),
    (251, 191, 36),
    (244, 114, 182),
    (251, 146, 60),
    (45, 212, 191),
    (129, 140, 248),
    (163, 230, 53),
    (248, 113, 113),
];

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

impl Colors {
    /// Цвета из палитры приложения; `bg` — фон, на котором рисуется диаграмма.
    pub fn from_palette(pal: &Palette, bg: Color32) -> Self {
        let amber = Color32::from_rgb(245, 180, 40);
        Self {
            node_fill: mix(bg, pal.accent, if pal.dark { 0.16 } else { 0.09 }),
            node_stroke: mix(pal.accent, bg, if pal.dark { 0.15 } else { 0.05 }),
            text: pal.text,
            muted: pal.text_muted,
            edge: mix(pal.text_muted, bg, 0.12),
            label_bg: bg,
            cluster_fill: mix(bg, pal.accent, if pal.dark { 0.05 } else { 0.035 }),
            cluster_stroke: mix(pal.border, pal.accent, 0.25),
            note_fill: mix(bg, amber, if pal.dark { 0.16 } else { 0.18 }),
            note_stroke: mix(amber, bg, if pal.dark { 0.35 } else { 0.2 }),
            accent: pal.accent,
            bg,
            strong: pal.heading,
            actor_fill: mix(bg, pal.accent, if pal.dark { 0.22 } else { 0.12 }),
            dark: pal.dark,
        }
    }

    fn role(&self, r: Role) -> Color32 {
        match r {
            Role::NodeFill => self.node_fill,
            Role::NodeStroke => self.node_stroke,
            Role::Text => self.text,
            Role::Muted => self.muted,
            Role::Edge => self.edge,
            Role::LabelBg => self.label_bg,
            Role::ClusterFill => self.cluster_fill,
            Role::ClusterStroke => self.cluster_stroke,
            Role::NoteFill => self.note_fill,
            Role::NoteStroke => self.note_stroke,
            Role::Accent => self.accent,
            Role::Bg => self.bg,
            Role::Strong => self.strong,
            Role::ActorFill => self.actor_fill,
            Role::Series(i) => {
                let (r, g, b) = SERIES[i as usize % SERIES.len()];
                let c = Color32::from_rgb(r, g, b);
                if self.dark { c } else { mix(c, Color32::BLACK, 0.08) }
            }
        }
    }

    fn get(&self, p: Paint) -> Option<Color32> {
        match p {
            Paint::None => None,
            Paint::Role(r) => Some(self.role(r)),
            Paint::Rgb(c) => Some(c),
        }
    }
}

impl Scene {
    pub fn push(&mut self, p: Prim) {
        self.prims.push(p);
    }

    pub fn text(&mut self, pos: Pos2, text: impl Into<String>, font: Font, anchor: Anchor, color: impl Into<Paint>, width: f32) {
        let text = text.into();
        if text.is_empty() {
            return;
        }
        self.prims.push(Prim::Text { pos, text, font, anchor, color: color.into(), width });
    }

    /// Сдвигает всё так, чтобы сцена начиналась с отступа `margin`, и вычисляет её размер.
    pub fn finish(&mut self, margin: f32) {
        let mut min = pos2(f32::MAX, f32::MAX);
        let mut max = pos2(f32::MIN, f32::MIN);
        let mut add = |p: Pos2| {
            min = min.min(p);
            max = max.max(p);
        };
        for p in &self.prims {
            match p {
                Prim::Rect { rect, .. } => {
                    add(rect.min);
                    add(rect.max);
                }
                Prim::Ellipse { center, radius, .. } => {
                    add(*center - *radius);
                    add(*center + *radius);
                }
                Prim::Poly { pts, .. } | Prim::Curve { pts, .. } => pts.iter().for_each(|p| add(*p)),
                Prim::Wedge { center, radius, .. } => {
                    add(*center - Vec2::splat(*radius));
                    add(*center + Vec2::splat(*radius));
                }
                Prim::Text { pos, font, anchor, width, .. } => {
                    let x0 = match anchor {
                        Anchor::Start => pos.x,
                        Anchor::Middle => pos.x - width / 2.0,
                    };
                    add(pos2(x0, pos.y - font.size * 0.7));
                    add(pos2(x0 + width, pos.y + font.size * 0.7));
                }
            }
        }
        if min.x > max.x {
            self.size = Vec2::ZERO;
            return;
        }
        let shift = vec2(margin, margin) - min.to_vec2();
        for p in &mut self.prims {
            match p {
                Prim::Rect { rect, .. } => *rect = rect.translate(shift),
                Prim::Ellipse { center, .. } | Prim::Wedge { center, .. } => *center += shift,
                Prim::Poly { pts, .. } | Prim::Curve { pts, .. } => pts.iter_mut().for_each(|p| *p += shift),
                Prim::Text { pos, .. } => *pos += shift,
            }
        }
        self.size = (max - min) + vec2(margin, margin) * 2.0;
    }

    // ----------------------------------------------------------------------------------------
    // Рисование через egui
    // ----------------------------------------------------------------------------------------

    pub fn paint(&self, painter: &Painter, origin: Pos2, scale: f32, c: &Colors) {
        let tp = |p: Pos2| origin + p.to_vec2() * scale;
        for prim in &self.prims {
            match prim {
                Prim::Rect { rect, radius, fill, stroke, width, dash } => {
                    let r = Rect::from_min_max(tp(rect.min), tp(rect.max));
                    let cr = CornerRadius::same((radius * scale).round().clamp(0.0, 255.0) as u8);
                    if let Some(f) = c.get(*fill) {
                        painter.rect_filled(r, cr, f);
                    }
                    if let Some(s) = c.get(*stroke) {
                        let st = Stroke::new(width * scale, s);
                        if *dash == Dash::Solid {
                            painter.rect_stroke(r, cr, st, eframe::egui::StrokeKind::Middle);
                        } else {
                            let pts = vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                            dashed(painter, &pts, st, *dash, scale);
                        }
                    }
                }
                Prim::Ellipse { center, radius, fill, stroke, width } => {
                    painter.add(EllipseShape {
                        center: tp(*center),
                        radius: *radius * scale,
                        fill: c.get(*fill).unwrap_or(Color32::TRANSPARENT),
                        stroke: c.get(*stroke).map_or(Stroke::NONE, |s| Stroke::new(width * scale, s)),
                        angle: 0.0,
                    });
                }
                Prim::Poly { pts, closed, fill, stroke, width, dash } => {
                    let pts: Vec<Pos2> = pts.iter().map(|p| tp(*p)).collect();
                    if let Some(f) = c.get(*fill) {
                        fill_polygon(painter, &pts, f);
                    }
                    if let Some(s) = c.get(*stroke) {
                        let st = Stroke::new(width * scale, s);
                        if *dash == Dash::Solid {
                            if *closed {
                                painter.add(PathShape::closed_line(pts, st));
                            } else {
                                painter.add(PathShape::line(pts, st));
                            }
                        } else {
                            let mut pts = pts;
                            if *closed {
                                pts.push(pts[0]);
                            }
                            dashed(painter, &pts, st, *dash, scale);
                        }
                    }
                }
                Prim::Curve { pts, stroke, width, dash } => {
                    let Some(s) = c.get(*stroke) else { continue };
                    let pts: Vec<Pos2> = flatten(pts).into_iter().map(tp).collect();
                    let st = Stroke::new(width * scale, s);
                    if *dash == Dash::Solid {
                        painter.add(PathShape { points: pts, closed: false, fill: Color32::TRANSPARENT, stroke: PathStroke::from(st) });
                    } else {
                        dashed(painter, &pts, st, *dash, scale);
                    }
                }
                Prim::Wedge { center, radius, a0, a1, fill } => {
                    let Some(f) = c.get(*fill) else { continue };
                    let ctr = tp(*center);
                    let r = radius * scale;
                    let n = (((a1 - a0).abs() * r / 4.0).ceil() as usize).clamp(4, 256);
                    let arc: Vec<Pos2> =
                        (0..=n).map(|i| a0 + (a1 - a0) * i as f32 / n as f32).map(|a| ctr + vec2(a.sin(), -a.cos()) * r).collect();
                    let mut mesh = Mesh::default();
                    mesh.colored_vertex(ctr, f);
                    for p in &arc {
                        mesh.colored_vertex(*p, f);
                    }
                    for i in 0..n as u32 {
                        mesh.add_triangle(0, i + 1, i + 2);
                    }
                    painter.add(Shape::mesh(mesh));
                    // Сглаживаем внешний край тонкой линией того же цвета.
                    painter.add(PathShape::line(arc, Stroke::new(1.0, f)));
                }
                Prim::Text { pos, text, font, anchor, color, .. } => {
                    let Some(col) = c.get(*color) else { continue };
                    let align = match anchor {
                        Anchor::Start => Align2::LEFT_CENTER,
                        Anchor::Middle => Align2::CENTER_CENTER,
                    };
                    painter.text(tp(*pos), align, text, FontId::new(font.size * scale, super::text::family(*font)), col);
                }
            }
        }
    }

    // ----------------------------------------------------------------------------------------
    // SVG для экспорта
    // ----------------------------------------------------------------------------------------

    /// SVG-разметка. Цвета ролей берутся из CSS-переменных `--dg-*` страницы.
    pub fn to_svg(&self) -> String {
        let w = self.size.x.ceil();
        let h = self.size.y.ceil();
        let mut s = format!(
            r#"<svg class="dg" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img">"#
        );
        for prim in &self.prims {
            match prim {
                Prim::Rect { rect, radius, fill, stroke, width, dash } => {
                    let _ = write!(
                        s,
                        r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.1}" style="{}"/>"#,
                        rect.min.x,
                        rect.min.y,
                        rect.width(),
                        rect.height(),
                        radius,
                        style(*fill, *stroke, *width, *dash)
                    );
                }
                Prim::Ellipse { center, radius, fill, stroke, width } => {
                    let _ = write!(
                        s,
                        r#"<ellipse cx="{:.1}" cy="{:.1}" rx="{:.1}" ry="{:.1}" style="{}"/>"#,
                        center.x,
                        center.y,
                        radius.x,
                        radius.y,
                        style(*fill, *stroke, *width, Dash::Solid)
                    );
                }
                Prim::Poly { pts, closed, fill, stroke, width, dash } => {
                    let tag = if *closed { "polygon" } else { "polyline" };
                    let fill = if *closed { *fill } else { Paint::None };
                    let _ = write!(s, r#"<{tag} points="{}" style="{}"/>"#, points(pts), style(fill, *stroke, *width, *dash));
                }
                Prim::Curve { pts, stroke, width, dash } => {
                    if pts.is_empty() {
                        continue;
                    }
                    let mut d = format!("M{:.1} {:.1}", pts[0].x, pts[0].y);
                    for seg in pts[1..].chunks(3) {
                        if let [c1, c2, p] = seg {
                            let _ = write!(d, " C{:.1} {:.1} {:.1} {:.1} {:.1} {:.1}", c1.x, c1.y, c2.x, c2.y, p.x, p.y);
                        }
                    }
                    let _ = write!(s, r#"<path d="{d}" style="{}"/>"#, style(Paint::None, *stroke, *width, *dash));
                }
                Prim::Wedge { center, radius, a0, a1, fill } => {
                    let p = |a: f32| (center.x + a.sin() * radius, center.y - a.cos() * radius);
                    let (x0, y0) = p(*a0);
                    let (x1, y1) = p(*a1);
                    let large = if (a1 - a0).abs() > std::f32::consts::PI { 1 } else { 0 };
                    if (a1 - a0).abs() >= std::f32::consts::TAU - 1e-3 {
                        let _ = write!(
                            s,
                            r#"<circle cx="{:.1}" cy="{:.1}" r="{:.1}" style="{}"/>"#,
                            center.x,
                            center.y,
                            radius,
                            style(*fill, Paint::None, 0.0, Dash::Solid)
                        );
                    } else {
                        let _ = write!(
                            s,
                            r#"<path d="M{:.1} {:.1} L{x0:.1} {y0:.1} A{r:.1} {r:.1} 0 {large} 1 {x1:.1} {y1:.1} Z" style="{}"/>"#,
                            center.x,
                            center.y,
                            style(*fill, Paint::None, 0.0, Dash::Solid),
                            r = radius
                        );
                    }
                }
                Prim::Text { pos, text, font, anchor, color, .. } => {
                    let anchor = match anchor {
                        Anchor::Start => "start",
                        Anchor::Middle => "middle",
                    };
                    let mut st = format!("fill:{};font-size:{:.1}px", css(*color), font.size);
                    if font.bold {
                        st.push_str(";font-weight:600");
                    }
                    if font.italic {
                        st.push_str(";font-style:italic");
                    }
                    if font.mono {
                        st.push_str(";font-family:var(--mono)");
                    }
                    let _ = write!(
                        s,
                        r#"<text x="{:.1}" y="{:.1}" text-anchor="{anchor}" dominant-baseline="central" style="{st}">{}</text>"#,
                        pos.x,
                        pos.y,
                        esc(text)
                    );
                }
            }
        }
        s.push_str("</svg>");
        s
    }
}

fn points(pts: &[Pos2]) -> String {
    pts.iter().map(|p| format!("{:.1},{:.1}", p.x, p.y)).collect::<Vec<_>>().join(" ")
}

fn css(p: Paint) -> String {
    match p {
        Paint::None => "none".to_owned(),
        Paint::Rgb(c) if c.a() < 255 => {
            format!("rgba({},{},{},{:.2})", c.r(), c.g(), c.b(), c.a() as f32 / 255.0)
        }
        Paint::Rgb(c) => format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()),
        Paint::Role(r) => {
            let name = match r {
                Role::NodeFill => "node-fill".to_owned(),
                Role::NodeStroke => "node-stroke".to_owned(),
                Role::Text => "text".to_owned(),
                Role::Muted => "muted".to_owned(),
                Role::Edge => "edge".to_owned(),
                Role::LabelBg => "bg".to_owned(),
                Role::ClusterFill => "cluster-fill".to_owned(),
                Role::ClusterStroke => "cluster-stroke".to_owned(),
                Role::NoteFill => "note-fill".to_owned(),
                Role::NoteStroke => "note-stroke".to_owned(),
                Role::Accent => "accent".to_owned(),
                Role::Bg => "bg".to_owned(),
                Role::Strong => "strong".to_owned(),
                Role::ActorFill => "actor-fill".to_owned(),
                Role::Series(i) => {
                    let (r, g, b) = SERIES[i as usize % SERIES.len()];
                    return format!("#{r:02x}{g:02x}{b:02x}");
                }
            };
            format!("var(--dg-{name})")
        }
    }
}

fn style(fill: Paint, stroke: Paint, width: f32, dash: Dash) -> String {
    let mut s = format!("fill:{}", css(fill));
    if stroke != Paint::None {
        let _ = write!(s, ";stroke:{};stroke-width:{width:.1}", css(stroke));
        match dash {
            Dash::Solid => {}
            Dash::Dashed => s.push_str(";stroke-dasharray:6 4"),
            Dash::Dotted => s.push_str(";stroke-dasharray:3 3"),
        }
    }
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Точки кривой Безье (с запасом, чтобы изгибы были гладкими).
pub fn flatten(pts: &[Pos2]) -> Vec<Pos2> {
    let Some(first) = pts.first() else { return Vec::new() };
    let mut out = vec![*first];
    let mut p0 = *first;
    for seg in pts[1..].chunks(3) {
        let [c1, c2, p1] = seg else { break };
        let len = p0.distance(*c1) + c1.distance(*c2) + c2.distance(*p1);
        let n = ((len / 5.0).ceil() as usize).clamp(2, 64);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let p = p0.to_vec2() * (u * u * u)
                + c1.to_vec2() * (3.0 * u * u * t)
                + c2.to_vec2() * (3.0 * u * t * t)
                + p1.to_vec2() * (t * t * t);
            out.push(p.to_pos2());
        }
        p0 = *p1;
    }
    out
}

fn dashed(painter: &Painter, pts: &[Pos2], stroke: Stroke, dash: Dash, scale: f32) {
    let (d, g) = match dash {
        Dash::Dotted => (3.0 * scale, 3.0 * scale),
        _ => (6.0 * scale, 4.0 * scale),
    };
    painter.extend(Shape::dashed_line(pts, stroke, d.max(1.0), g.max(1.0)));
}

/// Заливка многоугольника: выпуклые — штатно egui (со сглаживанием), остальные — веером
/// треугольников от центра и тонкой линией по краю для сглаживания.
fn fill_polygon(painter: &Painter, pts: &[Pos2], color: Color32) {
    if pts.len() < 3 {
        return;
    }
    if is_convex(pts) {
        painter.add(Shape::convex_polygon(pts.to_vec(), color, Stroke::NONE));
        return;
    }
    let c = pts.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / pts.len() as f32;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c.to_pos2(), color);
    for p in pts {
        mesh.vertices.push(eframe::egui::epaint::Vertex { pos: *p, uv: WHITE_UV, color });
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, i + 1, (i + 1) % n + 1);
    }
    painter.add(Shape::mesh(mesh));
    painter.add(PathShape::closed_line(pts.to_vec(), Stroke::new(1.0, color)));
}

fn is_convex(pts: &[Pos2]) -> bool {
    let n = pts.len();
    let mut sign = 0.0f32;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let c = pts[(i + 2) % n];
        let cross = (b - a).x * (c - b).y - (b - a).y * (c - b).x;
        if cross.abs() < 1e-4 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign {
            return false;
        }
    }
    true
}
