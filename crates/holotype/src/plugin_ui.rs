//! Плагины в Holotype: их панели (вид редактора области, как в меню вида у Blender), блоки
//! заметок (` ```lang `), пункты контекстного меню и команды.
//!
//! Деревья элементов от плагинов хранятся здесь: плагин спрашивают, только когда нужно —
//! новый блок, событие панели, просьба самого плагина, — а не каждый кадр. Когда меняется
//! набор плагинов, хранилище или язык ([`forma_plugin_host::Runtime::generation`]), всё
//! сохранённое забывается и рисуется заново.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

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
    /// Связанные папки открытого хранилища (имя → полный путь): их картинки показываются в
    /// блоках и панелях плагинов.
    pub linked: Vec<(String, PathBuf)>,
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

/// Связанные папки хранилища-курса: `practice` и `related` из манифеста курса
/// (`.holotype/assistant.toml` → `course = "…/course.toml"`), пути — от корня хранилища.
/// Папки, которых нет на диске, пропускаются. У обычного хранилища связанных папок нет.
pub fn linked_dirs(root: &Path) -> Vec<(String, PathBuf)> {
    let read = |p: &Path| std::fs::read_to_string(p).ok().and_then(|t| t.parse::<toml::Table>().ok());
    let Some(course) = read(&root.join(".holotype").join("assistant.toml")).and_then(|a| a.get("course")?.as_str().map(str::to_owned)) else { return Vec::new() };
    let Some(course) = read(&root.join(course)) else { return Vec::new() };
    let mut out = Vec::new();
    let mut add = |name: String, rel: &str| {
        if let Ok(dir) = std::fs::canonicalize(root.join(rel)).map(plain)
            && dir.is_dir()
            && !out.iter().any(|(n, _): &(String, PathBuf)| *n == name)
        {
            out.push((name, dir));
        }
    };
    if let Some(p) = course.get("practice").and_then(|v| v.as_str()) {
        add("practice".into(), p);
    }
    for r in course.get("related").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_str()) {
        let name = Path::new(r).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| r.to_owned());
        add(name, r);
    }
    out
}

/// Путь без приставки `\\?\` (её добавляет `canonicalize` на Windows): такой путь понятен
/// людям, плагинам и загрузчику картинок (`file:///C:/…`).
pub fn plain(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    match (s.strip_prefix(r"\\?\UNC\"), s.strip_prefix(r"\\?\")) {
        (Some(unc), _) => PathBuf::from(format!(r"\\{unc}")),
        (None, Some(local)) => PathBuf::from(local),
        _ => path,
    }
}

/// Файл, о котором говорит плагин: `linked:<имя>/…`, полный путь внутри хранилища или
/// связанной папки — или вложение заметки `note`, как у картинок в заметках.
pub fn resolve_file(vault: &Vault, linked: &[(String, PathBuf)], note: &Path, path: &str) -> Option<PathBuf> {
    if let Some(rest) = path.strip_prefix("linked:") {
        let (name, rel) = rest.split_once('/').unwrap_or((rest, ""));
        let file = linked.iter().find(|(n, _)| n == name)?.1.join(rel);
        return file.is_file().then_some(file);
    }
    let full = Path::new(path);
    if full.is_absolute() {
        let real = std::fs::canonicalize(full).ok().map(plain)?;
        let inside = |root: &Path| std::fs::canonicalize(root).map(plain).is_ok_and(|r| real.starts_with(r));
        return (inside(&vault.root) || linked.iter().any(|(_, r)| inside(r))).then_some(real);
    }
    vault.resolve_attachment(path, note)
}

/// Пункт раздела «Практика» с жирным началом `title` отмечен (`- [x]`); `None` — такого
/// пункта нет. Уже отмеченный остаётся как был. Правило то же, что у `learn`
/// (`learn_core::chapter::tick` в MyuriTeach): раздел — строка `## Практика` до следующего
/// `## `, пункт — `- [ ] **Название.** …`, название сравнивается целиком.
pub fn tick(text: &str, title: &str) -> Option<String> {
    let mut inside = false;
    let mut found: Option<(usize, bool)> = None;
    for (n, line) in text.lines().enumerate() {
        if line.trim_end() == "## Практика" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if line.starts_with("## ") {
            break;
        }
        let head = line.strip_prefix("- [").and_then(|rest| {
            let (mark, rest) = rest.split_at_checked(1)?;
            let done = match mark {
                " " => false,
                "x" | "X" => true,
                _ => return None,
            };
            let (t, _) = rest.strip_prefix("] **")?.split_once("**")?;
            Some((done, t.trim().to_owned()))
        });
        if let Some((done, t)) = head
            && t == title
        {
            found = Some((n, done));
            break;
        }
    }
    let (line, done) = found?;
    let mut out = String::with_capacity(text.len());
    for (n, piece) in text.split_inclusive('\n').enumerate() {
        if n == line && !done {
            out.push_str(&piece.replacen("- [ ]", "- [x]", 1));
        } else {
            out.push_str(piece);
        }
    }
    Some(out)
}

/// Markdown в конце заметки: отделён пустой строкой, заметка кончается переводом строки.
pub fn append_to_note(text: &str, markdown: &str) -> String {
    let body = text.trim_end_matches(['\n', '\r']);
    let add = markdown.trim_matches(['\n', '\r']);
    if body.is_empty() { format!("{add}\n") } else { format!("{body}\n\n{add}\n") }
}

/// Markdown в конце раздела с заголовком `heading` (без учёта регистра, любого уровня) —
/// перед следующим заголовком того же или более высокого уровня. `None` — нет раздела.
pub fn append_to_section(text: &str, heading: &str, markdown: &str) -> Option<String> {
    let wanted = heading.trim().to_lowercase();
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let level_of = |l: &str| {
        let t = l.trim_start();
        let n = t.chars().take_while(|c| *c == '#').count();
        ((1..=6).contains(&n) && t[n..].starts_with(' ')).then(|| (n, t[n..].trim().trim_end_matches('#').trim().to_lowercase()))
    };
    let mut fence = false;
    let mut start: Option<(usize, usize)> = None;
    let mut end = lines.len();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        match (start, level_of(l)) {
            (None, Some((lvl, h))) if h == wanted => start = Some((i, lvl)),
            (Some((_, lvl)), Some((other, _))) if other <= lvl => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    start?;
    let before: String = lines[..end].concat();
    let after: String = lines[end..].concat();
    let add = markdown.trim_matches(['\n', '\r']);
    let body = before.trim_end_matches(['\n', '\r']);
    let tail = if after.is_empty() { String::new() } else { format!("\n{after}") };
    Some(format!("{body}\n\n{add}\n{tail}"))
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
    /// Связанные папки хранилища: картинки из них тоже показываются.
    pub linked: &'a [(String, PathBuf)],
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
        resolve_file(self.vault, self.linked, self.note, path).map(|f| crate::app::file_url(&f))
    }

    fn image(&mut self, ui: &mut Ui, _pal: &forma_ui::theme::Palette, path: &str, width: Option<f32>, alt: &str) {
        match resolve_file(self.vault, self.linked, self.note, path) {
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

    const CHAPTER: &str = "---\nchapter: \"0.1\"\n---\n\n- [ ] **Не практика.** Пункт до раздела.\n\n## Практика\n\n- [ ] **Свой package.** Создай `airfield`.\n  Готово, когда: печатает три строки.\n- [x] **Обнови toolchain.** Выполни `rustup update`.\n\n## Что дальше\n\n- [ ] **Тоже не практика.**\n";

    #[test]
    fn tick_matches_learn_exactly() {
        // Те же случаи, что у `learn_core::chapter::tick`.
        let ticked = tick(CHAPTER, "Свой package.").unwrap();
        assert!(ticked.contains("- [x] **Свой package.**"));
        assert!(ticked.contains("- [ ] **Не практика.**") && ticked.contains("- [ ] **Тоже не практика.**"), "пункты вне раздела не трогаются");
        assert_eq!(ticked.len(), CHAPTER.len());
        assert_eq!(tick(&ticked, "Свой package.").unwrap(), ticked, "второй раз — без изменений");
        assert_eq!(tick(CHAPTER, "Обнови toolchain.").unwrap(), CHAPTER, "уже отмеченный — как был");
        assert!(tick(CHAPTER, "Не практика.").is_none() && tick(CHAPTER, "Свой package").is_none());
        assert!(tick(&CHAPTER.replace('\n', "\r\n"), "Свой package.").unwrap().contains("- [x] **Свой package.** Создай `airfield`.\r\n"), "CRLF сохраняется");
    }

    #[test]
    fn appends_go_to_the_end_of_the_note_or_section() {
        assert_eq!(append_to_note("", "## Вопрос\n\nОтвет\n"), "## Вопрос\n\nОтвет\n");
        assert_eq!(append_to_note("# Вопросы\n\n\n", "## Второй"), "# Вопросы\n\n## Второй\n");
        let text = "# Глава\n\n## Практика\n\n- [ ] **А.**\n\n```md\n## не заголовок\n```\n\n### Подраздел\n\nтекст\n\n## Что дальше\n\nконец\n";
        let out = append_to_section(text, "практика", "Заметка Мьюри.").unwrap();
        assert!(out.contains("текст\n\nЗаметка Мьюри.\n\n## Что дальше"), "в конец раздела, подразделы — его часть: {out}");
        assert!(append_to_section("# A\n", "Нет такого", "x").is_none());
        assert!(append_to_section("## Последний\nтекст", "последний", "x").unwrap().ends_with("текст\n\nx\n"));
    }

    #[test]
    fn course_vault_links_practice_and_related_folders() {
        let base = std::env::temp_dir().join(format!("holo-linked-{}", std::process::id()));
        let vault = base.join("Vaults").join("ModelLearn");
        std::fs::create_dir_all(vault.join(".holotype")).unwrap();
        std::fs::create_dir_all(vault.join(".model-kit")).unwrap();
        std::fs::create_dir_all(base.join("Projects").join("Practice")).unwrap();
        std::fs::create_dir_all(base.join("Vaults").join("RustLearn")).unwrap();
        std::fs::write(vault.join(".holotype").join("assistant.toml"), "course = \".model-kit/course.toml\"\n").unwrap();
        std::fs::write(vault.join(".model-kit").join("course.toml"), "practice = \"../../Projects/Practice\"\nrelated = [\"../RustLearn\", \"../Missing\"]\n").unwrap();
        let linked = linked_dirs(&vault);
        assert_eq!(linked.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["practice", "RustLearn"], "несуществующие пропущены");
        assert!(linked[0].1.ends_with("Practice"));
        assert!(!linked[0].1.to_string_lossy().starts_with(r"\\?\"), "без приставки verbatim: {:?}", linked[0].1);
        assert_eq!(plain(PathBuf::from(r"\\?\C:\a\b")), PathBuf::from(r"C:\a\b"));
        assert_eq!(plain(PathBuf::from(r"\\?\UNC\srv\share\x")), PathBuf::from(r"\\srv\share\x"));
        assert!(linked_dirs(&base).is_empty(), "не курс — нет связанных папок");
        let _ = std::fs::remove_dir_all(&base);
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
