//! Шрифты заметок. Шрифты интерфейса и редактора ставит платформа Forma (их выбирают на
//! странице «Шрифты» или задаёт тема), а Holotype дописывает к ним свои шрифты формул.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{FontData, FontDefinitions, FontFamily};

pub use forma_ui::fonts::{bold, bold_italic, italic, semibold};

static HAS_MATH: AtomicBool = AtomicBool::new(false);

/// Есть ли настоящий курсивный шрифт (иначе курсив имитируется наклоном).
pub fn has_italic() -> bool {
    forma_ui::fonts::ui_has_italic()
}

/// Жирный моноширинный — начертание шрифта редактора (вывод cargo в консоли).
pub fn mono_bold() -> FontFamily {
    forma_ui::fonts::editor(true, false)
}

/// Шрифт для формул: цифры, операторы, большие символы.
pub fn math() -> FontFamily {
    FontFamily::Name("math".into())
}

/// Курсив для переменных в формулах.
pub fn math_italic() -> FontFamily {
    FontFamily::Name("math-italic".into())
}

/// Найден ли математический шрифт (Cambria Math и т. п.).
pub fn has_math() -> bool {
    HAS_MATH.load(Ordering::Relaxed)
}

/// Подключает шрифты формул к шрифтам платформы. Вызывать при запуске.
pub fn register() {
    forma_ui::fonts::register_extra(add_math);
}

struct Candidate {
    path: &'static str,
    index: u32,
}

const fn c(path: &'static str) -> Candidate {
    Candidate { path, index: 0 }
}

fn load_first(candidates: &[Candidate]) -> Option<FontData> {
    candidates.iter().find_map(|cand| {
        let bytes = std::fs::read(cand.path).ok()?;
        let mut data = FontData::from_owned(bytes);
        data.index = cand.index;
        Some(data)
    })
}

/// Семейства `math` и `math-italic`: Cambria Math (или STIX, DejaVu Serif), а за ними —
/// шрифты интерфейса, чтобы кириллица и остальные знаки тоже нашлись.
fn add_math(defs: &mut FontDefinitions) {
    // В cambria.ttc второй шрифт (индекс 1) — это Cambria Math.
    let math_data = load_first(&[
        Candidate { path: "C:/Windows/Fonts/cambria.ttc", index: 1 },
        c("/usr/share/fonts/opentype/stix/STIXTwoMath-Regular.otf"),
        c("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf"),
    ]);
    let math_italic_data = load_first(&[c("C:/Windows/Fonts/cambriai.ttf"), c("/usr/share/fonts/truetype/dejavu/DejaVuSerif-Italic.ttf")]);
    HAS_MATH.store(math_data.is_some(), Ordering::Relaxed);
    let has = |defs: &FontDefinitions, name: &str| defs.font_data.contains_key(name);
    if let Some(data) = math_data {
        defs.font_data.insert("math-regular".into(), Arc::new(data));
    }
    if let Some(data) = math_italic_data {
        defs.font_data.insert("math-italic".into(), Arc::new(data));
    }
    let proportional = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let italic_chain = defs.families.get(&italic()).cloned().unwrap_or_default();
    let chain = |first: &[&str], rest: &[String]| -> Vec<String> {
        let mut v: Vec<String> = first.iter().filter(|n| has(defs, n)).map(|n| (*n).to_owned()).collect();
        for f in rest {
            if !v.contains(f) {
                v.push(f.clone());
            }
        }
        v
    };
    let math_family = chain(&["math-regular"], &proportional);
    let math_italic_family = chain(&["math-italic", "math-regular"], &italic_chain);
    defs.families.insert(math(), math_family);
    defs.families.insert(self::math_italic(), math_italic_family);
}
