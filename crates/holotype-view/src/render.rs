//! Красивый просмотр заметки: рисует дерево документа виджетами egui.
//!
//! Код в строке, теги, клавиши, бейджи и выделение рисуются как аккуратные «пилюли»:
//! текст раскладывается одной строкой egui, а под нужными участками мы сами рисуем
//! скруглённые подложки — так они всегда стоят ровно на базовой линии текста.

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, Frame, Galley, Label, Layout, Margin,
    Rect, RichText, ScrollArea, Sense, Shape, Stroke, Ui, UiBuilder, Vec2, pos2, vec2,
    collapsing_header::CollapsingState,
    text::{CCursor, LayoutJob, TextFormat},
    text_selection::LabelSelectionState,
};

use crate::markdown::{Align as ColAlign, Block, CodeBlock, Doc, Inline, Link, LinkKind, ListItem, Style};
use crate::diagram;
use crate::plugin_ui::{PluginUi, Tree};
use crate::fonts;
use crate::highlight;
use crate::math::{self, MathBox};
use crate::term::{self, Kind, LineKind};
use crate::theme::{self, Palette};
use crate::vault::Vault;
use forma_i18n::t;

pub enum Action {
    /// Открыть заметку по цели вики-ссылки (может содержать `#раздел`).
    OpenNote { target: String, new_tab: bool },
    OpenUrl(String),
    ToggleTask(Range<usize>),
    SearchTag(String),
    /// Указатель над символом `offset` текста `text`: спросить подсказку у плагинов. Ответ
    /// ([`HoverAnswer`]) программа кладёт в память egui по ключу `key`.
    Hover { key: egui::Id, text: String, offset: usize },
    /// Блок ` ```lang ` рисует плагин, а его дерева ещё нет: спросить плагин.
    RenderBlock { lang: String, source: String },
    /// Кнопка в блоке плагина: запустить команду плагина.
    PluginCommand { command: String, args: serde_json::Value },
    /// Что сделали с элементом блока плагина (API 1.3): плагин отвечает новым деревом блока.
    BlockEvent { lang: String, source: String, event: forma_plugin_ui::Event },
}

/// Что было под правым щелчком в режиме чтения: для контекстного меню.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuHit {
    /// Заголовок раздела над этим местом.
    pub heading: String,
    /// Текст абзаца (строки, ячейки) под указателем.
    pub paragraph: String,
    /// Ссылка под указателем: цель и внешняя ли она.
    pub link: Option<(String, bool)>,
}

/// Подсказка плагина к тексту под указателем: к каким символам относится и что показать.
#[derive(Clone, Debug, PartialEq)]
pub struct HoverAnswer {
    /// Отпечаток текста, к которому подсказка (текст абзаца мог поменяться).
    pub text_hash: u64,
    pub range: Range<usize>,
    pub markdown: String,
}

/// Отпечаток текста для [`HoverAnswer`].
pub fn text_hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Базовый формат текста для набора инлайнов.
#[derive(Clone)]
struct Fmt {
    size: f32,
    color: Color32,
    family: FontFamily,
    heading: bool,
    line_height: f32,
    strike: bool,
    italic: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum DecoKind {
    Code,
    Tag,
    Kbd,
    Badge,
    Mark,
}

impl DecoKind {
    /// Горизонтальный отступ подложки (в долях кегля текста).
    fn pad(self) -> f32 {
        match self {
            DecoKind::Code => 0.32,
            DecoKind::Tag => 0.42,
            DecoKind::Kbd => 0.38,
            DecoKind::Badge => 0.5,
            DecoKind::Mark => 0.14,
        }
    }
}

/// Подложка под участком текста.
struct Deco {
    kind: DecoKind,
    chars: Range<usize>,
    color: Color32,
    link: Option<u32>,
    /// Зачёркивание рисуем сами — на той же высоте, что у обычного текста.
    strike: bool,
}

struct Built {
    job: LayoutJob,
    links: Vec<(Range<usize>, u32)>,
    maths: Vec<(usize, Arc<MathBox>, f32)>,
    decos: Vec<Deco>,
}

pub struct Renderer<'a> {
    pub pal: &'a Palette,
    pub vault: &'a Vault,
    pub note_rel: &'a Path,
    pub doc: &'a Doc,
    pub size: f32,
    pub actions: Vec<Action>,
    /// Спрашивать ли подсказки у плагинов (термины курса), когда указатель над текстом.
    pub hovers: bool,
    /// Запрошенная прокрутка к заголовку (текст в нижнем регистре) или сноске (`^метка`).
    pub scroll_to: Option<String>,
    /// Плагины: блоки, которые они рисуют, и цвета платформы для их элементов.
    pub plugins: Option<(&'a PluginUi, &'a forma_ui::theme::Palette)>,
    /// Где был правый щелчок этого кадра (контекстное меню) и что под ним оказалось.
    pub menu_at: Option<egui::Pos2>,
    pub menu_hit: Option<MenuHit>,
    /// Заголовок раздела, который рисуется сейчас.
    heading_now: String,
    anchors: Vec<(String, Rect)>,
    counter: usize,
    muted: bool,
    strike: bool,
    /// Задача, которую переключает щелчок по её тексту.
    task: Option<Range<usize>>,
    task_hover: bool,
    in_quote: bool,
    list_depth: usize,
    depth: usize,
}

impl<'a> Renderer<'a> {
    pub fn new(pal: &'a Palette, vault: &'a Vault, note_rel: &'a Path, doc: &'a Doc, size: f32) -> Self {
        Self {
            pal,
            vault,
            note_rel,
            doc,
            size,
            actions: Vec::new(),
            hovers: false,
            scroll_to: None,
            plugins: None,
            menu_at: None,
            menu_hit: None,
            heading_now: String::new(),
            anchors: Vec::new(),
            counter: 0,
            muted: false,
            strike: false,
            task: None,
            task_hover: false,
            in_quote: false,
            list_depth: 0,
            depth: 0,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let doc = self.doc;
        if !doc.properties.is_empty() {
            self.properties(ui);
            ui.add_space(self.size * 1.2);
        }
        self.blocks(ui, &doc.blocks, self.size * 0.9);
    }

    fn text_color(&self) -> Color32 {
        if self.muted { self.pal.text_muted } else { self.pal.text }
    }

    fn body_fmt(&self) -> Fmt {
        if self.in_quote {
            // Цитаты — курсивом с засечками, чуть крупнее.
            return Fmt {
                size: self.size * 1.06,
                color: self.pal.text,
                family: fonts::math_italic(),
                heading: true,
                line_height: 1.6,
                strike: self.strike,
                italic: false,
            };
        }
        Fmt {
            size: self.size,
            color: self.text_color(),
            family: FontFamily::Proportional,
            heading: false,
            line_height: 1.62,
            strike: self.strike,
            italic: false,
        }
    }

    fn next_id(&mut self, ui: &Ui, kind: &str) -> egui::Id {
        self.counter += 1;
        ui.id().with((kind, self.counter, self.depth))
    }

    fn blocks(&mut self, ui: &mut Ui, blocks: &[Block], gap: f32) {
        for (i, b) in blocks.iter().enumerate() {
            if i > 0 {
                let extra = match b {
                    Block::Heading { level, .. } => match level {
                        1 => self.size * 1.1,
                        2 => self.size * 1.0,
                        3 => self.size * 0.7,
                        _ => self.size * 0.45,
                    },
                    _ => 0.0,
                };
                ui.add_space(gap + extra);
            }
            self.block(ui, b);
        }
    }

    fn block(&mut self, ui: &mut Ui, block: &Block) {
        match block {
            Block::Heading { level, inlines, text } => self.heading(ui, *level, inlines, text),
            Block::Paragraph(items) => {
                let fmt = self.body_fmt();
                self.inlines(ui, items, &fmt);
            }
            Block::Code(cb) => self.code_blocks(ui, std::slice::from_ref(cb)),
            Block::CodeGroup(group) => self.code_blocks(ui, group),
            Block::Quote { blocks, author } => self.quote(ui, blocks, author.as_deref()),
            Block::Callout { kind, title, folded, body } => self.callout(ui, kind, title, *folded, body),
            Block::List { start, items } => self.list(ui, *start, items),
            Block::Table { aligns, head, rows, cell_aligns } => self.table(ui, aligns, cell_aligns, head, rows),
            Block::Rule => {
                ui.add_space(self.size * 0.6);
                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, self.pal.border));
                ui.add_space(self.size * 0.6);
            }
            Block::Html(html) => self.html(ui, html),
            Block::Math(tex) => self.display_math(ui, tex),
            Block::Footnote { label, number, blocks } => self.footnote(ui, label, *number, blocks),
        }
    }

    // ----------------------------------------------------------------------------------------
    // Инлайны
    // ----------------------------------------------------------------------------------------

    fn inlines(&mut self, ui: &mut Ui, items: &[Inline], fmt: &Fmt) {
        // Картинки — отдельные виджеты, текст между ними — отдельные абзацы.
        let mut start = 0;
        for (i, it) in items.iter().enumerate() {
            if let Inline::Image { src, alt, width, wiki } = it {
                if has_visible_text(&items[start..i]) {
                    self.text_run(ui, &items[start..i], fmt);
                }
                self.image(ui, src, alt, *width, *wiki);
                start = i + 1;
            }
        }
        if start == 0 {
            self.text_run(ui, items, fmt);
        } else if has_visible_text(&items[start..]) {
            self.text_run(ui, &items[start..], fmt);
        }
    }

    fn link_color(&self, link: &Link) -> Color32 {
        match link.kind {
            LinkKind::Wiki => {
                if link.target.starts_with('#') || self.vault.resolve(&link.target, Some(self.note_rel)).is_some() {
                    self.pal.link
                } else {
                    self.pal.link_unresolved
                }
            }
            LinkKind::Url => self.pal.link,
            LinkKind::Tag => self.pal.tag_text,
            LinkKind::Footnote => self.pal.accent,
        }
    }

    fn deco_kind(&self, st: &Style) -> Option<DecoKind> {
        if st.kbd {
            Some(DecoKind::Kbd)
        } else if st.code && st.badge {
            Some(DecoKind::Badge)
        } else if st.code {
            Some(DecoKind::Code)
        } else if st.link.and_then(|l| self.doc.links.get(l as usize)).is_some_and(|l| l.kind == LinkKind::Tag) {
            Some(DecoKind::Tag)
        } else if st.mark {
            Some(DecoKind::Mark)
        } else {
            None
        }
    }

    /// Собирает LayoutJob, а также диапазоны ссылок, места формул и подложки.
    fn build_job(&self, ui: &Ui, items: &[Inline], fmt: &Fmt, hovered: Option<u32>) -> Built {
        let mut job = LayoutJob::default();
        let mut links = Vec::new();
        let mut maths = Vec::new();
        let mut decos = Vec::new();
        let mut chars = 0usize;
        // egui добавляет межбуквенный интервал перед каждым символом, кроме самого первого в абзаце.
        let mut at_para_start = true;
        // Отступ справа от предыдущей подложки переносим в начало следующего куска.
        let mut pending_pad = 0.0f32;
        let line_height = Some(fmt.size * fmt.line_height);

        let base_format = |color: Color32| TextFormat {
            font_id: FontId::new(fmt.size, fmt.family.clone()),
            color,
            line_height,
            italics: fmt.italic && !fonts::has_italic(),
            strikethrough: if fmt.strike { Stroke::new(1.2, color) } else { Stroke::NONE },
            ..Default::default()
        };

        for item in items {
            match item {
                Inline::SoftBreak => {
                    job.append(" ", std::mem::take(&mut pending_pad), base_format(fmt.color));
                    chars += 1;
                    at_para_start = false;
                }
                Inline::HardBreak => {
                    job.append("\n", 0.0, base_format(fmt.color));
                    pending_pad = 0.0;
                    chars += 1;
                    at_para_start = true;
                }
                Inline::Image { .. } => {}
                Inline::Text(text, st) if st.math => {
                    let mbox = math::layout(ui.ctx(), text, fmt.size, false);
                    let fid = FontId::new(fmt.size, FontFamily::Proportional);
                    let glyph_w = ui.fonts_mut(|f| f.glyph_width(&fid, 'M'));
                    // Заглушка из двух невидимых «M», растянутых интервалом до ширины формулы.
                    let extra = mbox.width + fmt.size * 0.1 - 2.0 * glyph_w;
                    let (spacing, shift) = if at_para_start { (extra, 0.0) } else { (extra / 2.0, extra / 2.0) };
                    let format = TextFormat {
                        font_id: fid,
                        color: Color32::TRANSPARENT,
                        line_height,
                        extra_letter_spacing: spacing,
                        ..Default::default()
                    };
                    maths.push((chars, mbox, shift));
                    job.append("MM", std::mem::take(&mut pending_pad), format);
                    chars += 2;
                    at_para_start = false;
                }
                Inline::Text(text, st) => {
                    let (format, deco_color) = self.text_format(ui, st, fmt, hovered);
                    let n = text.chars().count();
                    if let Some(l) = st.link {
                        links.push((chars..chars + n, l));
                    }
                    let mut leading = std::mem::take(&mut pending_pad);
                    if let Some(kind) = self.deco_kind(st) {
                        let pad = kind.pad() * fmt.size;
                        leading += pad;
                        pending_pad = pad;
                        decos.push(Deco { kind, chars: chars..chars + n, color: deco_color, link: st.link, strike: st.strike || fmt.strike });
                    }
                    job.append(text, leading, format);
                    chars += n;
                    if !text.is_empty() {
                        at_para_start = text.ends_with('\n');
                    }
                }
            }
        }
        Built { job, links, maths, decos }
    }

    /// Формат куска текста и цвет его подложки (если она есть).
    fn text_format(&self, ui: &Ui, st: &Style, fmt: &Fmt, hovered: Option<u32>) -> (TextFormat, Color32) {
        let pal = self.pal;
        let link = st.link.and_then(|i| self.doc.links.get(i as usize));
        let bold = st.bold;
        let italic = st.italic || fmt.italic;
        let kind = self.deco_kind(st);
        let family = match kind {
            Some(DecoKind::Code) => FontFamily::Monospace,
            Some(DecoKind::Kbd | DecoKind::Badge) => fonts::semibold(),
            _ => match (bold, italic) {
                (true, true) => fonts::bold_italic(),
                (true, false) if fmt.heading => fmt.family.clone(),
                (true, false) => fonts::bold(),
                (false, true) if fmt.heading => fmt.family.clone(),
                (false, true) => fonts::italic(),
                (false, false) => fmt.family.clone(),
            },
        };
        let mut size = fmt.size;
        let mut color = fmt.color;
        let mut underline = Stroke::NONE;
        let mut valign = Align::BOTTOM;
        let mut deco_color = Color32::TRANSPARENT;

        if bold && kind.is_none() && !fmt.heading {
            color = if self.muted { pal.text_muted } else { pal.heading };
        }
        match kind {
            Some(DecoKind::Code) => {
                size *= 0.86;
                let tint = st.tint.map_or(pal.code_tint, |c| Color32::from_rgb(c[0], c[1], c[2]));
                color = pal.tint_text(tint);
                deco_color = tint;
            }
            Some(DecoKind::Badge) => {
                size *= 0.78;
                match st.tint {
                    Some(c) => {
                        let tint = Color32::from_rgb(c[0], c[1], c[2]);
                        color = pal.tint_text(tint);
                        deco_color = tint.gamma_multiply(0.22);
                    }
                    None => {
                        color = pal.badge_text;
                        deco_color = pal.badge_bg;
                    }
                }
            }
            Some(DecoKind::Kbd) => {
                size *= 0.8;
                color = pal.heading;
                deco_color = pal.kbd_bg;
            }
            Some(DecoKind::Mark) => {
                color = pal.heading;
                deco_color = st.tint.map_or(pal.mark_bg, |c| Color32::from_rgb(c[0], c[1], c[2]).gamma_multiply(0.32));
            }
            Some(DecoKind::Tag) => {
                size *= 0.9;
                color = pal.tag_text;
                deco_color = if hovered == st.link { pal.accent.gamma_multiply(0.3) } else { pal.accent.gamma_multiply(0.16) };
            }
            None => {}
        }
        if let Some(l) = link
            && kind != Some(DecoKind::Tag)
        {
            color = self.link_color(l);
            let is_hovered = hovered == st.link;
            if l.kind == LinkKind::Url && !is_hovered {
                underline = Stroke::new(1.0, color.gamma_multiply(0.4));
            }
            if is_hovered {
                underline = Stroke::new(1.0, color);
            }
        }
        if st.footnote {
            size *= 0.7;
            valign = Align::TOP;
        }
        let strike = st.strike || fmt.strike;
        let font_id = FontId::new(size, family);
        // egui ставит базовую линию на высоту подъёма конкретного шрифта. Чтобы код, теги и другие
        // шрифты стояли на одной линии с текстом, компенсируем разницу через высоту строки.
        let line = fmt.size * fmt.line_height;
        let line_height = if st.footnote {
            line
        } else {
            let base = FontId::new(fmt.size, fmt.family.clone());
            line - (font_ascent(ui, &base) - font_ascent(ui, &font_id))
        };
        let format = TextFormat {
            font_id,
            color,
            line_height: Some(line_height),
            italics: italic && !fonts::has_italic() && kind.is_none(),
            underline,
            // У «пилюль» своя высота строки, и линия egui съехала бы вниз — их зачёркиваем отдельно.
            strikethrough: if strike && kind.is_none() { Stroke::new(1.2, color) } else { Stroke::NONE },
            valign,
            ..Default::default()
        };
        (format, deco_color)
    }

    /// Рисует подложки под кодом, тегами, клавишами, бейджами и выделением.
    fn paint_decos(&self, ui: &Ui, pos: egui::Pos2, galley: &Galley, decos: &[Deco], size: f32, hovered: Option<u32>) {
        let pal = self.pal;
        let painter = ui.painter();
        for d in decos {
            let pad = d.kind.pad() * size;
            for (x0, x1, baseline) in span_segments(galley, &d.chars) {
                let center = pos.y + baseline - size * 0.3;
                let half = match d.kind {
                    DecoKind::Code => size * 0.64,
                    DecoKind::Kbd => size * 0.66,
                    DecoKind::Badge => size * 0.6,
                    DecoKind::Tag => size * 0.62,
                    DecoKind::Mark => size * 0.62,
                };
                let r = Rect::from_min_max(pos2(pos.x + x0 - pad, center - half), pos2(pos.x + x1 + pad, center + half));
                match d.kind {
                    DecoKind::Code => {
                        painter.rect(
                            r,
                            CornerRadius::same(5),
                            d.color.gamma_multiply(if pal.dark { 0.1 } else { 0.08 }),
                            Stroke::new(1.0, d.color.gamma_multiply(if pal.dark { 0.32 } else { 0.35 })),
                            egui::StrokeKind::Inside,
                        );
                    }
                    DecoKind::Kbd => {
                        // «Объёмная» клавиша: тень снизу, корпус и рамка.
                        painter.rect_filled(r.translate(vec2(0.0, 1.5)), CornerRadius::same(5), pal.kbd_shadow);
                        painter.rect(r, CornerRadius::same(5), d.color, Stroke::new(1.0, pal.kbd_border), egui::StrokeKind::Inside);
                    }
                    DecoKind::Badge => {
                        painter.rect_filled(r, CornerRadius::same(6), d.color);
                    }
                    DecoKind::Tag => {
                        let fill = if d.link.is_some() && d.link == hovered { pal.accent.gamma_multiply(0.3) } else { d.color };
                        painter.rect_filled(r, CornerRadius::same(u8::MAX), fill);
                    }
                    DecoKind::Mark => {
                        painter.rect_filled(r, CornerRadius::same(4), d.color);
                    }
                }
            }
        }
    }

    /// Зачёркивание кода, клавиш и бейджей — на высоте зачёркивания обычного текста абзаца.
    fn paint_deco_strikes(&self, ui: &Ui, pos: egui::Pos2, galley: &Galley, decos: &[Deco], fmt: &Fmt) {
        if !decos.iter().any(|d| d.strike) {
            return;
        }
        let offset = strike_offset(ui, fmt);
        for d in decos.iter().filter(|d| d.strike) {
            let pad = d.kind.pad() * fmt.size;
            for (x0, x1, baseline) in span_segments(galley, &d.chars) {
                let y = pos.y + baseline + offset;
                ui.painter().hline(pos.x + x0 - pad..=pos.x + x1 + pad, y, Stroke::new(1.2, fmt.color));
            }
        }
    }

    fn text_run(&mut self, ui: &mut Ui, items: &[Inline], fmt: &Fmt) -> egui::Response {
        let id = self.next_id(ui, "run");
        let hovered: Option<u32> = ui.data(|d| d.get_temp::<Option<u32>>(id)).flatten();
        let built = self.build_job(ui, items, fmt, hovered);
        let Built { job, links, maths, decos } = built;
        let (pos, galley, resp) = Label::new(job).wrap().selectable(true).sense(Sense::click()).layout_in_ui(ui);

        let mut new_hover = None;
        if !links.is_empty() {
            if let Some(p) = resp.hover_pos() {
                new_hover = link_at(&galley, p - pos, &links);
            }
            if new_hover != hovered {
                ui.data_mut(|d| d.insert_temp(id, new_hover));
                ui.ctx().request_repaint();
            }
            let modifiers = ui.input(|i| i.modifiers);
            let clicked = resp.clicked() || resp.middle_clicked();
            if clicked
                && let Some(li) = resp.interact_pointer_pos().and_then(|p| link_at(&galley, p - pos, &links))
            {
                let new_tab = modifiers.command || resp.middle_clicked();
                self.link_action(ui, li, new_tab);
            }
        }

        // Правый щелчок по этому тексту: запомнить раздел, абзац и ссылку для меню.
        if let Some(p) = self.menu_at
            && resp.rect.contains(p)
        {
            let link = link_at(&galley, p - pos, &links).and_then(|li| self.doc.links.get(li as usize)).and_then(|l| match l.kind {
                LinkKind::Wiki => Some((l.target.clone(), false)),
                LinkKind::Url => Some((l.target.clone(), true)),
                _ => None,
            });
            self.menu_hit = Some(MenuHit { heading: self.heading_now.clone(), paragraph: galley.text().to_owned(), link });
        }

        // Подсказки плагинов (термины курса): что под указателем — запрос программе. Ответ
        // приходит в следующем кадре и держится, пока указатель внутри найденного term.
        let mut plugin_hover: Option<HoverAnswer> = None;
        if self.hovers
            && new_hover.is_none()
            && let Some(p) = resp.hover_pos()
        {
            let offset: usize = galley.cursor_from_pos(p - pos).index.into();
            let hash = text_hash(galley.text());
            let key = id.with("plugin-hover");
            let answer: Option<HoverAnswer> = ui.data(|d| d.get_temp::<Option<HoverAnswer>>(key)).flatten();
            match answer.filter(|a| a.text_hash == hash && a.range.contains(&offset)) {
                Some(a) => plugin_hover = Some(a),
                None => {
                    let asked: Option<(u64, usize)> = ui.data(|d| d.get_temp(key.with("asked")));
                    if asked != Some((hash, offset)) {
                        ui.data_mut(|d| d.insert_temp(key.with("asked"), (hash, offset)));
                        self.actions.push(Action::Hover { key, text: galley.text().to_owned(), offset });
                    }
                }
            }
        }

        if ui.is_rect_visible(resp.rect) {
            self.paint_decos(ui, pos, &galley, &decos, fmt.size, hovered);
            // Найденный term — пунктиром снизу, как подсказка в IDE.
            if let Some(a) = &plugin_hover {
                for (x0, x1, baseline) in span_segments(&galley, &a.range) {
                    let y = pos.y + baseline + fmt.size * 0.2;
                    let line = [egui::pos2(pos.x + x0, y), egui::pos2(pos.x + x1, y)];
                    ui.painter().extend(egui::Shape::dotted_line(&line, self.pal.accent, 3.0, 0.9));
                }
            }
            LabelSelectionState::label_text_selection(ui, &resp, pos, galley.clone(), fmt.color, Stroke::NONE);
            self.paint_deco_strikes(ui, pos, &galley, &decos, fmt);
            for (ci, mbox, shift) in &maths {
                if let Some(origin) = math_origin(&galley, *ci) {
                    let p = pos + origin + vec2(fmt.size * 0.05 - shift, 0.0);
                    mbox.paint(ui.painter(), p, fmt.color);
                }
            }
        }
        // Карточка подсказки — маленькая заметка: markdown рисует тот же renderer.
        if let Some(a) = plugin_hover {
            let (pal, vault, note, size) = (self.pal, self.vault, self.note_rel, self.size);
            resp.clone().on_hover_ui_at_pointer(|ui| {
                ui.set_max_width(size * 24.0);
                let doc = crate::markdown::parse(&a.markdown);
                Renderer::new(pal, vault, note, &doc, size * 0.92).show(ui);
            });
        }
        // Щелчок по тексту задачи переключает её галочку (если щёлкнули не по ссылке).
        if let Some(range) = self.task.clone()
            && new_hover.is_none()
        {
            if resp.hovered() {
                self.task_hover = true;
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.actions.push(Action::ToggleTask(range));
            }
        }
        if let Some(li) = new_hover {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            if let Some(link) = self.doc.links.get(li as usize) {
                if link.kind == LinkKind::Url {
                    let target = link.target.clone();
                    resp.clone().on_hover_text_at_pointer(target);
                } else if link.kind == LinkKind::Wiki
                    && self.vault.resolve(&link.target, Some(self.note_rel)).is_none()
                    && !link.target.starts_with('#')
                {
                    resp.clone().on_hover_text_at_pointer(t!("preview-missing-link"));
                }
            }
        }
        resp
    }

    /// Базовая линия первой строки текста (от верха), если ячейка начинается с текста.
    fn first_baseline(&self, ui: &Ui, items: &[Inline], fmt: &Fmt, width: f32) -> Option<f32> {
        let end = items.iter().position(|i| matches!(i, Inline::Image { .. })).unwrap_or(items.len());
        if end == 0 || !has_visible_text(&items[..end]) {
            return None;
        }
        let mut built = self.build_job(ui, &items[..end], fmt, None);
        built.job.wrap.max_width = width;
        let galley = ui.fonts_mut(|f| f.layout_job(built.job));
        let row = galley.rows.first()?;
        row.row.glyphs.iter().map(|g| row.pos.y + g.pos.y).reduce(f32::max)
    }

    /// Ширина набора инлайнов одной строкой (для расчёта колонок таблицы).
    fn measure(&self, ui: &Ui, items: &[Inline], fmt: &Fmt) -> f32 {
        let mut built = self.build_job(ui, items, fmt, None);
        built.job.wrap.max_width = f32::INFINITY;
        let galley = ui.fonts_mut(|f| f.layout_job(built.job));
        let pad = built.decos.last().map_or(0.0, |d| d.kind.pad() * fmt.size);
        galley.size().x + pad
    }

    fn link_action(&mut self, ui: &Ui, li: u32, new_tab: bool) {
        let Some(link) = self.doc.links.get(li as usize) else { return };
        match link.kind {
            LinkKind::Url => self.actions.push(Action::OpenUrl(link.target.clone())),
            LinkKind::Tag => self.actions.push(Action::SearchTag(link.target.clone())),
            LinkKind::Footnote => self.request_scroll(ui, format!("^{}", link.target)),
            LinkKind::Wiki => {
                if link.target.starts_with('#') {
                    // Раздел разбираем так же, как при переходе в другую заметку: без `|текста` и пробелов.
                    if let Some(h) = crate::markdown::link_heading_part(&link.target) {
                        self.request_scroll(ui, h.to_lowercase());
                    }
                } else {
                    self.actions.push(Action::OpenNote { target: link.target.clone(), new_tab });
                }
            }
        }
    }

    fn request_scroll(&mut self, ui: &Ui, key: String) {
        if let Some((_, rect)) = self.anchors.iter().find(|(k, _)| *k == key) {
            ui.scroll_to_rect(*rect, Some(Align::TOP));
        } else {
            self.scroll_to = Some(key);
        }
    }

    fn anchor(&mut self, ui: &Ui, key: String, rect: Rect) {
        if self.scroll_to.as_deref() == Some(key.as_str()) {
            ui.scroll_to_rect(rect.expand(8.0), Some(Align::TOP));
            self.scroll_to = None;
        }
        self.anchors.push((key, rect));
    }

    // ----------------------------------------------------------------------------------------
    // Блоки
    // ----------------------------------------------------------------------------------------

    fn heading(&mut self, ui: &mut Ui, level: u8, inlines: &[Inline], text: &str) {
        let s = self.size;
        let (scale, family) = match level {
            1 => (2.0, fonts::bold()),
            2 => (1.6, fonts::bold()),
            3 => (1.3, fonts::semibold()),
            4 => (1.12, fonts::semibold()),
            5 => (1.0, fonts::semibold()),
            _ => (0.92, fonts::semibold()),
        };
        let color = if level >= 6 { self.pal.text_muted } else { self.pal.heading };
        self.heading_now = text.to_owned();
        let fmt = Fmt { size: s * scale, color, family, heading: true, line_height: 1.3, strike: false, italic: false };
        let resp = self.text_run(ui, inlines, &fmt);
        if level <= 2 {
            ui.add_space(s * 0.35);
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            let c = if level == 1 { self.pal.border } else { self.pal.border.gamma_multiply(0.7) };
            ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, c));
        }
        self.anchor(ui, text.to_lowercase(), resp.rect);
    }

    /// Свойства из YAML-шапки — карточкой с иконками.
    fn properties(&mut self, ui: &mut Ui) {
        let pal = self.pal;
        let s = self.size;
        let doc = self.doc;
        // Колонка имён — не шире 40 % карточки: в узкой области (заметки рядом с кодом в
        // Instar) значениям тоже остаётся место.
        let key_w = (s * 9.0).min(ui.available_width() * 0.4);
        Frame::new()
            .fill(pal.card_bg)
            .stroke(Stroke::new(1.0, pal.border))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin { left: 16, right: 16, top: 12, bottom: 10 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.label(RichText::new(t!("preview-properties").to_uppercase()).size(s * 0.68).family(fonts::semibold()).color(pal.text_faint));
                ui.add_space(s * 0.45);
                for (i, (k, v)) in doc.properties.iter().enumerate() {
                    if i > 0 {
                        let r = ui.available_rect_before_wrap();
                        ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, pal.border.gamma_multiply(0.6)));
                    }
                    ui.allocate_ui_with_layout(vec2(ui.available_width(), s * 2.1), Layout::left_to_right(Align::Center), |ui| {
                        let (icon_rect, _) = ui.allocate_exact_size(vec2(s * 1.4, s * 1.2), Sense::hover());
                        ui.painter().text(
                            icon_rect.center(),
                            Align2::CENTER_CENTER,
                            property_icon(k),
                            FontId::proportional(s * 0.85),
                            pal.text_faint,
                        );
                        ui.add_space(s * 0.3);
                        let (key_rect, _) = ui.allocate_exact_size(vec2(key_w, s * 1.2), Sense::hover());
                        ui.painter().with_clip_rect(key_rect).text(
                            key_rect.left_center(),
                            Align2::LEFT_CENTER,
                            k,
                            FontId::proportional(s * 0.86),
                            pal.text_muted,
                        );
                        let is_tags = k.eq_ignore_ascii_case("tags") || k.eq_ignore_ascii_case("теги") || k.eq_ignore_ascii_case("aliases");
                        if is_tags {
                            ui.spacing_mut().item_spacing.x = s * 0.35;
                            for t in v.split(',').map(str::trim).filter(|t| !t.is_empty()) {
                                let t = t.trim_start_matches('#');
                                if tag_pill(ui, pal, &format!("#{t}"), s * 0.82).clicked() {
                                    self.actions.push(Action::SearchTag(t.to_owned()));
                                }
                            }
                        } else {
                            ui.add(Label::new(RichText::new(v).size(s * 0.92).color(pal.text)).wrap());
                        }
                    });
                }
            });
    }

    /// Блок кода (или группа с вкладками) в стиле современных сайтов документации.
    /// Сеансы консоли и логи — тоже блоки кода, со своей шапкой и раскраской.
    fn code_blocks(&mut self, ui: &mut Ui, group: &[CodeBlock]) {
        if group.is_empty() {
            return;
        }
        let pal = self.pal;
        let s = self.size;
        let id = self.next_id(ui, "code");
        let sel_id = id.with("tab");
        let mut sel: usize = ui.data(|d| d.get_temp(sel_id)).unwrap_or(0);
        sel = sel.min(group.len() - 1);
        let code_size = s * 0.84;
        let radius = 12u8;
        let view_id = id.with(("view", sel));
        let mut show_code: bool = ui.data(|d| d.get_temp(view_id)).unwrap_or(false);
        let rendered = diagram::is_diagram(&group[sel].lang).then(|| diagram_scene(ui.ctx(), id.with(("diagram", sel)), &group[sel]));
        let diagram_kind = rendered.as_deref().and_then(|r| r.as_ref().ok()).map(|(k, _)| *k);
        // Блок, который рисует плагин: его название и дерево (если плагин уже ответил).
        let plugin_block = self.plugins.and_then(|(p, _)| {
            let cb = &group[sel];
            let title = p.block_title(&cb.lang)?;
            let tree = p.block_trees.get(&PluginUi::block_key(&cb.lang, &cb.code, self.note_rel));
            Some((title.to_owned(), tree))
        });
        let plugin_title = plugin_block.as_ref().map(|(t, _)| if t.is_empty() { highlight::display_name(&group[sel].lang) } else { t.clone() });
        // Блок начинается со своей шапки (API 1.3) — она встаёт в шапку рамки вместо названия.
        let plugin_header = plugin_block.as_ref().and_then(|(_, tree)| match tree {
            Some(Tree::Ready(forma_plugin_ui::Element::Column { children, .. })) => match children.first() {
                Some(forma_plugin_ui::Element::Header { title, icon, badge, aside }) => Some((title.clone(), icon.clone(), badge.clone(), aside.clone())),
                _ => None,
            },
            _ => None,
        });

        Frame::new()
            .fill(pal.code_bg)
            .stroke(Stroke::new(1.0, pal.code_border))
            .corner_radius(CornerRadius::same(radius))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // Шапка: вкладки, имя файла и кнопка копирования — одной невысокой строкой.
                Frame::new()
                    .fill(pal.code_header)
                    .corner_radius(CornerRadius { nw: radius - 1, ne: radius - 1, sw: 0, se: 0 })
                    .inner_margin(Margin { left: 12, right: 8, top: 4, bottom: 4 })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.allocate_ui_with_layout(vec2(ui.available_width(), s * 1.5), Layout::left_to_right(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = s * 0.35;
                            let tabs = group.len() > 1 || group[0].tab.is_some();
                            if tabs {
                                for (i, cb) in group.iter().enumerate() {
                                    let label = cb.tab.clone().unwrap_or_else(|| highlight::display_name(&cb.lang));
                                    if code_tab(ui, pal, &label, i == sel, s * 0.78).clicked() && i != sel {
                                        sel = i;
                                        ui.data_mut(|d| d.insert_temp(sel_id, sel));
                                        theme::restart_fade(ui.ctx(), id.with(("fade", sel)));
                                    }
                                }
                                ui.add_space(s * 0.4);
                            }
                            let cb = &group[sel];
                            let kind = term::kind(&cb.lang);
                            // Название блока плагина — обычным шрифтом (это не имя файла и не язык).
                            let title_font = if plugin_title.is_some() && cb.title.is_none() { FontId::proportional(s * 0.8) } else { FontId::monospace(s * 0.8) };
                            match kind {
                                Kind::Code if plugin_header.is_some() && cb.title.is_none() => {
                                    let (title, icon, badge, _) = plugin_header.clone().unwrap_or_default();
                                    if let Some(icon) = icon {
                                        let (r, _) = ui.allocate_exact_size(Vec2::splat(s * 0.95), Sense::hover());
                                        forma_ui::icons::paint(ui.painter(), r, &icon, pal.accent);
                                    }
                                    ui.label(RichText::new(title).font(FontId::new(s * 0.84, fonts::semibold())).color(pal.heading));
                                    if !badge.is_empty() {
                                        header_badge(ui, pal, &badge, s * 0.72);
                                    }
                                }
                                Kind::Code => {
                                    let kind_title = diagram_kind.map(|k| k.title().to_owned()).or_else(|| plugin_title.clone());
                                    let title = cb
                                        .title
                                        .clone()
                                        .or(kind_title)
                                        .unwrap_or_else(|| if tabs { String::new() } else { highlight::display_name(&cb.lang) });
                                    if !title.is_empty() {
                                        ui.label(RichText::new(title).font(title_font).color(pal.text_muted));
                                    }
                                }
                                Kind::Console | Kind::Log => {
                                    let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(s * 0.95), Sense::hover());
                                    let icon = if kind == Kind::Console { "console" } else { "log" };
                                    forma_ui::icons::paint(ui.painter(), icon_rect, icon, pal.text_muted);
                                    let title = match (&cb.title, kind) {
                                        (Some(t), _) => t.clone(),
                                        (None, Kind::Log) => "log".to_owned(),
                                        _ => String::new(),
                                    };
                                    if !title.is_empty() {
                                        ui.label(RichText::new(title).font(title_font).color(pal.text_muted));
                                    }
                                    let badge = if kind == Kind::Console {
                                        "bash".to_owned()
                                    } else {
                                        t!("code-lines", count = term::line_count(&cb.code))
                                    };
                                    header_badge(ui, pal, &badge, s * 0.72);
                                }
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().button_padding = vec2(s * 0.5, s * 0.15);
                                // Блок с шапкой (задача, тест): копировать нечего — только «Код/Блок».
                                let copyable = plugin_header.is_none();
                                let copied_id = id.with("copied");
                                let copied_at: Option<f64> = ui.data(|d| d.get_temp(copied_id));
                                let now = ui.input(|i| i.time);
                                let recently = copied_at.is_some_and(|t| now - t < 1.6);
                                let label = match kind {
                                    Kind::Code => t!("code-copy"),
                                    Kind::Console => t!("code-copy-commands"),
                                    Kind::Log => t!("code-copy-log"),
                                };
                                let text = if recently { format!("✔ {}", t!("code-copied")) } else { format!("⧉ {label}") };
                                let color = if recently { pal.success } else { pal.text_muted };
                                let btn = egui::Button::new(RichText::new(text).size(s * 0.8).family(fonts::semibold()).color(color))
                                    .frame_when_inactive(false)
                                    .corner_radius(6);
                                if copyable && ui.add(btn).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    // Копируем всегда весь текст, даже у свёрнутого блока.
                                    let text = match kind {
                                        Kind::Console => term::commands_text(&cb.code),
                                        _ => cb.code.clone(),
                                    };
                                    ui.ctx().copy_text(text);
                                    ui.data_mut(|d| d.insert_temp(copied_id, now));
                                }
                                if recently {
                                    ui.ctx().request_repaint_after_secs(0.4);
                                }
                                // Диаграмма (или блок плагина) — или её исходный код.
                                if diagram_kind.is_some() || plugin_block.is_some() {
                                    ui.add_space(s * 0.3);
                                    let shown = if diagram_kind.is_some() { t!("code-view-diagram") } else { t!("code-view-block") };
                                    for (label, code_view) in [(t!("code-view-code"), true), (shown, false)] {
                                        if code_tab(ui, pal, &label, show_code == code_view, s * 0.74).clicked() && show_code != code_view {
                                            show_code = code_view;
                                            ui.data_mut(|d| d.insert_temp(view_id, show_code));
                                            theme::restart_fade(ui.ctx(), id.with(("fade", sel)));
                                        }
                                    }
                                }
                                // Текст справа в шапке блока («4 вопроса»).
                                if let Some((_, _, _, aside)) = &plugin_header
                                    && !aside.is_empty()
                                {
                                    ui.add_space(s * 0.5);
                                    ui.label(RichText::new(aside).size(s * 0.78).color(pal.text_muted));
                                }
                            });
                        });
                    });
                let r = ui.available_rect_before_wrap();
                ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, pal.code_border));

                // Тело: номера строк и подсвеченный код, сеанс консоли или лог.
                let cb = &group[sel];
                let kind = term::kind(&cb.lang);
                let line_h = code_size
                    * match kind {
                        Kind::Code => 1.7,
                        Kind::Console => 1.6,
                        Kind::Log => 1.5,
                    };
                let text_shown = (diagram_kind.is_none() && plugin_block.is_none()) || show_code;
                let limit = term::collapse_at(cb).filter(|_| text_shown);
                let expanded_id = id.with(("expanded", sel));
                let expanded: bool = ui.data(|d| d.get_temp(expanded_id)).unwrap_or(false);
                let shown = limit.filter(|_| !expanded);
                // egui ставит текст в верх строки высотой `line_h`, а весь межстрочный зазор
                // оставляет под ним: под последней строкой он лишний, его вычитаем из поля.
                let row_h = ui.fonts_mut(|f| f.row_height(&FontId::monospace(code_size)));
                let leading = if text_shown { (line_h - row_h).max(0.0) } else { 0.0 };
                let pad = 12.0;
                let fade = theme::fade(ui.ctx(), id.with(("fade", sel)), 0.2);
                let body = Frame::new()
                    .inner_margin(Margin { left: 14, right: 16, top: pad as i8, bottom: (pad - leading).round().max(2.0) as i8 })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.multiply_opacity(fade);
                        if let Some((_, tree)) = &plugin_block
                            && !show_code
                            && self.plugin_block(ui, cb, *tree, id.with(("plugin", sel)))
                        {
                            return;
                        }
                        if let Some(result) = rendered.as_deref() {
                            match result {
                                Ok((_, scene)) if !show_code => {
                                    self.paint_diagram(ui, scene, id.with(("dscroll", sel)));
                                    return;
                                }
                                Err(err) => {
                                    let color = if matches!(err, diagram::Error::Unsupported(_)) { pal.text_faint } else { pal.warning };
                                    ui.label(RichText::new(err.to_string()).size(s * 0.8).color(color));
                                    ui.add_space(s * 0.6);
                                }
                                _ => {}
                            }
                        }
                        let code = shown.map_or(cb.code.as_str(), |n| term::head(&cb.code, n));
                        match kind {
                            Kind::Code => self.code_body(ui, cb, code, id.with(("scroll", sel)), code_size, line_h),
                            Kind::Console => self.console_body(ui, cb, code, id.with(("console", sel)), code_size, line_h),
                            Kind::Log => self.log_body(ui, cb, code, code_size, line_h),
                        }
                    });

                // Свёрнутый блок: последние строки растворяются в фоне, ниже — «Показать ещё».
                let Some(limit) = limit else { return };
                if shown.is_some() {
                    let r = body.response.rect;
                    let top = (r.bottom() - line_h * 2.0 - pad).max(r.top());
                    fade_down(ui.painter(), Rect::from_min_max(pos2(r.left(), top), r.max), pal.code_bg);
                }
                let r = ui.available_rect_before_wrap();
                ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, pal.code_border));
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), s * 1.9), Sense::click());
                let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
                let corner = CornerRadius { nw: 0, ne: 0, sw: radius - 1, se: radius - 1 };
                ui.painter().rect_filled(rect.shrink(0.5), corner, pal.bg_hover.gamma_multiply(hover * 0.6));
                let (icon, label) = if expanded {
                    ("chevron-up", t!("code-collapse"))
                } else {
                    ("chevron-down", t!("code-show-more", count = term::line_count(&cb.code) - limit))
                };
                let color = mix(pal.text_muted, pal.heading, hover * 0.6);
                let galley = ui.painter().layout_no_wrap(label, FontId::new(s * 0.8, fonts::semibold()), color);
                let icon_side = s * 0.9;
                let gap = s * 0.35;
                let x0 = rect.center().x - (icon_side + gap + galley.size().x) / 2.0;
                let icon_rect = Rect::from_min_size(pos2(x0, rect.center().y - icon_side / 2.0), Vec2::splat(icon_side));
                forma_ui::icons::paint(ui.painter(), icon_rect, icon, color);
                ui.painter().galley(pos2(icon_rect.right() + gap, rect.center().y - galley.size().y / 2.0), galley, color);
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    ui.data_mut(|d| d.insert_temp(expanded_id, !expanded));
                    theme::restart_fade(ui.ctx(), id.with(("fade", sel)));
                }
            });
    }

    /// Тело блока плагина: его дерево элементов. Пока плагин не ответил — просьба нарисовать
    /// и «Плагин готовится». Не нарисовался — `false` и пометка: дальше покажется код.
    fn plugin_block(&mut self, ui: &mut Ui, cb: &CodeBlock, tree: Option<&Tree>, id: egui::Id) -> bool {
        let Some((_, fpal)) = self.plugins else { return false };
        match tree {
            Some(Tree::Ready(e)) => {
                let linked = self.plugins.map_or(&[][..], |(p, _)| p.linked.as_slice());
                let mut hooks = crate::plugin_ui::Hooks { pal: self.pal, vault: self.vault, linked, note: self.note_rel, size: self.size * 0.95, actions: Vec::new() };
                // Шапка блока уже стоит в шапке рамки — тело без неё.
                let body = match e {
                    forma_plugin_ui::Element::Column { children, gap } if matches!(children.first(), Some(forma_plugin_ui::Element::Header { .. })) => {
                        std::borrow::Cow::Owned(forma_plugin_ui::Element::Column { children: children[1..].to_vec(), gap: *gap })
                    }
                    _ => std::borrow::Cow::Borrowed(e),
                };
                let out = forma_ui::elements::show(ui, fpal, id, &body, &mut hooks);
                self.actions.append(&mut hooks.actions);
                for o in out {
                    match o {
                        forma_ui::elements::Output::Command { command, args } => self.actions.push(Action::PluginCommand { command, args }),
                        forma_ui::elements::Output::Event(event) => self.actions.push(Action::BlockEvent { lang: cb.lang.clone(), source: cb.code.clone(), event }),
                    }
                }
                true
            }
            Some(Tree::Failed(err)) => {
                let text = t!("plugin-block-failed", lang = cb.lang.clone(), error = err.clone());
                ui.add(Label::new(RichText::new(text).size(self.size * 0.8).color(self.pal.warning)).wrap());
                ui.add_space(self.size * 0.6);
                false
            }
            None => {
                self.actions.push(Action::RenderBlock { lang: cb.lang.clone(), source: cb.code.clone() });
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(self.size * 0.8).color(self.pal.accent));
                    ui.label(RichText::new(t!("plugin-panel-loading")).size(self.size * 0.8).color(self.pal.text_faint));
                });
                true
            }
        }
    }

    /// Обычный код: номера строк (если строк больше одной) и подсветка с прокруткой вбок.
    fn code_body(&mut self, ui: &mut Ui, cb: &CodeBlock, code: &str, scroll: egui::Id, code_size: f32, line_h: f32) {
        let pal = self.pal;
        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
            let total = term::line_count(&cb.code);
            if total >= 2 {
                let digits = total.to_string().len();
                let numbers: String = (1..=term::line_count(code)).map(|n| format!("{n:>digits$}\n")).collect();
                let mut job = LayoutJob::default();
                job.append(
                    numbers.trim_end_matches('\n'),
                    0.0,
                    TextFormat { font_id: FontId::monospace(code_size), color: pal.code_gutter, line_height: Some(line_h), ..Default::default() },
                );
                ui.add(Label::new(job).extend().selectable(false));
                ui.add_space(self.size * 1.1);
            }
            let job = highlight::job(ui.ctx(), code, &cb.lang, code_size, line_h);
            ScrollArea::horizontal().id_salt(scroll).show(ui, |ui| {
                ui.add(Label::new(job).extend().selectable(true));
            });
        });
    }

    /// Сеанс консоли: приглашение рисуется отдельно (его не выделить и не скопировать),
    /// у команды под указателем — кнопка копирования.
    fn console_body(&mut self, ui: &mut Ui, cb: &CodeBlock, code: &str, id: egui::Id, code_size: f32, line_h: f32) {
        let pal = self.pal;
        let font = FontId::monospace(code_size);
        let row_h = ui.fonts_mut(|f| f.row_height(&font));
        let fmt = |color: Color32| TextFormat { font_id: font.clone(), color, line_height: Some(line_h), ..Default::default() };
        // Приглашение: папка (если есть) и зелёный `$`.
        let mut prompt = LayoutJob::default();
        if let Some(cwd) = &cb.cwd {
            prompt.append(cwd, 0.0, fmt(mix(pal.term_info, pal.text_muted, 0.35)));
            prompt.append(" ", 0.0, fmt(pal.text_muted));
        }
        prompt.append("$ ", 0.0, TextFormat { font_id: FontId::new(code_size, fonts::mono_bold()), ..fmt(pal.success) });
        let prompt = ui.fonts_mut(|f| f.layout_job(prompt));
        let view = console_view(ui.ctx(), pal, code, code_size, line_h, prompt.size().x);
        let galley = ui.fonts_mut(|f| f.layout_job(view.job.clone()));

        let out = ScrollArea::horizontal().id_salt(id.with("scroll")).show(ui, |ui| {
            let resp = ui.add(Label::new(galley.clone()).selectable(true));
            let origin = resp.rect.min;
            for c in &view.commands {
                if let Some(row) = galley.rows.get(c.lines.start) {
                    ui.painter().galley(origin + row.pos.to_vec2(), prompt.clone(), pal.text);
                }
            }
            resp.rect
        });

        // Кнопка «копировать команду» справа у строки под указателем.
        let label = out.inner;
        let view_rect = out.inner_rect;
        let side = row_h.max(code_size * 1.4);
        for (i, c) in view.commands.iter().enumerate() {
            if c.text.trim().is_empty() {
                continue;
            }
            let (Some(first), Some(last)) = (galley.rows.get(c.lines.start), galley.rows.get(c.lines.end - 1)) else { continue };
            let rows = Rect::from_x_y_ranges(view_rect.x_range(), label.top() + first.pos.y..=label.top() + last.pos.y + line_h);
            let icon_rect = Rect::from_center_size(pos2(view_rect.right() - side / 2.0, rows.top() + row_h / 2.0), Vec2::splat(side));
            let btn_id = id.with(("copy", i));
            let hovered = ui.rect_contains_pointer(rows);
            let copied_id = id.with(("copied", i));
            let now = ui.input(|inp| inp.time);
            let recently = ui.data(|d| d.get_temp::<f64>(copied_id)).is_some_and(|t| now - t < 1.4);
            if !hovered && !recently {
                continue;
            }
            let resp = ui.interact(icon_rect, btn_id, Sense::click());
            let painter = ui.painter();
            let fill = if resp.hovered() { mix(pal.code_bg, pal.bg_hover, 0.9) } else { pal.code_bg };
            painter.rect(icon_rect, CornerRadius::same(5), fill, Stroke::new(1.0, pal.code_border), egui::StrokeKind::Inside);
            let (icon, color) = if recently { ("check", pal.success) } else { ("copy", if resp.hovered() { pal.heading } else { pal.text_muted }) };
            forma_ui::icons::paint(painter, icon_rect.shrink(side * 0.22), icon, color);
            if resp.on_hover_text(t!("code-copy-command")).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                ui.ctx().copy_text(c.text.clone());
                ui.data_mut(|d| d.insert_temp(copied_id, now));
            }
            if recently {
                ui.ctx().request_repaint_after_secs(0.4);
            }
        }
    }

    /// Лог: номера строк, длинные строки переносятся (продолжение — под началом текста).
    fn log_body(&mut self, ui: &mut Ui, cb: &CodeBlock, code: &str, code_size: f32, line_h: f32) {
        let pal = self.pal;
        let font = FontId::monospace(code_size);
        let digits = term::line_count(&cb.code).to_string().len();
        let gutter_w = ui.fonts_mut(|f| f.layout_no_wrap("0".repeat(digits), font.clone(), pal.code_gutter)).size().x;
        let gap = self.size * 1.1;
        let mut job = log_job(ui.ctx(), pal, code, code_size, line_h);
        job.wrap.max_width = (ui.available_width() - gutter_w - gap).max(self.size * 8.0);
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        ui.horizontal_top(|ui| {
            let (gutter, _) = ui.allocate_exact_size(vec2(gutter_w + gap, galley.size().y), Sense::hover());
            let resp = ui.add(Label::new(galley.clone()).selectable(true));
            let mut number = 0;
            for (i, row) in galley.rows.iter().enumerate() {
                if i > 0 && !galley.rows[i - 1].ends_with_newline {
                    continue;
                }
                number += 1;
                let pos = pos2(gutter.left() + gutter_w, resp.rect.top() + row.pos.y);
                if !ui.is_rect_visible(Rect::from_min_size(pos2(gutter.left(), pos.y), vec2(gutter_w, line_h))) {
                    continue;
                }
                let num = ui.painter().layout_job(LayoutJob::single_section(
                    number.to_string(),
                    TextFormat { font_id: font.clone(), color: pal.code_gutter, line_height: Some(line_h), ..Default::default() },
                ));
                ui.painter().galley(pos2(pos.x - num.size().x, pos.y), num, pal.code_gutter);
            }
        });
    }

    /// Диаграмма по центру блока; широкая — уменьшается до ширины, а если совсем широкая — прокручивается.
    fn paint_diagram(&self, ui: &mut Ui, scene: &diagram::Scene, id: egui::Id) {
        let pal = self.pal;
        let colors = diagram::Colors::from_palette(pal, pal.code_bg);
        let avail = ui.available_width();
        let mut scale = (self.size / 15.0).clamp(0.85, 1.3);
        if scene.size.x * scale > avail {
            scale = (avail / scene.size.x).max(0.55);
        }
        let (w, h) = (scene.size.x * scale, scene.size.y * scale);
        let draw = |ui: &mut Ui| {
            let width = avail.max(w);
            let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
            if ui.is_rect_visible(rect) {
                let origin = pos2(rect.left() + ((width - w) / 2.0).max(0.0), rect.top());
                scene.paint(ui.painter(), origin, scale, &colors);
            }
        };
        if w > avail + 0.5 {
            ScrollArea::horizontal().id_salt(id).show(ui, draw);
        } else {
            draw(ui);
        }
    }

    fn quote(&mut self, ui: &mut Ui, blocks: &[Block], author: Option<&[Inline]>) {
        let pal = self.pal;
        let s = self.size;
        let was = self.in_quote;
        self.in_quote = true;
        let resp = Frame::new()
            .fill(pal.accent.gamma_multiply(if pal.dark { 0.07 } else { 0.05 }))
            .corner_radius(CornerRadius { nw: 4, sw: 4, ne: 14, se: 14 })
            .inner_margin(Margin { left: (s * 2.9) as i8, right: 20, top: 16, bottom: 16 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.blocks(ui, blocks, s * 0.55);
                if let Some(author) = author {
                    ui.add_space(s * 0.7);
                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        let fmt = Fmt {
                            size: s * 0.88,
                            color: pal.text_muted,
                            family: fonts::semibold(),
                            heading: true,
                            line_height: 1.4,
                            strike: false,
                            italic: false,
                        };
                        let mut items = vec![Inline::Text("— ".to_owned(), Style::default())];
                        items.extend_from_slice(author);
                        self.in_quote = false;
                        self.text_run(ui, &items, &fmt);
                        self.in_quote = true;
                    });
                }
            });
        self.in_quote = was;
        let r = resp.response.rect;
        let painter = ui.painter();
        painter.rect_filled(Rect::from_min_max(r.left_top(), pos2(r.left() + 3.0, r.bottom())), CornerRadius::same(2), pal.quote_bar);
        // Большие кавычки слева от текста.
        painter.text(
            pos2(r.left() + s * 1.45, r.top() + s * 0.35),
            Align2::CENTER_TOP,
            "“",
            FontId::new(s * 3.0, fonts::math_italic()),
            pal.accent.gamma_multiply(0.55),
        );
    }

    fn callout(&mut self, ui: &mut Ui, kind: &str, title: &[Inline], folded: Option<bool>, body: &[Block]) {
        let pal = self.pal;
        let s = self.size;
        let style = theme::callout(kind);
        let id = self.next_id(ui, "callout");
        let foldable = folded.is_some();
        let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, folded != Some(true));
        let alpha = if pal.dark { 0.13 } else { 0.09 };
        Frame::new()
            .fill(style.color.gamma_multiply(alpha))
            .stroke(Stroke::new(1.0, style.color.gamma_multiply(0.25)))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin { left: 16, right: 16, top: 11, bottom: 12 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let title_resp = ui
                    .horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                        ui.label(RichText::new(style.icon).size(s * 1.0).color(style.color));
                        let fmt = Fmt {
                            size: s * 0.98,
                            color: style.color,
                            family: fonts::semibold(),
                            heading: true,
                            line_height: 1.4,
                            strike: false,
                            italic: false,
                        };
                        if title.is_empty() {
                            ui.label(RichText::new(style.title).size(s * 0.98).family(fonts::semibold()).color(style.color));
                        } else {
                            ui.scope(|ui| {
                                ui.set_max_width(ui.available_width() - s * 1.5);
                                let was = self.in_quote;
                                self.in_quote = false;
                                self.text_run(ui, title, &fmt);
                                self.in_quote = was;
                            });
                        }
                        if foldable {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (r, _) = ui.allocate_exact_size(Vec2::splat(s), Sense::hover());
                                let openness = state.openness(ui.ctx());
                                chevron(ui.painter(), r.center(), s * 0.28, openness, style.color);
                            });
                        }
                    })
                    .response;
                if foldable {
                    let r = ui.interact(title_resp.rect, id.with("toggle"), Sense::click());
                    if r.clicked() {
                        state.toggle(ui);
                    }
                    r.on_hover_cursor(CursorIcon::PointingHand);
                }
                if !body.is_empty() {
                    let was = self.in_quote;
                    self.in_quote = false;
                    state.show_body_unindented(ui, |ui| {
                        ui.add_space(s * 0.5);
                        self.blocks(ui, body, s * 0.6);
                    });
                    self.in_quote = was;
                }
            });
        state.store(ui.ctx());
    }

    fn list(&mut self, ui: &mut Ui, start: Option<u64>, items: &[ListItem]) {
        let s = self.size;
        let depth = self.list_depth;
        self.list_depth += 1;
        let fmt_size = self.body_fmt().size;
        let line_h = fmt_size * 1.62;
        let marker_w = if start.is_some() { s * 1.7 } else { s * 1.3 };
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                ui.add_space(s * 0.22);
            }
            ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
                let sense = if item.task.is_some() { Sense::click() } else { Sense::hover() };
                let (rect, resp) = ui.allocate_exact_size(vec2(marker_w, line_h), sense);
                let text_hover_id = resp.id.with("text-hover");
                let text_hovered: bool = ui.data(|d| d.get_temp(text_hover_id)).unwrap_or(false);
                let center = pos2(rect.right() - s * 0.65, rect.top() + line_h / 2.0);
                if let Some(task) = &item.task {
                    let r = Rect::from_center_size(pos2(rect.right() - s * 0.78, center.y), Vec2::splat(s * 0.98));
                    let t = ui.ctx().animate_bool_with_time(resp.id, task.checked, 0.14);
                    let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered() || text_hovered, 0.12);
                    let painter = ui.painter();
                    let border = mix(mix(self.pal.text_faint, self.pal.accent, hover), self.pal.accent, t);
                    painter.rect(r, CornerRadius::same(4), self.pal.accent.gamma_multiply(t), Stroke::new(1.5, border), egui::StrokeKind::Inside);
                    if t > 0.01 {
                        let check_color = if self.pal.dark { self.pal.bg } else { Color32::WHITE };
                        painter.line(
                            vec![
                                pos2(r.left() + r.width() * 0.24, r.center().y + r.height() * 0.02),
                                pos2(r.left() + r.width() * 0.43, r.bottom() - r.height() * 0.26),
                                pos2(r.right() - r.width() * 0.22, r.top() + r.height() * 0.28),
                            ],
                            Stroke::new(2.0, check_color.gamma_multiply(t)),
                        );
                    }
                    if resp.clicked() {
                        self.actions.push(Action::ToggleTask(task.range.clone()));
                    }
                    resp.on_hover_cursor(CursorIcon::PointingHand);
                } else if let Some(n0) = start {
                    let n = n0 + i as u64;
                    let painter = ui.painter();
                    let galley = painter.layout_no_wrap(format!("{n}."), FontId::new(s, FontFamily::Proportional), self.pal.text_muted);
                    let gp = pos2(rect.right() - s * 0.35 - galley.size().x, center.y - galley.size().y / 2.0);
                    painter.galley(gp, galley, self.pal.text_muted);
                } else {
                    let painter = ui.painter();
                    let c = self.pal.text_faint;
                    let r = s * 0.17;
                    match depth % 3 {
                        0 => {
                            painter.circle_filled(center, r, c);
                        }
                        1 => {
                            painter.circle_stroke(center, r * 0.95, Stroke::new(1.3, c));
                        }
                        _ => {
                            painter.rect_filled(Rect::from_center_size(center, Vec2::splat(r * 1.7)), CornerRadius::same(1), c);
                        }
                    }
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let (was_muted, was_strike) = (self.muted, self.strike);
                    if item.task.as_ref().is_some_and(|t| t.checked) {
                        self.muted = true;
                        self.strike = true;
                    }
                    let was_task = std::mem::replace(&mut self.task, item.task.as_ref().map(|t| t.range.clone()));
                    let was_hover = std::mem::take(&mut self.task_hover);
                    self.blocks(ui, &item.blocks, s * 0.35);
                    let hovered = std::mem::replace(&mut self.task_hover, was_hover);
                    self.task = was_task;
                    if item.task.is_some() && hovered != text_hovered {
                        ui.data_mut(|d| d.insert_temp(text_hover_id, hovered));
                        ui.ctx().request_repaint();
                    }
                    self.muted = was_muted;
                    self.strike = was_strike;
                });
            });
        }
        self.list_depth -= 1;
    }

    /// Таблица с сеткой, шапкой и подсветкой строки под курсором.
    fn table(&mut self, ui: &mut Ui, aligns: &[ColAlign], cell_aligns: &[Vec<ColAlign>], head: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) {
        let pal = self.pal;
        let s = self.size;
        let ncols = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
        if ncols == 0 {
            return;
        }
        let pad_x = s * 1.0;
        let pad_y = s * 0.7;
        let avail = ui.available_width();

        let mut head_fmt = self.body_fmt();
        head_fmt.family = fonts::semibold();
        head_fmt.color = pal.heading;
        head_fmt.heading = true;
        head_fmt.size = s * 0.9;
        head_fmt.line_height = 1.45;
        let mut body_fmt = self.body_fmt();
        body_fmt.size = s * 0.95;
        body_fmt.line_height = 1.5;

        // Естественная ширина колонок.
        let mut natural = vec![0.0f32; ncols];
        for (j, cell) in head.iter().enumerate() {
            natural[j] = natural[j].max(self.measure(ui, cell, &head_fmt));
        }
        for row in rows {
            for (j, cell) in row.iter().enumerate() {
                natural[j] = natural[j].max(self.measure(ui, cell, &body_fmt));
            }
        }
        let mut widths: Vec<f32> = natural.iter().map(|w| w + pad_x * 2.0 + 1.0).collect();
        let total: f32 = widths.iter().sum();
        if total < avail {
            // Растягиваем на всю ширину, отдавая место пропорционально содержимому.
            let extra = avail - total;
            let sum_nat: f32 = natural.iter().sum::<f32>().max(1.0);
            for (w, n) in widths.iter_mut().zip(&natural) {
                *w += extra * n / sum_nat;
            }
        } else {
            // Узкие колонки оставляем, широкие сжимаем (текст в них переносится).
            let fair = avail / ncols as f32;
            let small: f32 = widths.iter().filter(|w| **w <= fair).sum();
            let big: f32 = widths.iter().filter(|w| **w > fair).sum::<f32>().max(1.0);
            let room = (avail - small).max(ncols as f32 * 40.0);
            for w in widths.iter_mut() {
                if *w > fair {
                    *w = (*w / big * room).max(s * 4.0);
                }
            }
        }
        let table_w: f32 = widths.iter().sum();

        let id = self.next_id(ui, "table");
        let origin = ui.cursor().min;
        let painter = ui.painter().clone();
        let bg_idx = painter.add(Shape::Noop);
        let mut row_bounds: Vec<(f32, f32, usize)> = Vec::new();
        let mut row_shapes = Vec::new();
        let mut y = origin.y;

        let all_rows = std::iter::once((head, true)).chain(rows.iter().map(|r| (r.as_slice(), false)));
        for (ri, (cells, is_head)) in all_rows.enumerate() {
            let shape_idx = painter.add(Shape::Noop);
            row_shapes.push(shape_idx);
            let fmt = if is_head { &head_fmt } else { &body_fmt };
            // Первые строки ячеек ставим на общую базовую линию: иначе ячейка с одним кодом
            // или бейджем (у них шрифт мельче) оказывается выше соседнего текста.
            let baselines: Vec<Option<f32>> = (0..ncols)
                .map(|j| {
                    let cell = cells.get(j)?;
                    let inner = (widths[j] - pad_x * 2.0).max(10.0);
                    self.first_baseline(ui, cell, fmt, inner)
                })
                .collect();
            let row_base = baselines.iter().flatten().copied().fold(0.0f32, f32::max);
            let mut x = origin.x;
            let mut row_h: f32 = 0.0;
            for j in 0..ncols {
                let w = widths[j];
                let shift = baselines[j].map_or(0.0, |b| row_base - b);
                let cell_rect = Rect::from_min_size(pos2(x + pad_x, y + pad_y + shift), vec2((w - pad_x * 2.0).max(10.0), 10_000.0));
                // Выравнивание ячейки (`{центр}`) важнее выравнивания колонки.
                let own = cell_aligns.get(ri).and_then(|r| r.get(j)).copied().unwrap_or(ColAlign::None);
                let align = if own == ColAlign::None { aligns.get(j).copied().unwrap_or(ColAlign::None) } else { own };
                let layout = match align {
                    ColAlign::Right => Layout::top_down(Align::Max),
                    ColAlign::Center => Layout::top_down(Align::Center),
                    _ => Layout::top_down(Align::Min),
                };
                let mut child = ui.new_child(UiBuilder::new().id_salt((id, ri, j)).max_rect(cell_rect).layout(layout));
                child.spacing_mut().item_spacing = Vec2::ZERO;
                if let Some(cell) = cells.get(j)
                    && !cell.is_empty()
                {
                    self.inlines(&mut child, cell, fmt);
                }
                row_h = row_h.max(child.min_rect().height() + shift);
                x += w;
            }
            let h = row_h.max(fmt.size) + pad_y * 2.0;
            row_bounds.push((y, y + h, ri));
            y += h;
        }
        let table_rect = Rect::from_min_max(origin, pos2(origin.x + table_w, y));
        let radius = CornerRadius::same(12);
        painter.set(bg_idx, egui::epaint::RectShape::filled(table_rect, radius, pal.table_bg));
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let last = row_bounds.len().saturating_sub(1);
        for (k, &(y0, y1, ri)) in row_bounds.iter().enumerate() {
            let row_rect = Rect::from_min_max(pos2(table_rect.left(), y0), pos2(table_rect.right(), y1));
            let corner = CornerRadius {
                nw: if k == 0 { 12 } else { 0 },
                ne: if k == 0 { 12 } else { 0 },
                sw: if k == last { 12 } else { 0 },
                se: if k == last { 12 } else { 0 },
            };
            let hovered = ri > 0 && hover_pos.is_some_and(|p| row_rect.contains(p)) && ui.ui_contains_pointer();
            let t = ui.ctx().animate_bool_with_time(id.with(("row", ri)), hovered, 0.12);
            let fill = if ri == 0 {
                pal.table_head
            } else {
                Color32::TRANSPARENT.lerp_to_gamma(pal.bg_hover.gamma_multiply(0.6), t)
            };
            painter.set(row_shapes[k], egui::epaint::RectShape::filled(row_rect, corner, fill));
            if k > 0 {
                painter.hline(table_rect.x_range(), y0, Stroke::new(1.0, pal.table_line));
            }
        }
        let mut x = table_rect.left();
        for w in widths.iter().take(ncols - 1) {
            x += w;
            painter.vline(x, table_rect.y_range(), Stroke::new(1.0, pal.table_line));
        }
        painter.rect_stroke(table_rect, radius, Stroke::new(1.0, pal.table_line), egui::StrokeKind::Inside);
        ui.allocate_rect(table_rect, Sense::hover());
    }

    fn image(&mut self, ui: &mut Ui, src: &str, alt: &str, width: Option<f32>, wiki: bool) {
        let pal = self.pal;
        let s = self.size;
        if src.starts_with("http://") || src.starts_with("https://") {
            let text = if alt.is_empty() { src.to_owned() } else { alt.to_owned() };
            let resp = ui.add(Label::new(RichText::new(format!("🖼 {text}")).size(s * 0.9).color(pal.link)).sense(Sense::click()));
            if resp.on_hover_text(t!("preview-external-image")).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                self.actions.push(Action::OpenUrl(src.to_owned()));
            }
            return;
        }
        if wiki && !crate::markdown::is_image_path(src) {
            self.embed_note(ui, src);
            return;
        }
        match self.vault.resolve_attachment(src, self.note_rel) {
            Some(path) => {
                // На Windows загрузчику нужен вид file:///C:/…, иначе путь считается сетевым.
                let uri = format!("file:///{}", path.to_string_lossy().replace('\\', "/").trim_start_matches('/'));
                let avail = ui.available_width();
                let max_w = width.unwrap_or(avail).min(avail);
                let img = egui::Image::new(uri.clone())
                    .fit_to_original_size(1.0)
                    .max_width(max_w)
                    .corner_radius(CornerRadius::same(10))
                    .show_loading_spinner(true)
                    .sense(Sense::click());
                let resp = ui.add(img);
                let hint = if alt.is_empty() { t!("preview-open-image") } else { format!("{alt}\n{}", t!("preview-open-image")) };
                if resp.on_hover_text(hint).double_clicked() {
                    self.actions.push(Action::OpenUrl(uri));
                }
                if !alt.is_empty() && !wiki {
                    ui.add_space(6.0);
                    ui.label(RichText::new(alt).size(s * 0.8).italics().color(pal.text_faint));
                }
            }
            None => {
                ui.label(RichText::new(format!("⚠ {}", t!("preview-image-missing", src = src.to_owned()))).size(s * 0.85).color(pal.warning));
            }
        }
    }

    /// Встраивание заметки `![[Заметка]]`: показываем её содержимое в рамке.
    fn embed_note(&mut self, ui: &mut Ui, target: &str) {
        let pal = self.pal;
        let s = self.size;
        let Some(idx) = self.vault.resolve(target, Some(self.note_rel)) else {
            ui.label(RichText::new(format!("⚠ {}", t!("preview-embed-missing", target = target.to_owned()))).color(pal.warning));
            return;
        };
        let note = &self.vault.notes[idx];
        let resp = Frame::new()
            .fill(pal.card_bg)
            .stroke(Stroke::new(1.0, pal.border))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin { left: 20, right: 16, top: 12, bottom: 14 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let title = Label::new(RichText::new(format!("↪ {}", note.name)).size(s * 0.85).color(pal.link)).sense(Sense::click());
                if ui.add(title).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    self.actions.push(Action::OpenNote { target: target.to_owned(), new_tab: false });
                }
                ui.add_space(s * 0.5);
                if self.depth >= 2 || note.rel == self.note_rel {
                    ui.label(RichText::new("…").color(pal.text_faint));
                    return;
                }
                let doc = crate::markdown::parse(&note.content);
                let mut inner = Renderer::new(pal, self.vault, &note.rel, &doc, s * 0.95);
                inner.depth = self.depth + 1;
                ui.scope(|ui| inner.show(ui));
                self.actions.append(&mut inner.actions);
            });
        let r = resp.response.rect;
        ui.painter().rect_filled(
            Rect::from_min_max(r.left_top() + vec2(0.0, 10.0), pos2(r.left() + 3.0, r.bottom() - 10.0)),
            CornerRadius::same(2),
            pal.accent.gamma_multiply(0.7),
        );
    }

    fn html(&mut self, ui: &mut Ui, html: &str) {
        let trimmed = html.trim();
        if trimmed.starts_with("<!--") {
            return;
        }
        // <img src="…"> показываем картинкой.
        // ASCII-регистр не меняет длину в байтах: позиция годится для среза исходной строки.
        if let Some(pos) = trimmed.to_ascii_lowercase().find("<img")
            && let Some(src) = attr_value(&trimmed[pos..], "src")
        {
            self.image(ui, &src, "", attr_value(&trimmed[pos..], "width").and_then(|w| w.parse().ok()), false);
            return;
        }
        let text = strip_tags(trimmed);
        if text.trim().is_empty() {
            return;
        }
        let fmt = self.body_fmt();
        let items = vec![Inline::Text(text, Style::default())];
        self.text_run(ui, &items, &fmt);
    }

    fn display_math(&mut self, ui: &mut Ui, tex: &str) {
        let s = self.size;
        let mbox = math::layout(ui.ctx(), tex, s * 1.12, true);
        let avail = ui.available_width();
        let pad = s * 0.35;
        let h = mbox.height() + pad * 2.0;
        let color = self.text_color();
        let paint = |ui: &mut Ui, width: f32| {
            let (rect, resp) = ui.allocate_exact_size(vec2(width, h), Sense::click());
            let x = rect.left() + ((width - mbox.width) / 2.0).max(0.0);
            mbox.paint(ui.painter(), pos2(x, rect.top() + pad + mbox.ascent), color);
            resp
        };
        let resp = if mbox.width <= avail {
            paint(ui, avail)
        } else {
            let id = self.next_id(ui, "math");
            ScrollArea::horizontal().id_salt(id).show(ui, |ui| paint(ui, mbox.width)).inner
        };
        resp.on_hover_text(tex.trim());
    }

    fn footnote(&mut self, ui: &mut Ui, label: &str, number: usize, blocks: &[Block]) {
        let s = self.size;
        let saved = self.size;
        let resp = ui
            .with_layout(Layout::left_to_right(Align::TOP), |ui| {
                ui.label(RichText::new(format!("{number}.")).size(s * 0.85).color(self.pal.accent));
                ui.add_space(s * 0.4);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    self.size = s * 0.85;
                    self.blocks(ui, blocks, s * 0.3);
                    self.size = saved;
                });
            })
            .response;
        self.anchor(ui, format!("^{label}"), resp.rect);
    }
}

type DiagramResult = Arc<Result<(diagram::Kind, diagram::Scene), diagram::Error>>;

/// Разобранная диаграмма блока. Для каждого блока храним одну последнюю версию
/// и пересчитываем её, только когда меняется текст.
fn diagram_scene(ctx: &egui::Context, slot: egui::Id, cb: &CodeBlock) -> DiagramResult {
    let hash = egui::Id::new((&cb.lang, &cb.code));
    if let Some((h, r)) = ctx.data(|d| d.get_temp::<(egui::Id, DiagramResult)>(slot))
        && h == hash
    {
        return r;
    }
    let r: DiagramResult = Arc::new(diagram::render(&cb.lang, &cb.code, &diagram::EguiMeasure(ctx)));
    ctx.data_mut(|d| d.insert_temp(slot, (hash, r.clone())));
    r
}

/// Маленькая серая плашка в шапке блока: `bash`, «19 строк».
fn header_badge(ui: &mut Ui, pal: &Palette, text: &str, size: f32) {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::new(size, fonts::semibold()), pal.text_muted);
    let pad = vec2(size * 0.55, size * 0.16);
    let (rect, _) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(5), mix(pal.code_header, pal.code_border, 0.8));
    ui.painter().galley(rect.min + pad, galley, pal.text_muted);
}

/// Вертикальный градиент: сверху прозрачно, внизу — цвет `color`.
fn fade_down(painter: &egui::Painter, rect: Rect, color: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), Color32::TRANSPARENT);
    mesh.colored_vertex(rect.right_top(), Color32::TRANSPARENT);
    mesh.colored_vertex(rect.left_bottom(), color);
    mesh.colored_vertex(rect.right_bottom(), color);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// Раскладка сеанса консоли: текст без приглашений (на их месте отступ) и команды.
struct ConsoleView {
    job: LayoutJob,
    commands: Vec<term::Command>,
}

#[derive(Clone, Copy, Hash)]
struct TermKey<'a> {
    code: &'a str,
    pal: &'a Palette,
    size_bits: u32,
    line_height_bits: u32,
    indent_bits: u32,
}

impl TermKey<'_> {
    fn sizes(&self) -> (f32, f32, f32) {
        (f32::from_bits(self.size_bits), f32::from_bits(self.line_height_bits), f32::from_bits(self.indent_bits))
    }
}

#[derive(Default)]
struct ConsoleBuilder;

impl egui::cache::ComputerMut<TermKey<'_>, Arc<ConsoleView>> for ConsoleBuilder {
    fn compute(&mut self, key: TermKey<'_>) -> Arc<ConsoleView> {
        let (size, line_h, indent) = key.sizes();
        Arc::new(build_console(key.pal, key.code, size, line_h, indent))
    }
}

#[derive(Default)]
struct LogBuilder;

impl egui::cache::ComputerMut<TermKey<'_>, LayoutJob> for LogBuilder {
    fn compute(&mut self, key: TermKey<'_>) -> LayoutJob {
        let (size, line_h, _) = key.sizes();
        build_log(key.pal, key.code, size, line_h)
    }
}

/// Сеанс консоли с кэшем между кадрами (подсветка команд не бесплатна).
fn console_view(ctx: &egui::Context, pal: &Palette, code: &str, size: f32, line_h: f32, indent: f32) -> Arc<ConsoleView> {
    let key = TermKey { code, pal, size_bits: size.to_bits(), line_height_bits: line_h.to_bits(), indent_bits: indent.to_bits() };
    ctx.memory_mut(|m| m.caches.cache::<egui::cache::FrameCache<Arc<ConsoleView>, ConsoleBuilder>>().get(key).clone())
}

fn log_job(ctx: &egui::Context, pal: &Palette, code: &str, size: f32, line_h: f32) -> LayoutJob {
    let key = TermKey { code, pal, size_bits: size.to_bits(), line_height_bits: line_h.to_bits(), indent_bits: 0 };
    ctx.memory_mut(|m| m.caches.cache::<egui::cache::FrameCache<LayoutJob, LogBuilder>>().get(key).clone())
}

fn term_format(size: f32, line_h: f32, color: Color32, bold: bool) -> TextFormat {
    let family = if bold { fonts::mono_bold() } else { FontFamily::Monospace };
    TextFormat { font_id: FontId::new(size, family), color, line_height: Some(line_h), ..Default::default() }
}

/// Команды подсвечены как Bash, вывод чуть тусклее команд и раскрашен по-cargo-вски,
/// комментарии — бледные. Перед командой — отступ `indent` под приглашение.
fn build_console(pal: &Palette, code: &str, size: f32, line_h: f32, indent: f32) -> ConsoleView {
    let lines = term::console_lines(code);
    let commands = term::commands(&lines);
    let output = mix(pal.code_text, pal.code_bg, 0.18);
    let fmt = |color: Color32, bold: bool| term_format(size, line_h, color, bold);
    let mut job = LayoutJob::default();
    let mut level = None;
    let mut next_cmd = commands.iter().peekable();
    let mut i = 0;
    while i < lines.len() {
        if i > 0 {
            job.append("\n", 0.0, fmt(output, false));
        }
        let line = lines[i];
        match line.kind {
            LineKind::Command => {
                let Some(c) = next_cmd.next() else { break };
                level = None;
                let mut lead = indent;
                for (r, color, italics) in highlight::ranges(&c.text, "bash", pal.dark) {
                    job.append(&c.text[r], lead, TextFormat { italics, ..fmt(color, false) });
                    lead = 0.0;
                }
                if lead > 0.0 {
                    job.append("", lead, fmt(output, false));
                }
                i = c.lines.end;
                continue;
            }
            LineKind::Comment => job.append(line.text, 0.0, fmt(pal.text_faint, false)),
            LineKind::Continuation | LineKind::Output => {
                let mut at = 0;
                for (r, tone) in term::output_spans(line.text, &mut level) {
                    if r.start > at {
                        job.append(&line.text[at..r.start], 0.0, fmt(output, false));
                    }
                    let (color, bold) = tone_style(pal, tone);
                    job.append(&line.text[r.clone()], 0.0, fmt(color, bold));
                    at = r.end;
                }
                if at < line.text.len() {
                    job.append(&line.text[at..], 0.0, fmt(output, false));
                }
            }
        }
        i += 1;
    }
    ConsoleView { job, commands }
}

/// Цвета вывода Cargo и rustc: как в настоящем терминале, но из палитры темы.
fn tone_style(pal: &Palette, tone: term::Tone) -> (Color32, bool) {
    use term::Tone as T;
    match tone {
        T::Status => (pal.success, true),
        T::Error => (pal.danger, true),
        T::Warning => (pal.warning, true),
        T::Hint => (pal.term_hint, true),
        T::Message => (pal.heading, true),
        T::Gutter => (pal.term_info, true),
        T::Added => (pal.success, false),
    }
}

fn build_log(pal: &Palette, code: &str, size: f32, line_h: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut prev = None;
    for (i, line) in term::lines(code).enumerate() {
        if i > 0 {
            job.append("\n", 0.0, term_format(size, line_h, pal.code_text, false));
        }
        let parsed = term::log_line(line, prev);
        prev = parsed.level;
        for (r, tone) in parsed.spans {
            let (color, bold) = log_style(pal, tone);
            job.append(&line[r], 0.0, term_format(size, line_h, color, bold));
        }
    }
    job
}

/// Цвета лога: время и источник приглушены, уровень — своим цветом, а сообщения
/// предупреждений и ошибок подкрашены целиком.
fn log_style(pal: &Palette, tone: term::LogTone) -> (Color32, bool) {
    use term::{Level, LogTone as T};
    match tone {
        T::Plain => (pal.code_text, false),
        T::Time => (pal.text_faint, false),
        T::Group => (mix(pal.accent, pal.text_muted, 0.45), false),
        T::Level(level) => {
            let color = match level {
                Level::Trace => pal.text_faint,
                Level::Debug => mix(pal.accent, pal.text_faint, 0.35),
                Level::Info => pal.term_info,
                Level::Warn => pal.warning,
                Level::Error => pal.danger,
            };
            (color, true)
        }
        T::Tinted(Level::Error) => (mix(pal.danger, pal.code_text, 0.15), false),
        T::Tinted(_) => (mix(pal.warning, pal.code_text, 0.2), false),
    }
}

/// Кнопка-вкладка в шапке блока кода.
fn code_tab(ui: &mut Ui, pal: &Palette, label: &str, selected: bool, size: f32) -> egui::Response {
    let font = FontId::new(size, fonts::semibold());
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, pal.text);
    let pad = vec2(size * 0.85, size * 0.26);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, selected, 0.15);
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
    let painter = ui.painter();
    let fill = pal.bg_hover.gamma_multiply(hover * (1.0 - t) * 0.8).lerp_to_gamma(pal.accent.gamma_multiply(0.12), t);
    let stroke = mix(pal.code_border, pal.accent.gamma_multiply(0.85), t);
    painter.rect(rect, CornerRadius::same(8), fill, Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    let text_color = mix(pal.text_muted, pal.heading, (t + hover * 0.5).min(1.0));
    painter.galley(rect.min + pad, galley, text_color);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Тег-пилюля вне текста (свойства, панели).
pub fn tag_pill(ui: &mut Ui, pal: &Palette, text: &str, size: f32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(size), pal.tag_text);
    let pad = vec2(size * 0.6, size * 0.28);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let fill = pal.accent.gamma_multiply(0.16 + 0.12 * hover);
    ui.painter().rect_filled(rect, CornerRadius::same(u8::MAX), fill);
    ui.painter().galley(rect.min + pad, galley, pal.tag_text);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Стрелка-шеврон, повёрнутая на `openness` (0 — вправо, 1 — вниз).
pub fn chevron(painter: &egui::Painter, center: egui::Pos2, r: f32, openness: f32, color: Color32) {
    let angle = openness * std::f32::consts::FRAC_PI_2;
    let rot = |v: Vec2| vec2(v.x * angle.cos() - v.y * angle.sin(), v.x * angle.sin() + v.y * angle.cos());
    let pts = [vec2(-0.45, -0.9), vec2(0.55, 0.0), vec2(-0.45, 0.9)].map(|p| center + rot(p * r));
    painter.add(Shape::line(pts.to_vec(), Stroke::new(1.6, color)));
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

fn property_icon(key: &str) -> &'static str {
    match key.to_lowercase().as_str() {
        "tags" | "теги" | "тег" => "#",
        "date" | "дата" | "created" | "updated" | "создано" | "изменено" | "deadline" | "срок" => "📅",
        "author" | "автор" | "authors" => "👤",
        "status" | "статус" => "◉",
        "aliases" | "alias" | "синонимы" => "≈",
        "url" | "link" | "ссылка" | "source" | "источник" => "🔗",
        "курс" | "course" | "project" | "проект" => "📚",
        _ => "≡",
    }
}

/// Горизонтальные отрезки, занятые символами `chars`: (x0, x1, базовая линия) в координатах галереи.
fn span_segments(galley: &Galley, chars: &Range<usize>) -> Vec<(f32, f32, f32)> {
    let mut segs: Vec<(f32, f32, f32, f32)> = Vec::new();
    for i in chars.clone() {
        let mut start = CCursor::new(i);
        start.prefer_next_row = true;
        let a = galley.pos_from_cursor(start);
        // Правый край берём по ширине самого глифа: позиция следующего символа уже включает отступ.
        let Some((x0, x1, baseline)) = glyph_span(galley, a) else { continue };
        match segs.last_mut() {
            Some(seg) if (seg.2 - a.top()).abs() < 0.5 => seg.1 = seg.1.max(x1),
            _ => segs.push((x0, x1, a.top(), baseline)),
        }
    }
    segs.into_iter().map(|(x0, x1, _, base)| (x0, x1, base)).collect()
}

/// Глиф в позиции курсора: (левый край, правый край, базовая линия).
fn glyph_span(galley: &Galley, r: Rect) -> Option<(f32, f32, f32)> {
    let cy = r.center().y;
    let row = galley.rows.iter().find(|row| {
        let rr = row.rect();
        cy >= rr.top() - 0.5 && cy <= rr.bottom() + 0.5
    })?;
    let g = row.row.glyphs.iter().min_by(|a, b| {
        let da = (row.pos.x + a.pos.x - r.left()).abs();
        let db = (row.pos.x + b.pos.x - r.left()).abs();
        da.total_cmp(&db)
    })?;
    if g.chr == '\n' {
        return None;
    }
    let x0 = row.pos.x + g.pos.x;
    Some((x0, x0 + g.advance_width, row.pos.y + g.pos.y))
}

/// Смещение линии зачёркивания от базовой линии для основного шрифта абзаца
/// (так же, как его считает egui: середина логической высоты строки).
fn strike_offset(ui: &Ui, fmt: &Fmt) -> f32 {
    let mut job = LayoutJob::default();
    job.append(
        "x",
        0.0,
        TextFormat {
            font_id: FontId::new(fmt.size, fmt.family.clone()),
            line_height: Some(fmt.size * fmt.line_height),
            ..Default::default()
        },
    );
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    galley
        .rows
        .first()
        .and_then(|r| r.row.glyphs.first())
        .map_or(-fmt.size * 0.3, |g| g.logical_rect().center().y - g.pos.y)
}

/// Высота подъёма шрифта над базовой линией (берём из раскладки одного символа).
pub fn font_ascent(ui: &Ui, font: &FontId) -> f32 {
    let galley = ui.fonts_mut(|f| f.layout_no_wrap("M".to_owned(), font.clone(), Color32::WHITE));
    galley.rows.first().and_then(|r| r.row.glyphs.first()).map_or(font.size * 0.8, |g| g.font_face_ascent)
}

/// Короткий фрагмент заметки с оформлением (для обратных ссылок): разметка не видна,
/// жирный — жирным, ссылки — цветом.
pub fn snippet_job(pal: &Palette, line: &str, size: f32, wrap: f32) -> LayoutJob {
    let doc = crate::markdown::parse(line);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    fn first_inlines(blocks: &[Block]) -> Option<&[Inline]> {
        for b in blocks {
            match b {
                Block::Paragraph(i) | Block::Heading { inlines: i, .. } => return Some(i),
                Block::List { items, .. } => {
                    if let Some(i) = items.iter().find_map(|it| first_inlines(&it.blocks)) {
                        return Some(i);
                    }
                }
                Block::Quote { blocks, .. } | Block::Callout { body: blocks, .. } => {
                    if let Some(i) = first_inlines(blocks) {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    }
    let Some(items) = first_inlines(&doc.blocks) else {
        job.append(line, 0.0, TextFormat { font_id: FontId::proportional(size), color: pal.text_muted, ..Default::default() });
        return job;
    };
    for item in items {
        let (text, st) = match item {
            Inline::Text(t, st) if st.math => (math::to_plain(t), *st),
            Inline::Text(t, st) => (t.clone(), *st),
            Inline::SoftBreak | Inline::HardBreak => (" ".to_owned(), Style::default()),
            Inline::Image { alt, .. } => (alt.clone(), Style::default()),
        };
        let text = text.as_str();
        let link = st.link.and_then(|l| doc.links.get(l as usize));
        let family = if st.math {
            fonts::math_italic()
        } else if st.code {
            FontFamily::Monospace
        } else {
            match (st.bold, st.italic) {
                (true, true) => fonts::bold_italic(),
                (true, false) => fonts::semibold(),
                (false, true) => fonts::italic(),
                _ => FontFamily::Proportional,
            }
        };
        let mut color = pal.text_muted;
        if st.bold {
            color = pal.text;
        }
        if st.code {
            color = st.tint.map_or(pal.code_tint, |c| pal.tint_text(Color32::from_rgb(c[0], c[1], c[2])));
        }
        if link.is_some() {
            color = if link.is_some_and(|l| l.kind == LinkKind::Tag) { pal.tag_text } else { pal.link };
        }
        let fs = if st.code { size * 0.92 } else { size };
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: FontId::new(fs, family),
                color,
                italics: st.italic && !fonts::has_italic(),
                strikethrough: if st.strike { Stroke::new(1.0, color) } else { Stroke::NONE },
                background: if st.mark { pal.mark_bg } else { Color32::TRANSPARENT },
                ..Default::default()
            },
        );
    }
    job
}

fn has_visible_text(items: &[Inline]) -> bool {
    items.iter().any(|i| match i {
        Inline::Text(t, _) => !t.trim().is_empty(),
        _ => false,
    })
}

/// Какая ссылка под точкой `p` (относительно галереи текста).
fn link_at(galley: &Galley, p: Vec2, links: &[(Range<usize>, u32)]) -> Option<u32> {
    let c = galley.cursor_from_pos(p);
    let idx = c.index.0;
    for i in [idx, idx.saturating_sub(1)] {
        let r0 = galley.pos_from_cursor(CCursor::new(i));
        let r1 = galley.pos_from_cursor(CCursor::new(i + 1));
        let same_row = (r1.top() - r0.top()).abs() < 1.0;
        if same_row && p.x >= r0.left() - 1.0 && p.x <= r1.left() + 1.0 && p.y >= r0.top() && p.y <= r0.bottom() {
            return links.iter().find(|(r, _)| r.contains(&i)).map(|(_, l)| *l);
        }
    }
    None
}

/// Положение базовой линии в месте символа-заполнителя под формулу.
fn math_origin(galley: &Galley, char_index: usize) -> Option<Vec2> {
    let r = galley.pos_from_cursor(CCursor::new(char_index));
    let cy = r.center().y;
    for row in &galley.rows {
        let rr = row.rect();
        if cy >= rr.top() - 0.5 && cy <= rr.bottom() + 0.5 {
            let baseline = row
                .row
                .glyphs
                .iter()
                .filter(|g| g.chr == 'M')
                .min_by(|a, b| {
                    let da = (row.pos.x + a.pos.x - r.left()).abs();
                    let db = (row.pos.x + b.pos.x - r.left()).abs();
                    da.total_cmp(&db)
                })
                .map(|g| row.pos.y + g.pos.y)
                .unwrap_or(rr.top() + rr.height() * 0.75);
            return Some(vec2(r.left(), baseline));
        }
    }
    None
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    // Только ASCII: `to_lowercase` меняет длину ('İ' → 'i̇'), и срез `tag` попал бы внутрь символа.
    let lower = tag.to_ascii_lowercase();
    let pos = lower.find(&format!("{name}="))?;
    let rest = &tag[pos + name.len() + 1..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)?;
        Some(rest[1..1 + end].to_owned())
    } else {
        Some(rest.split(|c: char| c.is_whitespace() || c == '>').next()?.to_owned())
    }
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attr_value_after_non_ascii() {
        // 'İ' в нижнем регистре длиннее на байт: раньше срез попадал внутрь «к» и падал.
        assert_eq!(attr_value("<img alt=\"İİ\" src=\"кот.png\" width=200>", "src").as_deref(), Some("кот.png"));
        assert_eq!(attr_value("<img İ WIDTH=120>", "width").as_deref(), Some("120"));
        assert_eq!(attr_value("<IMG alt='İ' SRC='a.png'>", "src").as_deref(), Some("a.png"));
    }
}
