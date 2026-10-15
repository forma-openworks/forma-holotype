//! Подсветка Markdown прямо в редакторе: заголовки крупнее, жирный — жирным,
//! ссылки, теги, код и формулы — своими цветами. Разметка остаётся видимой.

use eframe::egui::{
    self, Color32, FontFamily, FontId, Stroke,
    text::{LayoutJob, TextFormat},
};

use crate::fonts;
use crate::theme::Palette;

#[derive(Clone, Hash, PartialEq)]
pub struct EditorStyle {
    size_bits: u32,
    /// Насколько поднять высоту строки для кода в строке, чтобы его базовая линия совпала с текстом.
    code_delta_bits: u32,
    mono: bool,
    has_italic: bool,
    text: Color32,
    muted: Color32,
    faint: Color32,
    heading: Color32,
    accent: Color32,
    link: Color32,
    code_text: Color32,
    code_bg: Color32,
    mark_bg: Color32,
    tag_text: Color32,
    tag_bg: Color32,
}

impl EditorStyle {
    pub fn new(pal: &Palette, size: f32, mono: bool, code_delta: f32) -> Self {
        Self {
            size_bits: size.to_bits(),
            code_delta_bits: code_delta.to_bits(),
            mono,
            has_italic: fonts::has_italic(),
            text: pal.text,
            muted: pal.text_muted,
            faint: pal.text_faint,
            heading: pal.heading,
            accent: pal.accent,
            link: pal.link,
            code_text: pal.inline_code_text,
            code_bg: pal.inline_code_bg,
            mark_bg: pal.mark_bg,
            tag_text: pal.tag_text,
            tag_bg: pal.tag_bg,
        }
    }

    fn size(&self) -> f32 {
        f32::from_bits(self.size_bits)
    }
}

#[derive(Default)]
struct Highlighter;

impl egui::cache::ComputerMut<(&str, &EditorStyle), LayoutJob> for Highlighter {
    fn compute(&mut self, (text, style): (&str, &EditorStyle)) -> LayoutJob {
        build(text, style)
    }
}

type HighlightCache = egui::cache::FrameCache<LayoutJob, Highlighter>;

/// Подсвеченный текст с кэшированием (пересчитывается только при изменении текста или стиля).
pub fn highlight(ctx: &egui::Context, text: &str, style: &EditorStyle) -> LayoutJob {
    ctx.memory_mut(|m| m.caches.cache::<HighlightCache>().get((text, style)).clone())
}

const BOLD: u16 = 1;
const ITALIC: u16 = 1 << 1;
const STRIKE: u16 = 1 << 2;
const CODE: u16 = 1 << 3;
const MARK: u16 = 1 << 4;
const LINK: u16 = 1 << 5;
const FAINT: u16 = 1 << 6;
const TAG: u16 = 1 << 7;
const ACCENT: u16 = 1 << 8;
const MUTED: u16 = 1 << 9;
const MATH: u16 = 1 << 10;

#[derive(Clone, Copy, PartialEq)]
enum LineKind {
    Normal,
    Heading(u8),
    CodeBlock,
    Meta,
}

fn build(text: &str, st: &EditorStyle) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut in_code: Option<String> = None;
    let mut in_front = false;
    let mut in_math = false;
    for (line_no, line) in text.split_inclusive('\n').enumerate() {
        let content = line.trim_end_matches(['\n', '\r']);
        let newline = &line[content.len()..];
        let trimmed = content.trim_start();
        let mut flags = vec![0u16; content.len()];
        let mut kind = LineKind::Normal;

        if line_no == 0 && content == "---" {
            in_front = true;
            kind = LineKind::Meta;
        } else if in_front {
            kind = LineKind::Meta;
            if content == "---" {
                in_front = false;
            }
        } else if let Some(fence) = &in_code {
            kind = LineKind::CodeBlock;
            if trimmed.starts_with(fence.as_str()) {
                in_code = None;
                fill(&mut flags, 0, content.len(), FAINT);
            }
        } else if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let fence: String = trimmed.chars().take_while(|c| *c == '`' || *c == '~').collect();
            in_code = Some(fence.clone());
            kind = LineKind::CodeBlock;
            let start = content.len() - trimmed.len();
            fill(&mut flags, 0, start + fence.len(), FAINT);
            fill(&mut flags, start + fence.len(), content.len(), ACCENT);
        } else if let Some(rest) = trimmed.strip_prefix("$$") {
            fill(&mut flags, 0, content.len(), MATH);
            let rest = rest.trim_end();
            if !(rest.ends_with("$$") && rest.len() >= 2) {
                in_math = !in_math;
            }
        } else if in_math {
            fill(&mut flags, 0, content.len(), MATH);
        } else {
            kind = mark_line(content, &mut flags);
        }

        let mut i = 0;
        while i < content.len() {
            let f = flags[i];
            let mut j = i + 1;
            while j < content.len() && flags[j] == f {
                j += 1;
            }
            job.append(&content[i..j], 0.0, format(st, kind, f));
            i = j;
        }
        if !newline.is_empty() {
            job.append(newline, 0.0, format(st, kind, 0));
        }
    }
    job
}

fn fill(flags: &mut [u16], from: usize, to: usize, f: u16) {
    let len = flags.len();
    for x in &mut flags[from.min(len)..to.min(len)] {
        *x |= f;
    }
}

/// Разметка блочного уровня и запуск разбора инлайнов.
fn mark_line(content: &str, flags: &mut [u16]) -> LineKind {
    let trimmed = content.trim_start();
    let indent = content.len() - trimmed.len();

    // Заголовок
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') && indent < 4 {
        fill(flags, 0, indent + hashes + 1, FAINT);
        inline_marks(content, flags, indent + hashes + 1);
        return LineKind::Heading(hashes as u8);
    }

    // Горизонтальная линия
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() >= 3
        && (compact.chars().all(|c| c == '-') || compact.chars().all(|c| c == '*') || compact.chars().all(|c| c == '_'))
    {
        fill(flags, 0, content.len(), FAINT);
        return LineKind::Normal;
    }

    let mut pos = indent;
    let bytes = content.as_bytes();
    let mut quoted = false;
    // Цитаты, возможно вложенные: `> > текст`
    while pos < bytes.len() && bytes[pos] == b'>' {
        quoted = true;
        fill(flags, pos, pos + 1, ACCENT);
        pos += 1;
        while pos < bytes.len() && bytes[pos] == b' ' {
            pos += 1;
        }
    }
    // Заголовок плашки `[!note]`
    if quoted && content[pos..].starts_with("[!")
        && let Some(end) = content[pos..].find(']')
    {
        let mut e = pos + end + 1;
        if e < bytes.len() && (bytes[e] == b'-' || bytes[e] == b'+') {
            e += 1;
        }
        fill(flags, pos, e, ACCENT | BOLD);
        fill(flags, e, content.len(), BOLD);
        pos = e;
    }

    // Маркер списка и чекбокс
    let rest = &content[pos..];
    let marker_len = if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        2
    } else {
        let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && digits < 10 && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") ")) {
            digits + 2
        } else {
            0
        }
    };
    let mut done = false;
    if marker_len > 0 {
        fill(flags, pos, pos + marker_len, ACCENT);
        pos += marker_len;
        let r = &content[pos..];
        if r.starts_with("[ ] ") || r.starts_with("[x] ") || r.starts_with("[X] ") {
            fill(flags, pos, pos + 3, ACCENT | BOLD);
            done = r.as_bytes()[1] != b' ';
            pos += 4;
        }
    }

    // Таблицы: вертикальные черты бледнее
    if trimmed.starts_with('|') {
        for (i, b) in bytes.iter().enumerate() {
            if *b == b'|' {
                flags[i] |= FAINT;
            }
        }
        if trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
            fill(flags, 0, content.len(), FAINT);
            return LineKind::Normal;
        }
    }

    inline_marks(content, flags, pos);
    if quoted {
        fill(flags, pos, content.len(), MUTED);
    }
    if done {
        fill(flags, pos, content.len(), STRIKE | MUTED);
    }
    LineKind::Normal
}

/// Разметка внутри строки: код, формулы, ссылки, жирный, курсив, зачёркивание, выделение, теги.
fn inline_marks(line: &str, flags: &mut [u16], from: usize) {
    let bytes = line.as_bytes();
    let n = bytes.len();
    let mut protected = vec![false; n];
    for p in protected.iter_mut().take(from.min(n)) {
        *p = true;
    }

    // `код`
    let mut i = from;
    while i < n {
        if bytes[i] == b'`' && !protected[i] {
            let run = bytes[i..].iter().take_while(|b| **b == b'`').count();
            let mut j = i + run;
            let mut found = None;
            while j < n {
                if bytes[j] == b'`' {
                    let r2 = bytes[j..].iter().take_while(|b| **b == b'`').count();
                    if r2 == run {
                        found = Some(j);
                        break;
                    }
                    j += r2;
                } else {
                    j += 1;
                }
            }
            if let Some(j) = found {
                fill(flags, i, i + run, FAINT | CODE);
                fill(flags, i + run, j, CODE);
                fill(flags, j, j + run, FAINT | CODE);
                for p in &mut protected[i..j + run] {
                    *p = true;
                }
                i = j + run;
                continue;
            }
            i += run;
            continue;
        }
        i += 1;
    }

    // {{Ctrl+S}} — клавиши
    let mut i = from;
    while let Some(rel) = line.get(i..).and_then(|s| s.find("{{")) {
        let s = i + rel;
        let Some(end_rel) = line[s + 2..].find("}}") else { break };
        let e = s + 2 + end_rel;
        if !protected[s] && end_rel > 0 && end_rel <= 64 {
            fill(flags, s, s + 2, FAINT);
            fill(flags, s + 2, e, ACCENT | BOLD);
            fill(flags, e, e + 2, FAINT);
            for p in &mut protected[s..e + 2] {
                *p = true;
            }
        }
        i = e + 2;
    }

    // <kbd>Ctrl</kbd>
    let lower = line.to_ascii_lowercase();
    let mut i = from;
    while let Some(rel) = lower.get(i..).and_then(|s| s.find("<kbd>")) {
        let s = i + rel;
        let Some(end_rel) = lower[s + 5..].find("</kbd>") else { break };
        let e = s + 5 + end_rel;
        fill(flags, s, s + 5, FAINT);
        fill(flags, s + 5, e, ACCENT | BOLD);
        fill(flags, e, e + 6, FAINT);
        for p in &mut protected[s..e + 6] {
            *p = true;
        }
        i = e + 6;
    }

    // $формула$
    let mut i = from;
    while i < n {
        if bytes[i] == b'$' && !protected[i] && i + 1 < n && bytes[i + 1] != b'$' && bytes[i + 1] != b' '
            && let Some(rel) = line[i + 1..].find('$')
        {
            let j = i + 1 + rel;
            if bytes[j - 1] != b' ' && !protected[j] {
                fill(flags, i, i + 1, FAINT | MATH);
                fill(flags, i + 1, j, MATH);
                fill(flags, j, j + 1, FAINT | MATH);
                for p in &mut protected[i..=j] {
                    *p = true;
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }

    // [[вики-ссылки]] и ![[встраивания]]
    let mut i = from;
    while let Some(rel) = line.get(i..).and_then(|s| s.find("[[")) {
        let s = i + rel;
        if protected[s] {
            i = s + 2;
            continue;
        }
        let Some(rel_end) = line[s + 2..].find("]]") else { break };
        let e = s + 2 + rel_end;
        let start = if s > 0 && bytes[s - 1] == b'!' { s - 1 } else { s };
        fill(flags, start, s + 2, FAINT);
        // Псевдоним после | показываем ярче, сам путь — как ссылку.
        fill(flags, s + 2, e, LINK);
        if let Some(pipe) = line[s + 2..e].find('|') {
            fill(flags, s + 2 + pipe, s + 3 + pipe, FAINT);
        }
        fill(flags, e, e + 2, FAINT);
        for p in &mut protected[start..e + 2] {
            *p = true;
        }
        i = e + 2;
    }

    // [текст](адрес) и ![подпись](адрес)
    let mut i = from;
    while i < n {
        if bytes[i] == b'[' && !protected[i]
            && let Some(close_rel) = line[i + 1..].find("](")
        {
            let close = i + 1 + close_rel;
            if let Some(paren_rel) = line[close + 2..].find(')') {
                let paren = close + 2 + paren_rel;
                if !protected[close] {
                    let start = if i > 0 && bytes[i - 1] == b'!' { i - 1 } else { i };
                    fill(flags, start, i + 1, FAINT);
                    fill(flags, i + 1, close, LINK);
                    fill(flags, close, paren + 1, FAINT);
                    for p in &mut protected[close..=paren] {
                        *p = true;
                    }
                    protected[start] = true;
                    protected[i] = true;
                    i = paren + 1;
                    continue;
                }
            }
        }
        i += 1;
    }

    pair(line, flags, &mut protected, from, "**", BOLD, false);
    pair(line, flags, &mut protected, from, "__", BOLD, true);
    pair(line, flags, &mut protected, from, "~~", STRIKE, false);
    pair(line, flags, &mut protected, from, "==", MARK, false);
    pair(line, flags, &mut protected, from, "*", ITALIC, false);
    pair(line, flags, &mut protected, from, "_", ITALIC, true);

    // Атрибуты после кода и выделения: `код`{green}, ==текст=={blue}
    for (i, ch) in line.char_indices() {
        if ch != '{' || i < from || protected[i] {
            continue;
        }
        let after_code = line[..i].ends_with('`');
        let after_mark = line[..i].ends_with("==");
        if !(after_code || after_mark) {
            continue;
        }
        if let Some(end_rel) = line[i..].find('}')
            && end_rel <= 48
        {
            fill(flags, i, i + end_rel + 1, FAINT);
            for p in &mut protected[i..=i + end_rel] {
                *p = true;
            }
        }
    }

    // #теги и голые ссылки
    let mut prev: Option<char> = None;
    let mut skip_until = 0;
    for (i, ch) in line.char_indices() {
        if i < from || i < skip_until || protected[i] {
            prev = Some(ch);
            continue;
        }
        let boundary = prev.is_none_or(|p| p.is_whitespace() || "([{,;".contains(p));
        if boundary && ch == '#' {
            let len: usize = line[i + 1..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))
                .map(char::len_utf8)
                .sum();
            let name = &line[i + 1..i + 1 + len];
            if !name.is_empty() && name.chars().any(|c| !c.is_ascii_digit()) {
                fill(flags, i, i + 1 + len, TAG);
                skip_until = i + 1 + len;
            }
        } else if boundary && (line[i..].starts_with("https://") || line[i..].starts_with("http://")) {
            let len = line[i..].find(char::is_whitespace).unwrap_or(line.len() - i);
            fill(flags, i, i + len, LINK);
            skip_until = i + len;
        }
        prev = Some(ch);
    }
}

/// Парные разделители вроде `**…**`. Найденные разделители бледнеют, содержимое получает флаг.
fn pair(line: &str, flags: &mut [u16], protected: &mut [bool], from: usize, delim: &str, flag: u16, word_boundary: bool) {
    let bytes = line.as_bytes();
    let d = delim.len();
    let is_word = |i: Option<usize>| i.and_then(|i| line.get(i..)).and_then(|s| s.chars().next()).is_some_and(char::is_alphanumeric);
    let matches_at = |protected: &[bool], k: usize| {
        k + d <= bytes.len() && &bytes[k..k + d] == delim.as_bytes() && !protected[k..k + d].iter().any(|p| *p)
    };
    let mut i = from;
    while i + d <= bytes.len() {
        if !matches_at(protected, i) {
            i += 1;
            continue;
        }
        // Открывающий разделитель: дальше не пробел.
        let next_char = line[i + d..].chars().next();
        if next_char.is_none_or(char::is_whitespace) || (word_boundary && i > 0 && is_word(prev_char_index(line, i))) {
            i += d;
            continue;
        }
        let mut j = i + d + 1;
        let mut found = None;
        while j + d <= bytes.len() {
            if matches_at(protected, j) {
                let prev_ws = line[..j].chars().next_back().is_none_or(char::is_whitespace);
                let after_word = word_boundary && is_word(Some(j + d));
                // Для одиночных * не путаем с половинкой **.
                let doubled = d == 1 && (bytes.get(j + 1) == Some(&bytes[j]));
                if !prev_ws && !after_word && !doubled {
                    found = Some(j);
                    break;
                }
            }
            j += 1;
        }
        if let Some(j) = found {
            fill(flags, i, i + d, FAINT);
            fill(flags, i + d, j, flag);
            fill(flags, j, j + d, FAINT);
            for p in &mut protected[i..i + d] {
                *p = true;
            }
            for p in &mut protected[j..j + d] {
                *p = true;
            }
            i += d;
        } else {
            i += d;
        }
    }
}

fn prev_char_index(line: &str, i: usize) -> Option<usize> {
    line[..i].char_indices().next_back().map(|(k, _)| k)
}

fn format(st: &EditorStyle, kind: LineKind, f: u16) -> TextFormat {
    let base = st.size();
    let (size, mut color) = match kind {
        LineKind::Heading(1) => (base * 1.55, st.heading),
        LineKind::Heading(2) => (base * 1.35, st.heading),
        LineKind::Heading(3) => (base * 1.18, st.heading),
        LineKind::Heading(_) => (base * 1.05, st.heading),
        LineKind::CodeBlock => (base * 0.92, st.code_text),
        LineKind::Meta => (base * 0.92, st.muted),
        LineKind::Normal => (base, st.text),
    };
    let heading = matches!(kind, LineKind::Heading(_));
    let mono_line = matches!(kind, LineKind::CodeBlock | LineKind::Meta) || f & CODE != 0;
    let bold = f & BOLD != 0 || heading;
    let italic = f & ITALIC != 0;
    let family = if mono_line || st.mono {
        FontFamily::Monospace
    } else {
        match (bold, italic) {
            (true, true) => fonts::bold_italic(),
            (true, false) => {
                if heading {
                    fonts::bold()
                } else {
                    fonts::semibold()
                }
            }
            (false, true) => fonts::italic(),
            (false, false) => FontFamily::Proportional,
        }
    };
    let mut font_size = size;
    if f & CODE != 0 && kind == LineKind::Normal && !st.mono {
        font_size *= 0.92;
    }
    let mut background = Color32::TRANSPARENT;
    if f & MUTED != 0 {
        color = st.muted;
    }
    if bold && !heading && kind == LineKind::Normal {
        color = st.heading;
    }
    if f & CODE != 0 && kind == LineKind::Normal {
        color = st.code_text;
        background = st.code_bg;
    }
    if f & MATH != 0 {
        color = st.accent;
    }
    if f & MARK != 0 {
        background = st.mark_bg;
    }
    if f & LINK != 0 {
        color = st.link;
    }
    if f & TAG != 0 {
        color = st.tag_text;
        background = st.tag_bg;
    }
    if f & ACCENT != 0 {
        color = st.accent;
    }
    if f & FAINT != 0 {
        color = st.faint;
    }
    TextFormat {
        font_id: FontId::new(font_size, family),
        color,
        background,
        line_height: Some(if f & CODE != 0 && kind == LineKind::Normal && !st.mono {
            size * 1.55 - f32::from_bits(st.code_delta_bits)
        } else {
            size * 1.55
        }),
        italics: italic && (!st.has_italic || st.mono || mono_line),
        strikethrough: if f & STRIKE != 0 { Stroke::new(1.2, color) } else { Stroke::NONE },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags_of(line: &str) -> Vec<u16> {
        let mut flags = vec![0u16; line.len()];
        mark_line(line, &mut flags);
        flags
    }

    #[test]
    fn bold_and_code() {
        let line = "a **b** `c*d*` e";
        let f = flags_of(line);
        assert!(f[line.find('b').unwrap()] & BOLD != 0);
        let star_in_code = line.find("c*").unwrap() + 1;
        assert!(f[star_in_code] & ITALIC == 0, "звёздочки внутри кода не должны давать курсив");
    }

    #[test]
    fn snake_case_is_not_italic() {
        let line = "some_variable_name";
        let f = flags_of(line);
        assert!(f.iter().all(|x| x & ITALIC == 0));
    }

    #[test]
    fn job_text_matches_source() {
        let src = "# Заголовок\n\n- [x] готово\n```rust\nfn main() {}\n```\nТекст с [[ссылкой]] и #тегом ==важно== $x^2$\n";
        let st = EditorStyle::new(&Palette::default_dark(), 16.0, false, 0.0);
        let job = build(src, &st);
        assert_eq!(job.text, src);
    }
}
