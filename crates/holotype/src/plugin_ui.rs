//! Плагины в Holotype: их панели (вид редактора области, как в меню вида у Blender), блоки
//! заметок (` ```lang `), пункты контекстного меню и команды.
//!
//! Деревья элементов от плагинов хранятся здесь: плагин спрашивают, только когда нужно —
//! новый блок, событие панели, просьба самого плагина, — а не каждый кадр. Когда меняется
//! набор плагинов, хранилище или язык ([`forma_plugin_host::Runtime::generation`]), всё
//! сохранённое забывается и рисуется заново.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;

use eframe::egui::{self, CornerRadius, Frame, Label, Margin, RichText, Stroke, Ui};
use forma_i18n::t;
use forma_plugin_host::{CommandInfo, PanelInfo, Runtime};
use forma_plugin_ui::Element;
use serde::Serialize;
use serde_json::Value;

use crate::render::{Action, Renderer};
use crate::theme::Palette;
use crate::vault::Vault;

/// Ответ плагина: дерево элементов или почему его нет.
pub enum Tree {
    Ready(Element),
    Failed(String),
}

impl Tree {
    /// Ответ рантайма: JSON дерева или ошибка плагина.
    pub fn from_answer(answer: Result<String, String>) -> Tree {
        match answer.and_then(|json| Element::from_json(&json)) {
            Ok(e) => Tree::Ready(e),
            Err(e) => Tree::Failed(e),
        }
    }
}

/// Что Holotype знает о плагинах: списки вкладов (обновляются каждый кадр) и ответы.
#[derive(Default)]
pub struct PluginUi {
    /// Поколение плагинов, для которого собраны ответы.
    generation: u64,
    pub panels: Vec<PanelInfo>,
    /// Пункты меню выделенного текста и всей заметки.
    pub selection_menu: Vec<CommandInfo>,
    pub note_menu: Vec<CommandInfo>,
    /// Блоки: язык (в нижнем регистре) → название.
    blocks: HashMap<String, String>,
    pub panel_trees: HashMap<String, Tree>,
    pub block_trees: HashMap<u64, Tree>,
}

impl PluginUi {
    /// Берёт вклады работающих плагинов; сменилось поколение — забывает ответы.
    pub fn sync(&mut self, rt: &Runtime) {
        if rt.generation() != self.generation {
            self.generation = rt.generation();
            self.panel_trees.clear();
            self.block_trees.clear();
        }
        self.panels = rt.panels();
        self.selection_menu = rt.menu("selection");
        self.note_menu = rt.menu("note");
        self.blocks = rt.block_langs().into_iter().map(|l| (l.to_lowercase(), rt.block(&l).unwrap_or_default())).collect();
    }

    pub fn panel(&self, key: &str) -> Option<&PanelInfo> {
        self.panels.iter().find(|p| p.key() == key)
    }

    /// Рисует ли блок ` ```lang ` плагин; его название (может быть пустым).
    pub fn block_title(&self, lang: &str) -> Option<&str> {
        self.blocks.get(&lang.to_lowercase()).map(String::as_str)
    }

    /// Ключ ответа на блок: язык, текст блока и заметка (блок может зависеть от неё).
    pub fn block_key(lang: &str, source: &str, note: &Path) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (lang.to_lowercase(), source, note).hash(&mut h);
        h.finish()
    }
}

/// Ключ панели `плагин/панель` → (плагин, панель).
pub fn split_key(key: &str) -> Option<(&str, &str)> {
    key.split_once('/')
}

/// Где и с чем вызвали команду плагина — то, что SDK читает как `CommandContext`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CommandCx {
    /// `menu`, `key`, `block`, `panel`.
    pub from: String,
    pub note: String,
    pub selection: String,
    pub heading: String,
    pub paragraph: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub args: Value,
}

impl CommandCx {
    pub fn new(from: &str, note: Option<&Path>) -> Self {
        Self { from: from.to_owned(), note: note.map(note_path).unwrap_or_default(), ..Default::default() }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Путь заметки для плагина: от корня хранилища, через `/`.
pub fn note_path(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

/// Заголовок раздела, в котором стоит байт `at` текста Markdown, и абзац вокруг него.
pub fn context_at(text: &str, at: usize) -> (String, String) {
    let at = at.min(text.len());
    let mut heading = String::new();
    let mut in_code = false;
    for line in text[..at].split_inclusive('\n') {
        let t = line.trim();
        if t.starts_with("```") {
            in_code = !in_code;
        } else if !in_code && t.starts_with('#') && t.trim_start_matches('#').starts_with(' ') {
            heading = t.trim_start_matches('#').trim().to_owned();
        }
    }
    // Абзац: от пустой строки до пустой строки.
    let before = text[..at].rfind("\n\n").map_or(0, |i| i + 2);
    let after = text[at..].find("\n\n").map_or(text.len(), |i| at + i);
    (heading, text[before..after].trim().to_owned())
}

/// Как Holotype рисует то, что в дереве элементов умеет лучше платформы: markdown — тем же
/// renderer, что и заметки, code — с подсветкой темы, картинки — из хранилища.
pub struct Hooks<'a> {
    pub pal: &'a Palette,
    pub vault: &'a Vault,
    /// Заметка, относительно которой разрешаются ссылки и картинки.
    pub note: &'a Path,
    pub size: f32,
    /// Ссылки, по которым щёлкнули в markdown.
    pub actions: Vec<Action>,
}

impl forma_ui::elements::Hooks for Hooks<'_> {
    fn markdown(&mut self, ui: &mut Ui, _pal: &forma_ui::theme::Palette, text: &str) {
        let doc = crate::markdown::parse(text);
        let mut r = Renderer::new(self.pal, self.vault, self.note, &doc, self.size);
        r.show(ui);
        self.actions.append(&mut r.actions);
    }

    fn code(&mut self, ui: &mut Ui, _pal: &forma_ui::theme::Palette, text: &str, lang: &str) {
        let size = self.size * 0.84;
        Frame::new()
            .fill(self.pal.code_bg)
            .stroke(Stroke::new(1.0, self.pal.code_border))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let job = crate::highlight::job(ui.ctx(), text.trim_end_matches('\n'), lang, size, size * 1.6);
                ui.add(Label::new(job).wrap());
            });
    }

    fn image_uri(&self, path: &str) -> Option<String> {
        self.vault.resolve_attachment(path, self.note).map(|f| crate::app::file_url(&f))
    }

    fn image(&mut self, ui: &mut Ui, _pal: &forma_ui::theme::Palette, path: &str, width: Option<f32>, alt: &str) {
        match self.vault.resolve_attachment(path, self.note) {
            Some(file) => {
                let uri = crate::app::file_url(&file);
                let avail = ui.available_width();
                let img = egui::Image::new(uri).fit_to_original_size(1.0).max_width(width.unwrap_or(avail).min(avail)).corner_radius(CornerRadius::same(8));
                let resp = ui.add(img);
                if !alt.is_empty() {
                    resp.on_hover_text(alt);
                }
            }
            None => {
                ui.label(RichText::new(format!("⚠ {}", t!("preview-image-missing", src = path.to_owned()))).size(self.size * 0.85).color(self.pal.warning));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_finds_the_section_and_the_paragraph() {
        let text = "# Глава\n\nВступление.\n\n## Сетка\n\nПервая строка\nвторая строка про UV.\n\n```md\n# не заголовок\n```\n\nХвост.";
        let at = text.find("UV").unwrap();
        assert_eq!(context_at(text, at), ("Сетка".to_owned(), "Первая строка\nвторая строка про UV.".to_owned()));
        let at = text.find("Хвост").unwrap();
        assert_eq!(context_at(text, at).0, "Сетка", "строка в блоке кода — не заголовок");
    }

    #[test]
    fn command_context_is_json_for_the_sdk() {
        let mut cx = CommandCx::new("menu", Some(Path::new("Курс\\1.3 Режим правки.md")));
        cx.selection = "Edit Mode".into();
        let json = cx.to_json();
        assert!(json.contains(r#""note":"Курс/1.3 Режим правки.md""#) && json.contains(r#""selection":"Edit Mode""#) && !json.contains("args"), "{json}");
        assert_eq!(split_key("terms/glossary"), Some(("terms", "glossary")));
    }
}
