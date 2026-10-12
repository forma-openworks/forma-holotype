//! Цвета Forma: Holotype поверх темы платформы Forma.
//!
//! Тема (встроенная «Holotype Dark/Light», из плагина или своя) задаёт ключи платформы
//! («bg», «accent», «menubar.bg»…) и ключи заметок «note.*» — их описывает [`NoteColors`].
//! Для отрисовки заметок всё сводится в привычную [`Palette`]: просмотр, редактор, граф
//! и диаграммы не знают, откуда взялся цвет.

use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle};
use forma_i18n::t;
use serde::{Deserialize, Serialize};

forma_ui::theme_tokens! {
    /// Цвета заметок Holotype: ссылки, код, выделение, теги, таблицы, клавиши.
    pub struct NoteColors {
        /// Ссылка на существующую заметку
        link = "note.link" => "$link",
        /// Ссылка на заметку, которой ещё нет
        link_unresolved = "note.link.unresolved" => "alpha($link, 55%)",
        /// Фон блока кода
        code_bg = "note.code.bg" => "$bg.panel",
        /// Текст блока кода без подсветки
        code_text = "note.code.text" => "$editor.text",
        /// Шапка блока кода (язык, заголовок, вкладки)
        code_header = "note.code.header" => "$bg.header",
        /// Рамка блока кода
        code_border = "note.code.border" => "$border",
        /// Номера строк в блоке кода
        code_gutter = "note.code.gutter" => "$text.faint",
        /// Код в строке без своего цвета — оттенок акцента
        code_tint = "note.code.tint" => "$accent",
        /// Фон кода в строке
        inline_code_bg = "note.inline-code.bg" => "$bg.hover",
        /// Текст кода в строке (в редакторе)
        inline_code_text = "note.inline-code.text" => "mix($accent, $text, 30%)",
        /// Выделение маркером ==текст==
        mark_bg = "note.mark.bg" => "alpha($warning, 30%)",
        /// Фон тега
        tag_bg = "note.tag.bg" => "alpha($accent, 16%)",
        /// Текст тега
        tag_text = "note.tag.text" => "$accent",
        /// Полоса цитаты
        quote_bar = "note.quote.bar" => "alpha($accent, 66%)",
        /// Фон клавиши {{Ctrl+S}}
        kbd_bg = "note.kbd.bg" => "$bg.elevated",
        /// Рамка клавиши
        kbd_border = "note.kbd.border" => "$border.strong",
        /// Тень под клавишей
        kbd_shadow = "note.kbd.shadow" => "$bg.deep",
        /// Фон бейджа `текст`{badge}
        badge_bg = "note.badge.bg" => "$bg.hover",
        /// Текст бейджа
        badge_text = "note.badge.text" => "$text",
        /// Фон таблицы
        table_bg = "note.table.bg" => "$bg",
        /// Шапка таблицы
        table_head = "note.table.head" => "$bg.panel",
        /// Линии таблицы
        table_line = "note.table.line" => "$border",
        /// Карточки: свойства заметки, обратные ссылки
        card_bg = "note.card.bg" => "mix($bg, $bg.elevated, 40%)",
    }
}

/// Встроенные темы: свои темы Holotype (первая — запасная) и темы Forma Mycelium, чтобы
/// программы платформы можно было одеть одинаково. Цвета заметок у тем Mycelium выводятся
/// из цветов платформы.
pub const BUILTIN_THEMES: &[&str] = &[
    include_str!("../themes/holotype-dark.toml"),
    include_str!("../themes/holotype-light.toml"),
    include_str!("../../../../ModdingIDE/crates/mycelium/themes/mycelium-dark.toml"),
    include_str!("../../../../ModdingIDE/crates/mycelium/themes/mycelium-light.toml"),
];
/// Тема по умолчанию.
pub const DEFAULT_THEME: &str = "holotype-dark";

/// Акцентный цвет заметок поверх темы (настройка «Заметки»).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Accent {
    Violet,
    Blue,
    Teal,
    Green,
    Amber,
    Rose,
}

impl Accent {
    pub const ALL: [Accent; 6] = [Accent::Violet, Accent::Blue, Accent::Teal, Accent::Green, Accent::Amber, Accent::Rose];

    pub fn label(self) -> String {
        match self {
            Accent::Violet => t!("accent-violet"),
            Accent::Blue => t!("accent-blue"),
            Accent::Teal => t!("accent-teal"),
            Accent::Green => t!("accent-green"),
            Accent::Amber => t!("accent-amber"),
            Accent::Rose => t!("accent-rose"),
        }
    }

    /// Цвет акцента: в светлой теме темнее, чтобы читался на белом.
    pub fn color(self, dark: bool) -> Color32 {
        let (d, l) = match self {
            Accent::Violet => ((150, 132, 255), (108, 84, 230)),
            Accent::Blue => ((96, 165, 250), (37, 99, 235)),
            Accent::Teal => ((45, 212, 191), (13, 148, 136)),
            Accent::Green => ((74, 222, 128), (22, 163, 74)),
            Accent::Amber => ((251, 191, 36), (202, 138, 4)),
            Accent::Rose => ((251, 113, 133), (225, 29, 72)),
        };
        let (r, g, b) = if dark { d } else { l };
        Color32::from_rgb(r, g, b)
    }

    /// Значение ключа `accent` для темы.
    pub fn hex(self, dark: bool) -> String {
        let c = self.color(dark);
        format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
    }
}

/// Все цвета заметок: ключи платформы и ключи «note.*» текущей темы.
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub bg_panel: Color32,
    pub bg_elevated: Color32,
    pub bg_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub heading: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub link: Color32,
    pub link_unresolved: Color32,
    pub code_bg: Color32,
    pub code_text: Color32,
    pub inline_code_bg: Color32,
    pub inline_code_text: Color32,
    pub mark_bg: Color32,
    pub tag_bg: Color32,
    pub tag_text: Color32,
    pub quote_bar: Color32,
    pub selection: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    /// Шапка и рамка блоков кода, номера строк.
    pub code_header: Color32,
    pub code_border: Color32,
    pub code_gutter: Color32,
    /// Цвет кода в строке по умолчанию — оттенок акцентного цвета.
    pub code_tint: Color32,
    /// Клавиши `<kbd>`.
    pub kbd_bg: Color32,
    pub kbd_border: Color32,
    pub kbd_shadow: Color32,
    /// Бейджи `` `текст`{badge} ``.
    pub badge_bg: Color32,
    pub badge_text: Color32,
    /// Таблицы.
    pub table_bg: Color32,
    pub table_head: Color32,
    pub table_line: Color32,
    /// Карточки (свойства, обратные ссылки).
    pub card_bg: Color32,
}

impl Palette {
    /// Цвета заметок из темы платформы.
    pub fn from_theme(theme: &forma_ui::theme::Theme) -> Self {
        let p = &theme.pal;
        let n = theme.group(NoteColors::TOKENS, NoteColors::resolve);
        Self {
            dark: p.dark,
            bg: p.bg,
            bg_panel: p.bg_panel,
            bg_elevated: p.bg_elevated,
            bg_hover: p.bg_hover,
            border: p.border,
            text: p.text,
            text_muted: p.text_muted,
            text_faint: p.text_faint,
            heading: p.heading,
            accent: p.accent,
            accent_soft: p.accent_soft,
            link: n.link,
            link_unresolved: n.link_unresolved,
            code_bg: n.code_bg,
            code_text: n.code_text,
            inline_code_bg: n.inline_code_bg,
            inline_code_text: n.inline_code_text,
            mark_bg: n.mark_bg,
            tag_bg: n.tag_bg,
            tag_text: n.tag_text,
            quote_bar: n.quote_bar,
            selection: p.selection,
            success: p.success,
            warning: p.warning,
            danger: p.danger,
            code_header: n.code_header,
            code_border: n.code_border,
            code_gutter: n.code_gutter,
            code_tint: n.code_tint,
            kbd_bg: n.kbd_bg,
            kbd_border: n.kbd_border,
            kbd_shadow: n.kbd_shadow,
            badge_bg: n.badge_bg,
            badge_text: n.badge_text,
            table_bg: n.table_bg,
            table_head: n.table_head,
            table_line: n.table_line,
            card_bg: n.card_bg,
        }
    }

    /// Палитра встроенной темы по умолчанию (для тестов).
    #[cfg(test)]
    pub fn default_dark() -> Self {
        let themes = forma_ui::theme::Themes::new(BUILTIN_THEMES, None);
        Self::from_theme(&themes.resolve(DEFAULT_THEME, DEFAULT_THEME))
    }

    /// Цвет текста для пользовательского оттенка: в светлой теме яркие цвета затемняем,
    /// иначе они плохо читаются на белом.
    pub fn tint_text(&self, c: Color32) -> Color32 {
        if self.dark { c } else { mix(c, Color32::BLACK, 0.38) }
    }
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Плавное появление: 0 → 1 за `time` секунд после вызова `restart_fade`.
pub fn fade(ctx: &egui::Context, id: egui::Id, time: f32) -> f32 {
    ctx.animate_bool_with_time(id, true, time)
}

/// Сбрасывает анимацию появления, чтобы `fade` снова началась с нуля.
pub fn restart_fade(ctx: &egui::Context, id: egui::Id) {
    ctx.animate_bool_with_time(id, false, 0.0);
}

pub use forma_ui::theme::context_menu;

/// Пункт меню с галочкой: на всю ширину, с подсветкой при наведении и теми же полями, что у кнопок.
pub fn menu_checkbox(ui: &mut egui::Ui, pal: &Palette, value: &mut bool, text: &str) -> egui::Response {
    let font = FontId::proportional(ui.style().text_styles[&TextStyle::Button].size);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, pal.text);
    let pad = ui.spacing().button_padding;
    let box_size = 15.0;
    let h = galley.size().y.max(box_size) + pad.y * 2.0;
    let w = pad.x * 2.0 + box_size + 9.0 + galley.size().x;
    // Как у кнопок egui: просим ширину по содержимому, а растягивает пункт сама раскладка меню.
    // Если просить всю доступную ширину, меню в первом кадре раздувается на пол-экрана.
    let (rect, mut resp) = ui.allocate_at_least(egui::vec2(w, h), egui::Sense::click());
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *value, 0.14);
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.12);
    let painter = ui.painter();
    if hover > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(6), pal.bg_hover.gamma_multiply(hover));
    }
    let b = egui::Rect::from_center_size(egui::pos2(rect.left() + pad.x + box_size / 2.0, rect.center().y), egui::Vec2::splat(box_size));
    paint_check(painter, pal, b, t, hover);
    let text_pos = egui::pos2(b.right() + 9.0, rect.center().y - galley.size().y / 2.0);
    painter.galley(text_pos, galley, pal.text);
    resp
}

/// Квадратная галочка с плавной анимацией (как у задач в заметках).
pub fn checkbox(ui: &mut egui::Ui, pal: &Palette, value: &mut bool, text: &str) -> egui::Response {
    let font = FontId::proportional(ui.style().text_styles[&TextStyle::Body].size);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, pal.text);
    let box_size = 16.0;
    let size = egui::vec2(box_size + 8.0 + galley.size().x, galley.size().y.max(22.0));
    let (rect, mut resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *value, 0.14);
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.12);
    let painter = ui.painter();
    let b = egui::Rect::from_center_size(egui::pos2(rect.left() + box_size / 2.0, rect.center().y), egui::Vec2::splat(box_size));
    paint_check(painter, pal, b, t, hover);
    let text_pos = egui::pos2(rect.left() + box_size + 8.0, rect.center().y - galley.size().y / 2.0);
    painter.galley(text_pos, galley, pal.text);
    resp
}

/// Рамка галочки и сама галочка, которая «прорисовывается» вместе с анимацией `t`.
fn paint_check(painter: &egui::Painter, pal: &Palette, b: egui::Rect, t: f32, hover: f32) {
    let border = mix(mix(pal.text_faint, pal.accent, hover), pal.accent, t);
    painter.rect(
        b,
        CornerRadius::same(4),
        pal.accent.gamma_multiply(t),
        Stroke::new(1.5 * (1.0 - t) + 0.01, border),
        egui::StrokeKind::Inside,
    );
    if t > 0.01 {
        let check = if pal.dark { pal.bg } else { Color32::WHITE };
        let p = |x: f32, y: f32| egui::pos2(b.left() + b.width() * x, b.top() + b.height() * y);
        // Галочка «прорисовывается» вместе с анимацией.
        let a = p(0.24, 0.52);
        let m = p(0.43, 0.72);
        let e = p(0.78, 0.30);
        let first = (t * 2.0).min(1.0);
        let second = (t * 2.0 - 1.0).clamp(0.0, 1.0);
        painter.line_segment([a, a + (m - a) * first], Stroke::new(2.0, check));
        if second > 0.0 {
            painter.line_segment([m, m + (e - m) * second], Stroke::new(2.0, check));
        }
    }
}

pub struct CalloutStyle {
    pub color: Color32,
    pub icon: &'static str,
    pub title: String,
}

/// Типы плашек (callouts), совместимые с Obsidian: `> [!tip] Заголовок`.
pub fn callout(kind: &str) -> CalloutStyle {
    let (rgb, icon, key) = match kind {
        "abstract" | "summary" | "tldr" => ((0x00, 0xb0, 0xff), "📋", "abstract"),
        "info" => ((0x00, 0x9c, 0xe8), "ℹ", "info"),
        "todo" => ((0x44, 0x8a, 0xff), "☑", "todo"),
        "tip" | "hint" | "important" => ((0x00, 0xbf, 0xa5), "💡", "tip"),
        "success" | "check" | "done" => ((0x22, 0xc5, 0x5e), "✔", "success"),
        "question" | "help" | "faq" => ((0xec, 0xa0, 0x1c), "❓", "question"),
        "warning" | "caution" | "attention" => ((0xf9, 0x73, 0x16), "⚠", "warning"),
        "failure" | "fail" | "missing" => ((0xef, 0x44, 0x44), "✖", "failure"),
        "danger" | "error" => ((0xe1, 0x1d, 0x48), "⚡", "danger"),
        "bug" => ((0xe1, 0x1d, 0x74), "🐞", "bug"),
        "example" => ((0x8b, 0x5c, 0xf6), "📑", "example"),
        "quote" | "cite" => ((0x9c, 0xa3, 0xaf), "❝", "quote"),
        _ => ((0x44, 0x8a, 0xff), "✎", "note"),
    };
    CalloutStyle { color: Color32::from_rgb(rgb.0, rgb.1, rgb.2), icon, title: forma_i18n::tr(&format!("callout-{key}")) }
}

/// Основные типы плашек — для подсказок и сокращений (синонимы вроде `hint` тоже работают).
pub const CALLOUT_KINDS: &[&str] =
    &["note", "info", "tip", "success", "question", "warning", "failure", "danger", "bug", "example", "quote", "abstract", "todo"];

/// Известен ли тип плашки (с учётом синонимов).
pub fn is_callout_kind(kind: &str) -> bool {
    const KNOWN: &[&str] = &[
        "note", "abstract", "summary", "tldr", "info", "todo", "tip", "hint", "important", "success", "check", "done", "question", "help", "faq",
        "warning", "caution", "attention", "failure", "fail", "missing", "danger", "error", "bug", "example", "quote", "cite",
    ];
    KNOWN.contains(&kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_themes_resolve_cleanly() {
        let themes = forma_ui::theme::Themes::new(BUILTIN_THEMES, None);
        let ids: Vec<&str> = themes.list().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["holotype-dark", "holotype-light", "mycelium-dark", "mycelium-light"]);
        for id in ids {
            let theme = themes.resolve(id, DEFAULT_THEME);
            assert!(theme.errors.is_empty(), "{id}: {:?}", theme.errors);
            let _ = theme.group(NoteColors::TOKENS, NoteColors::resolve);
            for (class, _) in forma_shell::highlight::CLASS_SELECTORS {
                assert!(theme.syntax.contains_key(*class), "{id}: нет подсветки «{class}»");
            }
        }
    }

    #[test]
    fn dark_theme_keeps_the_holotype_look() {
        let pal = Palette::default_dark();
        assert!(pal.dark);
        assert_eq!(pal.bg, Color32::from_rgb(28, 30, 36));
        assert_eq!(pal.bg_panel, Color32::from_rgb(22, 24, 29));
        assert_eq!(pal.accent, Color32::from_rgb(150, 132, 255));
        assert_eq!(pal.code_bg, Color32::from_rgb(20, 22, 27));
    }

    #[test]
    fn other_themes_get_note_colors_from_defaults() {
        // Тема без ключей заметок (например, из плагина) всё равно даёт все цвета.
        let file = forma_ui::theme::ThemeFile::parse("[theme]\nid = \"bare\"\ndark = false\n").unwrap();
        let pal = Palette::from_theme(&forma_ui::theme::Theme::resolve(file));
        assert!(!pal.dark);
        assert_ne!(pal.link, pal.bg);
    }

    #[test]
    fn callout_kinds_include_synonyms() {
        assert!(is_callout_kind("tip") && is_callout_kind("hint") && is_callout_kind("note"));
        assert!(!is_callout_kind("nonsense"));
    }
}
