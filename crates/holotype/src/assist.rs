//! Помощник редактора: всплывающие подсказки (заметки, картинки, теги…), разворачивание
//! сокращений по Tab, прыжки по местам для ввода и переходы по ячейкам таблиц.

use std::ops::Range;
use std::path::Path;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, Frame, Id, Key, Margin, Modifiers, Rect, RichText,
    ScrollArea, Sense, Stroke, Ui, pos2, vec2,
    text::{CCursor, CCursorRange},
    widgets::text_edit::{TextEditOutput, TextEditState},
};

use crate::app::{byte_to_char, char_to_byte, file_url};
use crate::complete::{self, Block, Context, Icon, Item};
use crate::emmet;
use crate::fonts;
use crate::theme::Palette;
use crate::vault::Vault;
use forma_i18n::t;

const ROW: f32 = 30.0;
const VISIBLE_ROWS: usize = 8;
const LIST_W: f32 = 340.0;
const PREVIEW_W: f32 = 190.0;

#[derive(Default)]
pub struct Assist {
    popup: Option<Popup>,
    /// Начало контекста, подсказки для которого закрыли по Esc.
    dismissed: Option<usize>,
    hint: Option<Hint>,
    snippet: Option<Snippet>,
    /// Ctrl+Space: показать подсказки и без набранного текста.
    force: bool,
    last_rev: u64,
    last_cursor: Option<usize>,
}

struct Popup {
    ctx: Context,
    items: Vec<Item>,
    selected: usize,
    /// Выбор сменился с клавиатуры — прокрутить список к нему.
    scroll: bool,
}

/// Подсказка о сокращении, которое развернётся по Tab.
struct Hint {
    title: String,
    preview: String,
}

/// Места для ввода после разворачивания (в символах).
struct Snippet {
    stops: Vec<Range<usize>>,
    current: usize,
    /// Последнее место — просто финальная позиция курсора, рамку для него не рисуем.
    final_stop: bool,
    /// Длина текста в символах: по ней видно правку и сдвиг остальных мест.
    chars: usize,
}

impl Assist {
    /// Показать подсказки и без набранного текста (действие «Автодополнение», `Ctrl+Space`).
    pub fn force_completion(&mut self) {
        self.force = true;
        self.dismissed = None;
    }

    /// Нужен ли редактору Esc (иначе egui по Esc снимает фокус).
    pub fn wants_escape(&self) -> bool {
        self.popup.is_some() || self.hint.is_some() || self.snippet.is_some()
    }

    /// Клавиши до отрисовки поля ввода. Возвращает true, если текст изменился.
    pub fn keys(&mut self, ctx: &egui::Context, text: &mut String, state: &mut TextEditState) -> bool {
        let Some(range) = state.cursor.char_range() else { return false };
        let cursor = char_to_byte(text, range.primary.index.0);
        let press = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));

        // Открытый список забирает стрелки, Enter, Tab и Esc.
        if let Some(popup) = &mut self.popup {
            let n = popup.items.len().max(1);
            if press(Modifiers::NONE, Key::ArrowDown) {
                popup.selected = (popup.selected + 1) % n;
                popup.scroll = true;
            }
            if press(Modifiers::NONE, Key::ArrowUp) {
                popup.selected = (popup.selected + n - 1) % n;
                popup.scroll = true;
            }
            if press(Modifiers::NONE, Key::Escape) {
                self.dismissed = Some(popup.ctx.start);
                self.popup = None;
                return false;
            }
            if press(Modifiers::NONE, Key::Enter) || press(Modifiers::NONE, Key::Tab) {
                let k = popup.selected;
                return self.accept(text, state, k);
            }
            return false;
        }

        let (tab, shift) = ctx.input(|i| (i.key_pressed(Key::Tab) && !i.modifiers.command && !i.modifiers.alt, i.modifiers.shift));
        if tab {
            let consume = || ctx.input_mut(|i| i.consume_key(if shift { Modifiers::SHIFT } else { Modifiers::NONE }, Key::Tab));
            if self.snippet.is_some() {
                consume();
                self.jump(state, shift);
                return false;
            }
            if !shift {
                if let Some(found) = emmet::at_cursor(text, cursor) {
                    consume();
                    self.expand(text, state, found);
                    return true;
                }
            }
            if let Some((sel, changed)) = table_cell(text, cursor, shift) {
                consume();
                select_bytes(text, state, sel);
                return changed;
            }
        }
        if (self.hint.is_some() || self.snippet.is_some()) && press(Modifiers::NONE, Key::Escape) {
            self.hint = None;
            self.snippet = None;
        }
        false
    }

    /// Вставляет выбранную подсказку. Возвращает true, если текст изменился.
    fn accept(&mut self, text: &mut String, state: &mut TextEditState, k: usize) -> bool {
        let Some(popup) = self.popup.take() else { return false };
        let Some(item) = popup.items.get(k) else { return false };
        let Some(range) = state.cursor.char_range() else { return false };
        let cursor = char_to_byte(text, range.primary.index.0);
        // Текст мог измениться — убеждаемся, что контекст тот же.
        let Some(c) = complete::detect(text, cursor, true) else { return false };
        if c.start != popup.ctx.start || c.kind != popup.ctx.kind {
            return false;
        }
        let mut insert = item.insert.clone();
        let caret_after = if c.needs_close {
            insert.push_str(&item.tail);
            item.tail_caret
        } else {
            c.close_len
        };
        text.replace_range(c.start..c.end, &insert);
        let caret = byte_to_char(text, c.start) + item.insert.chars().count() + caret_after;
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(caret))));
        self.dismissed = None;
        true
    }

    fn expand(&mut self, text: &mut String, state: &mut TextEditState, found: emmet::Found) {
        let exp = found.expansion;
        text.replace_range(found.range.clone(), &exp.text);
        let base = byte_to_char(text, found.range.start);
        let end = base + exp.text.chars().count();
        let mut stops: Vec<Range<usize>> = exp.stops.iter().map(|r| base + r.start..base + r.end).collect();
        let first = stops.first().cloned().unwrap_or(end..end);
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(first.start), CCursor::new(first.end))));
        self.hint = None;
        self.snippet = None;
        // Одно место для ввода — просто ставим туда курсор, иначе Tab начинает ходить по местам.
        if stops.len() >= 2 {
            let final_stop = stops.last().is_some_and(|s| s.end != end);
            if final_stop {
                stops.push(end..end);
            }
            self.snippet = Some(Snippet { stops, current: 0, final_stop, chars: text.chars().count() });
        }
    }

    fn jump(&mut self, state: &mut TextEditState, back: bool) {
        let Some(sn) = &mut self.snippet else { return };
        if back {
            sn.current = sn.current.saturating_sub(1);
        } else {
            sn.current = (sn.current + 1).min(sn.stops.len() - 1);
        }
        let r = sn.stops[sn.current].clone();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(r.start), CCursor::new(r.end))));
        if sn.current + 1 >= sn.stops.len() {
            self.snippet = None;
        }
    }

    /// После поля ввода: следит за местами для ввода, обновляет подсказки и рисует их.
    /// Возвращает true, если текст изменился (подсказку выбрали мышью).
    #[allow(clippy::too_many_arguments)]
    pub fn after(
        &mut self,
        ui: &Ui,
        out: &TextEditOutput,
        id: Id,
        text: &mut String,
        rev: u64,
        focused: bool,
        vault: &Vault,
        rel: &Path,
        pal: &Palette,
    ) -> bool {
        let cursor_c = out.cursor_range.map(|r| r.primary.index.0);
        let changed = rev != self.last_rev;
        let moved = cursor_c != self.last_cursor;
        self.last_rev = rev;
        self.last_cursor = cursor_c;
        let (true, Some(cc)) = (focused, cursor_c) else {
            self.popup = None;
            self.hint = None;
            self.snippet = None;
            self.force = false;
            return false;
        };

        self.track_snippet(text, cc, changed);

        let cursor = char_to_byte(text, cc);
        let force = std::mem::take(&mut self.force);
        if changed || force || (moved && self.popup.is_some()) {
            match complete::detect(text, cursor, force) {
                Some(c) if force || self.dismissed != Some(c.start) => {
                    let same = self.popup.as_ref().is_some_and(|p| p.ctx.start == c.start && p.ctx.kind == c.kind);
                    if !changed && !force && !same {
                        // Курсор ушёл в другую ссылку без набора текста — не навязываемся.
                        self.popup = None;
                    } else {
                        let items = complete::items(&c, text, vault, rel);
                        self.popup = if items.is_empty() {
                            None
                        } else {
                            let keep = self.popup.as_ref().filter(|p| same && p.ctx.query == c.query).map(|p| p.selected);
                            let selected = keep.unwrap_or(0).min(items.len() - 1);
                            Some(Popup { ctx: c, items, selected, scroll: true })
                        };
                    }
                }
                Some(_) => self.popup = None,
                None => {
                    self.popup = None;
                    self.dismissed = None;
                }
            }
        }

        if changed {
            self.hint = if self.popup.is_none() {
                emmet::at_cursor(text, cursor)
                    .filter(|f| f.whole_line && !f.quiet)
                    .map(|f| Hint { preview: preview(&f.expansion.text), title: f.expansion.title })
            } else {
                None
            };
        } else if moved {
            self.hint = None;
        }

        let galley_pos = out.galley_pos.to_vec2();
        let cursor_rect = |c: usize| out.galley.pos_from_cursor(CCursor::new(c)).translate(galley_pos);
        let clip = ui.clip_rect();

        // Рамки вокруг следующих мест для ввода, как в IDE.
        if let Some(sn) = &self.snippet {
            let painter = ui.painter();
            let last = sn.stops.len() - 1;
            for (k, r) in sn.stops.iter().enumerate() {
                if k == sn.current || (sn.final_stop && k == last) {
                    continue;
                }
                let a = cursor_rect(r.start);
                let b = cursor_rect(r.end);
                if (a.center().y - b.center().y).abs() > 1.0 {
                    continue;
                }
                let rect = Rect::from_min_max(pos2(a.left() - 2.0, a.top() + 1.0), pos2(b.right().max(a.left() + 4.0) + 2.0, a.bottom() - 1.0));
                painter.rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0, pal.accent.gamma_multiply(0.55)), egui::StrokeKind::Inside);
            }
        }

        let mut clicked = None;
        if let Some(popup) = &mut self.popup {
            let anchor = cursor_rect(byte_to_char(text, popup.ctx.start));
            if clip.contains(anchor.center()) {
                clicked = popup_ui(ui.ctx(), popup, anchor, id, pal);
            }
        } else if let Some(hint) = &self.hint {
            let anchor = cursor_rect(cc);
            if clip.contains(anchor.center()) {
                hint_ui(ui.ctx(), hint, anchor, id, pal);
            }
        }
        if let Some(k) = clicked {
            if let Some(mut state) = TextEditState::load(ui.ctx(), id) {
                if self.accept(text, &mut state, k) {
                    state.store(ui.ctx(), id);
                    return true;
                }
            }
        }
        false
    }

    /// Правка внутри текущего места для ввода сдвигает остальные; правка снаружи или уход
    /// курсора за пределы сокращения завершает обход.
    fn track_snippet(&mut self, text: &str, cc: usize, changed: bool) {
        let Some(sn) = &mut self.snippet else { return };
        let n = if changed { text.chars().count() } else { sn.chars };
        let keep = if n != sn.chars {
            let delta = n as isize - sn.chars as isize;
            let r = sn.stops[sn.current].clone();
            let new_end = r.end as isize + delta;
            let inside = new_end >= r.start as isize && cc >= r.start && cc as isize <= new_end;
            if inside {
                sn.stops[sn.current].end = new_end as usize;
                for s in &mut sn.stops[sn.current + 1..] {
                    s.start = (s.start as isize + delta) as usize;
                    s.end = (s.end as isize + delta) as usize;
                }
                sn.chars = n;
            }
            inside
        } else {
            let span = sn.stops[0].start..=sn.stops.last().map_or(0, |s| s.end);
            span.contains(&cc)
        };
        if !keep {
            self.snippet = None;
        }
    }
}

fn popup_frame(pal: &Palette) -> Frame {
    Frame::new()
        .fill(pal.bg_elevated)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(6))
        .shadow(egui::Shadow { offset: [0, 6], blur: 22, spread: 0, color: Color32::from_black_alpha(70) })
}

/// Список подсказок у курсора. Возвращает номер варианта, по которому щёлкнули.
fn popup_ui(ctx: &egui::Context, popup: &mut Popup, anchor: Rect, id: Id, pal: &Palette) -> Option<usize> {
    let visible = popup.items.len().min(VISIBLE_ROWS) as f32;
    let has_preview = popup.items.iter().any(|i| i.image.is_some());
    let est_h = visible * (ROW + 2.0) + 44.0;
    // Если внизу не помещается — открываем список над строкой.
    let below = anchor.bottom() + 6.0 + est_h <= ctx.content_rect().bottom() - 8.0;
    let (pos, pivot) = if below {
        (anchor.left_bottom() + vec2(-10.0, 6.0), Align2::LEFT_TOP)
    } else {
        (anchor.left_top() + vec2(-10.0, -6.0), Align2::LEFT_BOTTOM)
    };
    let mut clicked = None;
    let mut hovered = None;
    egui::Area::new(id.with("assist-popup")).order(egui::Order::Foreground).fixed_pos(pos).pivot(pivot).constrain(true).show(ctx, |ui| {
        popup_frame(pal).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(LIST_W);
                    ScrollArea::vertical().max_height(VISIBLE_ROWS as f32 * (ROW + 2.0)).auto_shrink([false, true]).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for (k, item) in popup.items.iter().enumerate() {
                            let r = item_row(ui, item, k == popup.selected, pal);
                            if k == popup.selected && popup.scroll {
                                r.scroll_to_me(None);
                            }
                            if r.hovered() {
                                hovered = Some(k);
                            }
                            if r.clicked() {
                                clicked = Some(k);
                            }
                        }
                    });
                    ui.add_space(3.0);
                    ui.label(RichText::new(t!("completion-keys")).size(11.0).color(pal.text_faint));
                });
                if has_preview {
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.set_width(PREVIEW_W);
                        let shown = hovered.or(Some(popup.selected)).and_then(|k| popup.items.get(k)).and_then(|i| i.image.as_ref());
                        match shown {
                            Some(path) => {
                                ui.add(
                                    egui::Image::new(file_url(path))
                                        .max_size(vec2(PREVIEW_W, 160.0))
                                        .corner_radius(CornerRadius::same(6))
                                        .show_loading_spinner(true),
                                );
                            }
                            None => {
                                ui.add_space(8.0);
                                ui.label(RichText::new(t!("completion-preview-empty")).size(11.5).color(pal.text_faint));
                            }
                        }
                    });
                }
            });
        });
    });
    popup.scroll = false;
    clicked
}

fn item_row(ui: &mut Ui, item: &Item, selected: bool, pal: &Palette) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(7), pal.accent_soft);
    } else if resp.hovered() {
        painter.rect_filled(rect, CornerRadius::same(7), pal.bg_hover);
    }
    let cy = rect.center().y;
    let ic = pos2(rect.left() + 15.0, cy);
    let center = Align2::CENTER_CENTER;
    match item.icon {
        Icon::Note => {
            painter.text(ic, center, "📄", FontId::proportional(13.0), pal.text_muted);
        }
        Icon::Image => {
            painter.text(ic, center, "🖼", FontId::proportional(13.0), pal.text_muted);
        }
        Icon::File => {
            painter.text(ic, center, "📎", FontId::proportional(13.0), pal.text_muted);
        }
        Icon::Heading(level) => {
            painter.text(ic, center, format!("H{level}"), FontId::new(10.5, fonts::semibold()), pal.text_faint);
        }
        Icon::Tag => {
            painter.text(ic, center, "#", FontId::new(14.0, fonts::semibold()), pal.tag_text);
        }
        Icon::Callout(color, icon) => {
            painter.text(ic, center, icon, FontId::proportional(13.0), color);
        }
        Icon::Lang => {
            painter.text(ic, center, "</>", FontId::monospace(9.5), pal.text_faint);
        }
        Icon::Color(color) => {
            painter.circle_filled(ic, 5.5, color);
        }
        Icon::Badge => {
            painter.rect_filled(Rect::from_center_size(ic, vec2(16.0, 9.0)), CornerRadius::same(4), pal.badge_bg);
        }
    }

    // Подпись справа (папка, уровень, количество) не шире половины строки.
    let detail = painter.layout_no_wrap(item.detail.clone(), FontId::proportional(12.0), pal.text_faint);
    let max_detail = rect.width() * 0.45;
    let detail_w = detail.size().x.min(max_detail);
    let detail_clip = Rect::from_min_max(pos2(rect.right() - 10.0 - detail_w, rect.top()), pos2(rect.right() - 10.0, rect.bottom()));
    let detail_pos = pos2(rect.right() - 10.0 - detail.size().x, cy - detail.size().y / 2.0);
    let label_x = rect.left() + 32.0;
    let label_right = if detail_w > 0.0 { detail_clip.left() - 12.0 } else { rect.right() - 10.0 };
    let label_clip = Rect::from_min_max(pos2(label_x, rect.top()), pos2(label_right.max(label_x + 40.0), rect.bottom()));
    let color = if selected { pal.heading } else { pal.text };
    painter.with_clip_rect(label_clip.intersect(ui.clip_rect())).text(pos2(label_x, cy), Align2::LEFT_CENTER, &item.label, FontId::proportional(14.0), color);
    painter.with_clip_rect(detail_clip.intersect(ui.clip_rect())).galley(detail_pos, detail, pal.text_faint);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp
}

/// Подсказка «Tab — развернуть» с превью результата.
fn hint_ui(ctx: &egui::Context, hint: &Hint, anchor: Rect, id: Id, pal: &Palette) {
    egui::Area::new(id.with("assist-hint"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor.left_bottom() + vec2(-8.0, 6.0))
        .interactable(false)
        .constrain(true)
        .show(ctx, |ui| {
            popup_frame(pal).inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    Frame::new()
                        .fill(pal.kbd_bg)
                        .stroke(Stroke::new(1.0, pal.kbd_border))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(6, 1))
                        .show(ui, |ui| ui.label(RichText::new("Tab").size(11.5).color(pal.text)));
                    ui.label(RichText::new(&hint.title).size(13.0).color(pal.heading));
                });
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new(&hint.preview).family(FontFamily::Monospace).size(11.5).color(pal.text_muted)).extend());
            });
        });
}

fn preview(text: &str) -> String {
    const MAX_LINES: usize = 8;
    const MAX_CHARS: usize = 56;
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = lines
        .iter()
        .take(MAX_LINES)
        .map(|l| if l.chars().count() > MAX_CHARS { format!("{}…", l.chars().take(MAX_CHARS - 1).collect::<String>()) } else { (*l).to_owned() })
        .collect();
    if lines.len() > MAX_LINES {
        let more = lines.len() - MAX_LINES;
        out.push(t!("hint-more-lines", count = more));
    }
    out.join("\n")
}

fn select_bytes(text: &str, state: &mut TextEditState, r: Range<usize>) {
    let a = byte_to_char(text, r.start);
    let b = byte_to_char(text, r.end);
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
}

// ---------------------------------------------------------------------------------------------
// Таблицы
// ---------------------------------------------------------------------------------------------

/// Tab в строке таблицы — к следующей ячейке, Shift+Tab — к предыдущей. За последней ячейкой
/// таблицы добавляется новая строка. Возвращает выделение (в байтах) и признак правки текста.
fn table_cell(text: &mut String, cursor: usize, back: bool) -> Option<(Range<usize>, bool)> {
    let ls = line_start(text, cursor);
    let le = line_end(text, cursor);
    if !is_row(&text[ls..le]) || complete::block_at(text, ls) != Block::Normal {
        return None;
    }
    let row = cells(text, ls, le);
    if back {
        if let Some(i) = row.iter().rposition(|c| c.start <= cursor) {
            if i > 0 {
                return Some((cell_target(text, &row[i - 1]), false));
            }
        }
        let mut s = ls;
        while s > 0 {
            let pe = s - 1;
            let ps = line_start(text, pe);
            let line = &text[ps..pe];
            if !is_row(line) {
                break;
            }
            if !is_separator(line) {
                return cells(text, ps, pe).last().map(|c| (cell_target(text, c), false));
            }
            s = ps;
        }
        // Уже первая ячейка — остаёмся на месте.
        return Some((cursor..cursor, false));
    }
    if let Some(c) = row.iter().find(|c| c.start > cursor) {
        return Some((cell_target(text, c), false));
    }
    let mut e = le;
    while e < text.len() {
        let ns = e + 1;
        let ne = line_end(text, ns);
        let line = &text[ns..ne];
        if !is_row(line) {
            break;
        }
        if !is_separator(line) {
            return cells(text, ns, ne).first().map(|c| (cell_target(text, c), false));
        }
        e = ne;
    }
    // Последняя ячейка таблицы — добавляем пустую строку.
    let n = row.len().max(1);
    text.insert_str(e, &format!("\n|{}", "  |".repeat(n)));
    let caret = e + 3;
    Some((caret..caret, true))
}

/// Байтовые диапазоны ячеек строки — содержимое между вертикальными чертами.
fn cells(text: &str, ls: usize, le: usize) -> Vec<Range<usize>> {
    let b = text[ls..le].as_bytes();
    let pipes: Vec<usize> = (0..b.len()).filter(|&i| b[i] == b'|' && (i == 0 || b[i - 1] != b'\\')).map(|i| ls + i).collect();
    let mut out: Vec<Range<usize>> = pipes.windows(2).map(|w| w[0] + 1..w[1]).collect();
    if let Some(&last) = pipes.last() {
        if !text[last + 1..le].trim().is_empty() {
            out.push(last + 1..le);
        }
    }
    out
}

/// Куда встать в ячейке: выделить её текст, а в пустой — поставить курсор после пробела.
fn cell_target(text: &str, c: &Range<usize>) -> Range<usize> {
    let s = &text[c.clone()];
    let lead = s.len() - s.trim_start().len();
    let trail = s.len() - s.trim_end().len();
    if lead + trail >= s.len() {
        let p = (c.start + 1).min(c.end);
        return p..p;
    }
    c.start + lead..c.end - trail
}

fn is_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

fn is_separator(line: &str) -> bool {
    let t = line.trim();
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn line_start(text: &str, i: usize) -> usize {
    text[..i].rfind('\n').map_or(0, |p| p + 1)
}

fn line_end(text: &str, i: usize) -> usize {
    text[i..].find('\n').map_or(text.len(), |p| i + p)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "| Имя | Возраст |\n| --- | --- |\n| Холо |  |";

    #[test]
    fn table_navigation() {
        let mut t = TABLE.to_owned();
        // Из первой ячейки шапки — во вторую (её текст выделяется).
        let (sel, changed) = table_cell(&mut t, 2, false).unwrap();
        assert!(!changed);
        assert_eq!(&t[sel], "Возраст");
        // Из конца шапки — через разделитель в первую ячейку тела.
        let end_of_head = t.find('\n').unwrap();
        let (sel, _) = table_cell(&mut t, end_of_head, false).unwrap();
        assert_eq!(&t[sel], "Холо");
        // Назад из тела — в последнюю ячейку шапки.
        let body = t.rfind("Холо").unwrap();
        let (sel, _) = table_cell(&mut t, body, true).unwrap();
        assert_eq!(&t[sel], "Возраст");
        // Из последней ячейки таблицы — новая строка.
        let len = t.len();
        let (sel, changed) = table_cell(&mut t, len - 1, false).unwrap();
        assert!(changed);
        assert!(t.ends_with("| Холо |  |\n|  |  |"));
        assert_eq!(sel.start, len + 3);
        // Не таблица — не трогаем.
        let mut plain = "просто текст".to_owned();
        assert!(table_cell(&mut plain, 4, false).is_none());
    }

    #[test]
    fn preview_is_short() {
        crate::i18n::init();
        let long = (0..12).map(|i| format!("строка {i}")).collect::<Vec<_>>().join("\n");
        let p = preview(&long);
        assert_eq!(p.lines().count(), 9);
        assert!(p.ends_with("и ещё 4 строки"));
    }
}
