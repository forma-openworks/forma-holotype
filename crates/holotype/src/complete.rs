//! Подсказки при вводе: заметки в `[[`, картинки и файлы в `![[`, разделы после `#`,
//! пути в `](`, теги, типы плашек, языки блоков кода и цвета атрибутов `{…}`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui::Color32;

use crate::markdown;
use crate::theme;
use crate::vault::Vault;
use forma_i18n::t;

/// Где находится строка: в обычном тексте, в блоке кода, в свойствах (`---`) или в формуле `$$`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Block {
    Normal,
    Code,
    Front,
    Math,
}

/// Состояние в начале строки `line_start` (правила те же, что у подсветки редактора).
pub fn block_at(text: &str, line_start: usize) -> Block {
    let mut fence: Option<String> = None;
    let mut front = false;
    let mut math = false;
    for (n, line) in text[..line_start.min(text.len())].lines().enumerate() {
        let t = line.trim_start();
        if n == 0 && line.trim_end() == "---" {
            front = true;
        } else if front {
            if line.trim_end() == "---" {
                front = false;
            }
        } else if let Some(f) = &fence {
            if t.starts_with(f.as_str()) {
                fence = None;
            }
        } else if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(t.chars().take_while(|c| *c == '`' || *c == '~').collect());
        } else if let Some(rest) = t.strip_prefix("$$") {
            let rest = rest.trim_end();
            if !(rest.ends_with("$$") && rest.len() >= 2) {
                math = !math;
            }
        }
    }
    if front {
        Block::Front
    } else if fence.is_some() {
        Block::Code
    } else if math {
        Block::Math
    } else {
        Block::Normal
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// `[[запрос` — заметки.
    Note,
    /// `![[запрос` — картинки, файлы и заметки для встраивания.
    Embed,
    /// `[[Заметка#запрос` — разделы заметки (пустое имя — текущая заметка).
    Heading(String),
    /// `](запрос` — путь к файлу; true — картинка `![…](`.
    Path(bool),
    /// `#запрос` в тексте или тег в свойствах `tags: [...]`.
    Tag,
    /// `> [!запрос` — тип плашки.
    Callout,
    /// ```` ```запрос ```` — язык блока кода. Внутри — забор, если блок ещё не закрыт.
    Lang(Option<String>),
    /// `` `код`{запрос `` и `==текст=={запрос` — цвета и бейдж.
    Attr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    pub kind: Kind,
    /// Байтовый диапазон, который заменяет выбранная подсказка.
    pub start: usize,
    pub end: usize,
    pub query: String,
    /// Длина закрывающей скобки сразу после `end` (курсор перепрыгнет через неё).
    pub close_len: usize,
    /// Закрывающей скобки нет — её нужно дописать.
    pub needs_close: bool,
}

/// Определяет, что подсказывать в позиции курсора (байтовое смещение).
/// `force` (Ctrl+Space) разрешает подсказки и без набранного текста.
pub fn detect(text: &str, cursor: usize, force: bool) -> Option<Context> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let ls = text[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let le = text[cursor..].find('\n').map_or(text.len(), |i| cursor + i);
    let line = &text[ls..cursor];
    let rest = &text[cursor..le];
    match block_at(text, ls) {
        Block::Code | Block::Math => return None,
        Block::Front => return front_tags(line, rest, ls, cursor, force),
        Block::Normal => {}
    }
    let t = line.trim_start();

    // ```язык
    if t.starts_with("```") || t.starts_with("~~~") {
        let fence: String = t.chars().take_while(|c| *c == '`' || *c == '~').collect();
        let q = &t[fence.len()..];
        let lang_char = |c: char| c.is_alphanumeric() || matches!(c, '+' | '-' | '#' | '_');
        if !q.chars().all(lang_char) {
            return None;
        }
        let len = rest.find(|c: char| !lang_char(c)).unwrap_or(rest.len());
        let open = fence_unclosed(text, le).then_some(fence);
        return Some(Context { kind: Kind::Lang(open), start: cursor - q.len(), end: cursor + len, query: q.to_owned(), close_len: 0, needs_close: true });
    }

    // Внутри `кода` подсказки не нужны.
    if line.matches('`').count() % 2 == 1 {
        return None;
    }

    // [[ссылка и ![[встраивание
    if let Some(s) = line.rfind("[[") {
        let inner = &line[s + 2..];
        if !inner.contains("]]") {
            if inner.contains('|') {
                return None;
            }
            let embed = line[..s].ends_with('!');
            let (end, close_len, needs_close) = wiki_end(rest, cursor);
            if let Some(h) = inner.find('#') {
                let note = inner[..h].trim().to_owned();
                let q = &inner[h + 1..];
                let start = ls + s + 2 + h + 1;
                return Some(Context { kind: Kind::Heading(note), start, end, query: q.to_owned(), close_len, needs_close });
            }
            let kind = if embed { Kind::Embed } else { Kind::Note };
            return Some(Context { kind, start: ls + s + 2, end, query: inner.to_owned(), close_len, needs_close });
        }
    }

    // [текст](путь и ![подпись](путь
    if let Some(p) = line.rfind("](") {
        let q = &line[p + 2..];
        if !q.contains(')') && !q.contains(char::is_whitespace) && !q.contains("://") {
            let image = line[..p].rfind('[').is_some_and(|b| line[..b].ends_with('!'));
            let len = rest.find(|c: char| c == ')' || c.is_whitespace()).unwrap_or(rest.len());
            let closed = rest[len..].starts_with(')');
            return Some(Context {
                kind: Kind::Path(image),
                start: ls + p + 2,
                end: cursor + len,
                query: q.to_owned(),
                close_len: usize::from(closed),
                needs_close: !closed,
            });
        }
    }

    // > [!тип плашки
    if t.starts_with('>') {
        let unq = t.trim_start_matches(|c: char| c == '>' || c.is_whitespace());
        if let Some(q) = unq.strip_prefix("[!") {
            if q.chars().all(char::is_alphanumeric) {
                let len = rest.find(|c: char| !c.is_alphanumeric()).unwrap_or(rest.len());
                let closed = rest[len..].starts_with(']');
                return Some(Context {
                    kind: Kind::Callout,
                    start: cursor - q.len(),
                    end: cursor + len,
                    query: q.to_owned(),
                    close_len: usize::from(closed),
                    needs_close: !closed,
                });
            }
        }
    }

    // `код`{цвет и ==текст=={цвет
    if let Some(b) = line.rfind('{') {
        let inner = &line[b + 1..];
        let before = &line[..b];
        if (before.ends_with('`') || before.ends_with("==")) && inner.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '#') {
            let word = inner.rfind(' ').map_or(0, |i| i + 1);
            let len = rest.find(|c: char| !c.is_alphanumeric() && c != '#').unwrap_or(rest.len());
            let closed = rest[len..].starts_with('}');
            return Some(Context {
                kind: Kind::Attr,
                start: ls + b + 1 + word,
                end: cursor + len,
                query: inner[word..].to_owned(),
                close_len: usize::from(closed),
                needs_close: !closed,
            });
        }
    }

    tag_context(line, rest, ls, cursor, force, true)
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

/// `#тег` перед курсором (или слово в `tags: [...]`, если `hash == false`).
fn tag_context(line: &str, rest: &str, ls: usize, cursor: usize, force: bool, hash: bool) -> Option<Context> {
    let q_start = line.char_indices().rev().take_while(|(_, c)| is_tag_char(*c)).last().map_or(line.len(), |(i, _)| i);
    let before = &line[..q_start];
    if hash {
        let pre = before.strip_suffix('#')?.chars().next_back();
        if !pre.is_none_or(|p| p.is_whitespace() || "([{,;".contains(p)) {
            return None;
        }
    } else if !before.ends_with(['[', ',', ' ', ':']) {
        return None;
    }
    let q = &line[q_start..];
    if q.is_empty() && !force {
        return None;
    }
    let len = rest.find(|c: char| !is_tag_char(c)).unwrap_or(rest.len());
    Some(Context { kind: Kind::Tag, start: ls + q_start, end: cursor + len, query: q.to_owned(), close_len: 0, needs_close: false })
}

/// Теги в свойствах заметки: `tags: [rust, вул|`.
fn front_tags(line: &str, rest: &str, ls: usize, cursor: usize, force: bool) -> Option<Context> {
    let t = line.trim_start();
    if !(t.starts_with("tags:") || t.starts_with("теги:")) {
        return None;
    }
    let colon = line.find(':')? + 1;
    let c = tag_context(line, rest, ls, cursor, force, false)?;
    (c.start >= ls + colon).then_some(c)
}

/// Конец вики-ссылки после курсора: до `]]`, `|` или `#`. Если ничего нет — ссылку надо закрыть.
fn wiki_end(rest: &str, cursor: usize) -> (usize, usize, bool) {
    for (i, c) in rest.char_indices() {
        if rest[i..].starts_with("]]") {
            return (cursor + i, 2, false);
        }
        if c == '|' || c == '#' {
            return (cursor + i, 0, false);
        }
        if rest[i..].starts_with("[[") {
            break;
        }
    }
    (cursor, 0, true)
}

/// Открывающий забор на строке, которая кончается в `line_end`, ещё не закрыт: ниже заборы идут
/// парами (чужие блоки), а наш закрывающий сделал бы их число нечётным.
fn fence_unclosed(text: &str, line_end: usize) -> bool {
    let below = text.get(line_end + 1..).unwrap_or("");
    below
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("```") || t.starts_with("~~~")
        })
        .count()
        % 2
        == 0
}

// ---------------------------------------------------------------------------------------------
// Варианты
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub enum Icon {
    Note,
    Image,
    File,
    Heading(u8),
    Tag,
    Callout(Color32, &'static str),
    Lang,
    Color(Color32),
    Badge,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub label: String,
    pub detail: String,
    pub icon: Icon,
    /// Чем заменить запрос.
    pub insert: String,
    /// Что дописать после вставки, если закрывающей скобки ещё нет.
    pub tail: String,
    /// Сколько символов `tail` оставить перед курсором.
    pub tail_caret: usize,
    /// Картинка для превью.
    pub image: Option<PathBuf>,
}

impl Item {
    fn new(label: impl Into<String>, detail: impl Into<String>, icon: Icon, insert: impl Into<String>) -> Self {
        Item { label: label.into(), detail: detail.into(), icon, insert: insert.into(), tail: String::new(), tail_caret: 0, image: None }
    }

    fn tail(mut self, tail: &str, caret: usize) -> Self {
        self.tail = tail.to_owned();
        self.tail_caret = caret;
        self
    }
}

const MAX_ITEMS: usize = 60;

/// Варианты для контекста. `text` — текст заметки (нужен для её собственных разделов).
pub fn items(ctx: &Context, text: &str, vault: &Vault, current: &Path) -> Vec<Item> {
    let q = ctx.query.as_str();
    let mut scored: Vec<(i32, Item)> = Vec::new();
    match &ctx.kind {
        Kind::Note => {
            for (i, n) in vault.notes.iter().enumerate() {
                if n.rel == current {
                    continue;
                }
                if let Some(s) = note_score(q, &n.name, &n.rel) {
                    let item = Item::new(&n.name, folder_of(&n.rel), Icon::Note, link_target(vault, i, current)).tail("]]", 2);
                    scored.push((s, item));
                }
            }
        }
        Kind::Embed => {
            let names = file_name_counts(vault);
            for a in &vault.attachments {
                let name = a.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let path = a.to_string_lossy().replace('\\', "/");
                let Some(s) = fuzzy_score(q, &name).max(fuzzy_score(q, &path).map(|s| s - 5)) else { continue };
                let image = markdown::is_image_path(&name);
                let unique = names.get(&name.to_lowercase()).copied().unwrap_or(0) <= 1;
                let mut item = Item::new(&name, folder_of(a), if image { Icon::Image } else { Icon::File }, if unique { name.clone() } else { path })
                    .tail("]]", 2);
                if image {
                    item.image = Some(vault.abs(a));
                }
                scored.push((s + if image { 8 } else { 0 }, item));
            }
            for (i, n) in vault.notes.iter().enumerate() {
                if n.rel == current {
                    continue;
                }
                if let Some(s) = note_score(q, &n.name, &n.rel) {
                    let item = Item::new(&n.name, folder_of(&n.rel), Icon::Note, link_target(vault, i, current)).tail("]]", 2);
                    scored.push((s - 4, item));
                }
            }
        }
        Kind::Heading(note) => {
            let headings = if note.is_empty() {
                markdown::parse(text).headings
            } else {
                match vault.resolve(note, Some(current)) {
                    Some(i) => vault.notes[i].headings.clone(),
                    None => Vec::new(),
                }
            };
            for h in headings {
                if let Some(s) = fuzzy_score(q, &h.text) {
                    let item = Item::new(&h.text, "", Icon::Heading(h.level), &h.text).tail("]]", 2);
                    scored.push((s, item));
                }
            }
        }
        Kind::Path(image_only) => {
            for a in &vault.attachments {
                let name = a.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let image = markdown::is_image_path(&name);
                if *image_only && !image {
                    continue;
                }
                let path = a.to_string_lossy().replace('\\', "/");
                let Some(s) = fuzzy_score(q, &name).max(fuzzy_score(q, &path).map(|s| s - 5)) else { continue };
                let mut item = Item::new(&name, folder_of(a), if image { Icon::Image } else { Icon::File }, url_path(&path)).tail(")", 1);
                if image {
                    item.image = Some(vault.abs(a));
                }
                scored.push((s, item));
            }
            if !*image_only {
                for n in &vault.notes {
                    if n.rel == current {
                        continue;
                    }
                    if let Some(s) = note_score(q, &n.name, &n.rel) {
                        let path = n.rel.to_string_lossy().replace('\\', "/");
                        scored.push((s, Item::new(&n.name, folder_of(&n.rel), Icon::Note, url_path(&path)).tail(")", 1)));
                    }
                }
            }
        }
        Kind::Tag => {
            // Недописанный тег уже попал в индекс текущей заметки при автосохранении — его не предлагаем.
            let own = vault.index_of(current).map(|i| vault.notes[i].tags.clone()).unwrap_or_default();
            for (tag, count) in vault.all_tags() {
                if count == 1 && tag.to_lowercase() == q.to_lowercase() && own.contains(&tag) {
                    continue;
                }
                if let Some(s) = fuzzy_score(q, &tag) {
                    let detail = t!("completion-notes-count", count = count);
                    scored.push((s, Item::new(&tag, detail, Icon::Tag, &tag)));
                }
            }
        }
        Kind::Callout => {
            for kind in theme::CALLOUT_KINDS {
                let style = theme::callout(kind);
                if let Some(s) = fuzzy_score(q, kind).max(fuzzy_score(q, &style.title).map(|s| s - 5)) {
                    scored.push((s, Item::new(*kind, style.title, Icon::Callout(style.color, style.icon), *kind).tail("] ", 2)));
                }
            }
        }
        Kind::Lang(open) => {
            for (lang, name) in LANGS {
                let title = lang_title(lang, name);
                if let Some(s) = fuzzy_score(q, lang).max(fuzzy_score(q, &title).map(|s| s - 5)) {
                    let mut item = Item::new(*lang, title, Icon::Lang, *lang);
                    if let Some(fence) = open {
                        // Блок ещё не закрыт — закрываем сразу и ставим курсор внутрь.
                        item = item.tail(&format!("\n\n{fence}"), 1);
                    }
                    scored.push((s, item));
                }
            }
        }
        Kind::Attr => {
            if let Some(s) = fuzzy_score(q, "badge").max(fuzzy_score(q, "бейдж")) {
                scored.push((s + 1, Item::new("badge", t!("completion-badge"), Icon::Badge, "badge").tail("}", 1)));
            }
            for (name, key) in COLORS {
                let title = forma_i18n::tr(key);
                let color = markdown::named_color(name).map_or(Color32::GRAY, |[r, g, b]| Color32::from_rgb(r, g, b));
                if let Some(s) = fuzzy_score(q, name).max(fuzzy_score(q, &title)) {
                    scored.push((s, Item::new(*name, title, Icon::Color(color), *name).tail("}", 1)));
                }
            }
        }
    }
    // Порядок при равных очках сохраняется: разделы идут как в заметке.
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.truncate(MAX_ITEMS);
    scored.into_iter().map(|(_, i)| i).collect()
}

fn note_score(q: &str, name: &str, rel: &Path) -> Option<i32> {
    let path = rel.with_extension("").to_string_lossy().replace('\\', "/");
    fuzzy_score(q, name).max(fuzzy_score(q, &path).map(|s| s - 5))
}

fn folder_of(rel: &Path) -> String {
    rel.parent().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default()
}

/// Как сослаться на заметку: по имени, если оно однозначно, иначе по пути.
fn link_target(vault: &Vault, idx: usize, current: &Path) -> String {
    let n = &vault.notes[idx];
    if vault.resolve(&n.name, Some(current)) == Some(idx) {
        n.name.clone()
    } else {
        let p = n.rel.to_string_lossy().replace('\\', "/");
        p.strip_suffix(".md").unwrap_or(&p).to_owned()
    }
}

fn file_name_counts(vault: &Vault) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for a in &vault.attachments {
        if let Some(n) = a.file_name() {
            *map.entry(n.to_string_lossy().to_lowercase()).or_default() += 1;
        }
    }
    map
}

/// Путь для `](…)`: пробелы и скобки кодируются, иначе Markdown оборвёт ссылку.
fn url_path(path: &str) -> String {
    path.replace(' ', "%20").replace('(', "%28").replace(')', "%29")
}

/// Подпись языка в подсказке: у шейдеров, диаграмм и простого текста — переведённая.
fn lang_title(lang: &str, name: &str) -> String {
    match lang {
        "glsl" | "wgsl" | "hlsl" => t!("lang-shaders", name = name),
        "text" => t!("lang-text"),
        "mermaid" => t!("lang-mermaid"),
        "dot" => t!("lang-dot"),
        _ => name.to_owned(),
    }
}

const LANGS: &[(&str, &str)] = &[
    ("rust", "Rust"),
    ("python", "Python"),
    ("js", "JavaScript"),
    ("ts", "TypeScript"),
    ("jsx", "React JSX"),
    ("tsx", "React TSX"),
    ("c", "C"),
    ("cpp", "C++"),
    ("cs", "C#"),
    ("java", "Java"),
    ("kotlin", "Kotlin"),
    ("go", "Go"),
    ("zig", "Zig"),
    ("swift", "Swift"),
    ("glsl", "GLSL"),
    ("wgsl", "WGSL"),
    ("hlsl", "HLSL"),
    ("html", "HTML"),
    ("css", "CSS"),
    ("json", "JSON"),
    ("toml", "TOML"),
    ("yaml", "YAML"),
    ("xml", "XML"),
    ("sql", "SQL"),
    ("bash", "Bash"),
    ("powershell", "PowerShell"),
    ("lua", "Lua"),
    ("markdown", "Markdown"),
    ("text", "text"),
    ("mermaid", "Mermaid"),
    ("dot", "Graphviz"),
];

/// Цвета атрибутов и ключи их названий.
const COLORS: &[(&str, &str)] = &[
    ("red", "color-red"),
    ("orange", "color-orange"),
    ("yellow", "color-yellow"),
    ("green", "color-green"),
    ("teal", "color-teal"),
    ("cyan", "color-cyan"),
    ("blue", "color-blue"),
    ("purple", "color-purple"),
    ("pink", "color-pink"),
    ("gray", "color-gray"),
];

/// Нечёткое совпадение: все символы запроса по порядку. Больше очков за подряд идущие
/// символы и начала слов. None — нет совпадения.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    let q: Vec<char> = query.trim().to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    if q.is_empty() {
        return Some(0);
    }
    let c: Vec<char> = candidate.to_lowercase().chars().collect();
    let mut score = 0;
    let mut qi = 0;
    let mut prev_match: Option<usize> = None;
    for (i, ch) in c.iter().enumerate() {
        if qi < q.len() && *ch == q[qi] {
            score += 10;
            if prev_match == Some(i.wrapping_sub(1)) {
                score += 15;
            }
            if i == 0 || !c[i - 1].is_alphanumeric() {
                score += 20;
            }
            prev_match = Some(i);
            qi += 1;
        }
    }
    if qi < q.len() {
        return None;
    }
    if candidate.to_lowercase().starts_with(&query.trim().to_lowercase()) {
        score += 50;
    }
    Some(score - c.len() as i32 / 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_end(src: &str) -> Option<Context> {
        detect(src, src.len(), false)
    }

    #[test]
    fn wiki_contexts() {
        let c = at_end("Смотри [[Ру").unwrap();
        assert_eq!(c.kind, Kind::Note);
        assert_eq!(c.query, "Ру");
        assert!(c.needs_close);
        let c = at_end("![[обл").unwrap();
        assert_eq!(c.kind, Kind::Embed);
        let c = at_end("[[Формулы#Мат").unwrap();
        assert_eq!(c.kind, Kind::Heading("Формулы".into()));
        assert_eq!(c.query, "Мат");
        assert!(at_end("[[Заметка|текст").is_none());
        assert!(at_end("[[Готово]] и дальше").is_none());

        // Курсор внутри уже закрытой ссылки: заменяем до `]]` и перепрыгиваем через скобки.
        let src = "[[Ru]] текст";
        let c = detect(src, 4, false).unwrap();
        assert_eq!((c.start, c.end, c.close_len, c.needs_close), (2, 4, 2, false));
    }

    #[test]
    fn other_contexts() {
        assert_eq!(at_end("![картинка](Влож").unwrap().kind, Kind::Path(true));
        assert_eq!(at_end("[текст](").unwrap().kind, Kind::Path(false));
        assert_eq!(at_end("> [!ti").unwrap().kind, Kind::Callout);
        assert_eq!(at_end("```ru").unwrap().kind, Kind::Lang(Some("```".into())));
        assert_eq!(at_end("```ru\nfn main() {}\n```").map(|c| c.kind), None);
        assert_eq!(detect("```ru\n```", 5, false).unwrap().kind, Kind::Lang(None));
        assert_eq!(at_end("`код`{gr").unwrap().kind, Kind::Attr);
        assert_eq!(at_end("`код`{badge gr").unwrap().query, "gr");
        assert_eq!(at_end("текст #ru").unwrap().kind, Kind::Tag);
        assert_eq!(at_end("---\ntags: [rust, вул").unwrap().query, "вул");
    }

    #[test]
    fn no_context() {
        assert!(at_end("C#").is_none());
        assert!(at_end("## Заголовок").is_none());
        assert!(at_end("текст #").is_none());
        assert!(detect("текст #", "текст #".len(), true).is_some(), "Ctrl+Space показывает все теги");
        assert!(at_end("`[[код").is_none());
        assert!(at_end("```\n[[внутри кода").is_none());
        assert!(at_end("https://example.com/#якорь").is_none());
    }

    #[test]
    fn fuzzy() {
        assert!(fuzzy_score("rв", "Rust — владение").is_some());
        assert!(fuzzy_score("xyz", "Rust").is_none());
        let a = fuzzy_score("фор", "Формулы").unwrap();
        let b = fuzzy_score("фор", "Добро пожаловать в форум").unwrap();
        assert!(a > b);
    }
}
