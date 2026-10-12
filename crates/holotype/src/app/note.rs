//! Вкладка заметки: шапка (история, название, вид), редактор Markdown, красивый просмотр
//! и сплит. Редактор подсвечивает разметку, продолжает списки, разворачивает сокращения и
//! подсказывает ссылки (см. `assist`).

use std::path::Path;

use eframe::egui::{
    self, Align, FontFamily, FontId, Frame, Id, Key, Layout, Margin, Modifiers, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, vec2,
    text::{CCursor, CCursorRange},
    widgets::text_edit::TextEditState,
};
use forma_actions::{Keymap, Registry};
use forma_i18n::t;

use super::docs::{NoteDoc, NoteView};
use super::{Cmd, byte_to_char, char_to_byte};
use crate::actions;
use crate::autoscroll;
use crate::editor::{self, EditorStyle};
use crate::markdown;
use crate::render::{Action, Renderer};
use crate::tabs::{Tab, ViewMode};
use crate::theme::{self, Palette};
use crate::vault::Vault;

/// Раскладка клавиш для редактора: он сам ловит свои сочетания (жирный, ссылка…).
#[derive(Clone, Copy)]
pub struct Keys<'a> {
    pub actions: &'a Registry,
    pub keymap: &'a Keymap,
}

/// Всё, что нужно вкладке заметки.
pub struct NoteCx<'a> {
    pub pal: &'a Palette,
    pub fpal: &'a forma_ui::theme::Palette,
    pub vault: &'a Vault,
    pub size: f32,
    pub mono: bool,
    pub readable: bool,
    pub default_mode: ViewMode,
    pub keys: Keys<'a>,
    pub now: f64,
}

/// Вкладка заметки `tab`: заметка `doc` и состояние вкладки `view`.
pub fn note_ui(ui: &mut Ui, cx: &NoteCx, tab: &mut Tab, rel: &Path, doc: &mut NoteDoc, view: &mut NoteView, cmds: &mut Vec<Cmd>) {
    let pal = cx.pal;
    let mode = tab.mode.unwrap_or(cx.default_mode);
    header(ui, cx, tab, rel, doc.dirty, view, mode, cmds);
    // Смена заметки или вида — содержимое мягко проявляется.
    let key_id = Id::new(("note-fade-key", tab.id));
    let key = (rel.to_path_buf(), mode);
    if ui.data(|d| d.get_temp::<(std::path::PathBuf, ViewMode)>(key_id)).as_ref() != Some(&key) {
        theme::restart_fade(ui.ctx(), key_id.with("fade"));
        ui.data_mut(|d| d.insert_temp(key_id, key));
    }
    let fade = theme::fade(ui.ctx(), key_id.with("fade"), 0.22);
    ui.multiply_opacity(fade);

    let size = cx.size;
    let max_w = if cx.readable { size * 50.0 } else { f32::INFINITY };
    let mut actions = Vec::new();
    let editor_id = Id::new(("editor", tab.id));
    match mode {
        ViewMode::Edit => {
            let view_h = ui.available_height();
            ScrollArea::vertical().id_salt(("edit", tab.id)).auto_shrink([false, false]).show(ui, |ui| {
                centered(ui, max_w, 32.0, |ui| {
                    ui.add_space(24.0);
                    inline_title(ui, rel, doc, pal, size, cmds);
                    editor_ui(ui, editor_id, cx, rel, doc, view, view_h - 120.0);
                    ui.add_space(view_h * 0.4);
                });
                autoscroll::area(ui, Id::new(("autoscroll-edit", tab.id)), pal);
            });
        }
        ViewMode::Preview => {
            ScrollArea::vertical().id_salt(("preview", tab.id)).auto_shrink([false, false]).show(ui, |ui| {
                centered(ui, max_w, 32.0, |ui| {
                    ui.add_space(24.0);
                    inline_title(ui, rel, doc, pal, size, cmds);
                    preview(ui, cx, rel, doc, view, &mut actions);
                    ui.add_space(160.0);
                });
                autoscroll::area(ui, Id::new(("autoscroll-preview", tab.id)), pal);
            });
        }
        ViewMode::Split => {
            let view_h = ui.available_height();
            let full = ui.available_rect_before_wrap();
            ui.columns(2, |cols| {
                ScrollArea::vertical().id_salt(("split-edit", tab.id)).auto_shrink([false, false]).show(&mut cols[0], |ui| {
                    centered(ui, max_w, 22.0, |ui| {
                        ui.add_space(20.0);
                        editor_ui(ui, editor_id, cx, rel, doc, view, view_h - 80.0);
                        ui.add_space(view_h * 0.4);
                    });
                    autoscroll::area(ui, Id::new(("autoscroll-split-edit", tab.id)), pal);
                });
                ScrollArea::vertical().id_salt(("split-preview", tab.id)).auto_shrink([false, false]).show(&mut cols[1], |ui| {
                    centered(ui, max_w, 22.0, |ui| {
                        ui.add_space(20.0);
                        preview(ui, cx, rel, doc, view, &mut actions);
                        ui.add_space(160.0);
                    });
                    autoscroll::area(ui, Id::new(("autoscroll-split-preview", tab.id)), pal);
                });
            });
            ui.painter().vline(full.center().x, full.y_range(), Stroke::new(1.0, pal.border));
        }
    }
    for a in actions {
        cmds.push(match a {
            Action::OpenNote { target, new_tab } => Cmd::OpenLink { target, new_tab, from: Some(rel.to_path_buf()), tab: Some(tab.id) },
            Action::OpenUrl(u) => Cmd::OpenUrl(u),
            Action::ToggleTask(r) => Cmd::ToggleTask { rel: rel.to_path_buf(), range: r },
            Action::SearchTag(t) => Cmd::SearchTag(t),
        });
    }
}

/// Красивый просмотр заметки.
fn preview(ui: &mut Ui, cx: &NoteCx, rel: &Path, doc: &mut NoteDoc, view: &mut NoteView, actions: &mut Vec<Action>) {
    let parsed = doc.parsed();
    let mut r = Renderer::new(cx.pal, cx.vault, rel, parsed, cx.size);
    r.scroll_to = view.preview_scroll.take();
    r.show(ui);
    actions.append(&mut r.actions);
    if r.scroll_to.is_some() {
        view.preview_scroll = r.scroll_to.take();
    }
}

/// Шапка вкладки: назад и вперёд, название с папкой, переключатель вида.
#[allow(clippy::too_many_arguments)]
fn header(ui: &mut Ui, cx: &NoteCx, tab: &mut Tab, rel: &Path, dirty: bool, view: &NoteView, mode: ViewMode, cmds: &mut Vec<Cmd>) {
    let pal = cx.pal;
    let fpal = cx.fpal;
    let mut left = Vec::new();
    let mut right = Vec::new();
    Frame::new().fill(pal.bg).inner_margin(Margin { left: 10, right: 10, top: 6, bottom: 4 }).show(ui, |ui| {
        forma_ui::widgets::bar(
            ui,
            Id::new(("note-bar", tab.id)),
            8.0,
            |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let back = ui.add_enabled_ui(!view.back.is_empty(), |ui| forma_ui::icons::flat_button(ui, "back", pal.text_muted)).inner;
                if back.on_hover_text(t!("action-back")).clicked() {
                    left.push(Cmd::Back(Some(tab.id)));
                }
                let fwd = ui.add_enabled_ui(!view.forward.is_empty(), |ui| forma_ui::icons::flat_button(ui, "forward", pal.text_muted)).inner;
                if fwd.on_hover_text(t!("action-forward")).clicked() {
                    left.push(Cmd::Forward(Some(tab.id)));
                }
                ui.add_space(6.0);
                let folder = rel.parent().map(|p| p.to_string_lossy().replace('\\', " › ")).filter(|s| !s.is_empty());
                if let Some(folder) = folder {
                    ui.label(RichText::new(format!("{folder} ›")).size(12.5).color(pal.text_faint));
                }
                let name = rel.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let r = ui.add(egui::Label::new(RichText::new(name).size(12.5).color(pal.text_muted)).sense(Sense::click()).truncate());
                if r.on_hover_text(t!("note-rename-hint")).double_clicked() {
                    left.push(Cmd::RenameDialog(rel.to_path_buf()));
                }
                if dirty {
                    ui.label(RichText::new("●").size(10.0).color(pal.accent)).on_hover_text(t!("status-unsaved"));
                }
            },
            |ui| {
                let modes = [ViewMode::Preview, ViewMode::Split, ViewMode::Edit];
                let labels: Vec<String> = modes.iter().map(|m| m.title()).collect();
                let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
                let current = modes.iter().position(|m| *m == mode).unwrap_or(0);
                if let Some(i) = forma_ui::widgets::segmented(ui, fpal, Id::new(("note-mode", tab.id)), &labels, current, false) {
                    right.push(Cmd::SetMode { tab: Some(tab.id), mode: modes[i] });
                }
            },
        );
    });
    cmds.extend(left);
    cmds.extend(right);
}

/// Колонка по центру с ограниченной шириной и боковыми отступами.
fn centered<R>(ui: &mut Ui, max_w: f32, side: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let avail = ui.available_width();
    let w = (avail - side * 2.0).min(max_w).max(100.0);
    let pad = ((avail - w) / 2.0).max(0.0);
    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.set_width(w);
            add(ui)
        })
        .inner
    })
    .inner
}

/// Имя заметки крупным заголовком над текстом (если сама заметка не начинается с `# `).
fn inline_title(ui: &mut Ui, rel: &Path, doc: &mut NoteDoc, pal: &Palette, size: f32, cmds: &mut Vec<Cmd>) {
    let starts_with_h1 = matches!(doc.parsed().blocks.first(), Some(markdown::Block::Heading { level: 1, .. }));
    if starts_with_h1 {
        return;
    }
    let title = rel.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let r = ui.add(egui::Label::new(RichText::new(title).size(size * 2.0).family(crate::fonts::bold()).color(pal.heading)).sense(Sense::click()));
    if r.on_hover_text(t!("note-rename-hint")).double_clicked() {
        cmds.push(Cmd::RenameDialog(rel.to_path_buf()));
    }
    ui.add_space(size * 0.8);
}

/// Редактор Markdown с подсветкой, сочетаниями форматирования, продолжением списков,
/// подсказками и сокращениями.
fn editor_ui(ui: &mut Ui, id: Id, cx: &NoteCx, rel: &Path, doc: &mut NoteDoc, view: &mut NoteView, min_h: f32) {
    let ctx = ui.ctx().clone();
    let has_focus = ctx.memory(|m| m.has_focus(id));
    let now = cx.now;

    // Форматирование сочетаниями клавиш и продолжение списков.
    if has_focus {
        let keys = cx.keys;
        let fired = |id: &str| ctx.input_mut(|i| forma_actions::consume(i, keys.actions, keys.keymap, id));
        if fired(actions::COMPLETION) {
            view.assist.force_completion();
        }
        // Подсказки, сокращения и места для ввода забирают свои клавиши первыми.
        if let Some(mut state) = TextEditState::load(&ctx, id) {
            if view.assist.keys(&ctx, &mut doc.text, &mut state) {
                doc.mark_edited(now);
            }
            state.store(&ctx, id);
        }
        let wrap = if fired(actions::STRIKE) {
            Some(("~~", "~~"))
        } else if fired(actions::HIGHLIGHT) {
            Some(("==", "=="))
        } else if fired(actions::BOLD) {
            Some(("**", "**"))
        } else if fired(actions::ITALIC) {
            Some(("*", "*"))
        } else if fired(actions::LINK) {
            Some(("[[", "]]"))
        } else if fired(actions::CODE) {
            Some(("`", "`"))
        } else {
            None
        };
        if let Some((pre, post)) = wrap
            && let Some(mut state) = TextEditState::load(&ctx, id)
        {
            wrap_selection(&mut doc.text, &mut state, pre, post);
            state.store(&ctx, id);
            doc.mark_edited(now);
        }
        let enter = ctx.input(|i| i.key_pressed(Key::Enter) && i.modifiers.is_none());
        if enter && let Some(mut state) = TextEditState::load(&ctx, id) {
            let mut text = doc.text.clone();
            if continue_list(&mut text, &mut state) {
                ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                doc.text = text;
                state.store(&ctx, id);
                doc.mark_edited(now);
            }
        }
    }

    let size = cx.size;
    let body_font = FontId::new(size, FontFamily::Proportional);
    let code_font = FontId::new(size * 0.92, FontFamily::Monospace);
    let code_delta = crate::render::font_ascent(ui, &body_font) - crate::render::font_ascent(ui, &code_font);
    let style = EditorStyle::new(cx.pal, size, cx.mono, code_delta);
    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
        let mut job = editor::highlight(ui.ctx(), buf.as_str(), &style);
        job.wrap.max_width = wrap_width;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let output = TextEdit::multiline(&mut doc.text)
        .id(id)
        .frame(Frame::NONE)
        .margin(Margin::ZERO)
        .desired_width(f32::INFINITY)
        .min_size(vec2(0.0, min_h.max(200.0)))
        .event_filter(egui::EventFilter {
            tab: true,
            horizontal_arrows: true,
            vertical_arrows: true,
            // Пока открыты подсказки, Esc закрывает их, а не снимает фокус с редактора.
            escape: view.assist.wants_escape(),
        })
        .hint_text(t!("editor-hint"))
        .layouter(&mut layouter)
        .show(ui);
    let resp = &output.response.response;
    if resp.changed() {
        doc.mark_edited(now);
    }
    // Подсказки, превью сокращений и рамки мест для ввода — поверх редактора.
    if view.assist.after(ui, &output, id, &mut doc.text, doc.rev, has_focus, cx.vault, rel, cx.pal) {
        doc.mark_edited(now);
        ctx.memory_mut(|m| m.request_focus(id));
    }
    if let Some(r) = output.cursor_range {
        view.cursor = Some(r.primary.index.0);
    }
    if view.focus_editor {
        resp.request_focus();
        view.focus_editor = false;
    }
    if let Some(offset) = view.editor_jump.take() {
        let ci = byte_to_char(&doc.text, offset);
        let mut state = output.state.clone();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(ci))));
        state.store(&ctx, id);
        resp.request_focus();
        let rect = output.galley.pos_from_cursor(CCursor::new(ci)).translate(output.galley_pos.to_vec2());
        ui.scroll_to_rect(rect.expand2(vec2(0.0, 60.0)), Some(Align::TOP));
    }
}

/// Обёртка выделения в разметку (или снятие, если уже обёрнуто).
fn wrap_selection(text: &mut String, state: &mut TextEditState, pre: &str, post: &str) {
    let (a, b) = state
        .cursor
        .char_range()
        .map(|r| {
            let [s, e] = r.sorted_cursors();
            (s.index.0, e.index.0)
        })
        .unwrap_or((0, 0));
    let ba = char_to_byte(text, a);
    let bb = char_to_byte(text, b);
    let pre_n = pre.chars().count();
    if text[..ba].ends_with(pre) && text[bb..].starts_with(post) {
        text.replace_range(bb..bb + post.len(), "");
        text.replace_range(ba - pre.len()..ba, "");
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a - pre_n), CCursor::new(b - pre_n))));
    } else {
        text.insert_str(bb, post);
        text.insert_str(ba, pre);
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a + pre_n), CCursor::new(b + pre_n))));
    }
}

/// Продолжение списка по Enter. Возвращает true, если Enter обработан.
fn continue_list(text: &mut String, state: &mut TextEditState) -> bool {
    let Some(range) = state.cursor.char_range() else { return false };
    if !range.is_empty() {
        return false;
    }
    let c = range.primary.index.0;
    let bp = char_to_byte(text, c);
    let ls = text[..bp].rfind('\n').map_or(0, |i| i + 1);
    let line = text[ls..bp].to_owned();
    let indent_len = line.len() - line.trim_start().len();
    let indent = &line[..indent_len];
    let rest = &line[indent_len..];

    let mut quote = String::new();
    let mut r = rest;
    while let Some(x) = r.strip_prefix("> ") {
        quote.push_str("> ");
        r = x;
    }
    let (marker, after) = if let Some(x) = r.strip_prefix("- ") {
        ("- ".to_owned(), x)
    } else if let Some(x) = r.strip_prefix("* ") {
        ("* ".to_owned(), x)
    } else if let Some(x) = r.strip_prefix("+ ") {
        ("+ ".to_owned(), x)
    } else {
        let digits: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && (r[digits.len()..].starts_with(". ") || r[digits.len()..].starts_with(") ")) {
            let n: u64 = digits.parse().unwrap_or(1);
            let delim = &r[digits.len()..digits.len() + 1];
            (format!("{}{delim} ", n + 1), &r[digits.len() + 2..])
        } else if !quote.is_empty() {
            (String::new(), r)
        } else {
            return false;
        }
    };
    let (task, content) = if let Some(x) = after.strip_prefix("[ ] ").or_else(|| after.strip_prefix("[x] ")).or_else(|| after.strip_prefix("[X] ")) {
        (true, x)
    } else {
        (false, after)
    };
    if content.trim().is_empty() {
        // Пустой пункт: завершаем список и оставляем пустую строку, чтобы следующий текст
        // стал новым абзацем, а не продолжением последнего пункта.
        text.replace_range(ls..bp, "\n");
        let new_c = byte_to_char(text, ls + 1);
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(new_c))));
        return true;
    }
    let insert = format!("\n{indent}{quote}{marker}{}", if task { "[ ] " } else { "" });
    text.insert_str(bp, &insert);
    let new_c = c + insert.chars().count();
    state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(new_c))));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_continuation() {
        let mut text = "- первый".to_owned();
        let mut state = TextEditState::default();
        let n = text.chars().count();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(n))));
        assert!(continue_list(&mut text, &mut state));
        assert_eq!(text, "- первый\n- ");
        // Пустой пункт завершает список и оставляет пустую строку.
        assert!(continue_list(&mut text, &mut state));
        assert_eq!(text, "- первый\n\n");

        let mut text = "1. раз".to_owned();
        let n = text.chars().count();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(n))));
        assert!(continue_list(&mut text, &mut state));
        assert_eq!(text, "1. раз\n2. ");
    }

    #[test]
    fn wrap_toggles() {
        let mut text = "слово".to_owned();
        let mut state = TextEditState::default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(5))));
        wrap_selection(&mut text, &mut state, "**", "**");
        assert_eq!(text, "**слово**");
        wrap_selection(&mut text, &mut state, "**", "**");
        assert_eq!(text, "слово");
    }
}
