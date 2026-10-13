//! Подсветка кода: около 170 языков из набора bat (через two-face) и собственные темы
//! в духе современных редакторов. Используется и в просмотре, и при экспорте в HTML.

use std::ops::Range;
use std::str::FromStr;
use std::sync::OnceLock;

use eframe::egui::{
    self, Color32, FontId,
    text::{LayoutJob, TextFormat},
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings};
use syntect::html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use forma_i18n::t;

/// Префикс CSS-классов подсветки в экспортированном HTML.
pub const CSS_CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "hl-" };

pub fn syntaxes() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(two_face::syntax::extra_newlines)
}

/// Загружает синтаксисы и темы в фоне, чтобы первый блок кода открылся без задержки.
pub fn warm_up() {
    std::thread::spawn(|| {
        syntaxes();
        theme(true);
        theme(false);
    });
}

/// Ищет синтаксис по обозначению языка из Markdown, учитывая привычные сокращения.
pub fn find(lang: &str) -> Option<&'static SyntaxReference> {
    let lower = lang.trim().to_lowercase();
    let token = match lower.as_str() {
        "" | "text" | "txt" | "plain" | "plaintext" | "output" | "log" => return None,
        // Сеансы консоли и логи раскрашивает `term`.
        "console" | "terminal" | "shell-session" | "sh-session" => return None,
        // Шейдерные языки: HLSL и Cg очень похожи на GLSL.
        "hlsl" | "shader" | "frag" | "vert" | "comp" | "geom" | "tesc" | "tese" | "fx" | "fxh" | "cg" | "shaderlab" => "glsl",
        "metal" | "cuda" | "cu" | "hpp" | "cc" | "cxx" | "h++" | "c++" => "cpp",
        "h" => "c",
        "c#" | "csharp" => "cs",
        "f#" | "fsharp" => "fs",
        "js" | "mjs" | "cjs" | "node" | "javascript" => "js",
        "ts" | "typescript" => "ts",
        // Отдельной подсветки JSX в наборе нет, а TypeScriptReact разбирает его без ошибок.
        "jsx" | "react" | "javascriptreact" | "typescriptreact" => "tsx",
        "objc" | "objective-c" | "objectivec" => "m",
        "objc++" | "objective-c++" => "mm",
        "svg" | "xaml" | "plist" => "xml",
        "env" | "dotenv" => ".env",
        "ex" | "exs" | "elixir" => "ex",
        "gql" => "graphql",
        "conf" | "cfg" => "ini",
        "asm" | "nasm" | "assembly" => "asm",
        "tf" | "terraform" => "tf",
        "gd" | "gdscript" | "godot" => "gd",
        "py" | "python3" | "py3" => "py",
        "sh" | "shell" | "zsh" => "bash",
        // Своего синтаксиса PowerShell в наборе нет (с fancy-regex его не собрать):
        // у Bash те же `$переменные`, `#комментарии` и строки — ближе, чем у Batch.
        "ps" | "ps1" | "psm1" | "pwsh" | "powershell" => "bash",
        "bat" | "cmd" => "bat",
        "yml" => "yaml",
        "md" => "markdown",
        "rs" => "rs",
        "kt" | "kts" => "kt",
        "golang" => "go",
        "docker" => "dockerfile",
        "make" => "makefile",
        "jsonc" | "json5" => "json",
        "vue" => "vue",
        other => other,
    };
    let ss = syntaxes();
    ss.find_syntax_by_token(token)
        .or_else(|| ss.find_syntax_by_name(token))
        .or_else(|| ss.syntaxes().iter().find(|s| s.name.eq_ignore_ascii_case(token)))
}

/// Человеческое имя языка для заголовка блока кода.
pub fn display_name(lang: &str) -> String {
    // Языки, подсвеченные «чужим» синтаксисом, называем своими именами.
    match lang.trim().to_lowercase().as_str() {
        "jsx" | "react" | "javascriptreact" => return "JSX".to_owned(),
        "hlsl" => return "HLSL".to_owned(),
        "metal" => return "Metal".to_owned(),
        "cuda" | "cu" => return "CUDA".to_owned(),
        "ps" | "ps1" | "psm1" | "pwsh" | "powershell" => return "PowerShell".to_owned(),
        "console" | "terminal" | "shell-session" | "sh-session" => return t!("lang-console"),
        "log" => return t!("lang-log"),
        _ => {}
    }
    match find(lang) {
        Some(s) => s.name.clone(),
        None if lang.trim().is_empty() => t!("lang-text"),
        None => lang.to_owned(),
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 0xFF }
}

/// Цвета темы: (селекторы, цвет, начертание).
fn rules(dark: bool) -> Vec<(&'static str, Color, Option<FontStyle>)> {
    let p = |d: (u8, u8, u8), l: (u8, u8, u8)| if dark { rgb(d.0, d.1, d.2) } else { rgb(l.0, l.1, l.2) };
    let comment = p((122, 134, 182), (128, 136, 160));
    let keyword = p((244, 114, 182), (196, 38, 120));
    let string = p((158, 206, 106), (34, 134, 58));
    let number = p((255, 158, 100), (194, 94, 0));
    let function = p((125, 207, 255), (0, 112, 182));
    let ty = p((224, 175, 104), (156, 104, 0));
    let param = p((255, 199, 140), (170, 90, 20));
    let operator = p((137, 221, 255), (0, 125, 160));
    let tag = p((247, 118, 142), (200, 40, 80));
    let preproc = p((187, 154, 247), (124, 58, 237));
    let punct = p((169, 177, 214), (100, 106, 120));
    let heading = p((122, 162, 247), (37, 99, 235));
    let red = p((247, 118, 142), (200, 30, 60));
    vec![
        ("comment, punctuation.definition.comment", comment, Some(FontStyle::ITALIC)),
        ("keyword, storage.modifier, storage.type.function, keyword.control, variable.language", keyword, None),
        ("keyword.operator, punctuation.accessor, punctuation.separator.key-value", operator, None),
        ("string, punctuation.definition.string, string.quoted", string, None),
        ("constant.character.escape, string.regexp", operator, None),
        ("constant.numeric, constant.language, support.constant, constant.other", number, None),
        (
            "entity.name.function, support.function, meta.function-call, variable.function, meta.method-call entity.name",
            function,
            None,
        ),
        (
            "storage.type, support.type, entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, entity.name.trait, entity.other.inherited-class, support.class",
            ty,
            None,
        ),
        ("variable.parameter, entity.name.lifetime, storage.modifier.lifetime", param, None),
        ("entity.name.tag, punctuation.definition.tag", tag, None),
        ("entity.other.attribute-name", ty, None),
        ("meta.preprocessor, keyword.control.directive, keyword.other.directive, entity.name.macro, support.macro", preproc, None),
        ("meta.annotation, meta.attribute, entity.name.decorator", preproc, None),
        ("punctuation.section, punctuation.terminator, punctuation.separator", punct, None),
        ("markup.heading, entity.name.section", heading, Some(FontStyle::BOLD)),
        ("markup.bold", ty, Some(FontStyle::BOLD)),
        ("markup.italic", keyword, Some(FontStyle::ITALIC)),
        ("markup.inserted", string, None),
        ("markup.deleted, invalid", red, None),
        ("markup.raw, markup.inline.raw", function, None),
    ]
}

/// Тёмная или светлая тема подсветки.
pub fn theme(dark: bool) -> &'static Theme {
    static DARK: OnceLock<Theme> = OnceLock::new();
    static LIGHT: OnceLock<Theme> = OnceLock::new();
    let cell = if dark { &DARK } else { &LIGHT };
    cell.get_or_init(|| {
        let fg = if dark { rgb(201, 209, 224) } else { rgb(40, 44, 52) };
        let bg = if dark { rgb(15, 17, 23) } else { rgb(248, 249, 251) };
        Theme {
            name: Some(if dark { "Holotype Dark" } else { "Holotype Light" }.to_owned()),
            author: None,
            settings: ThemeSettings { foreground: Some(fg), background: Some(bg), ..Default::default() },
            scopes: rules(dark)
                .into_iter()
                .filter_map(|(sel, color, font_style)| {
                    Some(ThemeItem {
                        scope: ScopeSelectors::from_str(sel).ok()?,
                        style: StyleModifier { foreground: Some(color), background: None, font_style },
                    })
                })
                .collect(),
        }
    })
}

fn to_color32(c: Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

#[derive(Default)]
struct CodeHighlighter;

#[derive(Clone, Copy, Hash)]
struct Key<'a> {
    code: &'a str,
    lang: &'a str,
    dark: bool,
    size_bits: u32,
    line_height_bits: u32,
}

impl egui::cache::ComputerMut<Key<'_>, LayoutJob> for CodeHighlighter {
    fn compute(&mut self, key: Key<'_>) -> LayoutJob {
        build_job(key.code, key.lang, key.dark, f32::from_bits(key.size_bits), f32::from_bits(key.line_height_bits))
    }
}

type HighlightCache = egui::cache::FrameCache<LayoutJob, CodeHighlighter>;

/// Подсвеченный код для egui (с кэшированием между кадрами).
pub fn job(ctx: &egui::Context, code: &str, lang: &str, dark: bool, size: f32, line_height: f32) -> LayoutJob {
    let key = Key { code, lang, dark, size_bits: size.to_bits(), line_height_bits: line_height.to_bits() };
    ctx.memory_mut(|m| m.caches.cache::<HighlightCache>().get(key).clone())
}

fn build_job(code: &str, lang: &str, dark: bool, size: f32, line_height: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::monospace(size);
    let theme = theme(dark);
    let fg = theme.settings.foreground.map(to_color32).unwrap_or(Color32::GRAY);
    let plain = |job: &mut LayoutJob, text: &str| {
        job.append(text, 0.0, TextFormat { font_id: font.clone(), color: fg, line_height: Some(line_height), ..Default::default() });
    };
    let Some(syntax) = find(lang) else {
        plain(&mut job, code);
        return job;
    };
    let mut h = HighlightLines::new(syntax, theme);
    for line in LinesWithEndings::from(code) {
        match h.highlight_line(line, syntaxes()) {
            Ok(ranges) => {
                for (style, text) in ranges {
                    job.append(
                        text,
                        0.0,
                        TextFormat {
                            font_id: font.clone(),
                            color: to_color32(style.foreground),
                            line_height: Some(line_height),
                            italics: style.font_style.contains(FontStyle::ITALIC),
                            underline: if style.font_style.contains(FontStyle::UNDERLINE) {
                                egui::Stroke::new(1.0, to_color32(style.foreground))
                            } else {
                                egui::Stroke::NONE
                            },
                            ..Default::default()
                        },
                    );
                }
            }
            Err(_) => plain(&mut job, line),
        }
    }
    job
}

/// Подсвеченный текст как участки «цвет, курсив» (для команд консоли).
pub fn ranges(code: &str, lang: &str, dark: bool) -> Vec<(Range<usize>, Color32, bool)> {
    let theme = theme(dark);
    let fg = theme.settings.foreground.map(to_color32).unwrap_or(Color32::GRAY);
    let Some(syntax) = find(lang) else { return vec![(0..code.len(), fg, false)] };
    let mut h = HighlightLines::new(syntax, theme);
    let mut out = Vec::new();
    let mut start = 0;
    for line in LinesWithEndings::from(code) {
        match h.highlight_line(line, syntaxes()) {
            Ok(parts) => {
                for (style, text) in parts {
                    out.push((start..start + text.len(), to_color32(style.foreground), style.font_style.contains(FontStyle::ITALIC)));
                    start += text.len();
                }
            }
            Err(_) => {
                out.push((start..start + line.len(), fg, false));
                start += line.len();
            }
        }
    }
    out
}

/// HTML с классами подсветки (для экспорта).
pub fn html(code: &str, lang: &str) -> Option<String> {
    let syntax = find(lang)?;
    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, syntaxes(), CSS_CLASS_STYLE);
    for line in LinesWithEndings::from(code) {
        generator.parse_html_for_line_which_includes_newline(line).ok()?;
    }
    Some(generator.finalize())
}

/// CSS для классов подсветки: светлая тема и тёмная под `prefers-color-scheme`.
pub fn css() -> String {
    let light = css_for_theme_with_class_style(theme(false), CSS_CLASS_STYLE).unwrap_or_default();
    let dark = css_for_theme_with_class_style(theme(true), CSS_CLASS_STYLE).unwrap_or_default();
    format!("{light}\n@media (prefers-color-scheme: dark) {{\n{dark}\n}}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_many_languages() {
        for lang in ["rust", "rs", "glsl", "hlsl", "frag", "wgsl", "ts", "tsx", "jsx", "objc", "env", "kotlin", "zig", "toml", "py", "js", "c++", "cs", "go", "lua", "sql", "yaml", "dockerfile", "cmake"] {
            assert!(find(lang).is_some(), "не найден язык {lang}");
        }
        assert!(find("text").is_none());
    }

    #[test]
    fn powershell_is_not_batch() {
        crate::i18n::init();
        // Настоящего синтаксиса PowerShell в наборе (fancy-regex) нет — берём Bash, не Batch.
        assert!(syntaxes().find_syntax_by_name("PowerShell").is_none());
        for lang in ["powershell", "ps1", "pwsh", "ps", "PowerShell"] {
            assert_eq!(find(lang).map(|s| s.name.as_str()), Some("Bourne Again Shell (bash)"), "{lang}");
            assert_eq!(display_name(lang), "PowerShell");
        }
        assert_eq!(find("bat").map(|s| s.name.as_str()), Some("Batch File"));
        assert_eq!(find("cmd").map(|s| s.name.as_str()), Some("Batch File"));
        // Консоль и лог рисуются отдельно, подсветки кода у них нет.
        assert!(find("console").is_none() && find("terminal").is_none() && find("log").is_none());
        assert!(find("sh").is_some());
    }

    #[test]
    fn ranges_cover_the_text() {
        let code = "cargo build --release # сборка";
        let r = ranges(code, "bash", true);
        assert_eq!(r.first().unwrap().0.start, 0);
        assert_eq!(r.last().unwrap().0.end, code.len());
        assert!(r.windows(2).all(|w| w[0].0.end == w[1].0.start));
        assert!(r.iter().any(|(_, _, italic)| *italic), "комментарий курсивом");
    }

    #[test]
    fn highlights_with_colors() {
        let job = build_job("fn main() { let x = 42; } // привет", "rust", true, 14.0, 20.0);
        let colors: std::collections::HashSet<_> = job.sections.iter().map(|s| s.format.color).collect();
        assert!(colors.len() >= 4, "слишком мало цветов: {}", colors.len());
        assert!(html("let a = 1;", "js").unwrap().contains("hl-"));
    }
}

