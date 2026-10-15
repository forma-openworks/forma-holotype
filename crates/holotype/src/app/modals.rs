//! Диалоги Holotype в общем виде диалогов Forma: новая заметка и папка, переименование,
//! удаление и быстрый переход (`Ctrl+O`).

use std::path::PathBuf;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Key, Margin, Modifiers, RichText, Sense, TextEdit, Ui, pos2, vec2,
    text::{CCursor, CCursorRange},
    widgets::text_edit::TextEditState,
};
use forma_i18n::t;
use forma_shell::chrome::{modal, modal_title, modal_width};
use forma_ui::settings_window::primary_button;
use forma_ui::theme::Palette;

use super::{Cmd, HolotypeApp};
use crate::complete::fuzzy_score;
use crate::vault::Vault;

pub enum Modal {
    NewNote { folder: PathBuf, name: String },
    NewFolder { parent: PathBuf, name: String },
    Rename { target: PathBuf, name: String },
    Delete { target: PathBuf },
    QuickOpen { query: String, selected: usize },
}

/// Поле имени: при открытии диалога имя выделено целиком, чтобы его сразу перепечатать.
fn name_field(ui: &mut Ui, name: &mut String, focus: bool) -> bool {
    let r = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY).margin(Margin::symmetric(10, 7)));
    if focus {
        r.request_focus();
        if let Some(mut state) = TextEditState::load(ui.ctx(), r.id) {
            let n = name.chars().count();
            state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(n))));
            state.store(ui.ctx(), r.id);
        }
    }
    r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
}

/// Кнопки диалога: главная (или «опасная») и «Отмена». Возвращает (главная, отмена).
fn buttons(ui: &mut Ui, pal: &Palette, ok: &str, danger: bool) -> (bool, bool) {
    ui.add_space(14.0);
    let mut res = (false, false);
    forma_ui::widgets::button_row(ui, |ui| {
        res.0 = if danger {
            ui.add(egui::Button::new(RichText::new(ok).color(Color32::WHITE)).fill(pal.danger.gamma_multiply(0.85)).min_size(vec2(96.0, 30.0))).clicked()
        } else {
            primary_button(ui, pal, ok).clicked()
        };
        if ui.add(egui::Button::new(t!("action-cancel")).min_size(vec2(84.0, 30.0))).clicked() {
            res.1 = true;
        }
    });
    res
}

/// Строка списка быстрого перехода.
fn switcher_row(ui: &mut Ui, name: &str, folder: &str, selected: bool, pal: &Palette) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    if selected || resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), if selected { pal.list_selected } else { pal.list_hover });
    }
    let painter = ui.painter();
    painter.text(pos2(rect.left() + 12.0, rect.center().y), Align2::LEFT_CENTER, name, FontId::proportional(14.5), pal.heading);
    if !folder.is_empty() {
        painter.text(pos2(rect.right() - 12.0, rect.center().y), Align2::RIGHT_CENTER, folder, FontId::proportional(12.0), pal.text_faint);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Что выбрали в быстром переходе.
enum Pick {
    Note(PathBuf, bool),
    Create(String, bool),
}

/// Быстрый переход: нечёткий поиск по именам и путям заметок. Возвращает выбор.
fn quick_open(ui: &mut Ui, pal: &Palette, vault: &Vault, query: &mut String, selected: &mut usize, focus: bool) -> Option<Pick> {
    ui.set_width(modal_width(ui.ctx(), 560.0));
    let mut results: Vec<(i32, usize)> = vault
        .notes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let path = n.rel.with_extension("").to_string_lossy().replace('\\', "/");
            let s1 = fuzzy_score(query, &n.name);
            let s2 = fuzzy_score(query, &path).map(|s| s - 5);
            s1.max(s2).map(|s| (s, i))
        })
        .collect();
    results.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| vault.notes[a.1].name.cmp(&vault.notes[b.1].name)));
    results.truncate(12);
    // Регистр сравниваем как `Vault::resolve` — с кириллицей.
    let lower = query.trim().to_lowercase();
    let can_create = !lower.is_empty() && !vault.notes.iter().any(|n| n.name.to_lowercase() == lower);
    let total = results.len() + usize::from(can_create);
    let (down, up, enter, ctrl) = ui.input_mut(|i| {
        (i.consume_key(Modifiers::NONE, Key::ArrowDown), i.consume_key(Modifiers::NONE, Key::ArrowUp), i.key_pressed(Key::Enter), i.modifiers.command)
    });
    if total > 0 {
        if down {
            *selected = (*selected + 1) % total;
        }
        if up {
            *selected = (*selected + total - 1) % total;
        }
        *selected = (*selected).min(total - 1);
    }
    let r = ui.add(TextEdit::singleline(query).hint_text(t!("quick-open-hint")).desired_width(f32::INFINITY).font(FontId::proportional(16.5)).margin(Margin::symmetric(10, 8)));
    if focus {
        r.request_focus();
    }
    if r.changed() {
        *selected = 0;
    }
    ui.separator();
    ui.spacing_mut().item_spacing.y = 2.0;
    let mut chosen = None;
    for (k, (_, i)) in results.iter().enumerate() {
        let n = &vault.notes[*i];
        let folder = n.rel.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        if switcher_row(ui, &n.name, &folder, k == *selected, pal).clicked() {
            chosen = Some(k);
        }
    }
    if can_create && switcher_row(ui, &t!("quick-open-create", name = query.trim().to_owned()), "", results.len() == *selected, pal).clicked() {
        chosen = Some(results.len());
    }
    if total == 0 {
        ui.label(RichText::new(t!("quick-open-empty")).color(pal.text_faint));
    }
    ui.add_space(4.0);
    ui.label(RichText::new(t!("quick-open-keys")).size(11.5).color(pal.text_faint));
    if enter && total > 0 {
        chosen = Some(*selected);
    }
    let k = chosen?;
    Some(if k < results.len() { Pick::Note(vault.notes[results[k].1].rel.clone(), ctrl) } else { Pick::Create(query.trim().to_owned(), ctrl) })
}

impl HolotypeApp {
    pub(super) fn app_modals(&mut self, ctx: &egui::Context) {
        // Пока открыт диалог оболочки, свои ждут.
        if self.shell.modal_open() && self.modal.is_none() {
            return;
        }
        let Some(mut m) = self.modal.take() else { return };
        let pal = self.shell.pal.clone();
        let focus = std::mem::take(&mut self.modal_focus);
        let vault = self.vault.as_ref();
        let mut confirm = false;
        let mut cancel = false;
        let mut pick = None;
        let (_, outside) = modal(ctx, &pal, false, forma_shell::chrome::MODAL_MARGIN, |ui| match &mut m {
            Modal::NewNote { folder, name } => {
                ui.set_width(modal_width(ui.ctx(), 420.0));
                modal_title(ui, &pal, &t!("dialog-new-note"));
                if !folder.as_os_str().is_empty() {
                    ui.label(RichText::new(t!("dialog-in-folder", folder = folder.display().to_string())).size(13.0).color(pal.text_faint));
                    ui.add_space(6.0);
                }
                let enter = name_field(ui, name, focus);
                let (ok, c) = buttons(ui, &pal, &t!("action-create"), false);
                confirm = ok || enter;
                cancel = c;
            }
            Modal::NewFolder { parent, name } => {
                ui.set_width(modal_width(ui.ctx(), 420.0));
                modal_title(ui, &pal, &t!("dialog-new-folder"));
                if !parent.as_os_str().is_empty() {
                    ui.label(RichText::new(t!("dialog-inside-folder", folder = parent.display().to_string())).size(13.0).color(pal.text_faint));
                    ui.add_space(6.0);
                }
                let enter = name_field(ui, name, focus);
                let (ok, c) = buttons(ui, &pal, &t!("action-create"), false);
                confirm = ok || enter;
                cancel = c;
            }
            Modal::Rename { name, .. } => {
                ui.set_width(modal_width(ui.ctx(), 420.0));
                modal_title(ui, &pal, &t!("dialog-rename"));
                ui.label(RichText::new(t!("dialog-rename-hint")).size(13.0).color(pal.text_faint));
                ui.add_space(8.0);
                let enter = name_field(ui, name, focus);
                let (ok, c) = buttons(ui, &pal, &t!("action-rename"), false);
                confirm = ok || enter;
                cancel = c;
            }
            Modal::Delete { target } => {
                ui.set_width(modal_width(ui.ctx(), 420.0));
                modal_title(ui, &pal, &t!("dialog-delete"));
                let name = target.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                ui.add(egui::Label::new(RichText::new(t!("dialog-delete-hint", name = name)).color(pal.text)).wrap());
                let (ok, c) = buttons(ui, &pal, &t!("action-delete"), true);
                confirm = ok;
                cancel = c;
            }
            Modal::QuickOpen { query, selected } => {
                if let Some(vault) = vault {
                    pick = quick_open(ui, &pal, vault, query, selected, focus);
                }
            }
        });
        if let Some(pick) = pick {
            match pick {
                Pick::Note(rel, new_tab) => self.cmds.push(Cmd::Open { rel, new_tab, tab: None }),
                Pick::Create(name, new_tab) => self.cmds.push(Cmd::OpenLink { target: name, new_tab, from: None, tab: None }),
            }
            return;
        }
        if confirm {
            match m {
                Modal::NewNote { folder, name } => self.create_note(folder, &name, false),
                Modal::NewFolder { parent, name } => {
                    if let Some(v) = &mut self.vault {
                        match v.create_folder(&parent, &name) {
                            Ok(rel) => {
                                self.files.expanded.insert(rel);
                            }
                            Err(e) => self.shell.toast(t!("toast-folder-create-failed", error = e.to_string()), true),
                        }
                    }
                }
                Modal::Rename { target, name } => self.rename(&target, &name),
                Modal::Delete { target } => self.delete(&target),
                Modal::QuickOpen { .. } => {}
            }
            return;
        }
        if !(cancel || outside) && self.modal.is_none() {
            self.modal = Some(m);
        }
    }
}
