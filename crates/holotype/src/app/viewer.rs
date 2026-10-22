//! Содержимое областей: как каждый редактор рисуется внутри составного интерфейса Forma.
//! [`Viewer`] собирается заново на каждом кадре из частей состояния программы.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Id, Layout, Margin, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, pos2, vec2,
    collapsing_header::CollapsingState,
    text::{LayoutJob, TextFormat},
};
use forma_actions::{Keymap, Registry};
use forma_i18n::t;
use forma_layout::{EditorKind, EditorViewer, KindGroup};
use forma_ui::widgets::{doc_tab, header_button};

use super::docs::{Docs, NoteView};
use super::note::{Keys, NoteCx, note_ui};
use super::{Backlinks, Cmd, FilesState, NodeKind, SearchState, TreeNode};
use crate::fonts;
use crate::graph::{Graph, GraphAction};
use crate::markdown;
use crate::panes::{Kind, NoteTab, Pane};
use crate::plugin_ui::{self, CommandCx, PluginUi};
use crate::settings::Notes;
use crate::theme::{self, Palette};
use crate::vault::Vault;

/// Всё, что нужно редакторам для отрисовки. Собирается заново на каждом кадре.
pub struct Viewer<'a> {
    pub pal: &'a Palette,
    pub fpal: &'a forma_ui::theme::Palette,
    pub vault: Option<&'a Vault>,
    pub docs: &'a mut Docs,
    pub views: &'a mut HashMap<u64, NoteView>,
    /// Вкладка заметки, с которой работали последней (обновляется щелчком внутри заметки).
    pub current: &'a mut Option<u64>,
    /// Заметка текущей вкладки — для оглавления, обратных ссылок и сведений.
    pub current_rel: Option<PathBuf>,
    pub files: &'a mut FilesState,
    pub search: &'a mut SearchState,
    pub backlinks: &'a mut Option<Backlinks>,
    pub graph: &'a mut Graph,
    pub notes: &'a Notes,
    pub actions: &'a Registry,
    pub keymap: &'a Keymap,
    /// Сработавшие общие сочетания клавиш в отдельных окнах.
    pub fired: &'a mut Vec<String>,
    /// Работают ли общие сочетания (пока открыт диалог — нет).
    pub shortcuts: bool,
    pub next_id: &'a mut u64,
    pub cmds: &'a mut Vec<Cmd>,
    pub now: f64,
    /// У плагинов есть подсказки при наведении.
    pub plugin_hovers: bool,
    /// Панели, блоки и пункты меню плагинов.
    pub plugins: &'a PluginUi,
}

/// Вид редактора «панель плагина» в меню вида: `plugin:плагин/панель`.
const PLUGIN_KIND: &str = "plugin:";

impl Viewer<'_> {
    fn new_id(&mut self) -> u64 {
        *self.next_id += 1;
        *self.next_id
    }

    /// Подпись сочетания для вида редактора в меню выбора вида.
    fn kind_keys(&self, kind: Kind) -> String {
        let action = match kind {
            Kind::Search => crate::actions::SEARCH,
            Kind::Graph => crate::actions::GRAPH,
            _ => return String::new(),
        };
        self.keymap.text(self.actions, action)
    }
}

fn stem(rel: &Path) -> String {
    rel.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Название и значок вкладки: заметка — имя без `.md`, картинка — имя файла целиком.
fn tab_label(rel: &Path) -> (&'static str, String) {
    if forma_images::is_image(rel) {
        ("file-image", rel.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
    } else {
        ("note", stem(rel))
    }
}

impl EditorViewer for Viewer<'_> {
    type Editor = Pane;

    /// Виды редакторов по столбцам меню; панели плагинов — своим столбцом «Плагины», как
    /// типы редакторов от аддонов в меню вида области у Blender.
    fn kinds(&mut self) -> Vec<KindGroup> {
        let mut groups: Vec<KindGroup> = Kind::GROUPS
            .iter()
            .map(|(title, kinds)| KindGroup {
                title: forma_i18n::tr(title),
                kinds: kinds
                    .iter()
                    .map(|&k| EditorKind { id: k.id().to_owned(), title: k.title(), icon: k.icon().to_owned(), shortcut: self.kind_keys(k) })
                    .collect(),
            })
            .collect();
        if !self.plugins.core.panels.is_empty() {
            groups.push(KindGroup {
                title: t!("kind-group-plugins"),
                kinds: self
                    .plugins
                    .core
                    .panels
                    .iter()
                    .map(|p| EditorKind {
                        id: format!("{PLUGIN_KIND}{}", p.key()),
                        title: p.title.clone(),
                        icon: panel_icon(p.icon.as_deref()),
                        shortcut: String::new(),
                    })
                    .collect(),
            });
        }
        groups
    }

    fn kind(&mut self, pane: &Pane) -> String {
        match (pane.kind, &pane.panel) {
            (Kind::Plugin, Some(key)) => format!("{PLUGIN_KIND}{key}"),
            (kind, _) => kind.id().to_owned(),
        }
    }

    fn create(&mut self, kind: &str) -> Option<Pane> {
        if let Some(key) = kind.strip_prefix(PLUGIN_KIND) {
            return Some(Pane::plugin(key, self.new_id()));
        }
        let kind = Kind::from_id(kind)?;
        Some(Pane::new(kind, self.new_id()))
    }

    /// Вид меняется, а вкладки заметок остаются — вернёшь заметки, вернутся и они.
    fn switch(&mut self, pane: &mut Pane, kind: &str) {
        if let Some(key) = kind.strip_prefix(PLUGIN_KIND) {
            pane.kind = Kind::Plugin;
            pane.panel = Some(key.to_owned());
        } else if let Some(kind) = Kind::from_id(kind) {
            pane.kind = kind;
            if kind == Kind::Graph {
                self.graph.reheat();
            }
        }
    }

    fn title(&mut self, pane: &Pane) -> String {
        match pane.active_note() {
            Some(n) => tab_label(&n.rel).1,
            None if pane.kind == Kind::Plugin => self.panel_title(pane),
            None => pane.kind.title(),
        }
    }

    fn icon(&mut self, pane: &Pane) -> Option<(String, Color32)> {
        let icon = match (pane.kind, &pane.panel) {
            (Kind::Plugin, Some(key)) => panel_icon(self.plugins.core.panel(key).and_then(|p| p.icon.as_deref())),
            (kind, _) => kind.icon().to_owned(),
        };
        Some((icon, self.pal.accent))
    }

    fn id(&mut self, pane: &Pane) -> Id {
        Id::new(("holotype-pane", pane.id))
    }

    fn header(&mut self, ui: &mut Ui, pane: &mut Pane) {
        match pane.kind {
            Kind::Note => self.note_tabs(ui, pane),
            Kind::Files => self.files_header(ui),
            Kind::Plugin => {
                ui.label(RichText::new(self.panel_title(pane)).size(13.0).color(self.fpal.tab_active_text));
            }
            kind => {
                ui.label(RichText::new(kind.title()).size(13.0).color(self.fpal.tab_active_text));
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, pane: &mut Pane) {
        // Щелчок внутри заметки делает её вкладку текущей.
        if let Some(n) = pane.active_note()
            && ui.ui_contains_pointer()
            && ui.input(|i| i.pointer.any_pressed())
        {
            *self.current = Some(n.id);
        }
        let Some(vault) = self.vault else { return };
        match pane.kind {
            Kind::Note => self.note(ui, vault, pane),
            Kind::Files => self.files(ui, vault),
            Kind::Search => self.search(ui, vault),
            Kind::Tags => self.tags(ui, vault),
            Kind::Graph => {
                self.graph.sync(vault);
                if let Some(action) = self.graph.ui(ui, self.pal, self.current_rel.as_deref()) {
                    self.cmds.push(match action {
                        GraphAction::Open(rel) => Cmd::Open { rel, new_tab: false, tab: None },
                        GraphAction::Create(name) => Cmd::OpenLink { target: name, new_tab: false, from: None, tab: None },
                    });
                }
            }
            Kind::Outline => self.outline(ui, vault),
            Kind::Backlinks => self.backlinks(ui, vault),
            Kind::Info => self.info(ui, vault),
            Kind::Plugin => self.plugin_panel(ui, vault, pane),
        }
    }

    fn duplicate(&mut self, pane: &Pane) -> Pane {
        let mut copy = pane.clone();
        copy.id = self.new_id();
        for n in &mut copy.notes {
            n.id = self.new_id();
        }
        copy
    }

    fn on_close(&mut self, pane: Pane) {
        // Закрытые заметки сохраняются сразу, не дожидаясь автосохранения.
        if !pane.notes.is_empty() {
            self.cmds.push(Cmd::SaveAll);
        }
        for n in &pane.notes {
            self.views.remove(&n.id);
        }
    }

    fn window_input(&mut self, ui: &mut Ui) {
        if self.shortcuts {
            forma_shell::Shell::<crate::settings::HolotypeSettings, crate::panes::Space>::fire_global(ui, self.actions, self.keymap, self.fired);
        }
    }
}

/// Что сделать с вкладкой заметки после отрисовки строки вкладок.
enum TabAction {
    Activate(usize),
    Close(usize),
    CloseOthers(usize),
    CloseAll,
}

impl Viewer<'_> {
    /// Вкладки заметок в заголовке области.
    fn note_tabs(&mut self, ui: &mut Ui, pane: &mut Pane) {
        let fpal = self.fpal;
        if pane.notes.is_empty() {
            ui.label(RichText::new(Kind::Note.title()).size(13.0).color(fpal.tab_text));
            return;
        }
        let mut action = None;
        let docs = &*self.docs;
        let cmds = &mut *self.cmds;
        ScrollArea::horizontal()
            .id_salt(("holotype-note-tabs", pane.id))
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (i, n) in pane.notes.iter().enumerate() {
                        let (icon, title) = tab_label(&n.rel);
                        let (resp, on_close) = doc_tab(ui, fpal, icon, &title, i == pane.active, docs.is_dirty(&n.rel));
                        if resp.middle_clicked() || (resp.clicked() && on_close) {
                            action = Some(TabAction::Close(i));
                        } else if resp.clicked() {
                            action = Some(TabAction::Activate(i));
                        }
                        egui::Popup::context_menu(&resp).show(|ui| {
                            if ui.button(t!("action-close")).clicked() {
                                action = Some(TabAction::Close(i));
                            }
                            if ui.add_enabled(pane.notes.len() > 1, egui::Button::new(t!("tab-close-others"))).clicked() {
                                action = Some(TabAction::CloseOthers(i));
                            }
                            if ui.button(t!("tab-close-all")).clicked() {
                                action = Some(TabAction::CloseAll);
                            }
                            ui.separator();
                            if ui.button(t!("action-rename")).clicked() {
                                cmds.push(Cmd::RenameDialog(n.rel.clone()));
                            }
                            if ui.button(t!("action-reveal")).clicked() {
                                cmds.push(Cmd::Reveal(n.rel.clone()));
                            }
                        });
                        resp.on_hover_text(n.rel.display().to_string());
                    }
                });
            });
        let closed: Vec<NoteTab> = match action {
            Some(TabAction::Activate(i)) => {
                pane.active = i;
                *self.current = Some(pane.notes[i].id);
                Vec::new()
            }
            Some(TabAction::Close(i)) => pane.close(i).into_iter().collect(),
            Some(TabAction::CloseOthers(i)) => {
                let keep = pane.notes.remove(i);
                pane.active = 0;
                std::mem::replace(&mut pane.notes, vec![keep])
            }
            Some(TabAction::CloseAll) => {
                pane.active = 0;
                std::mem::take(&mut pane.notes)
            }
            None => Vec::new(),
        };
        if !closed.is_empty() {
            self.cmds.push(Cmd::SaveAll);
            for n in closed {
                self.views.remove(&n.id);
            }
        }
    }

    /// Заголовок файлов: имя хранилища и кнопки «обновить», «новая папка», «новая заметка».
    fn files_header(&mut self, ui: &mut Ui) {
        let fpal = self.fpal;
        let Some(vault) = self.vault else {
            ui.label(RichText::new(Kind::Files.title()).size(13.0).color(fpal.tab_active_text));
            return;
        };
        ui.label(RichText::new(vault.name()).size(13.0).family(fonts::semibold()).color(fpal.tab_active_text)).on_hover_text(vault.root.display().to_string());
        let folder = self.current_rel.as_ref().and_then(|r| r.parent().map(Path::to_path_buf)).unwrap_or_default();
        let cmds = &mut *self.cmds;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if header_button(ui, fpal, "note-new").on_hover_text(t!("action-new-note")).clicked() {
                cmds.push(Cmd::NewNoteDialog { folder: folder.clone() });
            }
            if header_button(ui, fpal, "folder-new").on_hover_text(t!("action-new-folder")).clicked() {
                cmds.push(Cmd::NewFolderDialog { parent: folder.clone() });
            }
            if header_button(ui, fpal, "refresh").on_hover_text(t!("action-rescan")).clicked() {
                cmds.push(Cmd::Rescan);
            }
            // Папки и файлы с точкой: глаз открыт — видны.
            let (icon, hint) = if vault.show_hidden { ("show", t!("files-hide-hidden")) } else { ("hide", t!("files-show-hidden")) };
            if header_button(ui, fpal, icon).on_hover_text(hint).clicked() {
                cmds.push(Cmd::ToggleHidden);
            }
        });
    }
}

/// Отступы панелей.
fn panel(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    Frame::new().inner_margin(Margin { left: 10, right: 10, top: 10, bottom: 6 }).show(ui, add);
}

/// Значок панели плагина: из манифеста, если такой есть, иначе пазл плагина.
fn panel_icon(icon: Option<&str>) -> String {
    icon.filter(|i| forma_ui::icons::exists(i)).unwrap_or("plugin").to_owned()
}

/// Пустое состояние вкладки.
fn empty(ui: &mut Ui, fpal: &forma_ui::theme::Palette, icon: &str, title: &str, hint: &str) {
    let height = ui.available_height();
    ScrollArea::vertical().id_salt("holotype-empty").auto_shrink(false).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space((height * 0.3 - 50.0).max(12.0));
            let (rect, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
            forma_ui::icons::paint(ui.painter(), rect, icon, fpal.text_faint);
            ui.add_space(4.0);
            ui.label(RichText::new(title).size(15.0).family(fonts::semibold()).color(fpal.heading));
            ui.add_space(2.0);
            forma_ui::widgets::centered_text(ui, hint, 13.0, fpal.text_muted, 380.0);
            ui.add_space(12.0);
        });
    });
}

impl Viewer<'_> {
    fn note(&mut self, ui: &mut Ui, vault: &Vault, pane: &mut Pane) {
        let active = pane.active;
        let Some(tab) = pane.notes.get_mut(active) else {
            empty(ui, self.fpal, "note", &t!("note-empty-title"), &t!("note-empty-hint", open = self.keymap.text(self.actions, crate::actions::QUICK_OPEN)));
            ui.vertical_centered(|ui| {
                if ui.button(t!("action-new-note")).clicked() {
                    self.cmds.push(Cmd::NewNoteDialog { folder: PathBuf::new() });
                }
            });
            return;
        };
        let rel = tab.rel.clone();
        // Картинка хранилища — во вкладке просмотра.
        if forma_images::is_image(&rel) {
            if let Some(error) = super::image_view::show(ui, self.fpal, tab.id, &vault.abs(&rel)) {
                empty(ui, self.fpal, "file-image", &t!("image-failed-title"), &t!("image-failed-hint", error = error));
                if ui.vertical_centered(|ui| ui.button(t!("action-reveal"))).inner.clicked() {
                    self.cmds.push(Cmd::Reveal(rel));
                }
            }
            return;
        }
        let Some(doc) = self.docs.get_or_load(vault, &rel) else {
            empty(ui, self.fpal, "warning", &t!("note-missing-title"), &t!("note-missing-hint", path = rel.display().to_string()));
            return;
        };
        let view = self.views.entry(tab.id).or_default();
        let cx = NoteCx {
            pal: self.pal,
            fpal: self.fpal,
            vault,
            size: self.notes.text_size,
            mono: self.notes.mono_editor,
            readable: self.notes.readable_width,
            default_mode: self.notes.default_mode,
            keys: Keys { actions: self.actions, keymap: self.keymap },
            now: self.now,
            plugin_hovers: self.plugin_hovers,
            plugins: self.plugins,
        };
        note_ui(ui, &cx, tab, &rel, doc, view, self.cmds);
    }

    /// Название панели плагина (из манифеста), если плагин на месте.
    fn panel_title(&self, pane: &Pane) -> String {
        let key = pane.panel.as_deref().unwrap_or_default();
        self.plugins.core.panel(key).map(|p| p.title.clone()).unwrap_or_else(|| key.to_owned())
    }

    /// Панель плагина: его дерево элементов. Дерева ещё нет — просьба нарисовать; события
    /// элементов и кнопки с командами уходят плагину, ссылки из markdown открываются здесь.
    fn plugin_panel(&mut self, ui: &mut Ui, vault: &Vault, pane: &Pane) {
        let key = pane.panel.clone().unwrap_or_default();
        if self.plugins.core.panel(&key).is_none() {
            let plugin = plugin_ui::split_key(&key).map_or(key.as_str(), |(p, _)| p);
            empty(ui, self.fpal, "plugin", &t!("pane-plugin"), &t!("plugin-panel-gone", plugin = plugin.to_owned()));
            return;
        }
        let note = self.current_rel.clone().unwrap_or_default();
        let fpal = self.fpal;
        let size = self.notes.text_size * 0.9;
        if self.plugins.core.needs_render(&key) {
            self.cmds.push(Cmd::RenderPanel(key.clone()));
        }
        let mut hooks = plugin_ui::Hooks { pal: self.pal, vault, linked: &self.plugins.linked, note: &note, size, actions: Vec::new() };
        let out = self.plugins.core.show(ui, fpal, &key, Id::new(("plugin-panel", pane.id)), &mut hooks);
        for a in hooks.actions {
            self.cmds.push(super::note::render_cmd(a, &note, None));
        }
        for o in out {
            self.cmds.push(match o {
                forma_ui::elements::Output::Event(event) => Cmd::PanelEvent { key: key.clone(), event },
                forma_ui::elements::Output::Command { command, args } => {
                    let note = (!note.as_os_str().is_empty()).then_some(note.as_path());
                    Cmd::PluginCommand { command, cx: CommandCx { args, ..CommandCx::new("panel", note) } }
                }
            });
        }
    }

    fn files(&mut self, ui: &mut Ui, vault: &Vault) {
        if self.files.tree.as_ref().is_none_or(|(g, _)| *g != vault.generation) {
            self.files.tree = Some((vault.generation, build_tree(vault)));
        }
        let pal = self.pal;
        let active = self.current_rel.clone();
        let cmds = &mut *self.cmds;
        panel(ui, |ui| {
            let Some((_, tree)) = &self.files.tree else { return };
            let expanded = &mut self.files.expanded;
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for child in &tree.children {
                    tree_ui(ui, child, 0, expanded, active.as_deref(), pal, cmds);
                }
                // Пустое место под деревом: меню для корня и сброс перетаскивания в корень.
                let rest = ui.available_size().max(vec2(0.0, 40.0));
                let (_, r) = ui.allocate_exact_size(rest, Sense::click());
                if let Some(from) = r.dnd_release_payload::<PathBuf>() {
                    cmds.push(Cmd::Move { from: (*from).clone(), to: PathBuf::new() });
                }
                theme::context_menu(&r, |ui| {
                    if ui.button(t!("action-new-note")).clicked() {
                        cmds.push(Cmd::NewNoteDialog { folder: PathBuf::new() });
                    }
                    if ui.button(t!("action-new-folder")).clicked() {
                        cmds.push(Cmd::NewFolderDialog { parent: PathBuf::new() });
                    }
                });
            });
        });
    }

    fn search(&mut self, ui: &mut Ui, vault: &Vault) {
        let pal = self.pal;
        let search = &mut *self.search;
        let saves = self.docs.saves;
        let cmds = &mut *self.cmds;
        panel(ui, |ui| {
            let edit = TextEdit::singleline(&mut search.query).hint_text(t!("search-hint")).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 5));
            let resp = ui.add(edit);
            if search.focus {
                resp.request_focus();
                search.focus = false;
            }
            let key = (search.query.clone(), vault.generation, saves);
            if search.key.as_ref() != Some(&key) {
                search.hits = vault.search(&search.query, 300);
                search.key = Some(key);
            }
            ui.add_space(6.0);
            if search.query.trim().is_empty() {
                ui.label(RichText::new(t!("search-empty")).size(13.0).color(pal.text_faint));
                return;
            }
            if search.hits.is_empty() {
                ui.label(RichText::new(t!("search-nothing")).size(13.0).color(pal.text_faint));
                return;
            }
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let mut last_note = usize::MAX;
                for hit in &search.hits {
                    let note = &vault.notes[hit.note];
                    if hit.note != last_note {
                        last_note = hit.note;
                        ui.add_space(6.0);
                        let folder = note.rel.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                        let r = ui.add(egui::Label::new(RichText::new(&note.name).size(14.0).family(fonts::semibold()).color(pal.heading)).sense(Sense::click()));
                        if !folder.is_empty() {
                            ui.label(RichText::new(folder).size(11.5).color(pal.text_faint));
                        }
                        if r.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            cmds.push(Cmd::Open { rel: note.rel.clone(), new_tab: false, tab: None });
                        }
                    }
                    if hit.text.is_empty() {
                        continue;
                    }
                    let mut job = LayoutJob::default();
                    job.wrap.max_width = ui.available_width();
                    let base = TextFormat { font_id: FontId::proportional(13.0), color: pal.text_muted, ..Default::default() };
                    let strong = TextFormat { font_id: FontId::new(13.0, fonts::semibold()), color: pal.heading, background: pal.mark_bg, ..Default::default() };
                    let mut pos = 0;
                    for &(s, e) in &hit.ranges {
                        if s > pos {
                            job.append(&hit.text[pos..s], 0.0, base.clone());
                        }
                        job.append(&hit.text[s..e], 0.0, strong.clone());
                        pos = e;
                    }
                    if pos < hit.text.len() {
                        job.append(&hit.text[pos..], 0.0, base.clone());
                    }
                    let r = Frame::new().inner_margin(Margin { left: 8, right: 4, top: 2, bottom: 2 }).show(ui, |ui| ui.add(egui::Label::new(job).sense(Sense::click()))).inner;
                    if r.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        cmds.push(Cmd::JumpTo { rel: note.rel.clone(), offset: hit.offset });
                    }
                }
            });
        });
    }

    fn tags(&mut self, ui: &mut Ui, vault: &Vault) {
        let pal = self.pal;
        let cmds = &mut *self.cmds;
        panel(ui, |ui| {
            let tags = vault.all_tags();
            if tags.is_empty() {
                ui.label(RichText::new(t!("tags-empty")).size(13.0).color(pal.text_faint));
                return;
            }
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    for (tag, count) in tags {
                        let text = RichText::new(format!("#{tag}  {count}")).size(13.0).color(pal.tag_text);
                        let btn = egui::Button::new(text).fill(pal.tag_bg).corner_radius(12);
                        if ui.add(btn).clicked() {
                            cmds.push(Cmd::SearchTag(tag));
                        }
                    }
                });
            });
        });
    }

    /// Заметка текущей вкладки, если она открыта.
    fn current_doc(&mut self, vault: &Vault) -> Option<(PathBuf, &mut super::docs::NoteDoc)> {
        let rel = self.current_rel.clone()?;
        let doc = self.docs.get_or_load(vault, &rel)?;
        Some((rel, doc))
    }

    fn no_note(ui: &mut Ui, pal: &Palette) {
        panel(ui, |ui| {
            ui.label(RichText::new(t!("panel-no-note")).size(13.0).color(pal.text_faint));
        });
    }

    fn outline(&mut self, ui: &mut Ui, vault: &Vault) {
        let pal = self.pal;
        let tab_id = *self.current;
        let Some((_, doc)) = self.current_doc(vault) else { return Self::no_note(ui, pal) };
        let headings = doc.parsed().headings.clone();
        let cmds = &mut *self.cmds;
        panel(ui, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if headings.is_empty() {
                    ui.label(RichText::new(t!("outline-empty")).size(13.0).color(pal.text_faint));
                }
                let min_level = headings.iter().map(|h| h.level).min().unwrap_or(1);
                for h in &headings {
                    let indent = (h.level - min_level) as f32 * 12.0;
                    let color = if h.level == min_level { pal.text } else { pal.text_muted };
                    if nav_row(ui, pal, &h.text, indent, color).clicked()
                        && let Some(tab) = tab_id
                    {
                        cmds.push(Cmd::Heading { tab, text: h.text.to_lowercase(), offset: h.offset });
                    }
                }
            });
        });
    }

    fn backlinks(&mut self, ui: &mut Ui, vault: &Vault) {
        let pal = self.pal;
        let Some(rel) = self.current_rel.clone() else { return Self::no_note(ui, pal) };
        let key = (vault.generation, self.docs.saves, rel.clone());
        if self.backlinks.as_ref().is_none_or(|(k, _)| *k != key) {
            let list = vault.index_of(&rel).map(|i| vault.backlinks(i)).unwrap_or_default();
            *self.backlinks = Some((key, list));
        }
        let list = &self.backlinks.as_ref().expect("обратные ссылки только что посчитаны").1;
        let cmds = &mut *self.cmds;
        panel(ui, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if list.is_empty() {
                    ui.label(RichText::new(t!("backlinks-empty")).size(13.0).color(pal.text_faint));
                }
                for (i, line) in list {
                    let note = &vault.notes[*i];
                    // Карточка целиком кликабельна; фрагмент показываем с оформлением, без «сырой» разметки.
                    let card_id = Id::new(("backlink", &note.rel));
                    let hovered_before: bool = ui.data(|d| d.get_temp(card_id)).unwrap_or(false);
                    let t = ui.ctx().animate_bool_with_time(card_id.with("t"), hovered_before, 0.12);
                    let resp = Frame::new()
                        .fill(pal.card_bg)
                        .stroke(Stroke::new(1.0, pal.border.lerp_to_gamma(pal.accent.gamma_multiply(0.7), t)))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin { left: 12, right: 12, top: 9, bottom: 10 })
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.spacing_mut().item_spacing.y = 4.0;
                            ui.label(RichText::new(&note.name).size(13.5).family(fonts::semibold()).color(pal.link));
                            if !line.is_empty() {
                                let job = crate::render::snippet_job(pal, line, 12.5, ui.available_width());
                                ui.add(egui::Label::new(job).wrap());
                            }
                        })
                        .response
                        .interact(Sense::click());
                    ui.data_mut(|d| d.insert_temp(card_id, resp.hovered()));
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        cmds.push(Cmd::Open { rel: note.rel.clone(), new_tab: false, tab: None });
                    }
                    ui.add_space(6.0);
                }
            });
        });
    }

    fn info(&mut self, ui: &mut Ui, vault: &Vault) {
        let pal = self.pal;
        let Some((rel, doc)) = self.current_doc(vault) else { return Self::no_note(ui, pal) };
        let words = doc.text.split_whitespace().count();
        let chars = doc.text.chars().count();
        let parsed = doc.parsed();
        let links = parsed.note_links.len();
        let tags = parsed.tags.clone();
        let cmds = &mut *self.cmds;
        panel(ui, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let row = |ui: &mut Ui, k: String, v: String| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(k).size(12.5).color(pal.text_faint));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(v).size(12.5).color(pal.text_muted));
                        });
                    });
                };
                row(ui, t!("info-words"), words.to_string());
                row(ui, t!("info-chars"), chars.to_string());
                row(ui, t!("info-links"), links.to_string());
                if let Some(m) = vault.index_of(&rel).and_then(|i| vault.notes[i].modified) {
                    let dt: chrono::DateTime<chrono::Local> = m.into();
                    row(ui, t!("info-modified"), dt.format("%d.%m.%Y %H:%M").to_string());
                }
                if !tags.is_empty() {
                    ui.add_space(10.0);
                    ui.label(RichText::new(t!("info-tags").to_uppercase()).size(11.5).family(fonts::semibold()).color(pal.text_faint));
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        for t in &tags {
                            if crate::render::tag_pill(ui, pal, &format!("#{t}"), 12.5).clicked() {
                                cmds.push(Cmd::SearchTag(t.clone()));
                            }
                        }
                    });
                }
            });
        });
    }
}

/// Строка-ссылка в панели с плавной подсветкой при наведении.
fn nav_row(ui: &mut Ui, pal: &Palette, text: &str, indent: f32, color: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let painter = ui.painter();
    if t > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(7), pal.bg_hover.gamma_multiply(t));
    }
    painter.with_clip_rect(rect.shrink2(vec2(6.0, 0.0))).text(
        pos2(rect.left() + 8.0 + indent + t * 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.5),
        color.lerp_to_gamma(pal.heading, t),
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Дерево папок, заметок и вложений хранилища.
pub fn build_tree(vault: &Vault) -> TreeNode {
    let mut root = TreeNode { name: vault.name(), rel: PathBuf::new(), kind: NodeKind::Folder, children: Vec::new() };
    fn ensure_dir<'a>(node: &'a mut TreeNode, rel: &Path) -> &'a mut TreeNode {
        let mut cur = node;
        let mut acc = PathBuf::new();
        for comp in rel.components() {
            acc.push(comp);
            let name = comp.as_os_str().to_string_lossy().into_owned();
            let pos = cur.children.iter().position(|c| matches!(c.kind, NodeKind::Folder) && c.name == name);
            let idx = match pos {
                Some(i) => i,
                None => {
                    cur.children.push(TreeNode { name, rel: acc.clone(), kind: NodeKind::Folder, children: Vec::new() });
                    cur.children.len() - 1
                }
            };
            cur = &mut cur.children[idx];
        }
        cur
    }
    for f in &vault.folders {
        ensure_dir(&mut root, f);
    }
    for n in &vault.notes {
        let parent = n.rel.parent().map(Path::to_path_buf).unwrap_or_default();
        ensure_dir(&mut root, &parent).children.push(TreeNode { name: n.name.clone(), rel: n.rel.clone(), kind: NodeKind::Note, children: Vec::new() });
    }
    for a in &vault.attachments {
        let parent = a.parent().map(Path::to_path_buf).unwrap_or_default();
        let name = a.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        ensure_dir(&mut root, &parent).children.push(TreeNode { name, rel: a.clone(), kind: NodeKind::File, children: Vec::new() });
    }
    fn sort(node: &mut TreeNode) {
        node.children.sort_by(|a, b| {
            let ka = matches!(a.kind, NodeKind::Folder);
            let kb = matches!(b.kind, NodeKind::Folder);
            kb.cmp(&ka).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        for c in &mut node.children {
            sort(c);
        }
    }
    sort(&mut root);
    root
}

fn tree_ui(ui: &mut Ui, node: &TreeNode, depth: usize, expanded: &mut HashSet<PathBuf>, active: Option<&Path>, pal: &Palette, cmds: &mut Vec<Cmd>) {
    let is_folder = matches!(node.kind, NodeKind::Folder);
    let is_open = is_folder && expanded.contains(&node.rel);
    let is_active = active == Some(node.rel.as_path());

    let h = 27.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click_and_drag());
    let drop_hover = is_folder && resp.dnd_hover_payload::<PathBuf>().is_some();
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.12);
    let active_t = ui.ctx().animate_bool_with_time(resp.id.with("active"), is_active, 0.18);
    // Раскрытие папки анимируется через CollapsingState.
    let mut state = CollapsingState::load_with_default_open(ui.ctx(), resp.id.with("fold"), is_open);
    state.set_open(is_open);
    let openness = state.openness(ui.ctx());
    let painter = ui.painter();
    if active_t > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(7), pal.accent_soft.gamma_multiply(active_t));
    }
    if drop_hover {
        painter.rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.5, pal.accent), egui::StrokeKind::Inside);
    } else if hover > 0.0 && !is_active {
        painter.rect_filled(rect, CornerRadius::same(7), pal.bg_hover.gamma_multiply(hover));
    }
    let x0 = rect.left() + 8.0 + depth as f32 * 14.0;
    let cy = rect.center().y;
    let mut x = x0;
    if is_folder {
        // Стрелка папки плавно поворачивается при раскрытии.
        crate::render::chevron(painter, pos2(x + 4.0, cy), 4.0, openness, pal.text_faint);
        x += 16.0;
    }
    let (label, color, font, icon) = match node.kind {
        NodeKind::Folder => (node.name.clone(), pal.text, FontId::new(13.5, fonts::semibold()), None),
        NodeKind::Note => (node.name.clone(), if is_active { pal.heading } else { pal.text_muted }, FontId::proportional(13.5), None),
        NodeKind::File => {
            let icon = if markdown::is_image_path(&node.name) { "file-image" } else { "file" };
            (node.name.clone(), pal.text_faint, FontId::proportional(12.5), Some(icon))
        }
    };
    if let Some(icon) = icon {
        forma_ui::icons::paint(painter, egui::Rect::from_center_size(pos2(x + 6.0, cy), vec2(14.0, 14.0)), icon, pal.text_faint);
        x += 18.0;
    }
    painter.with_clip_rect(rect.shrink2(vec2(4.0, 0.0))).text(pos2(x, cy), Align2::LEFT_CENTER, label, font, color);

    // Перетаскивание: заметки и папки можно переносить в другие папки.
    if resp.drag_started() {
        resp.dnd_set_drag_payload(node.rel.clone());
    }
    if is_folder
        && let Some(from) = resp.dnd_release_payload::<PathBuf>()
        && *from != node.rel
    {
        cmds.push(Cmd::Move { from: (*from).clone(), to: node.rel.clone() });
    }

    let new_tab = ui.input(|i| i.modifiers.command);
    if resp.clicked() {
        match node.kind {
            NodeKind::Folder => {
                if is_open {
                    expanded.remove(&node.rel);
                } else {
                    expanded.insert(node.rel.clone());
                }
            }
            NodeKind::Note => cmds.push(Cmd::Open { rel: node.rel.clone(), new_tab, tab: None }),
            // Картинки открываются вкладкой, остальные файлы — в проводнике.
            NodeKind::File if forma_images::is_image(&node.rel) => cmds.push(Cmd::Open { rel: node.rel.clone(), new_tab, tab: None }),
            NodeKind::File => cmds.push(Cmd::Reveal(node.rel.clone())),
        }
    }
    let openable = matches!(node.kind, NodeKind::Note) || (matches!(node.kind, NodeKind::File) && forma_images::is_image(&node.rel));
    if resp.middle_clicked() && openable {
        cmds.push(Cmd::Open { rel: node.rel.clone(), new_tab: true, tab: None });
    }
    theme::context_menu(&resp, |ui| match node.kind {
        NodeKind::Folder => {
            if ui.button(t!("action-new-note")).clicked() {
                cmds.push(Cmd::NewNoteDialog { folder: node.rel.clone() });
            }
            if ui.button(t!("action-new-folder")).clicked() {
                cmds.push(Cmd::NewFolderDialog { parent: node.rel.clone() });
            }
            ui.separator();
            if ui.button(t!("action-rename")).clicked() {
                cmds.push(Cmd::RenameDialog(node.rel.clone()));
            }
            if ui.button(t!("action-reveal")).clicked() {
                cmds.push(Cmd::Reveal(node.rel.clone()));
            }
            ui.separator();
            if ui.button(RichText::new(t!("action-delete")).color(pal.danger)).clicked() {
                cmds.push(Cmd::DeleteDialog(node.rel.clone()));
            }
        }
        NodeKind::Note => {
            if ui.button(t!("action-open-new-tab")).clicked() {
                cmds.push(Cmd::Open { rel: node.rel.clone(), new_tab: true, tab: None });
            }
            if ui.button(t!("action-rename")).clicked() {
                cmds.push(Cmd::RenameDialog(node.rel.clone()));
            }
            if ui.button(t!("action-reveal")).clicked() {
                cmds.push(Cmd::Reveal(node.rel.clone()));
            }
            ui.separator();
            if ui.button(RichText::new(t!("action-delete")).color(pal.danger)).clicked() {
                cmds.push(Cmd::DeleteDialog(node.rel.clone()));
            }
        }
        NodeKind::File => {
            if openable && ui.button(t!("action-open-new-tab")).clicked() {
                cmds.push(Cmd::Open { rel: node.rel.clone(), new_tab: true, tab: None });
            }
            if ui.button(t!("action-reveal")).clicked() {
                cmds.push(Cmd::Reveal(node.rel.clone()));
            }
            if ui.button(RichText::new(t!("action-delete")).color(pal.danger)).clicked() {
                cmds.push(Cmd::DeleteDialog(node.rel.clone()));
            }
        }
    });

    if is_folder {
        state.show_body_unindented(ui, |ui| {
            for child in &node.children {
                tree_ui(ui, child, depth + 1, expanded, active, pal, cmds);
            }
        });
        state.store(ui.ctx());
    }
}
