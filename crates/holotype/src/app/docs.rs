//! Открытые заметки: текст, несохранённые правки, разобранный документ. Одна заметка в
//! двух вкладках — один текст. А у каждой вкладки своё состояние: история «назад/вперёд»,
//! прокрутка, курсор и подсказки редактора.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use forma_i18n::t;

use crate::assist::Assist;
use crate::markdown::{self, Doc};
use crate::vault::Vault;

/// Открытая заметка.
pub struct NoteDoc {
    pub text: String,
    pub dirty: bool,
    /// Когда текст правили последний раз: через секунду после этого он сохраняется сам.
    pub last_edit: f64,
    /// Номер правки: по нему пересобирается разобранный документ.
    pub rev: u64,
    doc: Option<(u64, Doc)>,
}

impl NoteDoc {
    fn new(text: String) -> Self {
        Self { text, dirty: false, last_edit: 0.0, rev: 1, doc: None }
    }

    pub fn mark_edited(&mut self, now: f64) {
        self.dirty = true;
        self.last_edit = now;
        self.rev += 1;
    }

    /// Разобранный документ (пересобирается после правок).
    pub fn parsed(&mut self) -> &Doc {
        if self.doc.as_ref().is_none_or(|(rev, _)| *rev != self.rev) {
            self.doc = Some((self.rev, markdown::parse(&self.text)));
        }
        &self.doc.as_ref().expect("документ только что разобран").1
    }
}

/// Все открытые заметки по путям от корня хранилища.
#[derive(Default)]
pub struct Docs {
    map: HashMap<PathBuf, NoteDoc>,
    /// Сколько раз сохраняли: от этого зависят кеши поиска и обратных ссылок.
    pub saves: u64,
}

impl Docs {
    /// Открытая заметка; ещё не открытая читается из индекса хранилища.
    pub fn get_or_load(&mut self, vault: &Vault, rel: &Path) -> Option<&mut NoteDoc> {
        if !self.map.contains_key(rel) {
            let i = vault.index_of(rel)?;
            self.map.insert(rel.to_path_buf(), NoteDoc::new(vault.notes[i].content.clone()));
        }
        self.map.get_mut(rel)
    }

    pub fn get(&self, rel: &Path) -> Option<&NoteDoc> {
        self.map.get(rel)
    }

    pub fn is_dirty(&self, rel: &Path) -> bool {
        self.map.get(rel).is_some_and(|d| d.dirty)
    }

    /// Сохраняет заметку, если в ней есть правки.
    pub fn save(&mut self, vault: &mut Vault, rel: &Path, now: f64) -> Result<(), String> {
        let Some(doc) = self.map.get_mut(rel) else { return Ok(()) };
        if !doc.dirty {
            return Ok(());
        }
        match vault.save(rel, &doc.text) {
            Ok(()) => {
                doc.dirty = false;
                self.saves += 1;
                Ok(())
            }
            Err(e) => {
                // Следующая попытка — не сразу, чтобы не сыпать ошибками каждый кадр.
                doc.last_edit = now + 5.0;
                let name = rel.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                Err(t!("toast-save-failed", name = name, error = e.to_string()))
            }
        }
    }

    /// Сохраняет все заметки с правками; возвращает ошибки.
    pub fn save_all(&mut self, vault: &mut Vault, now: f64) -> Vec<String> {
        let dirty: Vec<PathBuf> = self.map.iter().filter(|(_, d)| d.dirty).map(|(p, _)| p.clone()).collect();
        dirty.into_iter().filter_map(|rel| self.save(vault, &rel, now).err()).collect()
    }

    /// Сохраняет заметки, которые не правили уже секунду. Второе значение — есть ли ещё
    /// несохранённые правки (тогда кадр нужно повторить позже).
    pub fn autosave(&mut self, vault: &mut Vault, now: f64) -> (Vec<String>, bool) {
        let ready: Vec<PathBuf> = self.map.iter().filter(|(_, d)| d.dirty && now - d.last_edit > 1.0).map(|(p, _)| p.clone()).collect();
        let errors = ready.into_iter().filter_map(|rel| self.save(vault, &rel, now).err()).collect();
        let waiting = self.map.values().any(|d| d.dirty);
        (errors, waiting)
    }

    /// Убирает заметки, которых нет ни в одной вкладке (с правками — сначала сохраняет).
    pub fn retain_open(&mut self, vault: &mut Vault, open: &HashSet<PathBuf>, now: f64) -> Vec<String> {
        let closed: Vec<PathBuf> = self.map.keys().filter(|p| !open.contains(*p)).cloned().collect();
        let mut errors = Vec::new();
        for rel in closed {
            match self.save(vault, &rel, now) {
                Ok(()) => {
                    self.map.remove(&rel);
                }
                Err(e) => errors.push(e),
            }
        }
        errors
    }

    /// Заметка или папка переехала: открытые заметки — под новыми путями.
    pub fn moved(&mut self, from: &Path, to: &Path) {
        let keys: Vec<PathBuf> = self.map.keys().filter(|p| p.starts_with(from)).cloned().collect();
        for old in keys {
            let new = if old == from { to.to_path_buf() } else { to.join(old.strip_prefix(from).unwrap_or(&old)) };
            if let Some(doc) = self.map.remove(&old) {
                self.map.insert(new, doc);
            }
        }
    }

    /// Заметка или папка удалена.
    pub fn removed(&mut self, target: &Path) {
        self.map.retain(|p, _| !p.starts_with(target));
    }

    /// После переименования в заметках `changed` поменялись ссылки: берём новый текст.
    pub fn reload_changed(&mut self, vault: &Vault, changed: &[PathBuf]) {
        for rel in changed {
            if let (Some(doc), Some(i)) = (self.map.get_mut(rel), vault.index_of(rel)) {
                doc.text = vault.notes[i].content.clone();
                doc.dirty = false;
                doc.rev += 1;
            }
        }
    }

    /// Файлы могли поменять снаружи: заметки без правок берут текст с диска, исчезнувшие —
    /// закрываются.
    pub fn reload(&mut self, vault: &Vault) {
        self.map.retain(|rel, doc| {
            if doc.dirty {
                return true;
            }
            match vault.index_of(rel) {
                Some(i) => {
                    if vault.notes[i].content != doc.text {
                        doc.text = vault.notes[i].content.clone();
                        doc.rev += 1;
                    }
                    true
                }
                None => false,
            }
        });
    }
}

/// Состояние вкладки заметки.
#[derive(Default)]
pub struct NoteView {
    pub back: Vec<PathBuf>,
    pub forward: Vec<PathBuf>,
    /// Прокрутить просмотр к заголовку (текст в нижнем регистре).
    pub preview_scroll: Option<String>,
    /// Поставить курсор редактора на это байтовое смещение и прокрутить к нему.
    pub editor_jump: Option<usize>,
    pub focus_editor: bool,
    /// Последняя позиция курсора в символах — для вставки перетащенных файлов.
    pub cursor: Option<usize>,
    /// Подсказки, сокращения и места для ввода в редакторе.
    pub assist: Assist,
}

impl NoteView {
    /// Новая заметка во вкладке: прокрутка и курсор — с начала.
    pub fn reset_position(&mut self) {
        self.preview_scroll = None;
        self.editor_jump = None;
        self.cursor = None;
        self.assist = Assist::default();
    }
}
