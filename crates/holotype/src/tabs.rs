//! Вкладки областей и встроенные рабочие пространства Holotype.
//!
//! Любая панель — вкладка области: дерево файлов, поиск, теги, заметка, граф, оглавление,
//! обратные ссылки, сведения о заметке. Их можно перетаскивать между областями, делить
//! области и выносить в отдельные окна, как в Blender.

use std::path::PathBuf;

use forma_i18n::t;
use forma_layout::{Node, SplitDir, Surface};
use forma_shell::{Builtin, Ids, ShellTab};
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

/// Что показывает вкладка.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    Files,
    Search,
    Tags,
    Note,
    Graph,
    Outline,
    Backlinks,
    Info,
}

impl Pane {
    pub const ALL: [Pane; 8] = [Pane::Note, Pane::Files, Pane::Search, Pane::Tags, Pane::Graph, Pane::Outline, Pane::Backlinks, Pane::Info];

    pub fn title(self) -> String {
        match self {
            Pane::Files => t!("pane-files"),
            Pane::Search => t!("pane-search"),
            Pane::Tags => t!("pane-tags"),
            Pane::Note => t!("pane-note"),
            Pane::Graph => t!("pane-graph"),
            Pane::Outline => t!("pane-outline"),
            Pane::Backlinks => t!("pane-backlinks"),
            Pane::Info => t!("pane-info"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Pane::Files => "folder",
            Pane::Search => "search",
            Pane::Tags => "tags",
            Pane::Note => "note",
            Pane::Graph => "graph",
            Pane::Outline => "outline",
            Pane::Backlinks => "link",
            Pane::Info => "info",
        }
    }

    /// Можно ли открыть несколько таких вкладок (заметки и граф — да, панели — нет).
    pub fn many(self) -> bool {
        matches!(self, Pane::Note | Pane::Graph)
    }
}

/// Вкладка области.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tab {
    pub id: u64,
    pub pane: Pane,
    /// Заметка вкладки «Заметка» (путь от корня хранилища); пусто — вкладка ждёт заметку.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<PathBuf>,
    /// Как показана заметка; пусто — как в настройках.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ViewMode>,
}

impl Tab {
    pub fn new(pane: Pane, id: u64) -> Self {
        Self { id, pane, note: None, mode: None }
    }

    pub fn note(id: u64, rel: PathBuf) -> Self {
        Self { id, pane: Pane::Note, note: Some(rel), mode: None }
    }
}

impl ShellTab for Tab {
    fn id(&self) -> u64 {
        self.id
    }

    fn set_id(&mut self, id: u64) {
        self.id = id;
    }

    /// В стандартной раскладке заметки не открыты: остаётся пустая вкладка «Заметка».
    fn for_default_layout(&self) -> Option<Self> {
        let mut tab = self.clone();
        tab.note = None;
        tab.mode = None;
        Some(tab)
    }
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
    type Tab = Tab;

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

    fn surface(self, ids: &mut Ids<'_, Tab>) -> Surface<Tab> {
        let h = SplitDir::Horizontal;
        let v = SplitDir::Vertical;
        let area = |ids: &mut Ids<'_, Tab>, panes: &[Pane]| {
            let tabs = panes.iter().map(|p| Tab::new(*p, ids.next_id())).collect();
            ids.area(tabs)
        };
        let root = match self {
            Space::Notes => {
                let left = area(ids, &[Pane::Files, Pane::Search, Pane::Tags]);
                let note = area(ids, &[Pane::Note]);
                let outline = area(ids, &[Pane::Outline, Pane::Info]);
                let links = area(ids, &[Pane::Backlinks]);
                Node::split(h, 0.19, left, Node::split(h, 0.76, note, Node::split(v, 0.55, outline, links)))
            }
            Space::Graph => {
                let left = area(ids, &[Pane::Files]);
                let graph = area(ids, &[Pane::Graph]);
                let right = area(ids, &[Pane::Backlinks, Pane::Outline]);
                Node::split(h, 0.17, left, Node::split(h, 0.76, graph, right))
            }
            Space::Focus => area(ids, &[Pane::Note]),
        };
        Surface::new(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_shell::Layout;

    #[test]
    fn factory_layout_has_every_space_with_unique_ids() {
        let layout: Layout<Space> = Layout::factory();
        assert_eq!(layout.workspaces.len(), 3);
        let mut ids: Vec<u64> = layout.all_tabs().map(|t| t.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "номера вкладок не повторяются");
        assert!(layout.all_tabs().any(|t| t.pane == Pane::Note));
    }

    #[test]
    fn default_layout_forgets_open_notes() {
        let mut layout: Layout<Space> = Layout::factory();
        layout.update_tab(|t| t.pane == Pane::Note, |t| t.note = Some("Заметка.md".into()));
        let copy = layout.for_default().unwrap();
        assert!(copy.all_tabs().all(|t| t.note.is_none()));
    }
}
