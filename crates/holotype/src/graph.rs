//! Граф связей между заметками с силовой раскладкой (как в Obsidian).
//!
//! Узел, который перетащили мышью, закрепляется на месте: физика его больше не двигает,
//! а положение сохраняется в хранилище (`.holotype/graph.json`). Остальные узлы
//! раскладываются силами вокруг закреплённых. Кнопка «Сбросить расположение» откатывает всё.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Margin, Rect, Sense, Stroke, Ui, Vec2,
    pos2, vec2,
};

use crate::theme::{self, Palette};
use crate::vault::Vault;
use forma_i18n::t;

pub enum GraphAction {
    Open(PathBuf),
    /// Щелчок по ещё не созданной заметке.
    Create(String),
}

struct GNode {
    label: String,
    rel: Option<PathBuf>,
    pos: Vec2,
    vel: Vec2,
    degree: usize,
    /// Закреплён пользователем: стоит там, куда его поставили.
    pinned: bool,
}

/// Сохранённое расположение графа: закреплённые узлы и их координаты.
#[derive(Serialize, Deserialize, Default)]
struct LayoutFile {
    version: u32,
    pinned: BTreeMap<String, [f32; 2]>,
}

pub struct Graph {
    nodes: Vec<GNode>,
    edges: Vec<(usize, usize)>,
    neighbors: Vec<Vec<usize>>,
    built: Option<(u64, bool, bool)>,
    pan: Vec2,
    zoom: f32,
    alpha: f32,
    dragging: Option<usize>,
    auto_fit: bool,
    /// Узел, чьи соседи подсвечены (держим, пока подсветка плавно гаснет).
    last_focus: Option<usize>,
    /// Закреплённые узлы (ключ — путь заметки) и хранилище, для которого они загружены.
    pins: BTreeMap<String, Vec2>,
    pins_root: Option<PathBuf>,
    /// Узел под правым щелчком (для контекстного меню).
    menu_node: Option<usize>,
    pub show_orphans: bool,
    pub show_ghosts: bool,
    pub show_labels: bool,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            neighbors: Vec::new(),
            built: None,
            pan: Vec2::ZERO,
            zoom: 1.0,
            alpha: 1.0,
            dragging: None,
            auto_fit: true,
            last_focus: None,
            pins: BTreeMap::new(),
            pins_root: None,
            menu_node: None,
            show_orphans: true,
            show_ghosts: true,
            show_labels: true,
        }
    }
}

impl Graph {
    /// Перестраивает граф, если хранилище изменилось. Позиции существующих узлов сохраняются.
    pub fn sync(&mut self, vault: &Vault) {
        // Другое хранилище — подгружаем его сохранённое расположение.
        if self.pins_root.as_deref() != Some(vault.root.as_path()) {
            self.pins = load_layout(&vault.root);
            self.pins_root = Some(vault.root.clone());
            self.nodes.clear();
            self.built = None;
            self.auto_fit = true;
        }
        let key = (vault.generation, self.show_orphans, self.show_ghosts);
        if self.built == Some(key) {
            return;
        }
        self.built = Some(key);
        self.last_focus = None;
        let old: HashMap<String, (Vec2, Vec2)> =
            self.nodes.iter().map(|n| (node_key(n), (n.pos, n.vel))).collect();

        let mut nodes: Vec<GNode> = vault
            .notes
            .iter()
            .map(|n| GNode {
                label: n.name.clone(),
                rel: Some(n.rel.clone()),
                pos: Vec2::ZERO,
                vel: Vec2::ZERO,
                degree: 0,
                pinned: false,
            })
            .collect();
        let mut ghost_index: HashMap<String, usize> = HashMap::new();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for (i, note) in vault.notes.iter().enumerate() {
            for link in &note.links {
                let j = match vault.resolve(link, Some(&note.rel)) {
                    Some(j) => j,
                    None if self.show_ghosts => {
                        let name = crate::markdown::link_note_part(link).to_owned();
                        if name.is_empty() {
                            continue;
                        }
                        *ghost_index.entry(name.to_lowercase()).or_insert_with(|| {
                            nodes.push(GNode {
                                label: name.clone(),
                                rel: None,
                                pos: Vec2::ZERO,
                                vel: Vec2::ZERO,
                                degree: 0,
                                pinned: false,
                            });
                            nodes.len() - 1
                        })
                    }
                    None => continue,
                };
                if i != j {
                    let e = (i.min(j), i.max(j));
                    if !edges.contains(&e) {
                        edges.push(e);
                    }
                }
            }
        }
        for &(a, b) in &edges {
            nodes[a].degree += 1;
            nodes[b].degree += 1;
        }
        // Убираем «сироты» без связей, если их не нужно показывать.
        let keep: Vec<bool> = nodes.iter().map(|n| self.show_orphans || n.degree > 0).collect();
        let mut remap = vec![usize::MAX; nodes.len()];
        let mut kept = Vec::new();
        for (i, n) in nodes.into_iter().enumerate() {
            if keep[i] {
                remap[i] = kept.len();
                kept.push(n);
            }
        }
        let edges: Vec<(usize, usize)> = edges
            .into_iter()
            .filter(|(a, b)| keep[*a] && keep[*b])
            .map(|(a, b)| (remap[a], remap[b]))
            .collect();

        // Начальные позиции: закреплённые — на своих местах, остальные — прежние
        // или по золотой спирали.
        for (i, n) in kept.iter_mut().enumerate() {
            let key = node_key(n);
            if let Some(p) = self.pins.get(&key) {
                n.pos = *p;
                n.pinned = true;
            } else if let Some((p, v)) = old.get(&key) {
                n.pos = *p;
                n.vel = *v;
            } else {
                n.pos = spiral(i);
            }
        }
        let mut neighbors = vec![Vec::new(); kept.len()];
        for &(a, b) in &edges {
            neighbors[a].push(b);
            neighbors[b].push(a);
        }
        self.nodes = kept;
        self.edges = edges;
        self.neighbors = neighbors;
        self.alpha = self.alpha.max(0.6);
    }

    fn step(&mut self) {
        let n = self.nodes.len();
        if n == 0 {
            return;
        }
        let k_rep = 9000.0;
        let spring = 95.0;
        let k_spring = 0.045;
        let gravity = 0.01;
        let mut force = vec![Vec2::ZERO; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.nodes[i].pos - self.nodes[j].pos;
                let dist2 = d.length_sq().max(36.0);
                let f = d / dist2.sqrt() * (k_rep / dist2);
                force[i] += f;
                force[j] -= f;
            }
        }
        for &(a, b) in &self.edges {
            let d = self.nodes[b].pos - self.nodes[a].pos;
            let dist = d.length().max(0.1);
            let f = d / dist * (dist - spring) * k_spring;
            force[a] += f;
            force[b] -= f;
        }
        for (i, node) in self.nodes.iter_mut().enumerate() {
            // Схваченный и закреплённые узлы физика не двигает — остальные подстраиваются под них.
            if self.dragging == Some(i) || node.pinned {
                node.vel = Vec2::ZERO;
                continue;
            }
            let f = force[i] - node.pos * gravity;
            node.vel = (node.vel + f) * 0.82;
            let speed = node.vel.length();
            if speed > 40.0 {
                node.vel *= 40.0 / speed;
            }
            node.pos += node.vel * self.alpha;
        }
        self.alpha = (self.alpha * 0.988).max(if self.dragging.is_some() { 0.3 } else { 0.0 });
    }

    /// Масштаб и сдвиг, при которых весь граф помещается в прямоугольник.
    fn fit_target(&self, rect: Rect) -> Option<(f32, Vec2)> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut min = vec2(f32::MAX, f32::MAX);
        let mut max = vec2(f32::MIN, f32::MIN);
        for n in &self.nodes {
            min = min.min(n.pos);
            max = max.max(n.pos);
        }
        let size = (max - min).max(vec2(160.0, 160.0));
        let zoom = ((rect.width() - 160.0) / size.x).min((rect.height() - 160.0) / size.y).clamp(0.2, 2.2);
        Some((zoom, -(min + size / 2.0) * zoom))
    }

    pub fn ui(&mut self, ui: &mut Ui, pal: &Palette, current: Option<&Path>) -> Option<GraphAction> {
        let mut action = None;
        if self.alpha > 0.005 {
            for _ in 0..2 {
                self.step();
            }
            ui.ctx().request_repaint();
        }
        // Пока пользователь не двигал вид, плавно вписываем граф в окно.
        if self.auto_fit {
            if let Some((zoom, pan)) = self.fit_target(ui.available_rect_before_wrap()) {
                let t = 0.18;
                self.zoom += (zoom - self.zoom) * t;
                self.pan += (pan - self.pan) * t;
                if (zoom - self.zoom).abs() > 0.002 || (pan - self.pan).length() > 0.5 {
                    ui.ctx().request_repaint();
                }
            }
        }

        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        painter.rect_filled(rect, CornerRadius::ZERO, pal.bg);

        // Точечная сетка для атмосферы.
        let spacing = 36.0 * self.zoom;
        if spacing > 14.0 {
            let origin = rect.center() + self.pan;
            let dot = if pal.dark { Color32::from_white_alpha(10) } else { Color32::from_black_alpha(14) };
            let start_x = rect.left() + (origin.x - rect.left()).rem_euclid(spacing);
            let start_y = rect.top() + (origin.y - rect.top()).rem_euclid(spacing);
            let mut y = start_y;
            while y < rect.bottom() {
                let mut x = start_x;
                while x < rect.right() {
                    painter.circle_filled(pos2(x, y), 1.0, dot);
                    x += spacing;
                }
                y += spacing;
            }
        }

        // Пассивная «диффузия»: каждый узел медленно плавает вокруг своего места.
        // Это только визуальное смещение — раскладка графа от него не меняется.
        let time = ui.input(|i| i.time) as f32;
        let drift = |i: usize| -> Vec2 {
            let k = i as f32;
            let ph1 = (k * 1.618).fract() * std::f32::consts::TAU;
            let ph2 = (k * 2.414).fract() * std::f32::consts::TAU;
            let ph3 = (k * 0.732).fract() * std::f32::consts::TAU;
            vec2(
                (time * 0.55 + ph1).sin() * 2.4 + (time * 0.21 + ph3).sin() * 1.6,
                (time * 0.47 + ph2).cos() * 2.4 + (time * 0.17 + ph1).cos() * 1.6,
            )
        };
        // Схваченный узел плавно замирает, а после отпускания так же плавно «оживает» — без рывка.
        let positions: Vec<egui::Pos2> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let held = ui.ctx().animate_bool_with_time(egui::Id::new(("graph-held", i)), self.dragging == Some(i), 0.35);
                rect.center() + self.pan + (n.pos + drift(i) * (1.0 - held)) * self.zoom
            })
            .collect();
        ui.ctx().request_repaint();
        let radius = |deg: usize, zoom: f32| (3.5 + (deg as f32).sqrt() * 1.9) * zoom.clamp(0.6, 1.8);

        // Наведение
        let pointer = resp.hover_pos();
        let mut hovered = None;
        if let Some(p) = pointer {
            let mut best = f32::MAX;
            for (i, n) in self.nodes.iter().enumerate() {
                let d = positions[i].distance(p);
                if d < radius(n.degree, self.zoom) + 6.0 && d < best {
                    best = d;
                    hovered = Some(i);
                }
            }
        }
        if let Some(i) = self.dragging {
            hovered = Some(i);
        }

        // Масштаб колесом вокруг курсора
        if let Some(p) = pointer {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            let pinch = ui.input(|i| i.zoom_delta());
            let factor = (scroll * 0.0015).exp() * pinch;
            if (factor - 1.0).abs() > 1e-4 {
                let new_zoom = (self.zoom * factor).clamp(0.15, 4.0);
                let world = (p - rect.center() - self.pan) / self.zoom;
                self.zoom = new_zoom;
                self.auto_fit = false;
                self.pan = p - rect.center() - world * self.zoom;
            }
        }

        // Перетаскивание узла или всего полотна
        if resp.drag_started() {
            self.dragging = hovered;
        }
        if resp.dragged() {
            match self.dragging {
                Some(i) => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.nodes[i].pos = (p - rect.center() - self.pan) / self.zoom;
                        self.alpha = self.alpha.max(0.3);
                    }
                }
                None => {
                    self.pan += resp.drag_delta();
                    self.auto_fit = false;
                }
            }
        }
        if resp.drag_stopped() {
            // Перетащенный узел остаётся там, куда его поставили.
            if let Some(i) = self.dragging.take() {
                self.pin(i);
            }
        }
        if resp.clicked() {
            if let Some(i) = hovered {
                action = Some(self.open_action(i));
            }
        }
        // Правый щелчок: меню узла и сброс расположения.
        if resp.secondary_clicked() {
            self.menu_node = hovered;
        }
        theme::context_menu(&resp, |ui| {
            if let Some(i) = self.menu_node.filter(|&i| i < self.nodes.len()) {
                let open_label = if self.nodes[i].rel.is_some() { t!("graph-open-note") } else { t!("graph-create-note") };
                if ui.button(open_label).clicked() {
                    action = Some(self.open_action(i));
                }
                if self.nodes[i].pinned {
                    if ui.button(t!("graph-unpin")).clicked() {
                        self.unpin(i);
                    }
                } else if ui.button(t!("graph-pin")).clicked() {
                    self.pin(i);
                }
                ui.separator();
            }
            if ui.button(format!("↺ {}", t!("graph-reset-layout"))).clicked() {
                self.reset_layout();
            }
        });
        if hovered.is_some() {
            ui.ctx().set_cursor_icon(if self.dragging.is_some() { CursorIcon::Grabbing } else { CursorIcon::PointingHand });
        }

        // Подсветка соседей включается и гаснет плавно; после ухода курсора помним, кого подсвечивали.
        if hovered.is_some() {
            self.last_focus = hovered;
        }
        let ctx = ui.ctx().clone();
        let focus_t = ctx.animate_bool_with_time(egui::Id::new("graph-focus"), hovered.is_some(), 0.2);
        if focus_t <= 0.0 {
            self.last_focus = None;
        }
        let focus = self.last_focus;
        let is_lit = |i: usize| focus.is_none_or(|f| f == i || self.neighbors.get(f).is_some_and(|n| n.contains(&i)));

        // Рёбра
        for &(a, b) in &self.edges {
            let (pa, pb) = (positions[a], positions[b]);
            if !rect.intersects(Rect::from_two_pos(pa, pb).expand(2.0)) {
                continue;
            }
            let lit = focus.is_some_and(|f| f == a || f == b);
            let color = if lit {
                pal.border.lerp_to_gamma(pal.accent.gamma_multiply(0.85), focus_t)
            } else {
                pal.border.lerp_to_gamma(pal.border.gamma_multiply(0.35), focus_t)
            };
            let width = if lit { 1.0 + 0.6 * focus_t } else { 1.0 };
            painter.line_segment([pa, pb], Stroke::new(width, color));
        }

        // Узлы и подписи
        let label_size = 12.0 * self.zoom.clamp(0.8, 1.4);
        for (i, n) in self.nodes.iter().enumerate() {
            let sp = positions[i];
            if !rect.expand(40.0).contains(sp) {
                continue;
            }
            let node_id = egui::Id::new(("graph-node", i));
            let grow = ctx.animate_bool_with_time(node_id, hovered == Some(i), 0.16);
            let r = radius(n.degree, self.zoom) * (1.0 + 0.4 * grow);
            let is_current = current.is_some() && n.rel.as_deref() == current;
            let lit = is_lit(i);
            let base = if n.rel.is_none() {
                pal.text_faint
            } else if is_current {
                pal.accent
            } else {
                pal.text_muted
            };
            let dim = if lit { 0.0 } else { 0.7 * focus_t };
            let color = base.lerp_to_gamma(pal.accent, grow).gamma_multiply(1.0 - dim);
            if grow > 0.0 {
                // Мягкий ореол вокруг узла под курсором.
                painter.circle_filled(sp, r + 6.0 * grow, pal.accent.gamma_multiply(0.18 * grow));
            }
            if n.rel.is_none() {
                painter.circle_stroke(sp, r, Stroke::new(1.3, color));
            } else {
                painter.circle_filled(sp, r, color);
            }
            if is_current {
                painter.circle_stroke(sp, r + 3.0, Stroke::new(1.5, pal.accent.gamma_multiply(0.6)));
            }
            if n.pinned {
                // Закреплённый узел отмечен маленькой «булавкой» справа сверху.
                let pin = sp + vec2(1.0, -1.0) * (r * 0.72 + 2.5);
                painter.circle_filled(pin, 3.0, pal.accent.gamma_multiply(1.0 - dim));
                painter.circle_stroke(pin, 3.0, Stroke::new(1.2, pal.bg));
            }
            let near = focus.is_some() && lit;
            let label_alpha = if self.zoom > 0.7 || is_current { 1.0 } else if near { focus_t } else { grow };
            if self.show_labels && label_alpha > 0.01 {
                let c = if near || grow > 0.0 {
                    pal.text_muted.lerp_to_gamma(pal.heading, focus_t.max(grow))
                } else {
                    pal.text_muted.gamma_multiply(1.0 - dim)
                };
                smooth_label(&painter, sp + vec2(0.0, r + 3.0), &n.label, label_size * (1.0 + 0.12 * grow), c.gamma_multiply(label_alpha));
            }
        }

        // Панель настроек поверх графа
        let panel_rect = Rect::from_min_size(rect.left_top() + vec2(12.0, 12.0), vec2(236.0, 200.0));
        let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(panel_rect));
        Frame::new()
            .fill(pal.bg_elevated.gamma_multiply(0.95))
            .stroke(Stroke::new(1.0, pal.border))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(10))
            .show(&mut panel, |ui| {
                ui.label(egui::RichText::new(t!("graph-title")).strong().color(pal.heading));
                let mut changed = false;
                changed |= theme::checkbox(ui, pal, &mut self.show_orphans, &t!("graph-orphans")).changed();
                changed |= theme::checkbox(ui, pal, &mut self.show_ghosts, &t!("graph-ghosts")).changed();
                theme::checkbox(ui, pal, &mut self.show_labels, &t!("graph-labels"));
                if changed {
                    self.built = None;
                }
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if ui.button(format!("⌖ {}", t!("graph-fit"))).clicked() {
                        self.auto_fit = true;
                    }
                    let reset = ui.button(format!("↺ {}", t!("graph-reset"))).on_hover_text(t!("graph-reset-hint"));
                    if reset.clicked() {
                        self.reset_layout();
                    }
                });
                let hint = match self.pins.len() {
                    0 => t!("graph-pin-hint"),
                    n => t!("graph-pinned", count = n),
                };
                ui.add(egui::Label::new(egui::RichText::new(hint).size(11.5).color(pal.text_faint)).wrap());
            });

        let stats = t!("graph-stats", notes = self.nodes.len(), links = self.edges.len());
        painter.text(rect.right_bottom() - vec2(14.0, 12.0), Align2::RIGHT_BOTTOM, stats, FontId::proportional(12.0), pal.text_faint);

        if self.nodes.is_empty() {
            painter.text(rect.center(), Align2::CENTER_CENTER, t!("graph-empty"), FontId::proportional(15.0), pal.text_faint);
        }
        action
    }

    /// Встряхнуть раскладку (например, при открытии графа).
    pub fn reheat(&mut self) {
        self.alpha = self.alpha.max(0.5);
        self.auto_fit = true;
    }

    fn open_action(&self, i: usize) -> GraphAction {
        let n = &self.nodes[i];
        match &n.rel {
            Some(rel) => GraphAction::Open(rel.clone()),
            None => GraphAction::Create(n.label.clone()),
        }
    }

    /// Закрепляет узел на текущем месте и сохраняет расположение.
    fn pin(&mut self, i: usize) {
        let Some(n) = self.nodes.get_mut(i) else { return };
        n.pinned = true;
        n.vel = Vec2::ZERO;
        let (key, pos) = (node_key(n), n.pos);
        self.pins.insert(key, pos);
        self.save_layout();
    }

    /// Отпускает узел обратно в «свободное плавание».
    fn unpin(&mut self, i: usize) {
        let Some(n) = self.nodes.get_mut(i) else { return };
        n.pinned = false;
        let key = node_key(n);
        self.pins.remove(&key);
        self.alpha = self.alpha.max(0.5);
        self.save_layout();
    }

    /// Возвращает исходное расположение: снимает все закрепления и раскладывает граф заново.
    fn reset_layout(&mut self) {
        self.pins.clear();
        self.save_layout();
        self.dragging = None;
        for (i, n) in self.nodes.iter_mut().enumerate() {
            n.pinned = false;
            n.pos = spiral(i);
            n.vel = Vec2::ZERO;
        }
        self.alpha = 1.0;
        self.auto_fit = true;
    }

    /// Записывает закреплённые узлы в хранилище; без закреплений файл удаляется.
    fn save_layout(&self) {
        let Some(root) = &self.pins_root else { return };
        let path = layout_path(root);
        if self.pins.is_empty() {
            let _ = std::fs::remove_file(&path);
            if let Some(dir) = path.parent() {
                // Папка удалится, только если в ней больше ничего нет.
                let _ = std::fs::remove_dir(dir);
            }
            return;
        }
        let file = LayoutFile { version: 1, pinned: self.pins.iter().map(|(k, p)| (k.clone(), [p.x, p.y])).collect() };
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| serde_json::to_string_pretty(&file).map_err(std::io::Error::other))
            .and_then(|json| std::fs::write(&path, json));
        if let Err(err) = result {
            log::warn!("Failed to save graph layout to {}: {err}", path.display());
        }
    }
}

/// Где в хранилище лежит расположение графа (скрытая папка — заметками не считается).
fn layout_path(root: &Path) -> PathBuf {
    root.join(".holotype").join("graph.json")
}

fn load_layout(root: &Path) -> BTreeMap<String, Vec2> {
    std::fs::read_to_string(layout_path(root))
        .ok()
        .and_then(|s| serde_json::from_str::<LayoutFile>(&s).ok())
        .map(|f| {
            f.pinned
                .into_iter()
                .filter(|(_, p)| p.iter().all(|v| v.is_finite()))
                .map(|(k, p)| (k, vec2(p[0], p[1])))
                .collect()
        })
        .unwrap_or_default()
}

/// Исходное положение узла: точки золотой спирали вокруг центра.
fn spiral(i: usize) -> Vec2 {
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    let r = 28.0 * (i as f32 + 1.0).sqrt();
    let a = i as f32 * golden;
    vec2(r * a.cos(), r * a.sin())
}

/// Подпись, которая двигается плавно, без «ступенек».
///
/// egui округляет положение текста до целого пикселя, поэтому медленно плывущие подписи
/// дёргаются рядом с плавными узлами. Здесь глифы собираются в меш вручную с дробным
/// положением. Шрифт растеризуется в ближайшем большем целом кегле и чуть уменьшается
/// до нужного, поэтому плавными получаются и масштаб, и увеличение при наведении.
fn smooth_label(painter: &egui::Painter, top_center: egui::Pos2, text: &str, size: f32, color: Color32) {
    use egui::epaint::{Mesh, Vertex};
    let raster = size.ceil().max(6.0);
    let k = size / raster;
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(raster), color);
    let tex = painter.ctx().fonts(|f| f.font_image_size());
    let inv = vec2(1.0 / tex[0] as f32, 1.0 / tex[1] as f32);
    let origin = top_center - vec2(galley.size().x * k / 2.0, 0.0);
    let mut mesh = Mesh::with_texture(egui::TextureId::default());
    for row in &galley.rows {
        let visuals = &row.visuals;
        let base = mesh.vertices.len() as u32;
        for v in &visuals.mesh.vertices {
            mesh.vertices.push(Vertex {
                pos: origin + (row.pos.to_vec2() + v.pos.to_vec2()) * k,
                uv: (v.uv.to_vec2() * inv).to_pos2(),
                color,
            });
        }
        mesh.indices.extend(visuals.mesh.indices.iter().map(|i| i + base));
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Ключ узла: путь заметки с прямыми слэшами (одинаково на всех системах) или имя несозданной.
fn node_key(n: &GNode) -> String {
    match &n.rel {
        Some(r) => r.to_string_lossy().replace('\\', "/"),
        None => format!("ghost:{}", n.label.to_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str) -> GNode {
        GNode { label: name.into(), rel: Some(PathBuf::from(format!("Папка\\{name}.md"))), pos: Vec2::ZERO, vel: Vec2::ZERO, degree: 1, pinned: false }
    }

    #[test]
    fn pinned_nodes_stay_and_layout_roundtrips() {
        let dir = std::env::temp_dir().join(format!("holotype-graph-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut g = Graph { pins_root: Some(dir.clone()), ..Default::default() };
        g.nodes = vec![node("a"), node("b")];
        g.nodes[0].pos = vec2(300.0, -40.0);
        g.nodes[1].pos = vec2(10.0, 0.0);
        g.edges = vec![(0, 1)];
        g.neighbors = vec![vec![1], vec![0]];
        g.pin(0);
        for _ in 0..50 {
            g.step();
        }
        assert_eq!(g.nodes[0].pos, vec2(300.0, -40.0), "закреплённый узел не двигается");
        assert_ne!(g.nodes[1].pos, vec2(10.0, 0.0), "свободный узел подстраивается");
        let loaded = load_layout(&dir);
        assert_eq!(loaded.get("Папка/a.md"), Some(&vec2(300.0, -40.0)));
        g.reset_layout();
        assert!(!g.nodes[0].pinned && g.pins.is_empty());
        assert!(!layout_path(&dir).exists(), "после сброса файл расположения удаляется");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
