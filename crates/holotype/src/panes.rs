//! Редакторы областей и встроенные рабочие пространства Holotype.
//!
//! Как в Blender, в каждой области один редактор, а его вид выбирается в меню слева
//! в заголовке: заметка, файлы, поиск, теги, граф, оглавление, обратные ссылки, сведения.
//! У редактора заметок свои вкладки — открытые заметки.

use std::path::{Path, PathBuf};

use forma_i18n::t;
use forma_layout::{Node, SplitDir, Surface};
use forma_shell::{Builtin, Ids, Layout, ShellEditor};
use serde::{Deserialize, Serialize};

/// Как показана заметка: редактор, сплит или чтение.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewMode {
    Edit,
    Split,
    Preview,
}

impl ViewMode {
    pub fn title(self) -> String {
        match self {
            ViewMode::Edit => t!("mode-edit"),
            ViewMode::Split => t!("mode-split"),
            ViewMode::Preview => t!("mode-preview"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ViewMode::Edit => "write",
            ViewMode::Split => "split",
            ViewMode::Preview => "read",
        }
    }
}

/// Вид редактора области.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Files,
    Search,
    Tags,
    Note,
    Graph,
    Outline,
    Backlinks,
    Info,
}

impl Kind {
    pub const ALL: [Kind; 8] = [Kind::Note, Kind::Files, Kind::Search, Kind::Tags, Kind::Graph, Kind::Outline, Kind::Backlinks, Kind::Info];

    /// Столбцы меню вида: заметки и граф, навигация, о текущей заметке.
    pub const GROUPS: [(&'static str, &'static [Kind]); 3] = [
        ("kind-group-notes", &[Kind::Note, Kind::Graph]),
        ("kind-group-navigation", &[Kind::Files, Kind::Search, Kind::Tags]),
        ("kind-group-about", &[Kind::Outline, Kind::Backlinks, Kind::Info]),
    ];

    pub fn id(self) -> &'static str {
        match self {
            Kind::Files => "files",
            Kind::Search => "search",
            Kind::Tags => "tags",
            Kind::Note => "note",
            Kind::Graph => "graph",
            Kind::Outline => "outline",
            Kind::Backlinks => "backlinks",
            Kind::Info => "info",
        }
    }

    pub fn from_id(id: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn title(self) -> String {
        match self {
            Kind::Files => t!("pane-files"),
            Kind::Search => t!("pane-search"),
            Kind::Tags => t!("pane-tags"),
            Kind::Note => t!("pane-note"),
            Kind::Graph => t!("pane-graph"),
            Kind::Outline => t!("pane-outline"),
            Kind::Backlinks => t!("pane-backlinks"),
            Kind::Info => t!("pane-info"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Kind::Files => "folder",
            Kind::Search => "search",
            Kind::Tags => "tags",
            Kind::Note => "note",
            Kind::Graph => "graph",
            Kind::Outline => "outline",
            Kind::Backlinks => "link",
            Kind::Info => "info",
        }
    }
}

/// Вкладка редактора заметок.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteTab {
    /// Номер вкладки: по нему живут история, прокрутка и курсор.
    pub id: u64,
    /// Заметка (путь от корня хранилища).
    pub rel: PathBuf,
    /// Как показана заметка; пусто — как в настройках.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ViewMode>,
}

/// Редактор области.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "PaneRepr")]
pub struct Pane {
    pub id: u64,
    pub kind: Kind,
    /// Вкладки редактора заметок. Когда вид области меняют, вкладки остаются: вернёшь
    /// заметки — вернутся и они, как в Blender.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<NoteTab>,
    /// Номер открытой вкладки.
    #[serde(default)]
    pub active: usize,
}

/// Как редактор лежит в файле раскладки — нынешний и прежний (вкладка с одной заметкой).
#[derive(Deserialize)]
struct PaneRepr {
    id: u64,
    #[serde(alias = "pane")]
    kind: Kind,
    #[serde(default)]
    notes: Vec<NoteTab>,
    #[serde(default)]
    active: usize,
    #[serde(default)]
    note: Option<PathBuf>,
    #[serde(default)]
    mode: Option<ViewMode>,
}

impl From<PaneRepr> for Pane {
    fn from(r: PaneRepr) -> Self {
        let mut notes = r.notes;
        if let Some(rel) = r.note
            && notes.is_empty()
        {
            notes.push(NoteTab { id: r.id, rel, mode: r.mode });
        }
        Pane { id: r.id, kind: r.kind, notes, active: r.active }
    }
}

impl Pane {
    pub fn new(kind: Kind, id: u64) -> Self {
        Self { id, kind, notes: Vec::new(), active: 0 }
    }

    /// Открытая вкладка заметки.
    pub fn active_note(&self) -> Option<&NoteTab> {
        if self.kind != Kind::Note {
            return None;
        }
        self.notes.get(self.active)
    }

    /// Номер вкладки с номером `id` в этом редакторе заметок.
    pub fn position(&self, id: u64) -> Option<usize> {
        if self.kind != Kind::Note {
            return None;
        }
        self.notes.iter().position(|n| n.id == id)
    }

    /// Закрывает вкладку; открытой становится соседняя.
    pub fn close(&mut self, index: usize) -> Option<NoteTab> {
        if index >= self.notes.len() {
            return None;
        }
        let tab = self.notes.remove(index);
        if index < self.active || self.active >= self.notes.len() {
            self.active = self.active.saturating_sub(1);
        }
        Some(tab)
    }
}

impl ShellEditor for Pane {
    fn id(&self) -> u64 {
        self.id
    }

    fn set_id(&mut self, id: u64) {
        self.id = id;
    }

    fn max_id(&self) -> u64 {
        self.notes.iter().map(|n| n.id).fold(self.id, u64::max)
    }

    fn renumber(&mut self, next: &mut dyn FnMut() -> u64) {
        self.id = next();
        for n in &mut self.notes {
            n.id = next();
        }
    }

    /// В стандартной раскладке заметки не открыты.
    fn for_default_layout(&self) -> Self {
        Self::new(self.kind, self.id)
    }
}

/// Все вкладки заметок в видимых редакторах заметок (всех пространств и окон).
pub fn note_tabs(layout: &Layout<Space>) -> impl Iterator<Item = &NoteTab> {
    layout.all_editors().filter(|p| p.kind == Kind::Note).flat_map(|p| p.notes.iter())
}

/// Вкладка заметки по номеру.
pub fn note_tab(layout: &Layout<Space>, id: u64) -> Option<&NoteTab> {
    note_tabs(layout).find(|n| n.id == id)
}

/// Изменяет вкладку заметки с номером `id` (и её редактор) и делает её открытой и видимой.
/// `false` — такой вкладки нет в текущем пространстве и отдельных окнах.
pub fn update_note_tab(layout: &mut Layout<Space>, id: u64, change: impl FnOnce(&mut NoteTab)) -> bool {
    layout.update_editor(
        |p| p.position(id).is_some(),
        |p| {
            if let Some(i) = p.position(id) {
                p.active = i;
                change(&mut p.notes[i]);
            }
        },
    )
}

/// Заметки после переименования или переноса `from` → `to` (файла или папки).
pub fn retarget(layout: &mut Layout<Space>, from: &Path, to: &Path) {
    layout.for_each_editor_mut(|p| {
        for n in &mut p.notes {
            if let Ok(rest) = n.rel.strip_prefix(from) {
                n.rel = if rest.as_os_str().is_empty() { to.to_path_buf() } else { to.join(rest) };
            }
        }
    });
}

/// Встроенные рабочие пространства.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Space {
    /// Файлы слева, заметка в центре, оглавление и обратные ссылки справа.
    Notes,
    /// Граф связей на весь экран, рядом — файлы и обратные ссылки.
    Graph,
    /// Только заметка.
    Focus,
}

impl Builtin for Space {
    type Editor = Pane;

    fn all() -> &'static [Self] {
        &[Space::Notes, Space::Graph, Space::Focus]
    }

    fn title(self) -> String {
        match self {
            Space::Notes => t!("workspace-notes"),
            Space::Graph => t!("workspace-graph"),
            Space::Focus => t!("workspace-focus"),
        }
    }

    fn surface(self, ids: &mut Ids<'_, Pane>) -> Surface<Pane> {
        let h = SplitDir::Horizontal;
        let v = SplitDir::Vertical;
        let area = |ids: &mut Ids<'_, Pane>, kind: Kind| {
            let pane = Pane::new(kind, ids.next_id());
            ids.area(pane)
        };
        let root = match self {
            Space::Notes => {
                let left = area(ids, Kind::Files);
                let note = area(ids, Kind::Note);
                let outline = area(ids, Kind::Outline);
                let links = area(ids, Kind::Backlinks);
                Node::split(h, 0.19, left, Node::split(h, 0.76, note, Node::split(v, 0.55, outline, links)))
            }
            Space::Graph => {
                let left = area(ids, Kind::Files);
                let graph = area(ids, Kind::Graph);
                let right = area(ids, Kind::Backlinks);
                Node::split(h, 0.17, left, Node::split(h, 0.76, graph, right))
            }
            Space::Focus => area(ids, Kind::Note),
        };
        Surface::new(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_layout_has_every_space_with_unique_ids() {
        let layout: Layout<Space> = Layout::factory();
        assert_eq!(layout.workspaces.len(), 3);
        let mut ids: Vec<u64> = layout.all_editors().map(|p| p.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "номера редакторов не повторяются");
        assert!(layout.all_editors().any(|p| p.kind == Kind::Note));
    }

    #[test]
    fn default_layout_forgets_open_notes() {
        let mut layout: Layout<Space> = Layout::factory();
        let id = layout.next_id();
        assert!(layout.update_editor(|p| p.kind == Kind::Note, |p| p.notes.push(NoteTab { id, rel: "Заметка.md".into(), mode: None })));
        assert_eq!(note_tab(&layout, id).map(|n| n.rel.clone()), Some(PathBuf::from("Заметка.md")));
        let copy = layout.for_default().unwrap();
        assert!(copy.all_editors().all(|p| p.notes.is_empty()));
    }

    #[test]
    fn note_tab_ids_stay_unique_after_reload_and_copy() {
        let mut layout: Layout<Space> = Layout::factory();
        let id = layout.next_id() + 100;
        layout.update_editor(|p| p.kind == Kind::Note, |p| p.notes.push(NoteTab { id, rel: "a.md".into(), mode: None }));
        layout.sanitize();
        assert!(layout.next_id() > id, "счётчик выше номеров вкладок");
        let copy = layout.copy_workspace(0);
        let copied: Vec<u64> = copy.surface.editors().flat_map(|p| p.notes.iter().map(|n| n.id)).collect();
        assert!(!copied.contains(&id), "у копии свои номера вкладок");
    }

    #[test]
    fn old_layouts_keep_the_open_note() {
        let old = r#"{"id":7,"pane":"Note","note":"Дневник/2026-10-13.md","mode":"Split"}"#;
        let p: Pane = serde_json::from_str(old).unwrap();
        assert_eq!(p.kind, Kind::Note);
        assert_eq!(p.notes.len(), 1);
        assert_eq!(p.notes[0].rel, PathBuf::from("Дневник/2026-10-13.md"));
        assert_eq!(p.notes[0].mode, Some(ViewMode::Split));
        let again: Pane = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(again.notes.len(), 1, "нынешний формат читается обратно");
    }

    #[test]
    fn closing_keeps_a_neighbour_open() {
        let mut p = Pane::new(Kind::Note, 1);
        for (id, rel) in [(2, "a.md"), (3, "b.md"), (4, "c.md")] {
            p.notes.push(NoteTab { id, rel: rel.into(), mode: None });
        }
        p.active = 2;
        assert_eq!(p.close(0).map(|n| n.id), Some(2));
        assert_eq!(p.active_note().map(|n| n.id), Some(4));
        assert_eq!(p.close(1).map(|n| n.id), Some(4));
        assert_eq!(p.active_note().map(|n| n.id), Some(3));
    }
}
