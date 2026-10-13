//! Хранилище — обычная папка с .md-файлами и вложениями. Здесь же индекс заметок:
//! ссылки, теги, заголовки — всё, что нужно для навигации, обратных ссылок и графа.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use forma_i18n::t;

use crate::markdown::{self, HeadingInfo};

pub struct Note {
    /// Путь относительно корня хранилища, например `Конспекты/Rust.md`.
    pub rel: PathBuf,
    /// Имя файла без расширения — так на заметку ссылаются `[[Имя]]`.
    pub name: String,
    pub content: String,
    pub links: Vec<String>,
    pub tags: Vec<String>,
    pub headings: Vec<HeadingInfo>,
    pub modified: Option<SystemTime>,
}

impl Note {
    fn from_content(rel: PathBuf, content: String, modified: Option<SystemTime>) -> Self {
        let name = rel.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let mut note = Note { rel, name, content, links: Vec::new(), tags: Vec::new(), headings: Vec::new(), modified };
        note.reindex();
        note
    }

    fn reindex(&mut self) {
        let doc = markdown::parse(&self.content);
        self.links = doc.note_links;
        self.tags = doc.tags;
        self.headings = doc.headings;
    }

    pub fn folder(&self) -> PathBuf {
        self.rel.parent().map(Path::to_path_buf).unwrap_or_default()
    }
}

pub struct SearchHit {
    pub note: usize,
    pub text: String,
    /// Байтовые диапазоны совпадений внутри `text`.
    pub ranges: Vec<(usize, usize)>,
    /// Смещение начала строки в заметке — чтобы прокрутить редактор.
    pub offset: usize,
}

pub struct Vault {
    pub root: PathBuf,
    pub notes: Vec<Note>,
    pub folders: Vec<PathBuf>,
    pub attachments: Vec<PathBuf>,
    /// Увеличивается при любом изменении структуры или связей (для кэшей графа, дерева и т.п.).
    pub generation: u64,
}

/// Папки вложений, которые узнаются в любом хранилище (первая найденная и используется).
const ATTACHMENTS_DIRS: &[&str] = &["Вложения", "Attachments"];

fn is_hidden(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules" || name == "target"
}

fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

fn sort_key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase().replace('\\', "/")
}

/// Убирает из имени символы, недопустимые в именах файлов.
pub fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { ' ' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim().to_owned();
    if cleaned.is_empty() { t!("vault-untitled") } else { cleaned }
}

impl Vault {
    pub fn open(root: &Path) -> io::Result<Self> {
        if !root.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, t!("vault-error-not-found", path = root.display().to_string())));
        }
        let mut v = Vault { root: root.to_path_buf(), notes: Vec::new(), folders: Vec::new(), attachments: Vec::new(), generation: 1 };
        v.rescan();
        Ok(v)
    }

    pub fn name(&self) -> String {
        self.root.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| t!("vault-default-name"))
    }

    pub fn abs(&self, rel: &Path) -> PathBuf {
        self.root.join(rel)
    }

    pub fn rescan(&mut self) {
        let mut notes = Vec::new();
        let mut folders = Vec::new();
        let mut attachments = Vec::new();
        let mut stack = vec![PathBuf::new()];
        while let Some(rel_dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(self.root.join(&rel_dir)) else { continue };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if is_hidden(&name) {
                    continue;
                }
                let rel = rel_dir.join(&name);
                let Ok(ft) = entry.file_type() else { continue };
                if ft.is_dir() {
                    folders.push(rel.clone());
                    stack.push(rel);
                } else if is_markdown(&rel) {
                    let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                    let modified = entry.metadata().ok().and_then(|m| m.modified().ok());
                    notes.push(Note::from_content(rel, content, modified));
                } else {
                    attachments.push(rel);
                }
            }
        }
        notes.sort_by_key(|n| sort_key(&n.rel));
        folders.sort_by_key(|p| sort_key(p));
        attachments.sort_by_key(|p| sort_key(p));
        self.notes = notes;
        self.folders = folders;
        self.attachments = attachments;
        self.generation += 1;
    }

    fn resort(&mut self) {
        self.notes.sort_by_key(|n| sort_key(&n.rel));
        self.folders.sort_by_key(|p| sort_key(p));
        self.attachments.sort_by_key(|p| sort_key(p));
        self.generation += 1;
    }

    pub fn index_of(&self, rel: &Path) -> Option<usize> {
        self.notes.iter().position(|n| n.rel == rel)
    }

    /// Находит заметку по цели ссылки так же, как Obsidian: сначала по пути, потом по имени,
    /// а при нескольких совпадениях предпочитает заметку из той же папки.
    pub fn resolve(&self, target: &str, from: Option<&Path>) -> Option<usize> {
        let part = markdown::link_note_part(target).replace('\\', "/");
        if part.is_empty() {
            return from.and_then(|f| self.index_of(f));
        }
        let part = part.strip_suffix(".md").unwrap_or(&part).trim_start_matches("./").to_lowercase();
        if part.contains('/') {
            let exact = self.notes.iter().position(|n| {
                let p = sort_key(&n.rel);
                p.strip_suffix(".md").unwrap_or(&p) == part
            });
            if exact.is_some() {
                return exact;
            }
            // Путь относительно папки текущей заметки.
            if let Some(dir) = from.and_then(Path::parent) {
                let joined = sort_key(&dir.join(&part));
                if let Some(i) = self.notes.iter().position(|n| {
                    let p = sort_key(&n.rel);
                    p.strip_suffix(".md").unwrap_or(&p) == joined
                }) {
                    return Some(i);
                }
            }
            return self.notes.iter().position(|n| {
                let p = sort_key(&n.rel);
                p.strip_suffix(".md").unwrap_or(&p).ends_with(&format!("/{part}"))
            });
        }
        let candidates: Vec<usize> =
            self.notes.iter().enumerate().filter(|(_, n)| n.name.to_lowercase() == part).map(|(i, _)| i).collect();
        match candidates.len() {
            0 => None,
            1 => Some(candidates[0]),
            _ => {
                let from_dir = from.and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
                candidates
                    .iter()
                    .copied()
                    .find(|&i| self.notes[i].folder() == from_dir)
                    .or_else(|| candidates.iter().copied().min_by_key(|&i| self.notes[i].rel.components().count()))
            }
        }
    }

    /// Абсолютный путь к вложению (картинке), на которое ссылается заметка.
    pub fn resolve_attachment(&self, src: &str, from: &Path) -> Option<PathBuf> {
        let src = src.split('|').next().unwrap_or(src).trim();
        let src = src.strip_prefix("file:///").unwrap_or(src);
        let decoded = src.replace("%20", " ");
        let p = Path::new(&decoded);
        if p.is_absolute() && p.exists() {
            return Some(p.to_path_buf());
        }
        if let Some(dir) = from.parent() {
            let candidate = self.root.join(dir).join(p);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        let candidate = self.root.join(p);
        if candidate.exists() {
            return Some(candidate);
        }
        let file_name = p.file_name()?.to_string_lossy().to_lowercase();
        self.attachments
            .iter()
            .find(|a| a.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == file_name))
            .map(|a| self.root.join(a))
    }

    /// Заметки, ссылающиеся на заметку `idx`, с первой строкой, где встречается ссылка.
    pub fn backlinks(&self, idx: usize) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        let target_name = self.notes[idx].name.to_lowercase();
        for (i, n) in self.notes.iter().enumerate() {
            if i == idx {
                continue;
            }
            let links_here = n.links.iter().any(|l| self.resolve(l, Some(&n.rel)) == Some(idx));
            if links_here {
                let line = n
                    .content
                    .lines()
                    .find(|l| l.to_lowercase().contains(&target_name))
                    .unwrap_or("")
                    .trim()
                    .chars()
                    .take(140)
                    .collect();
                out.push((i, line));
            }
        }
        out
    }

    /// Все теги хранилища с количеством заметок.
    pub fn all_tags(&self) -> Vec<(String, usize)> {
        let mut map: BTreeMap<String, usize> = BTreeMap::new();
        for n in &self.notes {
            for t in &n.tags {
                *map.entry(t.clone()).or_default() += 1;
            }
        }
        map.into_iter().collect()
    }

    /// Полнотекстовый поиск. Запрос `#тег` ищет заметки с тегом.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let q = query.trim();
        let mut hits = Vec::new();
        if q.is_empty() {
            return hits;
        }
        if let Some(tag) = q.strip_prefix('#') {
            let tag = tag.to_lowercase();
            for (i, n) in self.notes.iter().enumerate() {
                if n.tags.iter().any(|t| t.to_lowercase() == tag || t.to_lowercase().starts_with(&format!("{tag}/"))) {
                    hits.push(SearchHit { note: i, text: n.name.clone(), ranges: Vec::new(), offset: 0 });
                }
            }
            return hits;
        }
        let ql = q.to_lowercase();
        for (i, n) in self.notes.iter().enumerate() {
            let mut offset = 0;
            let mut per_note = 0;
            if n.name.to_lowercase().contains(&ql) {
                hits.push(SearchHit { note: i, text: String::new(), ranges: Vec::new(), offset: 0 });
                per_note += 1;
            }
            for line in n.content.split('\n') {
                let lower = line.to_lowercase();
                // Диапазоны переносим на исходную строку, только если длины в байтах совпадают.
                if lower.len() == line.len() && lower.contains(&ql) {
                    let mut ranges: Vec<(usize, usize)> = lower.match_indices(&ql).map(|(s, m)| (s, s + m.len())).collect();
                    // Равная длина ещё не значит равные границы: 'İ' удлиняется, а 'Ω' (знак Ома) укорачивается.
                    if !ranges.iter().all(|&(s, e)| line.is_char_boundary(s) && line.is_char_boundary(e)) {
                        ranges.clear();
                    }
                    let (text, ranges) = clip_line(line, &ranges);
                    hits.push(SearchHit { note: i, text, ranges, offset });
                    per_note += 1;
                } else if lower.contains(&ql) {
                    let (text, _) = clip_line(line, &[]);
                    hits.push(SearchHit { note: i, text, ranges: Vec::new(), offset });
                    per_note += 1;
                }
                offset += line.len() + 1;
                if per_note >= 6 {
                    break;
                }
            }
            if hits.len() >= limit {
                break;
            }
        }
        hits
    }

    fn unique_path(&self, folder: &Path, stem: &str, ext: &str) -> PathBuf {
        let mut n = 0;
        loop {
            let name = if n == 0 { format!("{stem}{ext}") } else { format!("{stem} {n}{ext}") };
            let rel = folder.join(&name);
            if !self.abs(&rel).exists() {
                return rel;
            }
            n += 1;
        }
    }

    pub fn create_note(&mut self, folder: &Path, name: &str, content: &str) -> io::Result<PathBuf> {
        let stem = sanitize_name(name);
        let rel = self.unique_path(folder, &stem, ".md");
        let abs = self.abs(&rel);
        if let Some(dir) = abs.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&abs, content)?;
        let modified = std::fs::metadata(&abs).ok().and_then(|m| m.modified().ok());
        self.notes.push(Note::from_content(rel.clone(), content.to_owned(), modified));
        // Заметка могла лежать в новых папках — добавим их в список.
        let mut dir = rel.parent();
        while let Some(d) = dir {
            if d.as_os_str().is_empty() {
                break;
            }
            if !self.folders.iter().any(|f| f == d) {
                self.folders.push(d.to_path_buf());
            }
            dir = d.parent();
        }
        self.resort();
        Ok(rel)
    }

    pub fn create_folder(&mut self, parent: &Path, name: &str) -> io::Result<PathBuf> {
        let stem = sanitize_name(name);
        let rel = self.unique_path(parent, &stem, "");
        std::fs::create_dir_all(self.abs(&rel))?;
        self.folders.push(rel.clone());
        self.resort();
        Ok(rel)
    }

    pub fn save(&mut self, rel: &Path, content: &str) -> io::Result<()> {
        let abs = self.abs(rel);
        std::fs::write(&abs, content)?;
        let modified = std::fs::metadata(&abs).ok().and_then(|m| m.modified().ok());
        if let Some(i) = self.index_of(rel) {
            let note = &mut self.notes[i];
            let old_links = std::mem::take(&mut note.links);
            let old_tags = std::mem::take(&mut note.tags);
            note.content = content.to_owned();
            note.modified = modified;
            note.reindex();
            if note.links != old_links || note.tags != old_tags {
                self.generation += 1;
            }
        } else {
            self.notes.push(Note::from_content(rel.to_path_buf(), content.to_owned(), modified));
            self.resort();
        }
        Ok(())
    }

    /// Переименовывает заметку или папку. Для заметок обновляет `[[ссылки]]` во всём хранилище.
    /// Возвращает новый путь и список заметок, содержимое которых изменилось.
    pub fn rename(&mut self, rel: &Path, new_name: &str) -> io::Result<(PathBuf, Vec<PathBuf>)> {
        let abs = self.abs(rel);
        let parent = rel.parent().map(Path::to_path_buf).unwrap_or_default();
        let is_dir = abs.is_dir();
        let stem = sanitize_name(new_name);
        let new_rel = if is_dir { parent.join(&stem) } else { parent.join(format!("{stem}.md")) };
        if new_rel == rel {
            return Ok((new_rel, Vec::new()));
        }
        let new_abs = self.abs(&new_rel);
        // На Windows разрешаем смену регистра, иначе проверяем, что имя свободно.
        let only_case = sort_key(&new_rel) == sort_key(rel);
        if new_abs.exists() && !only_case {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, t!("vault-error-exists")));
        }
        std::fs::rename(&abs, &new_abs)?;

        let mut changed = Vec::new();
        if is_dir {
            let remap = |p: &mut PathBuf| {
                if let Ok(rest) = p.strip_prefix(rel) {
                    *p = new_rel.join(rest);
                }
            };
            for n in &mut self.notes {
                remap(&mut n.rel);
            }
            for f in &mut self.folders {
                remap(f);
            }
            for a in &mut self.attachments {
                remap(a);
            }
        } else if let Some(i) = self.index_of(rel) {
            let old_name = self.notes[i].name.clone();
            let old_path = sort_key(rel);
            let old_path = old_path.strip_suffix(".md").unwrap_or(&old_path).to_owned();
            self.notes[i].rel = new_rel.clone();
            self.notes[i].name = stem.clone();
            for j in 0..self.notes.len() {
                if let Some(updated) = rewrite_links(&self.notes[j].content, &old_name, &old_path, &stem) {
                    let path = self.abs(&self.notes[j].rel);
                    if std::fs::write(&path, &updated).is_ok() {
                        self.notes[j].content = updated;
                        self.notes[j].modified = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
                        self.notes[j].reindex();
                        changed.push(self.notes[j].rel.clone());
                    }
                }
            }
        }
        self.resort();
        Ok((new_rel, changed))
    }

    /// Перемещает заметку или папку в другую папку.
    pub fn move_to(&mut self, rel: &Path, folder: &Path) -> io::Result<PathBuf> {
        let Some(name) = rel.file_name() else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, t!("vault-error-empty-path")));
        };
        if folder.starts_with(rel) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, t!("vault-error-into-itself")));
        }
        let new_rel = folder.join(name);
        if new_rel == rel {
            return Ok(new_rel);
        }
        if self.abs(&new_rel).exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, t!("vault-error-exists-in-folder")));
        }
        std::fs::rename(self.abs(rel), self.abs(&new_rel))?;
        let remap = |p: &mut PathBuf| {
            if p == rel {
                *p = new_rel.clone();
            } else if let Ok(rest) = p.strip_prefix(rel) {
                *p = new_rel.join(rest);
            }
        };
        for n in &mut self.notes {
            remap(&mut n.rel);
        }
        for f in &mut self.folders {
            remap(f);
        }
        for a in &mut self.attachments {
            remap(a);
        }
        self.resort();
        Ok(new_rel)
    }

    /// Отправляет файл или папку в корзину системы.
    pub fn delete(&mut self, rel: &Path) -> Result<(), String> {
        trash::delete(self.abs(rel)).map_err(|e| e.to_string())?;
        self.notes.retain(|n| !n.rel.starts_with(rel));
        self.folders.retain(|f| !f.starts_with(rel));
        self.attachments.retain(|a| !a.starts_with(rel));
        self.generation += 1;
        Ok(())
    }

    /// Копирует внешний файл (например, перетащенную картинку) в папку вложений.
    /// Папка вложений: уже существующая («Вложения» или «Attachments»), а в новом
    /// хранилище — на языке интерфейса.
    pub fn attachments_dir(&self) -> PathBuf {
        ATTACHMENTS_DIRS
            .iter()
            .map(PathBuf::from)
            .find(|d| self.folders.contains(d))
            .unwrap_or_else(|| PathBuf::from(t!("vault-attachments")))
    }

    pub fn import_attachment(&mut self, src: &Path) -> io::Result<PathBuf> {
        let file_name = src.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| t!("vault-file"));
        let (stem, ext) = match file_name.rsplit_once('.') {
            Some((s, e)) => (sanitize_name(s), format!(".{e}")),
            None => (sanitize_name(&file_name), String::new()),
        };
        let folder = self.attachments_dir();
        std::fs::create_dir_all(self.abs(&folder))?;
        if !self.folders.iter().any(|f| f == &folder) {
            self.folders.push(folder.clone());
        }
        let rel = self.unique_path(&folder, &stem, &ext);
        std::fs::copy(src, self.abs(&rel))?;
        self.attachments.push(rel.clone());
        self.resort();
        Ok(rel)
    }
}

/// Обрезает длинную строку результата поиска вокруг первого совпадения.
fn clip_line(line: &str, ranges: &[(usize, usize)]) -> (String, Vec<(usize, usize)>) {
    const MAX: usize = 160;
    if line.len() <= MAX {
        return (line.to_owned(), ranges.to_vec());
    }
    let first = ranges.first().map_or(0, |r| r.0);
    let mut start = first.saturating_sub(50);
    while !line.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + MAX).min(line.len());
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    let prefix = if start > 0 { "…" } else { "" };
    let text = format!("{prefix}{}", &line[start..end]);
    let shift = prefix.len() as isize - start as isize;
    let ranges = ranges
        .iter()
        .filter(|(s, e)| *s >= start && *e <= end)
        .map(|(s, e)| ((*s as isize + shift) as usize, (*e as isize + shift) as usize))
        .collect();
    (text, ranges)
}

/// Заменяет вики-ссылки на переименованную заметку. Возвращает None, если ничего не изменилось.
fn rewrite_links(content: &str, old_name: &str, old_path: &str, new_name: &str) -> Option<String> {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    let mut changed = false;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start + 2]);
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else { break };
        let inner = &rest[..end];
        let split = inner.find(['#', '|']).unwrap_or(inner.len());
        let target = inner[..split].trim();
        let target_l = target.replace('\\', "/").to_lowercase();
        let target_l = target_l.strip_suffix(".md").unwrap_or(&target_l);
        if !target.is_empty() && (target_l == old_name.to_lowercase() || target_l == old_path) {
            // Если ссылка была с путём, сохраняем папку.
            let replaced = match target.rfind('/') {
                Some(slash) => format!("{}/{new_name}", &target[..slash]),
                None => new_name.to_owned(),
            };
            out.push_str(&replaced);
            out.push_str(&inner[split..]);
            changed = true;
        } else {
            out.push_str(inner);
        }
        out.push_str("]]");
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    changed.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_keeps_alias_and_heading() {
        let src = "См. [[Старое]] и [[старое#Раздел|тут]], но не [[Другое]]. ![[Старое]]";
        let out = rewrite_links(src, "Старое", "папка/старое", "Новое").unwrap();
        assert_eq!(out, "См. [[Новое]] и [[Новое#Раздел|тут]], но не [[Другое]]. ![[Новое]]");
        assert!(rewrite_links("без ссылок", "a", "a", "b").is_none());
    }

    #[test]
    fn sanitize() {
        assert_eq!(sanitize_name("  a/b:c?  "), "a b c");
        assert!(!sanitize_name("...").is_empty(), "пустое имя заменяется подписью «Без названия»");
    }

    #[test]
    fn search_ranges_on_char_boundaries() {
        let note = |rel: &str, content: &str| Note::from_content(PathBuf::from(rel), content.to_owned(), None);
        let vault = Vault {
            root: PathBuf::new(),
            notes: vec![note("a.md", "İ\u{2126}i"), note("b.md", "Привет, МИР")],
            folders: Vec::new(),
            attachments: Vec::new(),
            generation: 1,
        };
        // Длина строки после to_lowercase та же, но границы символов сдвинуты — подсветки нет, падения тоже.
        for hit in vault.search("i", 10).iter().chain(&vault.search("мир", 10)) {
            assert!(hit.ranges.iter().all(|&(s, e)| hit.text.is_char_boundary(s) && hit.text.is_char_boundary(e)), "{:?}", hit.ranges);
        }
        let hits = vault.search("мир", 10);
        assert_eq!(hits[0].ranges.len(), 1, "кириллица без сдвигов подсвечивается как раньше");
        assert_eq!(&hits[0].text[hits[0].ranges[0].0..hits[0].ranges[0].1], "МИР");
    }
}
