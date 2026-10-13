// В релизной сборке под Windows не показываем консольное окно.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Forma: Holotype — заметки, конспекты и документация FORMA. Программа платформы Forma:
//! оболочка, темы, настройки, раскладка клавиш и рабочие пространства — общие с Forma
//! Mycelium (крейты `forma-*`), а заметки, редактор, просмотр, граф, формулы и диаграммы —
//! свои.

mod actions;
mod app;
mod assist;
mod autoscroll;
mod complete;
mod diagram;
mod editor;
mod emmet;
mod export;
mod fonts;
mod graph;
mod highlight;
mod i18n;
mod icon;
mod icons;
mod math;
mod panes;
mod render;
mod settings;
mod term;
mod theme;

pub use holotype_vault::{markdown, sample, vault};

use forma_settings::AppInfo;
use forma_shell::Spec;

/// Holotype для оболочки Forma.
pub static SPEC: Spec = Spec {
    app: AppInfo { id: "holotype", dir_name: "Holotype", title: "Forma: Holotype", version: env!("CARGO_PKG_VERSION") },
    short_name: "Holotype",
    app_id: "forma-holotype",
    themes: theme::BUILTIN_THEMES,
    default_theme: theme::DEFAULT_THEME,
    colors: theme::NoteColors::TOKENS,
    logo: icon::rgba,
    mark: icon::rgba,
    locales: i18n::LOCALES,
    catalog: Vec::new,
    size: [1400.0, 880.0],
    min_size: [760.0, 480.0],
};

fn main() {
    // Путь к хранилищу можно передать аргументом: `forma-holotype "D:\Заметки"`.
    let vault_arg = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    fonts::register();
    icons::register();
    forma_shell::run::<settings::HolotypeSettings, panes::Space, _>(&SPEC, actions::register, move |cc, shell| app::HolotypeApp::new(cc, shell, vault_arg));
}
