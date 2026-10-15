//! Holotype внутри оболочки Forma: меню верхней строки, строка состояния, экран приветствия
//! и области с вкладками.

use std::path::PathBuf;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Id, RichText, Stroke, Ui};
use forma_i18n::t;
use forma_shell::chrome::{icon_item, item, menus};
use forma_shell::workspace;

use super::{Cmd, HolotypeApp, Viewer};
use crate::actions;
use crate::panes::{Kind, ViewMode};

impl HolotypeApp {
    /// Заголовки меню — по ним оболочка считает, помещаются ли меню целиком.
    fn menu_titles() -> Vec<String> {
        vec![t!("menu-file"), t!("menu-view"), t!("menu-go"), t!("menu-window"), t!("menu-help")]
    }

    pub(super) fn top_bar(&mut self, ui: &mut Ui) {
        let titles = Self::menu_titles();
        let mode = self.current.map(|id| self.mode_of(id));
        let has_note = self.current_rel().is_some();
        let recent: Vec<PathBuf> = self.shell.app_settings.vaults.recent.iter().filter(|p| p.is_dir()).cloned().collect();
        let cmds = &mut self.cmds;
        self.shell.top_bar(
            ui,
            &titles,
            0.0,
            |ui, shell, compact| {
                menus(ui, compact, |m| {
                    m.menu(&t!("menu-file"), false, |ui| {
                        let keys = |id: &str| shell.keys(id);
                        if ui.add(icon_item(ui, "note-new", t!("action-new-note"), keys(actions::NEW_NOTE))).clicked() {
                            cmds.push(Cmd::Shell(forma_shell::ShellCmd::Action(actions::NEW_NOTE.into())));
                        }
                        if ui.add(icon_item(ui, "folder-new", t!("action-new-folder"), keys(actions::NEW_FOLDER))).clicked() {
                            cmds.push(Cmd::Shell(forma_shell::ShellCmd::Action(actions::NEW_FOLDER.into())));
                        }
                        if ui.add(icon_item(ui, "daily", t!("action-daily-note"), keys(actions::DAILY_NOTE))).clicked() {
                            cmds.push(Cmd::DailyNote);
                        }
                        ui.separator();
                        if ui.add(item(t!("action-save-all"), keys(actions::SAVE_ALL))).clicked() {
                            cmds.push(Cmd::SaveAll);
                        }
                        ui.separator();
                        if ui.add_enabled(has_note, icon_item(ui, "export", t!("action-export-note"), keys(actions::EXPORT_NOTE))).clicked() {
                            cmds.push(Cmd::ExportNote);
                        }
                        if ui.add(icon_item(ui, "site", t!("action-export-site"), keys(actions::EXPORT_SITE))).clicked() {
                            cmds.push(Cmd::ExportSite);
                        }
                        ui.separator();
                        if ui.add(icon_item(ui, "open", t!("action-open-vault"), keys(actions::OPEN_VAULT))).clicked() {
                            cmds.push(Cmd::OpenVaultDialog);
                        }
                        ui.menu_button(t!("menu-recent-vaults"), |ui| {
                            if recent.is_empty() {
                                ui.label(RichText::new(t!("menu-recent-empty")).color(shell.pal.text_faint));
                            }
                            for p in &recent {
                                let name = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                                if ui.button(name).on_hover_text(p.display().to_string()).clicked() {
                                    cmds.push(Cmd::OpenVault(p.clone()));
                                }
                            }
                        });
                        if ui.add(icon_item(ui, "demo", t!("action-create-demo"), keys(actions::CREATE_DEMO))).clicked() {
                            cmds.push(Cmd::CreateDemo);
                        }
                        if ui.add(item(t!("action-reveal"), keys(actions::REVEAL))).clicked() {
                            cmds.push(Cmd::Shell(forma_shell::ShellCmd::Action(actions::REVEAL.into())));
                        }
                        if ui.add(item(t!("action-rescan"), keys(actions::RESCAN))).clicked() {
                            cmds.push(Cmd::Rescan);
                        }
                        ui.separator();
                        shell.file_menu_settings(ui);
                    });
                    // «Вид» не закрывается от переключателей, чтобы можно было поменять сразу несколько.
                    m.menu(&t!("menu-view"), true, |ui| {
                        for (m, key) in [(ViewMode::Preview, actions::TOGGLE_MODE), (ViewMode::Split, actions::SPLIT), (ViewMode::Edit, actions::TOGGLE_MODE)] {
                            let b = icon_item(ui, m.icon(), m.title(), shell.keys(key)).selected(mode == Some(m));
                            if ui.add_enabled(mode.is_some(), b).clicked() {
                                cmds.push(Cmd::SetMode { tab: None, mode: m });
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.add(icon_item(ui, "graph", t!("action-graph"), shell.keys(actions::GRAPH))).clicked() {
                            cmds.push(Cmd::ShowPane(Kind::Graph));
                            ui.close();
                        }
                        ui.separator();
                        let notes = &mut shell.app_settings.notes;
                        let pal = crate::theme::Palette::from_theme(&shell.theme);
                        crate::theme::menu_checkbox(ui, &pal, &mut notes.readable_width, &t!("notes-readable-width"));
                        crate::theme::menu_checkbox(ui, &pal, &mut notes.mono_editor, &t!("notes-mono-editor"));
                        ui.separator();
                        shell.view_menu_theme(ui);
                    });
                    m.menu(&t!("menu-go"), false, |ui| {
                        if ui.add(icon_item(ui, "search", t!("action-quick-open"), shell.keys(actions::QUICK_OPEN))).clicked() {
                            cmds.push(Cmd::QuickOpen);
                        }
                        if ui.add(item(t!("action-search"), shell.keys(actions::SEARCH))).clicked() {
                            cmds.push(Cmd::ShowPane(Kind::Search));
                        }
                        ui.separator();
                        if ui.add(icon_item(ui, "back", t!("action-back"), shell.keys(actions::BACK))).clicked() {
                            cmds.push(Cmd::Back(None));
                        }
                        if ui.add(icon_item(ui, "forward", t!("action-forward"), shell.keys(actions::FORWARD))).clicked() {
                            cmds.push(Cmd::Forward(None));
                        }
                    });
                    m.menu(&t!("menu-window"), false, |ui| {
                        shell.window_menu(ui, |ui| {
                            for kind in Kind::ALL {
                                if forma_ui::icons::item(ui, kind.icon(), kind.title(), false).clicked() {
                                    cmds.push(Cmd::ShowPane(kind));
                                }
                            }
                        });
                    });
                    m.menu(&t!("menu-help"), false, |ui| {
                        shell.help_menu(ui, |ui| {
                            if ui.add(icon_item(ui, "keyboard", t!("action-shortcuts"), String::new())).clicked() {
                                cmds.push(Cmd::Shell(forma_shell::ShellCmd::OpenSettingsPage(forma_ui::keymap_page::PAGE.into())));
                            }
                        });
                    });
                });
            },
            |_, _| {},
        );
    }

    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        let vault = self.vault.as_ref().map(|v| (v.name(), v.root.display().to_string(), v.notes.len()));
        let note = self.current_rel().and_then(|rel| self.docs.get(&rel).map(|d| (d.text.split_whitespace().count(), d.text.chars().count(), d.dirty)));
        self.shell.status_bar(ui, |ui, shell| {
            let pal = &shell.pal;
            let small = |s: String, c: Color32| RichText::new(s).size(12.0).color(c);
            if let Some((name, root, count)) = &vault {
                forma_shell_icon(ui, "vault", pal.statusbar_text);
                ui.label(small(name.clone(), pal.statusbar_text_strong)).on_hover_text(root.clone());
                ui.label(small(t!("status-notes", count = *count), pal.statusbar_text));
            }
            if let Some((words, chars, dirty)) = note {
                let minutes = (words as f32 / 200.0).ceil().max(1.0) as usize;
                ui.label(small("·".into(), pal.statusbar_text));
                ui.label(small(t!("status-note-stats", words = words, chars = chars, minutes = minutes), pal.statusbar_text));
                let (text, color) = if dirty { (format!("● {}", t!("status-unsaved")), pal.accent) } else { (format!("✔ {}", t!("status-saved")), pal.statusbar_text) };
                ui.label(small("·".into(), pal.statusbar_text));
                ui.label(small(text, color));
            }
        });
    }

    pub(super) fn welcome_ui(&mut self, ui: &mut Ui) {
        let recent: Vec<PathBuf> = self.shell.app_settings.vaults.recent.iter().filter(|p| p.is_dir()).cloned().collect();
        let cmds = &mut self.cmds;
        self.shell.welcome_ui(ui, |ui, shell, width| {
            if shell.welcome_button(ui, "demo", t!("welcome-create-demo"), width, true).clicked() {
                cmds.push(Cmd::CreateDemo);
            }
            ui.add_space(6.0);
            if shell.welcome_button(ui, "open", t!("welcome-open-vault"), width, false).clicked() {
                cmds.push(Cmd::OpenVaultDialog);
            }
            if !recent.is_empty() {
                ui.add_space(20.0);
                ui.label(RichText::new(t!("welcome-recent")).size(13.0).color(shell.pal.text_faint));
                ui.add_space(4.0);
                for p in recent.into_iter().take(6) {
                    let name = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let resp = ui.add(egui::Button::new(RichText::new(&name).size(14.5)).frame(false).truncate()).on_hover_text(p.display().to_string());
                    if resp.clicked() {
                        cmds.push(Cmd::OpenVault(p));
                    }
                }
            }
        });
    }

    /// Области текущего рабочего пространства и отдельные окна.
    pub(super) fn dock_ui(&mut self, ui: &mut Ui) {
        let now = self.shell.now();
        let current_rel = self.current_rel();
        let shortcuts = !self.shell.modal_open();
        let shell = &mut self.shell;
        let window_bg = shell.pal.window_bg;
        let plugin_hovers = shell.plugin_runtime.as_ref().is_some_and(|r| r.has_hovers());
        let (surface, dock, next_id) = shell.layout.parts();
        let mut viewer = Viewer {
            pal: &self.pal,
            fpal: &shell.pal,
            vault: self.vault.as_ref(),
            docs: &mut self.docs,
            views: &mut self.views,
            current: &mut self.current,
            current_rel,
            files: &mut self.files,
            search: &mut self.search,
            backlinks: &mut self.backlinks,
            graph: &mut self.graph,
            notes: &shell.app_settings.notes,
            actions: &shell.actions,
            keymap: &shell.settings.keymap,
            fired: &mut shell.fired,
            shortcuts,
            next_id,
            cmds: &mut self.cmds,
            now,
            plugin_hovers,
        };
        workspace::show(ui, surface, dock, &shell.dock_style, window_bg, &mut viewer);
    }

    /// Подсказка при перетаскивании файлов в окно.
    pub(super) fn drop_hint(&self, ctx: &egui::Context) {
        if ctx.input(|i| i.raw.hovered_files.is_empty()) {
            return;
        }
        let rect = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("holotype-drop")));
        painter.rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(120));
        painter.rect_stroke(rect.shrink(16.0), CornerRadius::same(16), Stroke::new(2.0, self.pal.accent), egui::StrokeKind::Inside);
        painter.text(rect.center(), Align2::CENTER_CENTER, t!("drop-hint"), FontId::proportional(20.0), Color32::WHITE);
    }
}

/// Значок в строке состояния.
fn forma_shell_icon(ui: &mut Ui, name: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(13.0, 13.0), egui::Sense::hover());
    forma_ui::icons::paint(ui.painter(), rect, name, color);
}
