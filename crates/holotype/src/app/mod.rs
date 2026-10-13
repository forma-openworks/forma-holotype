//! Главное окно Forma: Holotype на оболочке платформы Forma.
//!
//! Шапка, меню, рабочие пространства, окно настроек и диалоги платформы — у оболочки
//! (`forma_shell`). Holotype рисует внутри неё свои вкладки: дерево файлов, поиск, теги,
//! заметки, граф, оглавление, обратные ссылки и сведения о заметке.
//!
//! Раскладка принадлежит хранилищу и лежит в его `.holotype/workspace.json`, рядом с
//! раскладкой графа. Без хранилища показывается экран приветствия.

mod chrome;
mod docs;
mod modals;
mod note;
mod panes;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Ui};
use forma_i18n::t;
use forma_layout::SurfaceKey;
use forma_shell::{Shell, ShellCmd, reveal_in_explorer};

use crate::actions;
use crate::export;
use crate::graph::Graph;
use crate::markdown;
use crate::sample;
use crate::settings::HolotypeSettings;
use crate::tabs::{Pane, Space, Tab, ViewMode};
use crate::theme::Palette;
use crate::vault::{SearchHit, Vault};
pub use docs::{Docs, NoteView};
use modals::Modal;
pub use panes::Viewer;

/// Действия, которые копятся за кадр и выполняются в его конце.
pub enum Cmd {
    /// Открыть заметку: в текущей вкладке заметки, во вкладке `tab` или в новой.
    Open { rel: PathBuf, new_tab: bool, tab: Option<u64> },
    /// Перейти по ссылке из заметки `from` (вкладка `tab`).
    OpenLink { target: String, new_tab: bool, from: Option<PathBuf>, tab: Option<u64> },
    /// Открыть заметку и поставить курсор редактора на смещение.
    JumpTo { rel: PathBuf, offset: usize },
    /// Прокрутить вкладку заметки к заголовку.
    Heading { tab: u64, text: String, offset: usize },
    NewNoteDialog { folder: PathBuf },
    NewFolderDialog { parent: PathBuf },
    RenameDialog(PathBuf),
    DeleteDialog(PathBuf),
    Move { from: PathBuf, to: PathBuf },
    ToggleTask { rel: PathBuf, range: Range<usize> },
    SearchTag(String),
    OpenUrl(String),
    SetMode { tab: Option<u64>, mode: ViewMode },
    Back(Option<u64>),
    Forward(Option<u64>),
    /// Показать вкладку панели (а если её нет — открыть).
    ShowPane(Pane),
    QuickOpen,
    CloseNote,
    /// Следующая (`true`) или предыдущая заметка в области текущей.
    CycleNote(bool),
    OpenVaultDialog,
    OpenVault(PathBuf),
    CreateDemo,
    ExportNote,
    ExportSite,
    Reveal(PathBuf),
    Rescan,
    DailyNote,
    SaveAll,
    Shell(ShellCmd<Space>),
}

/// Дерево файлов хранилища.
#[derive(Clone)]
pub enum NodeKind {
    Folder,
    Note,
    File,
}

pub struct TreeNode {
    pub name: String,
    pub rel: PathBuf,
    pub kind: NodeKind,
    pub children: Vec<TreeNode>,
}

/// Состояние панели файлов.
#[derive(Default)]
pub struct FilesState {
    pub tree: Option<(u64, TreeNode)>,
    pub expanded: HashSet<PathBuf>,
}

/// Состояние поиска по заметкам.
#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub key: Option<(String, u64, u64)>,
    pub focus: bool,
}

pub struct HolotypeApp {
    pub(crate) shell: Shell<HolotypeSettings, Space>,
    /// Цвета заметок текущей темы и номер темы, из которой они посчитаны.
    pal: Palette,
    pal_gen: u64,
    vault: Option<Vault>,
    docs: Docs,
    /// Состояние вкладок заметок: история, прокрутка, курсор, подсказки.
    views: HashMap<u64, NoteView>,
    /// Вкладка заметки, с которой работали последней.
    current: Option<u64>,
    graph: Graph,
    files: FilesState,
    search: SearchState,
    backlinks: Option<((u64, u64, PathBuf), Vec<(usize, String)>)>,
    modal: Option<Modal>,
    modal_focus: bool,
    cmds: Vec<Cmd>,
    was_focused: bool,
}

/// Что Holotype помнит об интерфейсе хранилища рядом с раскладкой.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct ViewState {
    /// Раскрытые папки дерева (пути от корня хранилища, через «/»).
    expanded: Vec<String>,
}

impl HolotypeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, mut shell: Shell<HolotypeSettings, Space>, vault_arg: Option<PathBuf>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        crate::highlight::warm_up();
        let pal = Palette::from_theme(&shell.theme);
        let pal_gen = shell.theme_gen;
        shell.welcome = true;
        let mut app = Self {
            shell,
            pal,
            pal_gen,
            vault: None,
            docs: Docs::default(),
            views: HashMap::new(),
            current: None,
            graph: Graph::default(),
            files: FilesState::default(),
            search: SearchState::default(),
            backlinks: None,
            modal: None,
            modal_focus: false,
            cmds: Vec::new(),
            was_focused: true,
        };
        // Хранилище: из аргумента командной строки или последнее открытое.
        match vault_arg {
            Some(path) => {
                if !path.exists()
                    && let Err(e) = sample::create_demo(&path)
                {
                    app.shell.toast(t!("toast-vault-create-failed", error = e.to_string()), true);
                }
                app.open_vault(path);
            }
            None => {
                if let Some(path) = app.shell.app_settings.vaults.last.clone().filter(|p| p.is_dir()) {
                    app.open_vault(path);
                }
            }
        }
        app
    }

    fn notes(&self) -> &crate::settings::Notes {
        &self.shell.app_settings.notes
    }

    /// Файл раскладки хранилища.
    fn layout_file(root: &Path) -> PathBuf {
        root.join(".holotype").join("workspace.json")
    }

    fn save_view(&mut self) {
        let view = ViewState { expanded: self.files.expanded.iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect() };
        self.shell.view = serde_json::to_value(view).unwrap_or_default();
    }

    fn open_vault(&mut self, path: PathBuf) {
        self.save_all();
        match Vault::open(&path) {
            Ok(v) => {
                self.save_view();
                self.shell.app_settings.vaults.remember(&path);
                self.shell.switch_layout(Some(Self::layout_file(&path)));
                let view: ViewState = serde_json::from_value(self.shell.view.clone()).unwrap_or_default();
                self.files = FilesState { tree: None, expanded: view.expanded.into_iter().map(PathBuf::from).collect() };
                self.search = SearchState::default();
                self.backlinks = None;
                self.docs = Docs::default();
                self.views.clear();
                self.current = None;
                self.graph = Graph::default();
                self.vault = Some(v);
                self.shell.welcome = false;
                // Заметок в раскладке нет — открываем приветственную, если она есть.
                if !self.shell.layout.all_tabs().any(|t| t.note.is_some()) {
                    let first = self.vault.as_ref().and_then(|v| {
                        v.notes
                            .iter()
                            .find(|n| {
                                let name = n.name.to_lowercase();
                                name.contains("добро пожаловать") || name == "readme" || name == "welcome"
                            })
                            .or_else(|| v.notes.first())
                            .map(|n| n.rel.clone())
                    });
                    if let Some(rel) = first {
                        self.open_note(rel, false, None);
                    }
                }
            }
            Err(e) => self.shell.toast(t!("toast-vault-open-failed", error = e.to_string()), true),
        }
    }

    // ----------------------------------------------------------------------------------------
    // Заметки
    // ----------------------------------------------------------------------------------------

    fn save_all(&mut self) {
        let Some(vault) = &mut self.vault else { return };
        let now = self.shell.now();
        for err in self.docs.save_all(vault, now) {
            self.shell.toast(err, true);
        }
    }

    /// Вкладка заметки, с которой работали последней, и её заметка.
    fn current_note(&self) -> Option<(u64, Option<PathBuf>)> {
        let id = self.current?;
        self.shell.layout.all_tabs().find(|t| t.id == id && t.pane == Pane::Note).map(|t| (t.id, t.note.clone()))
    }

    fn current_rel(&self) -> Option<PathBuf> {
        self.current_note().and_then(|(_, rel)| rel)
    }

    /// Какая вкладка заметки текущая: активная во вкладках области, где работали последней.
    fn update_current(&mut self) {
        let layout = &self.shell.layout;
        let focused = layout
            .dock
            .focused(SurfaceKey::Main)
            .and_then(|a| layout.surface().area(a))
            .and_then(|a| a.active_tab())
            .filter(|t| t.pane == Pane::Note)
            .map(|t| t.id);
        if focused.is_some() {
            self.current = focused;
        }
        let alive = self.current.is_some_and(|id| layout.all_tabs().any(|t| t.id == id && t.pane == Pane::Note));
        if !alive {
            self.current = layout.visible_tabs().find(|t| t.pane == Pane::Note).map(|t| t.id);
        }
    }

    fn expand_to(&mut self, rel: &Path) {
        let mut p = rel.parent();
        while let Some(d) = p {
            if d.as_os_str().is_empty() {
                break;
            }
            self.files.expanded.insert(d.to_path_buf());
            p = d.parent();
        }
    }

    /// Открывает заметку: уже открытую — показывает, иначе заменяет заметку во вкладке
    /// `tab` (или в текущей вкладке заметки), а с `new_tab` — открывает новую вкладку.
    fn open_note(&mut self, rel: PathBuf, new_tab: bool, tab: Option<u64>) {
        let Some(vault) = &self.vault else { return };
        if vault.index_of(&rel).is_none() {
            return;
        }
        self.expand_to(&rel);
        let layout = &mut self.shell.layout;
        // 1. Вкладка с этой заметкой уже есть.
        let mut shown = None;
        if layout.show_tab(|t| {
            let hit = t.note.as_ref() == Some(&rel);
            if hit {
                shown = Some(t.id);
            }
            hit
        }) {
            self.current = shown;
            return;
        }
        // 2. Заменяем заметку во вкладке, с которой работали (история — назад/вперёд).
        let target = if new_tab { None } else { tab.or(self.current) };
        if let Some(id) = target {
            let mut old = None;
            let replaced = layout.update_tab(
                |t| t.id == id && t.pane == Pane::Note,
                |t| {
                    old = t.note.replace(rel.clone());
                },
            );
            if replaced {
                let view = self.views.entry(id).or_default();
                if let Some(old) = old {
                    view.back.push(old);
                    view.forward.clear();
                }
                view.reset_position();
                self.current = Some(id);
                return;
            }
        }
        // 3. Пустая вкладка заметки ждёт — заполняем её.
        let mut filled = None;
        if !new_tab
            && layout.update_tab(
                |t| t.pane == Pane::Note && t.note.is_none(),
                |t| {
                    t.note = Some(rel.clone());
                    filled = Some(t.id);
                },
            )
        {
            self.current = filled;
            return;
        }
        // 4. Новая вкладка рядом с другими заметками.
        let id = layout.next_id();
        layout.open_near(Tab::note(id, rel), |t| t.pane == Pane::Note);
        self.current = Some(id);
    }

    fn open_link(&mut self, target: &str, new_tab: bool, from: Option<PathBuf>, tab: Option<u64>) {
        let Some(vault) = &self.vault else { return };
        let from = from.or_else(|| self.current_rel());
        let heading = markdown::link_heading_part(target).map(str::to_owned);
        if target.starts_with('#') {
            if let (Some(h), Some(id)) = (heading, tab.or(self.current)) {
                let offset = from
                    .as_ref()
                    .and_then(|rel| self.docs.get_or_load(vault, rel))
                    .and_then(|d| d.parsed().headings.iter().find(|x| markdown::same_heading(&x.text, &h)).map(|x| x.offset));
                let view = self.views.entry(id).or_default();
                view.preview_scroll = Some(h.to_lowercase());
                view.editor_jump = offset;
            }
            return;
        }
        match vault.resolve(target, from.as_deref()) {
            Some(idx) => {
                let rel = vault.notes[idx].rel.clone();
                let offset = heading.as_ref().and_then(|h| vault.notes[idx].headings.iter().find(|x| markdown::same_heading(&x.text, h)).map(|x| x.offset));
                self.open_note(rel, new_tab, tab);
                if let (Some(h), Some(id)) = (heading, self.current) {
                    let view = self.views.entry(id).or_default();
                    view.preview_scroll = Some(h.to_lowercase());
                    view.editor_jump = offset;
                }
            }
            None => {
                // Заметки ещё нет — создаём её (как в Obsidian).
                let part = markdown::link_note_part(target).replace('\\', "/");
                let (folder, name) = match part.rsplit_once('/') {
                    Some((f, n)) => (PathBuf::from(f), n.to_owned()),
                    None => (PathBuf::new(), part.clone()),
                };
                self.create_note(folder, &name, new_tab);
            }
        }
    }

    fn create_note(&mut self, folder: PathBuf, name: &str, new_tab: bool) {
        let Some(vault) = &mut self.vault else { return };
        let untitled = t!("note-new-name");
        let name = if name.trim().is_empty() { untitled.as_str() } else { name.trim() };
        let content = format!("# {}\n\n", crate::vault::sanitize_name(name));
        match vault.create_note(&folder, name, &content) {
            Ok(rel) => {
                self.open_note(rel.clone(), new_tab, None);
                self.start_writing(&rel, None);
            }
            Err(e) => self.shell.toast(t!("toast-note-create-failed", error = e.to_string()), true),
        }
    }

    /// Переводит текущую вкладку в редактор и ставит курсор: в конец или на `offset`.
    fn start_writing(&mut self, rel: &Path, offset: Option<usize>) {
        let Some(id) = self.current else { return };
        let Some(vault) = &self.vault else { return };
        let len = self.docs.get_or_load(vault, rel).map(|d| d.text.len()).unwrap_or(0);
        let default_mode = self.notes().default_mode;
        self.shell.layout.update_tab(
            |t| t.id == id,
            |t| {
                if t.mode.unwrap_or(default_mode) == ViewMode::Preview {
                    t.mode = Some(ViewMode::Edit);
                }
            },
        );
        let view = self.views.entry(id).or_default();
        view.editor_jump = Some(offset.unwrap_or(len));
        view.focus_editor = true;
    }

    /// Папка, в которой создаются новые заметки: папка текущей заметки.
    fn current_folder(&self) -> PathBuf {
        self.current_rel().and_then(|r| r.parent().map(Path::to_path_buf)).unwrap_or_default()
    }

    /// Закрывает вкладку по номеру (как крестик на вкладке): заметка сохраняется.
    fn close_tab(&mut self, id: u64) {
        let mut closed: Option<Tab> = None;
        let mut take = |s: &mut forma_layout::Surface<Tab>| {
            if closed.is_some() {
                return;
            }
            let found = s.areas().into_iter().find_map(|a| a.tabs.iter().position(|t| t.id == id).map(|i| (a.id, i)));
            if let Some((area, i)) = found {
                closed = s.remove_tab(area, i);
            }
        };
        let layout = &mut self.shell.layout;
        let active = layout.active;
        take(&mut layout.workspaces[active].surface);
        for w in &mut layout.dock.windows {
            take(&mut w.surface);
        }
        if let Some(tab) = closed {
            if let (Some(rel), Some(vault)) = (&tab.note, &mut self.vault) {
                let now = self.shell.now();
                if let Err(e) = self.docs.save(vault, rel, now) {
                    self.shell.toast(e, true);
                }
            }
            self.views.remove(&tab.id);
        }
    }

    /// Показывает вкладку панели; одиночную панель, которой нет, открывает.
    fn show_pane(&mut self, pane: Pane) {
        let layout = &mut self.shell.layout;
        if !pane.many() && layout.show_tab(|t| t.pane == pane) {
            return;
        }
        if pane == Pane::Graph && layout.show_tab(|t| t.pane == Pane::Graph) {
            self.graph.reheat();
            return;
        }
        let id = layout.next_id();
        layout.dock.open_tab(Tab::new(pane, id));
        if pane == Pane::Graph {
            self.graph.reheat();
        }
    }

    fn set_mode(&mut self, tab: Option<u64>, mode: ViewMode) {
        let Some(id) = tab.or(self.current) else { return };
        self.shell.layout.update_tab(|t| t.id == id && t.pane == Pane::Note, |t| t.mode = Some(mode));
        if mode != ViewMode::Preview {
            self.views.entry(id).or_default().focus_editor = true;
        }
    }

    fn mode_of(&self, id: u64) -> ViewMode {
        let default = self.notes().default_mode;
        self.shell.layout.all_tabs().find(|t| t.id == id).and_then(|t| t.mode).unwrap_or(default)
    }

    fn history(&mut self, tab: Option<u64>, back: bool) {
        let Some(id) = tab.or(self.current) else { return };
        let Some(vault) = &self.vault else { return };
        let view = self.views.entry(id).or_default();
        let target = if back { view.back.pop() } else { view.forward.pop() };
        let Some(target) = target else { return };
        if vault.index_of(&target).is_none() {
            return;
        }
        let mut old = None;
        self.shell.layout.update_tab(|t| t.id == id, |t| old = t.note.replace(target.clone()));
        let view = self.views.entry(id).or_default();
        if let Some(old) = old {
            if back {
                view.forward.push(old);
            } else {
                view.back.push(old);
            }
        }
        view.reset_position();
        self.current = Some(id);
        self.expand_to(&target);
    }

    /// Следующая или предыдущая вкладка в области текущей заметки.
    fn cycle_note(&mut self, forward: bool) {
        let Some(id) = self.current else { return };
        let mut next = None;
        self.shell.layout.surface_mut().for_each_area_mut(|a| {
            if let Some(i) = a.tabs.iter().position(|t| t.id == id) {
                let n = a.tabs.len();
                let j = if forward { (i + 1) % n } else { (i + n - 1) % n };
                a.active = j;
                next = Some(a.tabs[j].id);
            }
        });
        if let Some(next) = next
            && self.shell.layout.all_tabs().any(|t| t.id == next && t.pane == Pane::Note)
        {
            self.current = Some(next);
        }
    }

    fn toggle_task(&mut self, rel: &Path, range: Range<usize>) {
        let Some(vault) = &self.vault else { return };
        let now = self.shell.now();
        let Some(doc) = self.docs.get_or_load(vault, rel) else { return };
        let replacement = match doc.text.get(range.clone()) {
            Some("[ ]") => "[x]",
            Some("[x]" | "[X]") => "[ ]",
            _ => return,
        };
        doc.text.replace_range(range, replacement);
        doc.mark_edited(now);
        // Сохраняем почти сразу, чтобы щелчок по галочке ощущался надёжным.
        doc.last_edit = now - 0.8;
    }

    fn daily_note(&mut self) {
        let Some(vault) = &mut self.vault else { return };
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        // Папка ежедневных заметок: уже существующая или на языке интерфейса.
        let folder = ["Ежедневник", "Daily"].iter().map(PathBuf::from).find(|d| vault.folders.contains(d)).unwrap_or_else(|| PathBuf::from(t!("daily-folder")));
        let rel = folder.join(format!("{today}.md"));
        if vault.index_of(&rel).is_some() {
            self.open_note(rel, false, None);
            return;
        }
        // Пробел в конце строки шаблона в .ftl легко потерять, а курсор встаёт после «- [ ] ».
        let content = t!("daily-template", date = today.clone()).replace("- [ ]
", "- [ ] 
");
        match vault.create_note(&folder, &today, &format!("{content}\n")) {
            Ok(rel) => {
                let offset = content.find("- [ ] ").map(|p| p + 6);
                self.open_note(rel.clone(), false, None);
                self.start_writing(&rel, offset);
            }
            Err(e) => self.shell.toast(t!("toast-note-create-failed", error = e.to_string()), true),
        }
    }

    fn rename(&mut self, target: &Path, name: &str) {
        self.save_all();
        let Some(v) = &mut self.vault else { return };
        match v.rename(target, name) {
            Ok((new_rel, changed)) => {
                self.docs.moved(target, &new_rel);
                self.docs.reload_changed(v, &changed);
                retarget_tabs(&mut self.shell.layout, target, &new_rel);
                if !changed.is_empty() {
                    self.shell.toast(t!("toast-links-updated", count = changed.len()), false);
                }
            }
            Err(e) => self.shell.toast(t!("toast-rename-failed", error = e.to_string()), true),
        }
    }

    fn delete(&mut self, target: &Path) {
        self.save_all();
        let Some(v) = &mut self.vault else { return };
        match v.delete(target) {
            Ok(()) => {
                self.docs.removed(target);
                let gone: Vec<u64> = self.shell.layout.all_tabs().filter(|t| t.note.as_ref().is_some_and(|r| r.starts_with(target))).map(|t| t.id).collect();
                for id in gone {
                    self.close_tab(id);
                }
                self.shell.toast(t!("toast-moved-to-trash"), false);
            }
            Err(e) => self.shell.toast(t!("toast-delete-failed", error = e), true),
        }
    }

    fn move_item(&mut self, from: &Path, to: &Path) {
        self.save_all();
        let Some(vault) = &mut self.vault else { return };
        match vault.move_to(from, to) {
            Ok(new_rel) => {
                self.docs.moved(from, &new_rel);
                retarget_tabs(&mut self.shell.layout, from, &new_rel);
                self.files.expanded.insert(to.to_path_buf());
            }
            Err(e) => self.shell.toast(t!("toast-move-failed", error = e.to_string()), true),
        }
    }

    fn rescan(&mut self) {
        let Some(vault) = &mut self.vault else { return };
        vault.rescan();
        // Подтягиваем изменения с диска в заметки без несохранённых правок.
        self.docs.reload(vault);
    }

    fn export_note(&mut self, ctx: &egui::Context) {
        self.save_all();
        let (Some(vault), Some(rel)) = (&self.vault, self.current_rel()) else {
            self.shell.toast(t!("toast-open-note-first"), true);
            return;
        };
        let Some(idx) = vault.index_of(&rel) else { return };
        let name = rel.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let file = rfd::FileDialog::new().set_title(t!("dialog-export-note")).set_file_name(format!("{name}.html")).add_filter("HTML", &["html"]).save_file();
        if let Some(file) = file {
            match export::export_note(vault, idx, &file) {
                Ok(()) => {
                    ctx.open_url(egui::OpenUrl::new_tab(file_url(&file)));
                    self.shell.toast(t!("toast-export-done", path = file.display().to_string()), false);
                }
                Err(e) => self.shell.toast(t!("toast-export-failed", error = e.to_string()), true),
            }
        }
    }

    fn export_site(&mut self, ctx: &egui::Context) {
        self.save_all();
        let Some(vault) = &self.vault else { return };
        let Some(dir) = rfd::FileDialog::new().set_title(t!("dialog-export-site")).pick_folder() else { return };
        match export::export_site(vault, &dir) {
            Ok(pages) => {
                ctx.open_url(egui::OpenUrl::new_tab(file_url(&dir.join("index.html"))));
                self.shell.toast(t!("toast-site-done", count = pages, path = dir.display().to_string()), false);
            }
            Err(e) => self.shell.toast(t!("toast-export-failed", error = e.to_string()), true),
        }
    }

    // ----------------------------------------------------------------------------------------
    // Команды
    // ----------------------------------------------------------------------------------------

    /// Команда действия Holotype (общие действия выполняет оболочка).
    fn action_cmd(&self, id: &str) -> Option<Cmd> {
        use actions::*;
        Some(match id {
            NEW_NOTE => Cmd::NewNoteDialog { folder: self.current_folder() },
            NEW_FOLDER => Cmd::NewFolderDialog { parent: self.current_folder() },
            DAILY_NOTE => Cmd::DailyNote,
            SAVE_ALL => Cmd::SaveAll,
            OPEN_VAULT => Cmd::OpenVaultDialog,
            CREATE_DEMO => Cmd::CreateDemo,
            RESCAN => Cmd::Rescan,
            REVEAL => Cmd::Reveal(self.current_rel().unwrap_or_default()),
            EXPORT_NOTE => Cmd::ExportNote,
            EXPORT_SITE => Cmd::ExportSite,
            QUICK_OPEN => Cmd::QuickOpen,
            SEARCH => Cmd::ShowPane(Pane::Search),
            BACK => Cmd::Back(None),
            FORWARD => Cmd::Forward(None),
            // Ctrl+E: чтение ⇄ правка.
            TOGGLE_MODE => {
                let id = self.current?;
                let mode = if self.mode_of(id) == ViewMode::Preview { ViewMode::Edit } else { ViewMode::Preview };
                Cmd::SetMode { tab: Some(id), mode }
            }
            SPLIT => Cmd::SetMode { tab: None, mode: ViewMode::Split },
            GRAPH => Cmd::ShowPane(Pane::Graph),
            RENAME => Cmd::RenameDialog(self.current_rel()?),
            CLOSE_NOTE => Cmd::CloseNote,
            NEXT_NOTE => Cmd::CycleNote(true),
            PREV_NOTE => Cmd::CycleNote(false),
            _ => return None,
        })
    }

    fn process_cmds(&mut self, ctx: &egui::Context) {
        while !self.cmds.is_empty() {
            for cmd in std::mem::take(&mut self.cmds) {
                self.run(ctx, cmd);
            }
        }
    }

    fn run(&mut self, ctx: &egui::Context, cmd: Cmd) {
        match cmd {
            Cmd::Open { rel, new_tab, tab } => self.open_note(rel, new_tab, tab),
            Cmd::OpenLink { target, new_tab, from, tab } => self.open_link(&target, new_tab, from, tab),
            Cmd::JumpTo { rel, offset } => {
                self.open_note(rel.clone(), false, None);
                if let Some(id) = self.current {
                    if self.mode_of(id) == ViewMode::Preview {
                        self.set_mode(Some(id), ViewMode::Split);
                    }
                    self.views.entry(id).or_default().editor_jump = Some(offset);
                }
            }
            Cmd::Heading { tab, text, offset } => {
                let view = self.views.entry(tab).or_default();
                view.preview_scroll = Some(text);
                view.editor_jump = Some(offset);
            }
            Cmd::NewNoteDialog { folder } => {
                if self.vault.is_some() {
                    self.modal = Some(Modal::NewNote { folder, name: t!("note-new-name") });
                    self.modal_focus = true;
                }
            }
            Cmd::NewFolderDialog { parent } => {
                if self.vault.is_some() {
                    self.modal = Some(Modal::NewFolder { parent, name: t!("folder-new-name") });
                    self.modal_focus = true;
                }
            }
            Cmd::RenameDialog(target) => {
                let is_note = self.vault.as_ref().is_some_and(|v| v.index_of(&target).is_some());
                let name = if is_note { target.file_stem() } else { target.file_name() };
                let name = name.map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                self.modal = Some(Modal::Rename { target, name });
                self.modal_focus = true;
            }
            Cmd::DeleteDialog(target) => self.modal = Some(Modal::Delete { target }),
            Cmd::Move { from, to } => self.move_item(&from, &to),
            Cmd::ToggleTask { rel, range } => self.toggle_task(&rel, range),
            Cmd::SearchTag(tag) => {
                self.search.query = format!("#{tag}");
                self.show_pane(Pane::Search);
            }
            Cmd::OpenUrl(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
            Cmd::SetMode { tab, mode } => self.set_mode(tab, mode),
            Cmd::Back(tab) => self.history(tab, true),
            Cmd::Forward(tab) => self.history(tab, false),
            Cmd::ShowPane(pane) => {
                self.show_pane(pane);
                if pane == Pane::Search {
                    self.search.focus = true;
                }
            }
            Cmd::QuickOpen => {
                if self.vault.is_some() {
                    self.modal = Some(Modal::QuickOpen { query: String::new(), selected: 0 });
                    self.modal_focus = true;
                }
            }
            Cmd::CloseNote => {
                if let Some(id) = self.current {
                    self.close_tab(id);
                }
            }
            Cmd::CycleNote(forward) => self.cycle_note(forward),
            Cmd::OpenVaultDialog => {
                if let Some(path) = rfd::FileDialog::new().set_title(t!("dialog-open-vault")).pick_folder() {
                    self.open_vault(path);
                }
            }
            Cmd::OpenVault(path) => self.open_vault(path),
            Cmd::CreateDemo => {
                let base = dirs::document_dir().unwrap_or_else(|| PathBuf::from("."));
                let path = base.join(t!("demo-vault-name"));
                match sample::create_demo(&path) {
                    Ok(()) => self.open_vault(path),
                    Err(e) => self.shell.toast(t!("toast-demo-failed", error = e.to_string()), true),
                }
            }
            Cmd::ExportNote => self.export_note(ctx),
            Cmd::ExportSite => self.export_site(ctx),
            Cmd::Reveal(rel) => {
                if let Some(v) = &self.vault {
                    reveal_in_explorer(&v.abs(&rel));
                }
            }
            Cmd::Rescan => self.rescan(),
            Cmd::DailyNote => self.daily_note(),
            Cmd::SaveAll => {
                self.save_all();
                self.shell.toast(t!("toast-saved"), false);
            }
            Cmd::Shell(cmd) => self.shell.cmds.push(cmd),
        }
    }

    // ----------------------------------------------------------------------------------------
    // Кадр
    // ----------------------------------------------------------------------------------------

    /// Фоновая работа кадра: автосохранение, файлы, изменённые снаружи, перетащенные файлы.
    fn background(&mut self, ctx: &egui::Context) {
        let now = self.shell.now();
        if let Some(vault) = &mut self.vault {
            let (errors, waiting) = self.docs.autosave(vault, now);
            for err in errors {
                self.shell.toast(err, true);
            }
            if waiting {
                ctx.request_repaint_after_secs(0.4);
            }
        }
        // Вернулись в окно — файлы могли поменять в другой программе.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.was_focused {
            self.rescan();
        }
        if !focused && self.was_focused {
            self.save_all();
        }
        self.was_focused = focused;

        // Перетащенные в окно файлы копируем во вложения и вставляем ссылку в заметку.
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if !dropped.is_empty() {
            self.insert_files(&dropped);
        }

        // Заметки без вкладок больше не нужны.
        let open: HashSet<PathBuf> = self.shell.layout.all_tabs().filter_map(|t| t.note.clone()).collect();
        if let Some(vault) = &mut self.vault {
            for err in self.docs.retain_open(vault, &open, now) {
                self.shell.toast(err, true);
            }
        }
        let tabs: HashSet<u64> = self.shell.layout.all_tabs().map(|t| t.id).collect();
        self.views.retain(|id, _| tabs.contains(id));

        // Цвета заметок — из текущей темы, акцентный цвет — из настроек «Заметки».
        let dark = self.shell.theme.pal.dark;
        let mut overrides = std::collections::BTreeMap::new();
        if let Some(a) = self.notes().accent {
            overrides.insert("accent".to_owned(), a.hex(dark));
        }
        if self.shell.theme_overrides != overrides {
            self.shell.theme_overrides = overrides;
        }
        if self.pal_gen != self.shell.theme_gen {
            self.pal = Palette::from_theme(&self.shell.theme);
            self.pal_gen = self.shell.theme_gen;
        }
    }

    fn insert_files(&mut self, files: &[PathBuf]) {
        let Some(vault) = &mut self.vault else { return };
        let mut snippet = String::new();
        for f in files {
            if f.is_dir() {
                continue;
            }
            match vault.import_attachment(f) {
                Ok(rel) => {
                    let name = rel.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let is_image = markdown::is_image_path(&name);
                    snippet.push_str(&format!("{}[[{}]]\n", if is_image { "!" } else { "" }, name));
                }
                Err(e) => {
                    self.shell.toast(t!("toast-file-add-failed", error = e.to_string()), true);
                    return;
                }
            }
        }
        if snippet.is_empty() {
            return;
        }
        let now = self.shell.now();
        let target = self.current.and_then(|id| self.current_rel().map(|rel| (id, rel)));
        match target {
            Some((id, rel)) => {
                let Some(vault) = &self.vault else { return };
                let Some(doc) = self.docs.get_or_load(vault, &rel) else { return };
                let view = self.views.entry(id).or_default();
                let at = view.cursor.map(|c| char_to_byte(&doc.text, c)).unwrap_or(doc.text.len());
                let prefix = if at > 0 && !doc.text[..at].ends_with('\n') { "\n" } else { "" };
                let insert = format!("{prefix}{snippet}");
                doc.text.insert_str(at, &insert);
                view.cursor = Some(byte_to_char(&doc.text, at + insert.len()));
                doc.mark_edited(now);
                self.shell.toast(t!("toast-file-added", folder = vault.attachments_dir().display().to_string()), false);
            }
            None => self.shell.toast(t!("toast-file-copied"), false),
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let app = crate::SPEC.app.title;
        let title = match (&self.vault, self.current_rel()) {
            (Some(v), Some(rel)) => {
                let name = rel.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                format!("{name} — {} — {app}", v.name())
            }
            (Some(v), None) => format!("{} — {app}", v.name()),
            _ => app.to_owned(),
        };
        self.shell.set_title(ctx, &title);
    }

    fn persist(&mut self) {
        self.save_all();
        self.save_view();
        self.shell.persist();
    }
}

impl eframe::App for HolotypeApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shell.begin_frame(&ctx);
        self.shell.app_modal = self.modal.is_some();
        self.shell.handle_keys(&ctx);
        self.background(&ctx);
        self.update_current();

        if self.vault.is_none() {
            self.shell.welcome = true;
        }
        if self.shell.welcome {
            self.welcome_ui(ui);
        } else {
            self.top_bar(ui);
            self.status_bar(ui);
            self.dock_ui(ui);
            self.drop_hint(&ctx);
        }
        self.shell.end_frame(&ctx, |ctx, shell| {
            let _ = (ctx, shell);
        });
        self.app_modals(&ctx);
        for id in self.shell.process(&ctx) {
            if let Some(cmd) = self.action_cmd(&id) {
                self.cmds.push(cmd);
            }
        }
        self.process_cmds(&ctx);
        self.update_title(&ctx);
    }

    /// eframe вызывает это раз в полминуты и при выходе.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
    }

    fn on_exit(&mut self) {
        self.persist();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.shell.pal.window_bg.to_normalized_gamma_f32()
    }
}

/// Вкладки заметок после переименования или переноса `from` → `to` (файла или папки).
fn retarget_tabs(layout: &mut forma_shell::Layout<Space>, from: &Path, to: &Path) {
    let fix = |s: &mut forma_layout::Surface<Tab>| {
        s.for_each_area_mut(|a| {
            for t in &mut a.tabs {
                if let Some(rel) = &mut t.note {
                    if rel == from {
                        *rel = to.to_path_buf();
                    } else if let Ok(rest) = rel.strip_prefix(from) {
                        *rel = to.join(rest);
                    }
                }
            }
        });
    };
    for w in &mut layout.workspaces {
        fix(&mut w.surface);
    }
    for w in &mut layout.dock.windows {
        fix(&mut w.surface);
    }
}

pub(crate) fn file_url(path: &Path) -> String {
    format!("file:///{}", path.to_string_lossy().replace('\\', "/").trim_start_matches('/'))
}

pub(crate) fn char_to_byte(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

pub(crate) fn byte_to_char(s: &str, bi: usize) -> usize {
    let mut b = bi.min(s.len());
    while !s.is_char_boundary(b) {
        b -= 1;
    }
    s[..b].chars().count()
}
