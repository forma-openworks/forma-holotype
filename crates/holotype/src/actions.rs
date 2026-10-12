//! Действия Forma: Holotype: id, группы, названия и сочетания по умолчанию. Общие действия
//! платформы (настройки, масштаб, тема, окна, «Найти действие») добавляет оболочка.

use forma_actions::{Action, Context, Registry};
use forma_shell::actions::{GROUP_FILE, GROUP_VIEW};

pub const NEW_NOTE: &str = "note.new";
pub const NEW_FOLDER: &str = "note.newFolder";
pub const DAILY_NOTE: &str = "note.daily";
pub const SAVE_ALL: &str = "file.saveAll";
pub const OPEN_VAULT: &str = "vault.open";
pub const CREATE_DEMO: &str = "vault.createDemo";
pub const RESCAN: &str = "vault.rescan";
pub const REVEAL: &str = "vault.reveal";
pub const EXPORT_NOTE: &str = "export.note";
pub const EXPORT_SITE: &str = "export.site";

pub const QUICK_OPEN: &str = "go.quickOpen";
pub const SEARCH: &str = "go.search";
pub const BACK: &str = "go.back";
pub const FORWARD: &str = "go.forward";

pub const TOGGLE_MODE: &str = "view.toggleReading";
pub const SPLIT: &str = "view.split";
pub const GRAPH: &str = "view.graph";

pub const RENAME: &str = "note.rename";
pub const CLOSE_NOTE: &str = "note.close";
pub const NEXT_NOTE: &str = "note.next";
pub const PREV_NOTE: &str = "note.previous";

pub const BOLD: &str = "editor.bold";
pub const ITALIC: &str = "editor.italic";
pub const LINK: &str = "editor.link";
pub const STRIKE: &str = "editor.strikethrough";
pub const HIGHLIGHT: &str = "editor.highlight";
pub const CODE: &str = "editor.code";
pub const COMPLETION: &str = "editor.completion";

const GROUP_GO: &str = "action-group-go";
const GROUP_NOTE: &str = "action-group-note";
const GROUP_EDITOR: &str = "action-group-editor";

/// Регистрирует действия Holotype (общие добавит оболочка).
pub fn register(r: &mut Registry) {
    use Context::{Editor, Global};
    let mut add = |id, group, title, context, keys: &[&str]| r.register(Action::new(id, group, title, context, keys));

    add(NEW_NOTE, GROUP_FILE, "action-new-note", Global, &["Ctrl+N"]);
    add(NEW_FOLDER, GROUP_FILE, "action-new-folder", Global, &[]);
    add(DAILY_NOTE, GROUP_FILE, "action-daily-note", Global, &["Ctrl+D"]);
    add(SAVE_ALL, GROUP_FILE, "action-save-all", Global, &["Ctrl+S"]);
    add(OPEN_VAULT, GROUP_FILE, "action-open-vault", Global, &[]);
    add(CREATE_DEMO, GROUP_FILE, "action-create-demo", Global, &[]);
    add(RESCAN, GROUP_FILE, "action-rescan", Global, &[]);
    add(REVEAL, GROUP_FILE, "action-reveal", Global, &[]);
    add(EXPORT_NOTE, GROUP_FILE, "action-export-note", Global, &[]);
    add(EXPORT_SITE, GROUP_FILE, "action-export-site", Global, &[]);

    add(TOGGLE_MODE, GROUP_VIEW, "action-toggle-reading", Global, &["Ctrl+E"]);
    add(SPLIT, GROUP_VIEW, "action-split", Global, &["Ctrl+Shift+E"]);
    add(GRAPH, GROUP_VIEW, "action-graph", Global, &["Ctrl+G"]);

    add(QUICK_OPEN, GROUP_GO, "action-quick-open", Global, &["Ctrl+O", "Ctrl+P"]);
    add(SEARCH, GROUP_GO, "action-search", Global, &["Ctrl+Shift+F"]);
    add(BACK, GROUP_GO, "action-back", Global, &["Alt+Left"]);
    add(FORWARD, GROUP_GO, "action-forward", Global, &["Alt+Right"]);

    add(RENAME, GROUP_NOTE, "action-rename-note", Global, &["F2"]);
    add(CLOSE_NOTE, GROUP_NOTE, "action-close-note", Global, &["Ctrl+W"]);
    add(NEXT_NOTE, GROUP_NOTE, "action-next-note", Global, &["Ctrl+Tab"]);
    add(PREV_NOTE, GROUP_NOTE, "action-previous-note", Global, &["Ctrl+Shift+Tab"]);

    add(BOLD, GROUP_EDITOR, "action-bold", Editor, &["Ctrl+B"]);
    add(ITALIC, GROUP_EDITOR, "action-italic", Editor, &["Ctrl+I"]);
    add(LINK, GROUP_EDITOR, "action-wiki-link", Editor, &["Ctrl+K"]);
    add(STRIKE, GROUP_EDITOR, "action-strikethrough", Editor, &["Ctrl+Shift+X"]);
    add(HIGHLIGHT, GROUP_EDITOR, "action-highlight", Editor, &["Ctrl+Shift+H"]);
    add(CODE, GROUP_EDITOR, "action-inline-code", Editor, &["Ctrl+Shift+C"]);
    add(COMPLETION, GROUP_EDITOR, "action-completion", Editor, &["Ctrl+Space"]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        forma_shell::actions::registry(register)
    }

    #[test]
    fn default_shortcuts_do_not_clash() {
        let r = registry();
        let keymap = forma_actions::Keymap::default();
        for a in r.iter() {
            for key in keymap.shortcuts(&r, &a.id) {
                assert!(keymap.conflicts(&r, *key, &a.id).is_empty(), "{key} у {} занято ещё кем-то", a.id);
            }
        }
    }

    #[test]
    fn every_title_is_translated() {
        crate::i18n::init();
        let r = registry();
        for lang in forma_i18n::builtin_languages() {
            for a in r.iter() {
                let forma_actions::Title::Key(key) = &a.title else { continue };
                assert!(forma_i18n::has_message(lang, key), "нет перевода {key} ({lang})");
                assert!(forma_i18n::has_message(lang, &a.group), "нет перевода {} ({lang})", a.group);
            }
        }
    }
}
