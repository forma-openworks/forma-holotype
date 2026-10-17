//! Forma: Holotype — просмотр заметок. То, чем Holotype рисует заметку: [`render::Renderer`]
//! по дереву документа, код и консоль, диаграммы, формулы, плашки, картинки и блоки
//! плагинов, — отдельной библиотекой, чтобы другие программы Forma (Instar) показывали
//! заметки и главы курса точно так же.
//!
//! Программа, которая рисует заметки:
//! - регистрирует шрифты ([`fonts::register`]) до запуска окна;
//! - добавляет строки рендера ([`LOCALES`], `locales/` этого crate) к своим переводам;
//! - берёт цвета заметок из темы ([`theme::Palette::from_theme`]);
//! - рисует: `Renderer::new(&pal, &vault, note_rel, &doc, size).show(ui)` и выполняет
//!   [`render::Action`], которые накопились (открыть ссылку, отметить задачу и т. д.).

pub mod diagram;
pub mod fonts;
pub mod highlight;
pub mod math;
pub mod plugin_ui;
pub mod render;
pub mod term;
pub mod theme;

pub use holotype_vault::{markdown, vault};

// Строки рендера: `pub static LOCALES`.
include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// Переводы на русском для тестов (ожидаемые строки в них русские).
#[cfg(test)]
pub(crate) mod i18n {
    pub fn init() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            forma_i18n::init_with(&[forma_shell::LOCALES, super::LOCALES], None);
            forma_i18n::set_language("ru");
        });
    }
}
