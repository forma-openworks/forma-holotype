//! Измерение текста и разбор подписей в диаграммах.

use eframe::egui::{self, Color32, FontFamily, FontId};

use super::scene::Font;

/// Базовый кегль текста в диаграмме (в единицах сцены).
pub const FONT: f32 = 14.0;
/// Высота строки подписи.
pub const LINE: f32 = FONT * 1.32;

/// Ширина строки текста заданным шрифтом.
pub trait Measure {
    fn width(&self, text: &str, font: Font) -> f32;
}

/// Точное измерение шрифтами egui (в приложении).
pub struct EguiMeasure<'a>(pub &'a egui::Context);

impl Measure for EguiMeasure<'_> {
    fn width(&self, text: &str, font: Font) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let id = FontId::new(font.size, family(font));
        self.0.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), id, Color32::WHITE).size().x)
    }
}

/// Приблизительное измерение (для экспорта в SVG, где шрифт выбирает браузер).
pub struct Approx;

impl Measure for Approx {
    fn width(&self, text: &str, font: Font) -> f32 {
        let em: f32 = text
            .chars()
            .map(|c| {
                if font.mono {
                    return 0.61;
                }
                match c {
                    ' ' => 0.28,
                    '.' | ',' | ':' | ';' | '\'' | '!' | '|' | 'i' | 'l' | 'j' | 'I' => 0.27,
                    'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '-' => 0.36,
                    'm' | 'w' | 'M' | 'W' | 'ш' | 'щ' | 'ж' | 'ю' | 'м' | 'Ш' | 'Щ' | 'Ж' | 'Ю' | 'М' => 0.84,
                    '0'..='9' => 0.58,
                    'a'..='z' => 0.53,
                    'A'..='Z' => 0.66,
                    'а'..='я' | 'ё' => 0.56,
                    'А'..='Я' | 'Ё' => 0.68,
                    c if (c as u32) >= 0x2E80 => 1.0,
                    _ => 0.6,
                }
            })
            .sum();
        em * font.size * if font.bold { 1.06 } else { 1.0 }
    }
}

/// Семейство шрифта egui для текста диаграммы.
pub fn family(font: Font) -> FontFamily {
    if font.mono {
        FontFamily::Monospace
    } else if font.bold && font.italic {
        crate::fonts::bold_italic()
    } else if font.bold {
        crate::fonts::semibold()
    } else if font.italic {
        crate::fonts::italic()
    } else {
        FontFamily::Proportional
    }
}

/// Подпись узла или стрелки: снимает кавычки, переводит `<br>` и `\n` в переносы строк,
/// раскрывает HTML-сущности mermaid (`#quot;`, `&lt;` …).
pub fn label_lines(raw: &str) -> Vec<String> {
    let mut s = raw.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s = &s[1..s.len() - 1];
    }
    // «Markdown-строки» mermaid: "`текст`".
    if s.len() >= 2 && s.starts_with('`') && s.ends_with('`') {
        s = &s[1..s.len() - 1];
    }
    let mut text = s.to_owned();
    for br in ["<br/>", "<br />", "<BR/>", "<BR>", "<br>", "\\n"] {
        text = text.replace(br, "\n");
    }
    let text = decode_entities(&text);
    let text = text.replace("**", "").replace("__", "");
    let lines: Vec<String> = text.lines().map(|l| l.trim().to_owned()).collect();
    if lines.is_empty() { vec![String::new()] } else { lines }
}

fn decode_entities(s: &str) -> String {
    s.replace("#quot;", "\"")
        .replace("#35;", "#")
        .replace("#59;", ";")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// Цвет из CSS-записи: `#abc`, `#aabbcc`, `rgb(1,2,3)` или имя.
pub fn parse_color(s: &str) -> Option<Color32> {
    let s = s.trim().trim_matches('"').trim().to_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        let v = u32::from_str_radix(hex, 16).ok()?;
        return match hex.len() {
            3 => Some(Color32::from_rgb(((v >> 8) & 0xF) as u8 * 17, ((v >> 4) & 0xF) as u8 * 17, (v & 0xF) as u8 * 17)),
            6 => Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)),
            8 => Some(Color32::from_rgba_unmultiplied((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8)),
            _ => None,
        };
    }
    if let Some(inner) = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb(")) {
        let nums: Vec<f32> = inner.trim_end_matches(')').split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if nums.len() >= 3 {
            let a = nums.get(3).map_or(255, |a| if *a <= 1.0 { (a * 255.0) as u8 } else { *a as u8 });
            return Some(Color32::from_rgba_unmultiplied(nums[0] as u8, nums[1] as u8, nums[2] as u8, a));
        }
        return None;
    }
    let (r, g, b) = match s.as_str() {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (239, 68, 68),
        "green" => (34, 197, 94),
        "blue" => (59, 130, 246),
        "yellow" => (250, 204, 21),
        "orange" => (249, 115, 22),
        "purple" | "violet" => (139, 92, 246),
        "pink" => (236, 72, 153),
        "cyan" | "aqua" => (6, 182, 212),
        "teal" => (20, 184, 166),
        "gray" | "grey" => (156, 163, 175),
        "lightgray" | "lightgrey" => (209, 213, 219),
        "darkgray" | "darkgrey" => (75, 85, 99),
        "brown" => (146, 64, 14),
        "navy" => (30, 58, 138),
        "gold" => (234, 179, 8),
        "lightblue" => (147, 197, 253),
        "lightgreen" => (134, 239, 172),
        "salmon" => (250, 128, 114),
        "magenta" | "fuchsia" => (217, 70, 239),
        "lime" => (132, 204, 22),
        "indigo" => (99, 102, 241),
        _ => return None,
    };
    Some(Color32::from_rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_colors() {
        assert_eq!(label_lines("\"Привет<br>мир\""), vec!["Привет", "мир"]);
        assert_eq!(label_lines("a #quot;b#quot;"), vec!["a \"b\""]);
        assert_eq!(parse_color("#f80"), Some(Color32::from_rgb(255, 136, 0)));
        assert_eq!(parse_color("rgb(1, 2, 3)"), Some(Color32::from_rgb(1, 2, 3)));
        assert!(Approx.width("Hello", Font::new(14.0)) > 20.0);
    }
}
