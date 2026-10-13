//! Разбор Markdown в собственное дерево документа (AST).
//!
//! Дерево строится из событий pulldown-cmark и дополняется возможностями в духе Obsidian:
//! вики-ссылки, плашки `> [!tip]`, теги `#тег`, выделение `==текст==`, голые URL и свойства
//! из YAML-шапки. Одно и то же дерево используют предпросмотр, индекс хранилища и экспорт в HTML.
//!
//! Собственные расширения синтаксиса:
//! - цвет и вид кода в строке: `` `код`{green} ``, `` `Модуль 3`{badge} ``, `` `x`{#ff8800} ``;
//! - цвет выделения: `==текст=={blue}`;
//! - клавиши: `<kbd>Ctrl</kbd>` или короче `{{Ctrl+S}}`;
//! - блоки кода с заголовком и вкладками: ```` ```js title="app.js" tab="Без React" ````
//!   (идущие подряд блоки с `tab` объединяются в один блок с вкладками), сворачивание
//!   длинных блоков `lines=12` и `open`, папка в приглашении консоли `cwd="~/hangar"`;
//! - автор цитаты: последняя строка цитаты, начинающаяся с «—».


use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd,
};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub mark: bool,
    pub math: bool,
    pub footnote: bool,
    /// Клавиша клавиатуры: `<kbd>Ctrl</kbd>` или `{{Ctrl}}`.
    pub kbd: bool,
    /// Код в строке, показанный как бейдж: `` `Модуль 3`{badge} ``.
    pub badge: bool,
    /// Свой цвет для кода в строке или выделения (RGB).
    pub tint: Option<[u8; 3]>,
    /// Индекс в `Doc::links`.
    pub link: Option<u32>,
}

#[derive(Clone, Debug)]
pub enum Inline {
    Text(String, Style),
    Image { src: String, alt: String, width: Option<f32>, wiki: bool },
    SoftBreak,
    HardBreak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    /// Ссылка на заметку: `[[Заметка]]`, `[[Заметка#Раздел|текст]]` или `[текст](путь.md)`.
    Wiki,
    Url,
    Tag,
    Footnote,
}

#[derive(Clone, Debug)]
pub struct Link {
    pub kind: LinkKind,
    pub target: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub enum Block {
    Heading { level: u8, inlines: Vec<Inline>, text: String },
    Paragraph(Vec<Inline>),
    Code(CodeBlock),
    /// Несколько блоков кода с вкладками.
    CodeGroup(Vec<CodeBlock>),
    Quote { blocks: Vec<Block>, author: Option<Vec<Inline>> },
    Callout { kind: String, title: Vec<Inline>, folded: Option<bool>, body: Vec<Block> },
    List { start: Option<u64>, items: Vec<ListItem> },
    /// `cell_aligns` — выравнивание отдельных ячеек (`{центр}` в начале ячейки):
    /// строка 0 — шапка, дальше строки тела. `Align::None` — как у колонки.
    Table { aligns: Vec<Align>, head: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>>, cell_aligns: Vec<Vec<Align>> },
    Rule,
    Html(String),
    Math(String),
    Footnote { label: String, number: usize, blocks: Vec<Block> },
}

#[derive(Clone, Debug, Default)]
pub struct CodeBlock {
    pub lang: String,
    pub code: String,
    /// Имя файла или подпись в шапке блока: `title="main.rs"`.
    pub title: Option<String>,
    /// Название вкладки: `tab="Вариант 1"`.
    pub tab: Option<String>,
    /// Папка в приглашении консоли: `cwd="~/hangar"`.
    pub cwd: Option<String>,
    /// Сколько строк видно у свёрнутого блока: `lines=12`.
    pub lines: Option<usize>,
    /// Не сворачивать никогда: атрибут `open` без значения.
    pub open: bool,
}

#[derive(Clone, Debug)]
pub struct ListItem {
    pub task: Option<Task>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub checked: bool,
    /// Байтовый диапазон маркера `[ ]` в исходном тексте — для переключения галочки.
    pub range: Range<usize>,
}

#[derive(Clone, Debug)]
pub struct HeadingInfo {
    pub level: u8,
    pub text: String,
    /// Байтовое смещение заголовка в исходном тексте.
    pub offset: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
    pub properties: Vec<(String, String)>,
    pub headings: Vec<HeadingInfo>,
    pub tags: Vec<String>,
    /// Цели всех ссылок на заметки и встраиваний (для обратных ссылок и графа).
    pub note_links: Vec<String>,
    pub has_math: bool,
}

pub fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_MATH
        | Options::ENABLE_WIKILINKS
}

pub fn parse(src: &str) -> Doc {
    let mut b = Builder::new();
    for (event, range) in Parser::new_ext(src, options()).into_offset_iter() {
        b.event(event, range, src);
    }
    b.finish()
}

/// Обычный текст набора инлайнов (для заголовков, оглавления и т.п.).
pub fn plain_text(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for i in inlines {
        match i {
            Inline::Text(t, _) => s.push_str(t),
            Inline::SoftBreak | Inline::HardBreak => s.push(' '),
            Inline::Image { alt, .. } => s.push_str(alt),
        }
    }
    s
}

/// Якорь заголовка для HTML: `Мой раздел` → `мой-раздел`.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in text.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() || ch == '_' {
            out.push(ch);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Цель вики-ссылки без раздела и псевдонима: `Заметка#Раздел|текст` → `Заметка`.
pub fn link_note_part(target: &str) -> &str {
    let t = target.split('|').next().unwrap_or(target);
    t.split('#').next().unwrap_or(t).trim()
}

/// Раздел из вики-ссылки: `Заметка#Раздел` → `Some("Раздел")`.
pub fn link_heading_part(target: &str) -> Option<&str> {
    let t = target.split('|').next().unwrap_or(target);
    t.split_once('#').map(|(_, h)| h.trim()).filter(|h| !h.is_empty())
}

/// Тот же ли это заголовок: без учёта регистра (и кириллицы) и краевых пробелов.
pub fn same_heading(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

pub fn is_image_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

// ---------------------------------------------------------------------------------------------

enum Container {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List { start: Option<u64>, items: Vec<ListItem> },
    Item { task: Option<Task>, blocks: Vec<Block> },
    Footnote { label: String, blocks: Vec<Block> },
    Table {
        aligns: Vec<Align>,
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
        row: Vec<Vec<Inline>>,
        row_aligns: Vec<Align>,
        cell_aligns: Vec<Vec<Align>>,
    },
}

#[derive(PartialEq, Clone, Copy)]
enum InlineKind {
    Paragraph,
    /// Текст в «плотном» списке приходит без тега Paragraph.
    Implicit,
    Heading,
    Cell,
}

struct ImageBuf {
    src: String,
    wiki: bool,
    alt: String,
}

struct Builder {
    doc: Doc,
    stack: Vec<Container>,
    inline: Option<(InlineKind, Vec<Inline>)>,
    bold: u32,
    italic: u32,
    strike: u32,
    kbd: u32,
    link_stack: Vec<u32>,
    image: Option<ImageBuf>,
    code: Option<(String, String)>,
    html: Option<String>,
    meta: Option<String>,
    heading: Option<(u8, usize)>,
    footnote_numbers: HashMap<String, usize>,
}

impl Builder {
    fn new() -> Self {
        Self {
            doc: Doc::default(),
            stack: vec![Container::Root(Vec::new())],
            inline: None,
            bold: 0,
            italic: 0,
            strike: 0,
            kbd: 0,
            link_stack: Vec::new(),
            image: None,
            code: None,
            html: None,
            meta: None,
            heading: None,
            footnote_numbers: HashMap::new(),
        }
    }

    fn finish(mut self) -> Doc {
        self.flush_implicit();
        while self.stack.len() > 1 {
            self.close_container();
        }
        if let Some(Container::Root(blocks)) = self.stack.pop() {
            self.doc.blocks = blocks;
        }
        self.doc.tags.sort();
        self.doc.tags.dedup();
        self.doc
    }

    fn style(&self) -> Style {
        Style {
            bold: self.bold > 0,
            italic: self.italic > 0,
            strike: self.strike > 0,
            kbd: self.kbd > 0,
            link: self.link_stack.last().copied(),
            ..Style::default()
        }
    }

    fn push_block(&mut self, block: Block) {
        for c in self.stack.iter_mut().rev() {
            match c {
                Container::Root(b)
                | Container::Quote(b)
                | Container::Item { blocks: b, .. }
                | Container::Footnote { blocks: b, .. } => {
                    push_merging_code(b, block);
                    return;
                }
                Container::List { .. } | Container::Table { .. } => continue,
            }
        }
    }

    fn flush_implicit(&mut self) {
        if matches!(self.inline, Some((InlineKind::Implicit, _))) {
            if let Some((_, items)) = self.inline.take() {
                if !items.is_empty() {
                    self.push_block(Block::Paragraph(items));
                }
            }
        }
    }

    fn inline_items(&mut self) -> &mut Vec<Inline> {
        if self.inline.is_none() {
            self.inline = Some((InlineKind::Implicit, Vec::new()));
        }
        &mut self.inline.as_mut().unwrap().1
    }

    fn push_inline(&mut self, item: Inline) {
        let items = self.inline_items();
        if let Inline::Text(text, style) = &item {
            if let Some(Inline::Text(prev, prev_style)) = items.last_mut() {
                if prev_style == style {
                    prev.push_str(text);
                    return;
                }
            }
        }
        items.push(item);
    }

    fn add_link(&mut self, kind: LinkKind, target: String) -> u32 {
        self.doc.links.push(Link { kind, target });
        (self.doc.links.len() - 1) as u32
    }

    /// Атрибуты сразу после кода в строке: `` `код`{green} `` или `` `текст`{badge} ``.
    fn take_code_attrs<'t>(&mut self, text: &'t str) -> &'t str {
        let Some(rest) = text.strip_prefix('{') else { return text };
        let Some(end) = rest.find('}') else { return text };
        if end > 48 {
            return text;
        }
        let items = self.inline_items();
        let Some(Inline::Text(_, style)) = items.last_mut() else { return text };
        if !style.code {
            return text;
        }
        let Some((tint, badge)) = parse_attrs(&rest[..end]) else { return text };
        style.tint = tint.or(style.tint);
        style.badge |= badge;
        &rest[end + 1..]
    }

    /// Обычный текст: разбивается на теги, URL, клавиши и `==выделение==`.
    fn push_text(&mut self, text: &str) {
        let text = self.take_code_attrs(text);
        if text.is_empty() {
            return;
        }
        let base = self.style();
        if base.link.is_some() || base.kbd {
            self.push_inline(Inline::Text(text.to_owned(), base));
            return;
        }
        for (segment, marked, tint) in split_marks(text) {
            for piece in split_tags_urls(segment) {
                let mut style = base;
                style.mark = marked;
                if marked {
                    style.tint = tint;
                }
                match piece {
                    Piece::Text(t) => self.push_inline(Inline::Text(t.to_owned(), style)),
                    Piece::Keys(keys) => {
                        // {{Ctrl+Shift+S}} → клавиши, разделённые плюсами.
                        let parts: Vec<&str> = keys.split('+').map(str::trim).filter(|k| !k.is_empty()).collect();
                        for (i, key) in parts.iter().enumerate() {
                            if i > 0 {
                                self.push_inline(Inline::Text(" + ".to_owned(), style));
                            }
                            let mut ks = style;
                            ks.kbd = true;
                            ks.mark = false;
                            self.push_inline(Inline::Text((*key).to_owned(), ks));
                        }
                    }
                    Piece::Tag(t) => {
                        let name = t.trim_start_matches('#').to_owned();
                        self.doc.tags.push(name.clone());
                        style.link = Some(self.add_link(LinkKind::Tag, name));
                        self.push_inline(Inline::Text(t.to_owned(), style));
                    }
                    Piece::Url(u) => {
                        style.link = Some(self.add_link(LinkKind::Url, u.to_owned()));
                        self.push_inline(Inline::Text(u.to_owned(), style));
                    }
                }
            }
        }
    }

    fn close_container(&mut self) {
        let Some(c) = self.stack.pop() else { return };
        match c {
            Container::Root(blocks) => self.stack.push(Container::Root(blocks)),
            Container::Quote(blocks) => {
                let block = match to_callout(blocks) {
                    Ok(callout) => callout,
                    Err(mut blocks) => {
                        let author = extract_author(&mut blocks);
                        Block::Quote { blocks, author }
                    }
                };
                self.push_block(block);
            }
            Container::List { start, items } => self.push_block(Block::List { start, items }),
            Container::Item { task, blocks } => {
                if let Some(Container::List { items, .. }) = self.stack.last_mut() {
                    items.push(ListItem { task, blocks });
                }
            }
            Container::Footnote { label, blocks } => {
                let n = self.footnote_number(&label);
                self.push_block(Block::Footnote { label, number: n, blocks });
            }
            Container::Table { aligns, head, rows, cell_aligns, .. } => self.push_block(Block::Table { aligns, head, rows, cell_aligns }),
        }
    }

    fn footnote_number(&mut self, label: &str) -> usize {
        let next = self.footnote_numbers.len() + 1;
        *self.footnote_numbers.entry(label.to_owned()).or_insert(next)
    }

    fn event(&mut self, event: Event<'_>, range: Range<usize>, src: &str) {
        // Внутри картинки весь текст — это подпись (alt).
        if let Some(img) = &mut self.image {
            match event {
                Event::Text(t) | Event::Code(t) => img.alt.push_str(&t),
                Event::End(TagEnd::Image) => {
                    let img = self.image.take().unwrap();
                    self.finish_image(img);
                }
                _ => {}
            }
            return;
        }
        if let Some((_, code)) = &mut self.code {
            match event {
                Event::Text(t) => code.push_str(&t),
                Event::End(TagEnd::CodeBlock) => {
                    let (info, mut code) = self.code.take().unwrap();
                    if code.ends_with('\n') {
                        code.pop();
                    }
                    self.push_block(Block::Code(CodeBlock { code, ..parse_code_info(&info) }));
                }
                _ => {}
            }
            return;
        }
        if let Some(meta) = &mut self.meta {
            match event {
                Event::Text(t) => meta.push_str(&t),
                Event::End(TagEnd::MetadataBlock(_)) => {
                    let meta = self.meta.take().unwrap();
                    self.parse_properties(&meta);
                }
                _ => {}
            }
            return;
        }
        if let Some(html) = &mut self.html {
            match event {
                Event::Html(t) | Event::Text(t) => html.push_str(&t),
                Event::End(TagEnd::HtmlBlock) => {
                    let html = self.html.take().unwrap();
                    self.push_block(Block::Html(html.trim_end().to_owned()));
                }
                _ => {}
            }
            return;
        }

        match event {
            Event::Start(tag) => self.start(tag, range, src),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => self.push_text(&t),
            Event::Code(t) => {
                let mut style = self.style();
                style.code = true;
                self.push_inline(Inline::Text(t.to_string(), style));
            }
            Event::InlineMath(t) => {
                self.doc.has_math = true;
                let mut style = self.style();
                style.math = true;
                self.push_inline(Inline::Text(t.to_string(), style));
            }
            Event::DisplayMath(t) => {
                self.doc.has_math = true;
                // Формула-блок разрывает абзац: текст до неё — отдельный абзац.
                let kind = self.inline.as_ref().map(|(k, _)| *k).unwrap_or(InlineKind::Implicit);
                if let Some((_, items)) = self.inline.take() {
                    if items.iter().any(|i| !matches!(i, Inline::SoftBreak | Inline::HardBreak)) {
                        self.push_block(Block::Paragraph(items));
                    }
                }
                self.push_block(Block::Math(t.trim().to_owned()));
                self.inline = Some((kind, Vec::new()));
            }
            Event::InlineHtml(t) => {
                let lower = t.to_lowercase();
                if lower.starts_with("<br") {
                    self.push_inline(Inline::HardBreak);
                } else if lower.starts_with("<kbd") {
                    self.kbd += 1;
                } else if lower.starts_with("</kbd") {
                    self.kbd = self.kbd.saturating_sub(1);
                }
            }
            Event::Html(t) => {
                self.flush_implicit();
                self.push_block(Block::Html(t.to_string()));
            }
            Event::FootnoteReference(label) => {
                let n = self.footnote_number(&label);
                let mut style = self.style();
                style.footnote = true;
                style.link = Some(self.add_link(LinkKind::Footnote, label.to_string()));
                self.push_inline(Inline::Text(n.to_string(), style));
            }
            Event::SoftBreak => self.push_inline(Inline::SoftBreak),
            Event::HardBreak => self.push_inline(Inline::HardBreak),
            Event::Rule => {
                self.flush_implicit();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(checked) => {
                let range = task_marker_range(src, range);
                if let Some(Container::Item { task, .. }) = self.stack.last_mut() {
                    *task = Some(Task { checked, range });
                }
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>, range: Range<usize>, _src: &str) {
        match tag {
            Tag::Paragraph => {
                self.flush_implicit();
                self.inline = Some((InlineKind::Paragraph, Vec::new()));
            }
            Tag::Heading { level, .. } => {
                self.flush_implicit();
                self.inline = Some((InlineKind::Heading, Vec::new()));
                self.heading = Some((heading_level(level), range.start));
            }
            Tag::BlockQuote(_) => {
                self.flush_implicit();
                self.stack.push(Container::Quote(Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                self.flush_implicit();
                let info = match kind {
                    CodeBlockKind::Fenced(info) => info.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((info, String::new()));
            }
            Tag::HtmlBlock => {
                self.flush_implicit();
                self.html = Some(String::new());
            }
            Tag::List(start) => {
                self.flush_implicit();
                self.stack.push(Container::List { start, items: Vec::new() });
            }
            Tag::Item => {
                self.flush_implicit();
                self.stack.push(Container::Item { task: None, blocks: Vec::new() });
            }
            Tag::FootnoteDefinition(label) => {
                self.flush_implicit();
                self.stack.push(Container::Footnote { label: label.to_string(), blocks: Vec::new() });
            }
            Tag::Table(aligns) => {
                self.flush_implicit();
                let aligns = aligns
                    .iter()
                    .map(|a| match a {
                        Alignment::None => Align::None,
                        Alignment::Left => Align::Left,
                        Alignment::Center => Align::Center,
                        Alignment::Right => Align::Right,
                    })
                    .collect();
                self.stack.push(Container::Table {
                    aligns,
                    head: Vec::new(),
                    rows: Vec::new(),
                    row: Vec::new(),
                    row_aligns: Vec::new(),
                    cell_aligns: Vec::new(),
                });
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(Container::Table { row, row_aligns, .. }) = self.stack.last_mut() {
                    row.clear();
                    row_aligns.clear();
                }
            }
            Tag::TableCell => self.inline = Some((InlineKind::Cell, Vec::new())),
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Superscript | Tag::Subscript => {}
            Tag::Link { link_type, dest_url, .. } => {
                let (kind, target) = classify_link(link_type, &dest_url);
                if kind == LinkKind::Wiki {
                    self.doc.note_links.push(link_note_part(&target).to_owned());
                }
                let idx = self.add_link(kind, target);
                self.link_stack.push(idx);
            }
            Tag::Image { link_type, dest_url, .. } => {
                let wiki = matches!(link_type, LinkType::WikiLink { .. });
                self.image = Some(ImageBuf { src: dest_url.to_string(), wiki, alt: String::new() });
            }
            Tag::MetadataBlock(_) => self.meta = Some(String::new()),
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                if let Some((_, items)) = self.inline.take() {
                    if items.iter().any(|i| !matches!(i, Inline::SoftBreak | Inline::HardBreak)) {
                        self.push_block(Block::Paragraph(items));
                    }
                }
            }
            TagEnd::Heading(_) => {
                let items = self.inline.take().map(|(_, i)| i).unwrap_or_default();
                let (level, offset) = self.heading.take().unwrap_or((1, 0));
                let text = plain_text(&items).trim().to_owned();
                self.doc.headings.push(HeadingInfo { level, text: text.clone(), offset });
                self.push_block(Block::Heading { level, inlines: items, text });
            }
            TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::FootnoteDefinition | TagEnd::Table => {
                self.flush_implicit();
                self.close_container();
            }
            TagEnd::Item => {
                self.flush_implicit();
                self.close_container();
            }
            TagEnd::TableHead => {
                if let Some(Container::Table { head, row, row_aligns, cell_aligns, .. }) = self.stack.last_mut() {
                    *head = std::mem::take(row);
                    cell_aligns.insert(0, std::mem::take(row_aligns));
                }
            }
            TagEnd::TableRow => {
                if let Some(Container::Table { rows, row, row_aligns, cell_aligns, .. }) = self.stack.last_mut() {
                    rows.push(std::mem::take(row));
                    cell_aligns.push(std::mem::take(row_aligns));
                }
            }
            TagEnd::TableCell => {
                let mut items = self.inline.take().map(|(_, i)| i).unwrap_or_default();
                let align = take_cell_align(&mut items);
                if let Some(Container::Table { row, row_aligns, .. }) = self.stack.last_mut() {
                    row.push(items);
                    row_aligns.push(align);
                }
            }
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => {
                self.link_stack.pop();
            }
            _ => {}
        }
    }

    fn finish_image(&mut self, img: ImageBuf) {
        // Размер можно задать как в Obsidian: `![[картинка.png|300]]` или `![подпись|300](путь)`.
        let (alt, width) = match img.alt.rsplit_once('|') {
            Some((a, w)) if parse_width(w).is_some() => (a.to_owned(), parse_width(w)),
            _ => match parse_width(&img.alt) {
                Some(w) if img.wiki => (String::new(), Some(w)),
                _ => (img.alt.clone(), None),
            },
        };
        if img.wiki && !is_image_path(&img.src) {
            self.doc.note_links.push(link_note_part(&img.src).to_owned());
        }
        self.push_inline(Inline::Image { src: img.src, alt, width, wiki: img.wiki });
    }

    /// Простейший разбор YAML-шапки: `ключ: значение` и списки `- элемент`.
    fn parse_properties(&mut self, meta: &str) {
        let mut props: Vec<(String, String)> = Vec::new();
        for line in meta.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(item) = trimmed.strip_prefix("- ") {
                if let Some((_, value)) = props.last_mut() {
                    if !value.is_empty() {
                        value.push_str(", ");
                    }
                    value.push_str(item.trim().trim_matches('"'));
                }
                continue;
            }
            if let Some((key, value)) = trimmed.split_once(':') {
                let value = value.trim().trim_start_matches('[').trim_end_matches(']');
                let value = value
                    .split(',')
                    .map(|v| v.trim().trim_matches('"').trim_matches('\''))
                    .filter(|v| !v.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ");
                props.push((key.trim().to_owned(), value));
            }
        }
        for (key, value) in &props {
            if key.eq_ignore_ascii_case("tags") || key.eq_ignore_ascii_case("теги") {
                for t in value.split(',') {
                    let t = t.trim().trim_start_matches('#');
                    if !t.is_empty() {
                        self.doc.tags.push(t.to_owned());
                    }
                }
            }
        }
        self.doc.properties = props;
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn parse_width(s: &str) -> Option<f32> {
    let s = s.trim();
    let w = s.split('x').next()?;
    if w.is_empty() || !w.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    w.parse().ok()
}

fn classify_link(link_type: LinkType, dest: &str) -> (LinkKind, String) {
    match link_type {
        LinkType::WikiLink { .. } => (LinkKind::Wiki, dest.to_owned()),
        LinkType::Email => (LinkKind::Url, format!("mailto:{dest}")),
        _ => {
            if dest.contains("://") || dest.starts_with("mailto:") {
                (LinkKind::Url, dest.to_owned())
            } else if let Some(anchor) = dest.strip_prefix('#') {
                (LinkKind::Wiki, format!("#{}", percent_decode(anchor)))
            } else {
                // Относительная ссылка на файл хранилища: `[текст](папка/Заметка.md#раздел)`.
                let decoded = percent_decode(dest);
                let (path, anchor) = match decoded.split_once('#') {
                    Some((p, a)) => (p.to_owned(), Some(a.to_owned())),
                    None => (decoded.clone(), None),
                };
                let path = path.strip_suffix(".md").unwrap_or(&path).to_owned();
                let target = match anchor {
                    Some(a) => format!("{path}#{a}"),
                    None => path,
                };
                (LinkKind::Wiki, target)
            }
        }
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_owned())
}

/// Находит точный диапазон `[ ]`/`[x]` внутри диапазона события.
fn task_marker_range(src: &str, range: Range<usize>) -> Range<usize> {
    let slice = &src[range.clone()];
    match slice.find('[') {
        Some(i) => range.start + i..range.start + i + 3,
        None => range,
    }
}

/// Превращает цитату в плашку, если она начинается с `[!тип]`.
fn to_callout(mut blocks: Vec<Block>) -> Result<Block, Vec<Block>> {
    let Some(Block::Paragraph(inlines)) = blocks.first() else {
        return Err(blocks);
    };
    let Some(Inline::Text(first, style)) = inlines.first() else {
        return Err(blocks);
    };
    if style.code || !first.starts_with("[!") {
        return Err(blocks);
    }
    let Some(close) = first.find(']') else {
        return Err(blocks);
    };
    let kind = first[2..close].trim().to_lowercase();
    let mut rest = &first[close + 1..];
    let folded = if let Some(r) = rest.strip_prefix('-') {
        rest = r;
        Some(true)
    } else if let Some(r) = rest.strip_prefix('+') {
        rest = r;
        Some(false)
    } else {
        None
    };
    let rest = rest.trim_start().to_owned();
    let first_style = *style;

    let Block::Paragraph(inlines) = blocks.remove(0) else { unreachable!() };
    let mut title = Vec::new();
    if !rest.is_empty() {
        title.push(Inline::Text(rest, first_style));
    }
    let mut iter = inlines.into_iter().skip(1);
    let mut body_first = Vec::new();
    for item in iter.by_ref() {
        if matches!(item, Inline::SoftBreak | Inline::HardBreak) {
            break;
        }
        title.push(item);
    }
    body_first.extend(iter);
    let mut body = Vec::new();
    if !body_first.is_empty() {
        body.push(Block::Paragraph(body_first));
    }
    body.extend(blocks);
    Ok(Block::Callout { kind, title, folded, body })
}

/// Делит текст по парам `==…==` на обычные и выделенные куски.
/// После выделения можно указать цвет: `==текст=={blue}`.
fn split_marks(s: &str) -> Vec<(&str, bool, Option<[u8; 3]>)> {
    let mut out = Vec::new();
    let mut rest = s;
    loop {
        let Some(open) = rest.find("==") else { break };
        let after = &rest[open + 2..];
        if after.starts_with(|c: char| c.is_whitespace() || c == '=') || after.is_empty() {
            out.push((&rest[..open + 2], false, None));
            rest = after;
            continue;
        }
        let Some(close_rel) = after.find("==") else { break };
        let inner = &after[..close_rel];
        if inner.ends_with(char::is_whitespace) {
            out.push((&rest[..open + 2], false, None));
            rest = after;
            continue;
        }
        if open > 0 {
            out.push((&rest[..open], false, None));
        }
        let mut tail = &after[close_rel + 2..];
        let mut tint = None;
        if let Some(attr) = tail.strip_prefix('{') {
            if let Some(end) = attr.find('}') {
                if let Some((t, _)) = parse_attrs(&attr[..end]) {
                    tint = t;
                    tail = &attr[end + 1..];
                }
            }
        }
        out.push((inner, true, tint));
        rest = tail;
    }
    if !rest.is_empty() {
        out.push((rest, false, None));
    }
    // Склеиваем соседние обычные куски, чтобы не плодить секции.
    let mut merged: Vec<(&str, bool, Option<[u8; 3]>)> = Vec::new();
    for (piece, marked, tint) in out {
        if let Some((prev, prev_marked, _)) = merged.last_mut() {
            let prev_end = prev.as_ptr() as usize + prev.len();
            // Склеиваем, только если куски идут подряд (между ними не было вырезанного атрибута).
            if !*prev_marked && !marked && prev_end == piece.as_ptr() as usize {
                let start = prev.as_ptr() as usize - s.as_ptr() as usize;
                let end = piece.as_ptr() as usize - s.as_ptr() as usize + piece.len();
                *prev = &s[start..end];
                continue;
            }
        }
        merged.push((piece, marked, tint));
    }
    merged
}

enum Piece<'a> {
    Text(&'a str),
    Tag(&'a str),
    Url(&'a str),
    /// Содержимое `{{Ctrl+S}}`.
    Keys(&'a str),
}

/// Именованные цвета для атрибутов вида `{green}`.
pub fn named_color(name: &str) -> Option<[u8; 3]> {
    let n = name.trim().to_lowercase();
    if let Some(hex) = n.strip_prefix('#') {
        let v = u32::from_str_radix(hex, 16).ok()?;
        return match hex.len() {
            6 => Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]),
            3 => Some([((v >> 8) & 0xF) as u8 * 17, ((v >> 4) & 0xF) as u8 * 17, (v & 0xF) as u8 * 17]),
            _ => None,
        };
    }
    Some(match n.as_str() {
        "red" | "красный" => [248, 113, 113],
        "orange" | "оранжевый" => [251, 146, 60],
        "yellow" | "жёлтый" | "желтый" => [250, 204, 21],
        "green" | "зелёный" | "зеленый" => [74, 222, 128],
        "teal" | "бирюзовый" => [45, 212, 191],
        "cyan" | "голубой" => [103, 232, 249],
        "blue" | "синий" => [96, 165, 250],
        "purple" | "violet" | "фиолетовый" => [167, 139, 250],
        "pink" | "розовый" => [244, 114, 182],
        "gray" | "grey" | "серый" => [156, 163, 175],
        _ => return None,
    })
}

/// Метка выравнивания в начале ячейки таблицы: `{центр}`, `{слева}`, `{справа}`
/// (или `{center}`, `{left}`, `{right}`). Метка убирается из текста ячейки.
fn take_cell_align(items: &mut Vec<Inline>) -> Align {
    let Some(Inline::Text(t, st)) = items.first_mut() else { return Align::None };
    if st.code || st.link.is_some() {
        return Align::None;
    }
    let Some(rest) = t.trim_start().strip_prefix('{') else { return Align::None };
    let Some(end) = rest.find('}') else { return Align::None };
    let align = match rest[..end].trim().to_lowercase().as_str() {
        "left" | "слева" | "влево" | "лево" => Align::Left,
        "center" | "centre" | "центр" | "по центру" | "середина" => Align::Center,
        "right" | "справа" | "вправо" | "право" => Align::Right,
        _ => return Align::None,
    };
    let after = rest[end + 1..].trim_start().to_owned();
    if after.is_empty() {
        items.remove(0);
    } else {
        *t = after;
    }
    align
}

/// Разбор атрибутов `{green badge}`. Возвращает None, если внутри не атрибуты, а обычный текст.
fn parse_attrs(s: &str) -> Option<(Option<[u8; 3]>, bool)> {
    let mut tint = None;
    let mut badge = false;
    let mut any = false;
    for token in s.split([' ', ',']).map(str::trim).filter(|t| !t.is_empty()) {
        let token = token.trim_start_matches('.');
        if token.eq_ignore_ascii_case("badge") || token == "бейдж" {
            badge = true;
        } else if let Some(c) = named_color(token) {
            tint = Some(c);
        } else {
            return None;
        }
        any = true;
    }
    any.then_some((tint, badge))
}

/// Разбор строки после ``` : язык и атрибуты `title="…"`, `tab="…"`, `cwd="…"`, `lines=N`
/// и флаг `open`. Код в ответе пустой.
fn parse_code_info(info: &str) -> CodeBlock {
    let info = info.trim();
    let lang_end = info.find(|c: char| c.is_whitespace() || c == '{').unwrap_or(info.len());
    let mut cb = CodeBlock { lang: info[..lang_end].trim_end_matches(',').to_owned(), ..Default::default() };
    let mut rest = &info[lang_end..];
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '{' | '}' | ','));
        if rest.is_empty() {
            break;
        }
        let key_end = rest.find(|c: char| c.is_whitespace() || matches!(c, '=' | '{' | '}' | ',')).unwrap_or(rest.len());
        let key = rest[..key_end].to_lowercase();
        let after = rest[key_end..].trim_start();
        // Без `=` — флаг вроде `open`.
        let Some(after) = after.strip_prefix('=') else {
            if key == "open" {
                cb.open = true;
            }
            rest = &rest[key_end.max(1)..];
            continue;
        };
        let after = after.trim_start();
        let (value, next) = if let Some(q) = after.strip_prefix('"') {
            match q.find('"') {
                Some(end) => (&q[..end], &q[end + 1..]),
                None => (q, ""),
            }
        } else {
            let end = after.find(|c: char| c.is_whitespace() || c == '}' || c == ',').unwrap_or(after.len());
            (&after[..end], &after[end..])
        };
        match key.as_str() {
            "title" | "file" | "filename" | "name" => cb.title = Some(value.to_owned()),
            "tab" | "label" => cb.tab = Some(value.to_owned()),
            "cwd" | "dir" => cb.cwd = Some(value.to_owned()).filter(|v| !v.is_empty()),
            "lines" => cb.lines = value.parse().ok(),
            "open" => cb.open = !matches!(value, "false" | "0" | "no"),
            _ => {}
        }
        rest = next;
    }
    cb
}

/// Подпись автора в цитате: последняя строка, начинающаяся с «—», «–» или «--».
fn extract_author(blocks: &mut Vec<Block>) -> Option<Vec<Inline>> {
    let author = take_author(blocks.last_mut()?)?;
    // Если подпись была отдельным абзацем, от него ничего не осталось.
    if matches!(blocks.last(), Some(Block::Paragraph(items)) if items.is_empty()) {
        blocks.pop();
    }
    Some(author)
}

fn take_author(block: &mut Block) -> Option<Vec<Inline>> {
    let Block::Paragraph(inlines) = block else { return None };
    let is_dash = |t: &str| {
        let t = t.trim_start();
        t.starts_with('—') || t.starts_with('–') || t.starts_with("--")
    };
    // Автор — после последнего переноса строки внутри абзаца…
    let split = inlines.iter().rposition(|i| matches!(i, Inline::SoftBreak | Inline::HardBreak));
    let start = match split {
        Some(pos) => pos + 1,
        None => 0,
    };
    let Some(Inline::Text(first, _)) = inlines.get(start) else { return None };
    if !is_dash(first) {
        return None;
    }
    let mut author: Vec<Inline> = inlines.drain(start..).collect();
    if split.is_some() {
        inlines.pop();
    }
    if let Some(Inline::Text(t, _)) = author.first_mut() {
        *t = t.trim_start().trim_start_matches(['—', '–', '-']).trim_start().to_owned();
    }
    Some(author)
}

/// Добавляет блок, объединяя идущие подряд блоки кода с вкладками в один.
fn push_merging_code(blocks: &mut Vec<Block>, block: Block) {
    if let Block::Code(cb) = &block {
        if cb.tab.is_some() {
            match blocks.last_mut() {
                Some(Block::CodeGroup(group)) => {
                    let Block::Code(cb) = block else { unreachable!() };
                    group.push(cb);
                    return;
                }
                Some(last @ Block::Code(_)) => {
                    if let Block::Code(prev) = last {
                        if prev.tab.is_some() {
                            let prev = std::mem::take(prev);
                            let Block::Code(cb) = block else { unreachable!() };
                            *last = Block::CodeGroup(vec![prev, cb]);
                            return;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    blocks.push(block);
}

/// Выделяет из текста теги `#тег` и голые ссылки `https://…`.
fn split_tags_urls(s: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut last = 0;
    let mut prev: Option<char> = None;
    let mut iter = s.char_indices().peekable();
    while let Some((i, ch)) = iter.next() {
        if i >= last && ch == '{' && s[i..].starts_with("{{") {
            if let Some(end) = s[i + 2..].find("}}") {
                let inner = &s[i + 2..i + 2 + end];
                if !inner.trim().is_empty() && !inner.contains('\n') && inner.len() <= 64 {
                    if i > last {
                        out.push(Piece::Text(&s[last..i]));
                    }
                    out.push(Piece::Keys(inner));
                    last = i + 4 + end;
                    prev = Some('}');
                    continue;
                }
            }
        }
        let boundary = prev.is_none_or(|p| p.is_whitespace() || "([{,;\"'«".contains(p));
        if boundary && i >= last {
            if ch == '#' {
                let rest = &s[i + 1..];
                let len: usize = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))
                    .map(char::len_utf8)
                    .sum();
                let name = &rest[..len];
                if !name.is_empty() && name.chars().any(|c| !c.is_ascii_digit()) {
                    if i > last {
                        out.push(Piece::Text(&s[last..i]));
                    }
                    out.push(Piece::Tag(&s[i..i + 1 + len]));
                    last = i + 1 + len;
                }
            } else if ch == 'h' && (s[i..].starts_with("https://") || s[i..].starts_with("http://")) {
                let rest = &s[i..];
                let mut len = rest.find(char::is_whitespace).unwrap_or(rest.len());
                while len > 0 && rest[..len].ends_with(['.', ',', ';', ':', '!', '?', ')', '"', '\'', '»']) {
                    len -= rest[..len].chars().next_back().map_or(1, char::len_utf8);
                }
                if len > 8 {
                    if i > last {
                        out.push(Piece::Text(&s[last..i]));
                    }
                    out.push(Piece::Url(&rest[..len]));
                    last = i + len;
                }
            }
        }
        prev = Some(ch);
    }
    if last < s.len() {
        out.push(Piece::Text(&s[last..]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callout_and_wikilinks() {
        let doc = parse("> [!tip]- Мой совет\n> Текст со [[Ссылка|ссылкой]] и #тег\n\n![[pic.png|200]]");
        match &doc.blocks[0] {
            Block::Callout { kind, folded, title, body } => {
                assert_eq!(kind, "tip");
                assert_eq!(*folded, Some(true));
                assert_eq!(plain_text(title), "Мой совет");
                assert_eq!(body.len(), 1);
            }
            other => panic!("ожидалась плашка, получено {other:?}"),
        }
        assert_eq!(doc.note_links, vec!["Ссылка".to_owned()]);
        assert_eq!(doc.tags, vec!["тег".to_owned()]);
        match &doc.blocks[1] {
            Block::Paragraph(items) => match &items[0] {
                Inline::Image { src, width, .. } => {
                    assert_eq!(src, "pic.png");
                    assert_eq!(*width, Some(200.0));
                }
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn tasks_have_source_ranges() {
        let src = "- [ ] один\n- [x] два\n";
        let doc = parse(src);
        let Block::List { items, .. } = &doc.blocks[0] else { panic!() };
        let t = items[0].task.as_ref().unwrap();
        assert_eq!(&src[t.range.clone()], "[ ]");
        assert!(items[1].task.as_ref().unwrap().checked);
    }

    #[test]
    fn marks_tags_urls() {
        let doc = parse("Это ==важно== и https://example.com, а 3#1 не тег");
        let Block::Paragraph(items) = &doc.blocks[0] else { panic!() };
        let marked: Vec<_> = items
            .iter()
            .filter_map(|i| match i {
                Inline::Text(t, s) if s.mark => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(marked, vec!["важно"]);
        assert!(doc.links.iter().any(|l| l.kind == LinkKind::Url && l.target == "https://example.com"));
        assert!(doc.tags.is_empty());
    }

    #[test]
    fn frontmatter_and_math() {
        let doc = parse("---\ntitle: Привет\ntags: [rust, vulkan]\n---\n\n$$\nE=mc^2\n$$\n");
        assert_eq!(doc.properties[0], ("title".to_owned(), "Привет".to_owned()));
        assert_eq!(doc.tags, vec!["rust".to_owned(), "vulkan".to_owned()]);
        assert!(doc.has_math);
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Math(m) if m == "E=mc^2")));
    }

    fn texts(doc: &Doc) -> Vec<(String, Style)> {
        let Block::Paragraph(items) = &doc.blocks[0] else { panic!("ожидался абзац") };
        items
            .iter()
            .filter_map(|i| match i {
                Inline::Text(t, s) => Some((t.clone(), *s)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn code_attrs_and_keys() {
        let doc = parse("Хук `useState`{blue} и `Модуль 3`{badge}, жми <kbd>Ctrl</kbd> или {{Ctrl+S}}, ==важно=={green}");
        let t = texts(&doc);
        let find = |s: &str| t.iter().find(|(x, _)| x == s).map(|(_, st)| *st).unwrap();
        assert_eq!(find("useState").tint, Some([96, 165, 250]));
        assert!(find("Модуль 3").badge);
        assert!(find("Ctrl").kbd);
        assert!(find("S").kbd);
        assert_eq!(find("важно").tint, Some([74, 222, 128]));
        assert!(!t.iter().any(|(x, _)| x.contains('{')), "атрибуты не должны попадать в текст: {t:?}");
    }

    #[test]
    fn table_cell_alignment() {
        let doc = parse("| a | {центр} b |\n|:--|--:|\n| {справа} x | `y` |\n| {center}`z` | {неизвестно} w |");
        let Block::Table { aligns, cell_aligns, head, rows } = &doc.blocks[0] else { panic!("ожидалась таблица") };
        assert_eq!(aligns, &vec![Align::Left, Align::Right]);
        assert_eq!(cell_aligns[0], vec![Align::None, Align::Center]);
        assert_eq!(cell_aligns[1], vec![Align::Right, Align::None]);
        assert_eq!(cell_aligns[2], vec![Align::Center, Align::None]);
        let text = |items: &[Inline]| -> String {
            items
                .iter()
                .filter_map(|i| match i {
                    Inline::Text(t, _) => Some(t.as_str()),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(text(&head[1]), "b");
        assert_eq!(text(&rows[0][0]), "x");
        assert!(matches!(&rows[1][0][0], Inline::Text(t, st) if t == "z" && st.code));
        assert_eq!(text(&rows[1][1]), "{неизвестно} w", "незнакомая метка остаётся текстом");
    }

    #[test]
    fn custom_colors_for_badges_and_marks() {
        let doc = parse("`v2`{badge #ff8800}, `ok`{#0f0 badge} и ==мой цвет=={#7c3aed}");
        let t = texts(&doc);
        let find = |s: &str| t.iter().find(|(x, _)| x == s).map(|(_, st)| *st).unwrap();
        assert!(find("v2").badge);
        assert_eq!(find("v2").tint, Some([255, 136, 0]));
        assert_eq!(find("ok").tint, Some([0, 255, 0]));
        assert!(find("мой цвет").mark);
        assert_eq!(find("мой цвет").tint, Some([124, 58, 237]));
    }

    #[test]
    fn code_groups_and_titles() {
        let src = "```js tab=\"Без React\" title=\"like.js\"\nlet a = 1;\n```\n\n```jsx tab=\"С React\"\n<A/>\n```\n\n```rust title=main.rs\nfn main() {}\n```";
        let doc = parse(src);
        match &doc.blocks[0] {
            Block::CodeGroup(g) => {
                assert_eq!(g.len(), 2);
                assert_eq!(g[0].tab.as_deref(), Some("Без React"));
                assert_eq!(g[0].title.as_deref(), Some("like.js"));
                assert_eq!(g[1].lang, "jsx");
            }
            other => panic!("ожидалась группа, получено {other:?}"),
        }
        match &doc.blocks[1] {
            Block::Code(c) => assert_eq!(c.title.as_deref(), Some("main.rs")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn code_info_attrs() {
        let cb = parse_code_info(r#"console title="Сборка" cwd="~/hangar" lines=8 open"#);
        assert_eq!(cb.lang, "console");
        assert_eq!(cb.title.as_deref(), Some("Сборка"));
        assert_eq!(cb.cwd.as_deref(), Some("~/hangar"));
        assert_eq!(cb.lines, Some(8));
        assert!(cb.open);
        // Порядок и скобки не важны, флаг может стоять первым.
        let cb = parse_code_info("log {open, lines=20, title=latest.log}");
        assert_eq!((cb.lines, cb.open, cb.title.as_deref()), (Some(20), true, Some("latest.log")));
        let cb = parse_code_info(r#"rust title = "main.rs" lines=x"#);
        assert_eq!(cb.title.as_deref(), Some("main.rs"));
        assert_eq!((cb.lines, cb.open, cb.cwd), (None, false, None));
        // `opened` — не флаг `open`.
        assert!(!parse_code_info("js opened").open);
        let doc = parse("```console cwd=~/x lines=3
$ ls
```");
        match &doc.blocks[0] {
            Block::Code(c) => assert_eq!((c.code.as_str(), c.cwd.as_deref(), c.lines), ("$ ls", Some("~/x"), Some(3))),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn quote_author() {
        let doc = parse("> Простота — залог надёжности.\n> — Эдсгер Дейкстра");
        match &doc.blocks[0] {
            Block::Quote { blocks, author } => {
                assert_eq!(plain_text(author.as_ref().unwrap()), "Эдсгер Дейкстра");
                let Block::Paragraph(body) = &blocks[0] else { panic!() };
                assert_eq!(plain_text(body), "Простота — залог надёжности.");
            }
            other => panic!("{other:?}"),
        }
        let doc = parse("> Просто цитата без автора");
        assert!(matches!(&doc.blocks[0], Block::Quote { author: None, .. }));
    }

    #[test]
    fn heading_match_ignores_case() {
        assert!(same_heading("Введение", "введение"));
        assert!(same_heading(" ЁЖИК В ТУМАНЕ ", "ёжик в тумане"));
        assert!(same_heading("Getting Started", "getting started"));
        assert!(!same_heading("Введение", "Заключение"));
        // Ссылка `[[Заметка#введение]]` и заголовок `# Введение` дают один якорь при экспорте.
        let h = link_heading_part("Заметка#введение|тут").unwrap();
        assert_eq!(slug(h), slug("Введение"));
    }
}
