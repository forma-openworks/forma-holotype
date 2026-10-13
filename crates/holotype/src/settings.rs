//! Настройки Holotype поверх настроек платформы Forma.
//!
//! Компоненты программы лежат рядом с компонентами платформы в `options/<id>.json` папки
//! `%APPDATA%\Forma\Holotype`:
//! - «Заметки» (`notes`): акцентный цвет, размер текста, ширина строки, вид по умолчанию;
//! - недавние хранилища (`vaults`) — память программы, в экспорт не попадает.
//!
//! Настройки первых версий (`%APPDATA%\Forma Holotype\config.json`) при первом запуске
//! переезжают сюда сами: тема, масштаб и русский язык интерфейса — в компоненты платформы.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Ui};
use forma_i18n::t;
use forma_settings::export::{Archive, Status};
use forma_settings::{Component, Store, decode};
use forma_shell::AppSettings;
use forma_ui::appearance::Appearance;
use forma_ui::settings_window::{self as sw, PageInfo};
use forma_ui::theme::Palette;
use serde::{Deserialize, Serialize};

use crate::panes::ViewMode;
use crate::theme::Accent;

/// Страница «Заметки» в окне настроек.
pub const NOTES_PAGE: &str = "notes";

/// Как выглядят и открываются заметки.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notes {
    /// Свой акцентный цвет поверх темы; `None` — как в теме.
    pub accent: Option<Accent>,
    /// Размер текста заметок в пикселях (в редакторе и при чтении).
    pub text_size: f32,
    /// Строки заметки не шире удобной для чтения ширины.
    pub readable_width: bool,
    /// Моноширинный шрифт в редакторе.
    pub mono_editor: bool,
    /// Как открывается заметка.
    pub default_mode: ViewMode,
}

impl Default for Notes {
    fn default() -> Self {
        Self { accent: None, text_size: 16.0, readable_width: true, mono_editor: false, default_mode: ViewMode::Preview }
    }
}

impl Component for Notes {
    const ID: &'static str = "notes";
}

/// Размеры текста заметок.
pub const TEXT_SIZE_RANGE: std::ops::RangeInclusive<f32> = 12.0..=24.0;

/// Недавние хранилища. Это память программы, а не настройка: в экспорт не попадает.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vaults {
    pub recent: Vec<PathBuf>,
    /// Хранилище, открытое при выходе: при запуске оно открывается снова.
    pub last: Option<PathBuf>,
}

impl Component for Vaults {
    const ID: &'static str = "vaults";
    const EXPORTABLE: bool = false;
}

impl Vaults {
    pub fn remember(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(8);
        self.last = Some(path.to_path_buf());
    }
}

/// Все свои настройки Holotype.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HolotypeSettings {
    pub notes: Notes,
    pub vaults: Vaults,
}

impl AppSettings for HolotypeSettings {
    fn load(store: &mut Store) -> Self {
        if !store.initialized()
            && let Some(old) = migrate_legacy(store)
        {
            return old;
        }
        Self { notes: store.load(), vaults: store.load() }
    }

    fn save(&self, store: &mut Store) {
        store.save(&self.notes);
        store.save(&self.vaults);
    }

    fn pages(&self) -> Vec<PageInfo> {
        vec![PageInfo {
            id: NOTES_PAGE.into(),
            section: crate::SPEC.app.title.into(),
            title: t!("settings-page-notes"),
            keywords: vec![t!("notes-accent"), t!("notes-text-size"), t!("notes-readable-width"), t!("notes-mono-editor"), t!("notes-default-mode")],
        }]
    }

    fn page_modified(&self, live: &Self, page: &str) -> bool {
        page == NOTES_PAGE && self.notes != live.notes
    }

    fn page_ui(&mut self, ui: &mut Ui, pal: &Palette, page: &str, search: &str) {
        if page == NOTES_PAGE {
            notes_page(ui, pal, search, &mut self.notes);
        }
    }

    fn exportable(&self) -> Vec<(&'static str, String, String)> {
        vec![(Notes::ID, t!("settings-page-notes"), t!("settings-component-notes"))]
    }

    fn export(&self, id: &str, archive: &mut Archive) {
        if id == Notes::ID {
            archive.add(&self.notes);
        }
    }

    fn import_status(&self, id: &str, value: &serde_json::Value) -> Option<Status> {
        (id == Notes::ID).then(|| match decode::<Notes>(value) {
            Ok((_, skipped)) if skipped.is_empty() => Status::Apply,
            Ok((_, skipped)) => Status::Partial(skipped),
            Err(e) => Status::Invalid(e),
        })
    }

    fn import_apply(&mut self, id: &str, value: &serde_json::Value) {
        if id == Notes::ID
            && let Ok((notes, _)) = decode::<Notes>(value)
        {
            self.notes = notes;
        }
    }
}

/// Страница «Заметки».
fn notes_page(ui: &mut Ui, pal: &Palette, search: &str, n: &mut Notes) {
    let def = Notes::default();
    sw::group(ui, pal, &t!("notes-group-text"));
    if sw::row(ui, pal, search, &t!("notes-text-size"), n.text_size != def.text_size, |ui| {
        ui.add(egui::Slider::new(&mut n.text_size, TEXT_SIZE_RANGE).step_by(0.5).suffix(" px"));
    }) {
        n.text_size = def.text_size;
    }
    let modified = n.readable_width != def.readable_width;
    if sw::check(ui, pal, search, &t!("notes-readable-width"), &mut n.readable_width, modified) {
        n.readable_width = def.readable_width;
    }
    let modified = n.mono_editor != def.mono_editor;
    if sw::check(ui, pal, search, &t!("notes-mono-editor"), &mut n.mono_editor, modified) {
        n.mono_editor = def.mono_editor;
    }

    sw::group(ui, pal, &t!("notes-group-look"));
    if sw::row(ui, pal, search, &t!("notes-default-mode"), n.default_mode != def.default_mode, |ui| {
        let modes = [ViewMode::Preview, ViewMode::Split, ViewMode::Edit];
        let labels: Vec<String> = modes.iter().map(|m| m.title()).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let current = modes.iter().position(|m| *m == n.default_mode).unwrap_or(0);
        if let Some(i) = forma_ui::widgets::segmented(ui, pal, sw::id(NOTES_PAGE, "mode"), &labels, current, false) {
            n.default_mode = modes[i];
        }
    }) {
        n.default_mode = def.default_mode;
    }
    if sw::row(ui, pal, search, &t!("notes-accent"), n.accent != def.accent, |ui| {
        let mut options: Vec<(Option<Accent>, String)> = vec![(None, t!("notes-accent-theme"))];
        options.extend(Accent::ALL.iter().map(|a| (Some(*a), format!("●  {}", a.label()))));
        if let Some(a) = forma_ui::widgets::dropdown(ui, pal, sw::id(NOTES_PAGE, "accent"), 220.0, &n.accent, &options) {
            n.accent = a;
        }
    }) {
        n.accent = def.accent;
    }
    sw::hint(ui, pal, &t!("notes-accent-hint"));
}

/// `config.json` первых версий.
#[derive(Deserialize)]
#[serde(default)]
struct Legacy {
    vault: Option<PathBuf>,
    recent_vaults: Vec<PathBuf>,
    theme: String,
    accent: Option<Accent>,
    zoom: f32,
    font_size: f32,
    mono_editor: bool,
    readable_width: bool,
    default_mode: ViewMode,
}

impl Default for Legacy {
    fn default() -> Self {
        let n = Notes::default();
        Self {
            vault: None,
            recent_vaults: Vec::new(),
            theme: "Dark".into(),
            accent: None,
            zoom: 1.0,
            font_size: n.text_size,
            mono_editor: n.mono_editor,
            readable_width: n.readable_width,
            default_mode: n.default_mode,
        }
    }
}

impl Legacy {
    fn into_settings(self) -> (HolotypeSettings, Appearance) {
        let theme = if self.theme == "Light" { "holotype-light" } else { "holotype-dark" };
        // Первые версии были только на русском — интерфейс остаётся привычным.
        let appearance = Appearance { language: Some("ru".into()), theme: theme.to_owned(), zoom: self.zoom, ..Appearance::default() };
        let notes = Notes {
            // Фиолетовый — акцент самих тем Holotype.
            accent: self.accent.filter(|a| *a != Accent::Violet),
            text_size: self.font_size.clamp(*TEXT_SIZE_RANGE.start(), *TEXT_SIZE_RANGE.end()),
            readable_width: self.readable_width,
            mono_editor: self.mono_editor,
            default_mode: self.default_mode,
        };
        let vaults = Vaults { recent: self.recent_vaults, last: self.vault };
        (HolotypeSettings { notes, vaults }, appearance)
    }
}

/// Переносит `config.json` первых версий в компоненты (внешний вид — в компонент платформы).
fn migrate_legacy(store: &mut Store) -> Option<HolotypeSettings> {
    // Отдельная папка настроек (FORMA_CONFIG_HOME) — чистый запуск, без чужих хранилищ.
    if std::env::var_os(forma_settings::CONFIG_HOME_VAR).is_some_and(|v| !v.is_empty()) {
        return None;
    }
    let base = dirs::config_dir()?;
    let path = [base.join("Forma Holotype").join("config.json"), base.join("DocApp").join("config.json")].into_iter().find(|p| p.is_file())?;
    let old: Legacy = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    let (settings, appearance) = old.into_settings();
    store.save(&appearance);
    settings.save(store);
    log::info!("{} migrated to options/", path.display());
    Some(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_config_maps_to_components() {
        let json = r#"{"vault":"D:/Заметки","recent_vaults":["D:/Заметки"],"theme":"Light","accent":"Teal","zoom":1.25,"font_size":17.0,"mono_editor":true,"readable_width":false,"show_left":true,"default_mode":"Split","open_tabs":[],"active_tab":0}"#;
        let (s, a) = serde_json::from_str::<Legacy>(json).unwrap().into_settings();
        assert_eq!(a.theme, "holotype-light");
        assert_eq!(a.language.as_deref(), Some("ru"));
        assert_eq!(a.zoom, 1.25);
        assert_eq!(s.notes.accent, Some(Accent::Teal));
        assert_eq!(s.notes.text_size, 17.0);
        assert!(s.notes.mono_editor && !s.notes.readable_width);
        assert_eq!(s.notes.default_mode, ViewMode::Split);
        assert_eq!(s.vaults.last, Some(PathBuf::from("D:/Заметки")));
    }

    #[test]
    fn violet_accent_means_theme_accent() {
        let (s, _) = serde_json::from_str::<Legacy>(r#"{"accent":"Violet"}"#).unwrap().into_settings();
        assert_eq!(s.notes.accent, None);
    }
}
