//! Сеансы консоли (```` ```console ````) и логи (```` ```log ````): разбор строк и раскраска.
//! Здесь только текст, без egui — просмотр и экспорт в HTML переводят оттенки в свои цвета.

use std::ops::Range;

use crate::markdown::CodeBlock;

/// Как рисовать блок кода.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Code,
    /// Сеанс bash: команды после `$`, их вывод и комментарии.
    Console,
    /// Лог: время, источник в скобках, уровень, сообщение.
    Log,
}

pub fn kind(lang: &str) -> Kind {
    match lang.trim().to_ascii_lowercase().as_str() {
        "console" | "terminal" | "shell-session" | "sh-session" => Kind::Console,
        "log" => Kind::Log,
        _ => Kind::Code,
    }
}

/// Сколько строк консоли и лога видно, пока блок свёрнут (если нет `lines=N`).
pub const DEFAULT_LINES: usize = 12;

/// Строки текста блока; пустой текст — одна пустая строка.
pub fn lines(code: &str) -> impl Iterator<Item = &str> {
    code.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l))
}

pub fn line_count(code: &str) -> usize {
    code.split('\n').count()
}

/// Первые `n` строк текста (без перевода строки в конце).
pub fn head(code: &str, n: usize) -> &str {
    if n == 0 {
        return "";
    }
    code.match_indices('\n').nth(n - 1).map_or(code, |(i, _)| &code[..i])
}

/// Сколько строк показать у свёрнутого блока; None — блок не сворачивается.
/// Консоль и лог сворачиваются сами после [`DEFAULT_LINES`] строк, остальной код — только с `lines=N`.
pub fn collapse_at(cb: &CodeBlock) -> Option<usize> {
    if cb.open {
        return None;
    }
    let limit = match (cb.lines, kind(&cb.lang)) {
        (Some(n), _) => n.max(1),
        (None, Kind::Console | Kind::Log) => DEFAULT_LINES,
        (None, Kind::Code) => return None,
    };
    (line_count(&cb.code) > limit).then_some(limit)
}

// --------------------------------------------------------------------------------------------
// Консоль
// --------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// Команда: строка `$ …` (сама `$` в текст не входит).
    Command,
    /// Продолжение команды после строки, оканчивающейся на `\`.
    Continuation,
    Comment,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsoleLine<'a> {
    pub kind: LineKind,
    pub text: &'a str,
}

/// Текст команды, если строка начинается с приглашения `$`.
fn prompt(line: &str) -> Option<&str> {
    if line == "$" { Some("") } else { line.strip_prefix("$ ") }
}

/// Разбирает сеанс: команды, их продолжения, комментарии и вывод.
/// `#` — комментарий, только если за ним (через комментарии и пустые строки) идёт команда
/// или текст кончается: `# This file is @generated…` посреди вывода `cat` остаётся выводом.
pub fn console_lines(code: &str) -> Vec<ConsoleLine<'_>> {
    let raw: Vec<&str> = lines(code).collect();
    let mut out = Vec::with_capacity(raw.len());
    let mut continues = false;
    for (i, &line) in raw.iter().enumerate() {
        let (kind, text) = if continues {
            (LineKind::Continuation, line)
        } else if let Some(cmd) = prompt(line) {
            (LineKind::Command, cmd)
        } else if line.starts_with('#') && comment_before_command(&raw[i + 1..]) {
            (LineKind::Comment, line)
        } else {
            (LineKind::Output, line)
        };
        continues = matches!(kind, LineKind::Command | LineKind::Continuation) && text.trim_end().ends_with('\\');
        out.push(ConsoleLine { kind, text });
    }
    out
}

fn comment_before_command(rest: &[&str]) -> bool {
    rest.iter().find(|l| !l.trim().is_empty() && !l.starts_with('#')).is_none_or(|l| prompt(l).is_some())
}

/// Команда сеанса вместе с продолжениями.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// Номера строк сеанса.
    pub lines: Range<usize>,
    /// Текст без приглашения; строки продолжения — через `\n`.
    pub text: String,
}

pub fn commands(lines: &[ConsoleLine<'_>]) -> Vec<Command> {
    let mut out: Vec<Command> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        match l.kind {
            LineKind::Command => out.push(Command { lines: i..i + 1, text: l.text.to_owned() }),
            LineKind::Continuation => {
                if let Some(c) = out.last_mut() {
                    c.lines.end = i + 1;
                    c.text.push('\n');
                    c.text.push_str(l.text);
                }
            }
            _ => {}
        }
    }
    out
}

/// Все команды сеанса для кнопки «Копировать команды»: без приглашений, вывода и комментариев.
pub fn commands_text(code: &str) -> String {
    let cmds = commands(&console_lines(code));
    cmds.into_iter().map(|c| c.text).filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n")
}

/// Оттенки вывода Cargo и rustc.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tone {
    /// Слово состояния Cargo: `Compiling`, `Finished`… — жирным зелёным.
    Status,
    Error,
    Warning,
    /// `note` и `help`.
    Hint,
    /// Текст сообщения в заголовке диагностики.
    Message,
    /// Служебные поля rustc: `-->`, `|`, номера строк, `=`.
    Gutter,
    /// Добавленное подсказкой (`+` под `help`).
    Added,
}

/// Слова состояния Cargo в начале строки (выровнены вправо).
const STATUS: &[&str] = &[
    "Compiling", "Checking", "Finished", "Running", "Removed", "Creating", "Documenting", "Generated", "Fresh", "Updating",
    "Downloading", "Downloaded", "Installing", "Installed", "Adding", "Locking", "Blocking", "Packaging", "Fixed", "Fixing",
    "Removing", "Replacing", "Summary", "Waiting", "Unpacking",
];

/// Заголовок диагностики: `error[E0425]:`, `warning:`, `note:`, `help:` — длина метки и оттенок.
fn diagnostic(line: &str) -> Option<(usize, Tone)> {
    for (word, tone) in [("error", Tone::Error), ("warning", Tone::Warning), ("note", Tone::Hint), ("help", Tone::Hint)] {
        let Some(rest) = line.strip_prefix(word) else { continue };
        let code = if rest.starts_with('[') { rest.find(']').map_or(0, |e| e + 1) } else { 0 };
        if rest[code..].starts_with(':') {
            return Some((word.len() + code, tone));
        }
    }
    None
}

/// Раскраска строки вывода. `level` — уровень последней диагностики rustc: по нему красятся
/// `^^^` и `---` под кодом; пустая строка и строка состояния Cargo его сбрасывают.
/// Участки идут по порядку и не перекрываются, промежутки — обычный текст.
pub fn output_spans(line: &str, level: &mut Option<Tone>) -> Vec<(Range<usize>, Tone)> {
    let indent = line.len() - line.trim_start().len();
    let body = &line[indent..];
    if body.trim_end().is_empty() {
        *level = None;
        return Vec::new();
    }
    if let Some(word) = STATUS.iter().find(|w| body.strip_prefix(**w).is_some_and(|r| r.starts_with(' ') && !r.trim().is_empty())) {
        *level = None;
        return vec![(indent..indent + word.len(), Tone::Status)];
    }
    if indent == 0
        && let Some((label, tone)) = diagnostic(line)
    {
        *level = Some(tone);
        let mut spans = vec![(0..label, tone)];
        let msg = label + 1 + (line[label + 1..].len() - line[label + 1..].trim_start().len());
        if msg < line.len() {
            spans.push((msg..line.len(), Tone::Message));
        }
        return spans;
    }
    let Some(lv) = *level else { return Vec::new() };
    // ` --> src\main.rs:4:28` и `::: путь`.
    if body.starts_with("--> ") || body.starts_with("::: ") {
        return vec![(indent..line.len(), Tone::Gutter)];
    }
    // `  = note: …`
    if let Some(rest) = body.strip_prefix("= ") {
        let mut spans = vec![(indent..indent + 1, Tone::Gutter)];
        if let Some((label, tone)) = diagnostic(rest) {
            spans.push((indent + 2..indent + 2 + label, tone));
        }
        return spans;
    }
    // Поле с номером строки: `4 |`, `  |`, `10 |`, а в подсказках ещё `4 +`.
    let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let after = &body[digits..];
    let bar = indent + digits + (after.len() - after.trim_start_matches(' ').len());
    match line[bar..].chars().next() {
        Some('|') => {
            let mut spans = vec![(indent..bar + 1, Tone::Gutter)];
            let rest = &line[bar + 1..];
            let marks = rest.trim_start();
            // Строка пометок под кодом: `^^^^ подпись`, `---`, `+`.
            if digits == 0 && marks.starts_with(['^', '-', '~', '+', '|', '_']) {
                let tone = if marks.starts_with('+') && lv == Tone::Hint { Tone::Added } else { lv };
                spans.push((line.len() - marks.len()..line.len(), tone));
            }
            spans
        }
        Some('+') if digits > 0 && lv == Tone::Hint => vec![(indent..bar, Tone::Gutter), (bar..line.len(), Tone::Added)],
        _ => Vec::new(),
    }
}

// --------------------------------------------------------------------------------------------
// Лог
// --------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    /// `ERROR`, `ERR` и `FATAL`.
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogTone {
    Plain,
    /// Метка времени.
    Time,
    /// Группа в скобках `[main/INFO]` или источник `hangar::net:`.
    Group,
    /// Слово уровня.
    Level(Level),
    /// Сообщение строки с уровнем WARN или ERROR.
    Tinted(Level),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    /// Уровень строки: свой или унаследованный строкой продолжения (`\tat …`, `Caused by: …`).
    pub level: Option<Level>,
    /// Участки по порядку, без промежутков, на всю строку.
    pub spans: Vec<(Range<usize>, LogTone)>,
}

/// Слово уровня в начале `s` целиком: длина и уровень.
fn level_word(s: &str) -> Option<(usize, Level)> {
    const WORDS: &[(&str, Level)] = &[
        ("TRACE", Level::Trace),
        ("DEBUG", Level::Debug),
        ("INFO", Level::Info),
        ("WARNING", Level::Warn),
        ("WARN", Level::Warn),
        ("ERROR", Level::Error),
        ("ERR", Level::Error),
        ("FATAL", Level::Error),
    ];
    WORDS.iter().find_map(|(w, l)| {
        let rest = s.strip_prefix(w)?;
        (!rest.starts_with(|c: char| c.is_alphanumeric() || c == '_')).then_some((w.len(), *l))
    })
}

/// Метка времени в начале строки: `[13:36:24]`, `13:36:24.123`, `2026-10-12 13:36:24`,
/// `2026-10-12T13:36:24.123Z`. Длина и `true`, если скобка открыта и не закрыта сразу после
/// времени (`[2026-10-12T10:00:00Z WARN  hangar]` у env_logger).
fn timestamp(line: &str) -> Option<(usize, bool)> {
    let b = line.as_bytes();
    let at = |i: usize, c: u8| b.get(i) == Some(&c);
    let digits = |i: usize, n: usize| b.len() >= i + n && b[i..i + n].iter().all(u8::is_ascii_digit);
    let bracket = at(0, b'[');
    let mut i = usize::from(bracket);
    let date = digits(i, 4) && at(i + 4, b'-') && digits(i + 5, 2) && at(i + 7, b'-') && digits(i + 8, 2);
    if date {
        i += 10;
        if (at(i, b' ') || at(i, b'T')) && digits(i + 1, 2) {
            i += 1;
        }
    }
    if digits(i, 2) && at(i + 2, b':') && digits(i + 3, 2) && at(i + 5, b':') && digits(i + 6, 2) {
        i += 8;
        if (at(i, b'.') || at(i, b',')) && digits(i + 1, 1) {
            i += 1;
            while digits(i, 1) {
                i += 1;
            }
        }
        if at(i, b'Z') {
            i += 1;
        } else if (at(i, b'+') || at(i, b'-')) && digits(i + 1, 2) {
            i += 3;
            if at(i, b':') && digits(i + 1, 2) {
                i += 3;
            } else if digits(i, 2) {
                i += 2;
            }
        }
    } else if !date {
        return None;
    }
    if b.get(i).is_some_and(u8::is_ascii_alphanumeric) {
        return None;
    }
    if bracket {
        if at(i, b']') {
            return Some((i + 1, false));
        }
        return Some((i, true));
    }
    Some((i, false))
}

/// Источник в духе tracing после уровня: `hangar::net:` — длина без двоеточия.
fn target(s: &str) -> Option<usize> {
    let end = s.find(": ").or_else(|| s.strip_suffix(':').map(str::len))?;
    let t = &s[..end];
    (t.starts_with(|c: char| c.is_alphabetic() || c == '_') && !t.contains(char::is_whitespace)).then_some(end)
}

/// Раскраска строки лога. `prev` — уровень предыдущей строки: его наследуют строки
/// продолжения без своей шапки (стек вызовов под ошибкой).
pub fn log_line(line: &str, prev: Option<Level>) -> LogLine {
    let mut spans: Vec<(Range<usize>, LogTone)> = Vec::new();
    let mut level = None;
    let mut pos = 0;
    let mut open = false;
    if let Some((len, o)) = timestamp(line) {
        spans.push((0..len, LogTone::Time));
        pos = len;
        open = o;
    }
    let mut after_level = false;
    loop {
        let ws = pos + (line[pos..].len() - line[pos..].trim_start().len());
        let rest = &line[ws..];
        if let Some((len, lv)) = level_word(rest) {
            level.get_or_insert(lv);
            spans.push((ws..ws + len, LogTone::Level(lv)));
            pos = ws + len;
            after_level = true;
            continue;
        }
        if open {
            // Внутри скобок env_logger: источник до `]`.
            let Some(end) = rest.find(']') else { break };
            spans.push((ws..ws + end + 1, LogTone::Group));
            pos = ws + end + 1;
            open = false;
            after_level = false;
            continue;
        }
        if rest.starts_with('[') {
            let Some(end) = rest.find(']') else { break };
            // Группа `[main/INFO]`: слово уровня внутри — своим цветом.
            let inner = &rest[1..end];
            let found = inner.char_indices().filter(|(i, _)| *i == 0 || !inner[..*i].ends_with(|c: char| c.is_alphanumeric())).find_map(|(i, _)| {
                level_word(&inner[i..]).map(|(len, lv)| (i, len, lv))
            });
            match found {
                Some((i, len, lv)) => {
                    level.get_or_insert(lv);
                    let at = ws + 1 + i;
                    spans.push((ws..at, LogTone::Group));
                    spans.push((at..at + len, LogTone::Level(lv)));
                    spans.push((at + len..ws + end + 1, LogTone::Group));
                }
                None => spans.push((ws..ws + end + 1, LogTone::Group)),
            }
            pos = ws + end + 1;
            after_level = false;
            continue;
        }
        if after_level && let Some(len) = target(rest) {
            spans.push((ws..ws + len, LogTone::Group));
            pos = ws + len;
        }
        break;
    }
    // Строка продолжения без шапки наследует уровень предыдущей.
    let continuation = spans.is_empty() && (line.starts_with([' ', '\t']) || line.starts_with("Caused by"));
    if continuation {
        level = prev;
        pos = 0;
    } else if !spans.is_empty() {
        // Разделитель `: ` перед сообщением — обычным цветом.
        let rest = &line[pos..];
        let sep = rest.len() - rest.trim_start_matches(|c: char| c == ':' || c.is_whitespace()).len();
        if sep > 0 {
            spans.push((pos..pos + sep, LogTone::Plain));
            pos += sep;
        }
    }
    if pos < line.len() {
        let tone = match level {
            Some(lv @ (Level::Warn | Level::Error)) => LogTone::Tinted(lv),
            _ => LogTone::Plain,
        };
        spans.push((pos..line.len(), tone));
    }
    LogLine { level, spans: fill_gaps(spans, line.len()) }
}

/// Заполняет промежутки между участками обычным текстом.
fn fill_gaps(spans: Vec<(Range<usize>, LogTone)>, len: usize) -> Vec<(Range<usize>, LogTone)> {
    let mut out = Vec::with_capacity(spans.len() + 2);
    let mut at = 0;
    for (r, tone) in spans {
        if r.start > at {
            out.push((at..r.start, LogTone::Plain));
        }
        if r.end > r.start {
            at = r.end;
            out.push((r, tone));
        }
    }
    if at < len {
        out.push((at..len, LogTone::Plain));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cb(lang: &str, code: &str) -> CodeBlock {
        CodeBlock { lang: lang.to_owned(), code: code.to_owned(), ..Default::default() }
    }

    #[test]
    fn kinds() {
        for lang in ["console", "terminal", "shell-session", "sh-session", "Console"] {
            assert_eq!(kind(lang), Kind::Console, "{lang}");
        }
        assert_eq!(kind("log"), Kind::Log);
        assert_eq!(kind("bash"), Kind::Code);
        assert_eq!(kind("sh"), Kind::Code);
    }

    #[test]
    fn console_classification() {
        use LineKind as K;
        let src = "# Сборка\n$ cargo build \\\n    --release\n   Compiling hangar v0.1.0\n\n$ cat Cargo.lock\n# This file is @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n# Ещё раз\n\n$\n$ ls\n# в конце";
        let lines = console_lines(src);
        let kinds: Vec<LineKind> = lines.iter().map(|l| l.kind).collect();
        #[rustfmt::skip]
        let expected = vec![
            K::Comment, K::Command, K::Continuation, K::Output, K::Output, K::Command, K::Output, K::Output, K::Output, K::Output,
            K::Comment, K::Output, K::Command, K::Command, K::Comment,
        ];
        assert_eq!(kinds, expected);
        assert_eq!(lines[1].text, "cargo build \\");
        assert_eq!(lines[2].text, "    --release");
        assert_eq!(lines[12].text, "");
        assert_eq!(lines[4].text, "", "пустые строки остаются");
        let cmds = commands(&lines);
        assert_eq!(cmds.len(), 4);
        assert_eq!(cmds[0], Command { lines: 1..3, text: "cargo build \\\n    --release".to_owned() });
        // Копируются только команды: без `$`, вывода, комментариев и пустых приглашений.
        assert_eq!(commands_text(src), "cargo build \\\n    --release\ncat Cargo.lock\nls");
        // `$` без пробела — не приглашение; продолжение тянется, пока строки кончаются на `\`.
        let k: Vec<LineKind> = console_lines("$x\n$ a \\\nb \\\nc\nd").iter().map(|l| l.kind).collect();
        assert_eq!(k, vec![K::Output, K::Command, K::Continuation, K::Continuation, K::Output]);
        // CRLF не мешает.
        assert_eq!(console_lines("$ ls\r\nfile\r\n").iter().map(|l| l.text).collect::<Vec<_>>(), vec!["ls", "file", ""]);
    }

    const RUSTC: &str = r#"   Compiling hangar v0.1.0 (C:\x\hangar)
error[E0425]: cannot find value `speed_km` in this scope
 --> src\main.rs:4:28
  |
4 |     println!("{aircraft}: {speed_km} km/h");
  |                            ^^^^^^^^
  |
help: a local variable with a similar name exists
  |
4 |     println!("{aircraft}: {speed_kmh} km/h");
  |                                    +
warning: unused variable: `fuel`
  = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.12s"#;

    #[test]
    fn cargo_and_rustc_output() {
        use Tone as T;
        let mut level = None;
        let all: Vec<Vec<(&str, Tone)>> =
            RUSTC.lines().map(|l| output_spans(l, &mut level).into_iter().map(|(r, t)| (&l[r], t)).collect()).collect();
        assert_eq!(all[0], vec![("Compiling", T::Status)]);
        assert_eq!(all[1], vec![("error[E0425]", T::Error), ("cannot find value `speed_km` in this scope", T::Message)]);
        assert_eq!(all[2], vec![("--> src\\main.rs:4:28", T::Gutter)]);
        assert_eq!(all[3], vec![("|", T::Gutter)]);
        assert_eq!(all[4], vec![("4 |", T::Gutter)]);
        assert_eq!(all[5], vec![("|", T::Gutter), ("^^^^^^^^", T::Error)]);
        assert_eq!(all[7], vec![("help", T::Hint), ("a local variable with a similar name exists", T::Message)]);
        assert_eq!(all[9], vec![("4 |", T::Gutter)]);
        assert_eq!(all[10], vec![("|", T::Gutter), ("+", T::Added)]);
        assert_eq!(all[11], vec![("warning", T::Warning), ("unused variable: `fuel`", T::Message)]);
        assert_eq!(all[12], vec![("=", T::Gutter), ("note", T::Hint)]);
        assert_eq!(all[13], vec![("Finished", T::Status)]);
        assert_eq!(level, None, "строка состояния сбрасывает уровень");
        // Карет под предупреждением — жёлтый; «For more information» — обычный текст.
        let mut level = None;
        output_spans("warning: unused import", &mut level);
        let l = "  |     ^^^^ help: remove it";
        assert_eq!(output_spans(l, &mut level).last().map(|(r, t)| (&l[r.clone()], *t)), Some(("^^^^ help: remove it", T::Warning)));
        let mut level = Some(T::Error);
        assert!(output_spans("For more information about this error, try `rustc --explain E0425`.", &mut level).is_empty());
        // Без диагностики `|` — просто текст, а пустая строка сбрасывает уровень.
        let mut level = None;
        assert!(output_spans("a | b", &mut level).is_empty());
        assert!(output_spans("Compiling", &mut level).is_empty(), "без продолжения — не состояние");
        let mut level = Some(T::Error);
        output_spans("", &mut level);
        assert_eq!(level, None);
    }

    fn log(line: &str, prev: Option<Level>) -> (Option<Level>, Vec<(&str, LogTone)>) {
        let l = log_line(line, prev);
        let covered: usize = l.spans.iter().map(|(r, _)| r.len()).sum();
        assert_eq!(covered, line.len(), "участки покрывают всю строку: {line}");
        (l.level, l.spans.into_iter().map(|(r, t)| (&line[r], t)).filter(|(s, _)| !s.trim().is_empty()).collect())
    }

    #[test]
    fn minecraft_log() {
        use LogTone as T;
        let (lv, spans) = log("[13:36:24] [main/WARN]: Reference map 'holo.refmap.json' could not be read", None);
        assert_eq!(lv, Some(Level::Warn));
        assert_eq!(
            spans,
            vec![
                ("[13:36:24]", T::Time),
                ("[main/", T::Group),
                ("WARN", T::Level(Level::Warn)),
                ("]", T::Group),
                (": ", T::Plain),
                ("Reference map 'holo.refmap.json' could not be read", T::Tinted(Level::Warn)),
            ]
        );
        let (lv, spans) = log("[13:36:25] [modloading-worker-0/INFO] [holo_rails/]: Loaded 12 rails", None);
        assert_eq!(lv, Some(Level::Info));
        assert_eq!(spans[1], ("[modloading-worker-0/", T::Group));
        assert_eq!(spans[2], ("INFO", T::Level(Level::Info)));
        assert_eq!(spans[4], ("[holo_rails/]", T::Group));
        assert_eq!(spans.last(), Some(&("Loaded 12 rails", T::Plain)));
        // Стек под ошибкой наследует её уровень.
        let (lv, _) = log("[13:36:26] [Render thread/ERROR]: Failed to load", None);
        assert_eq!(lv, Some(Level::Error));
        let stack = "\tat net.minecraft.Main.main(Main.java:12)";
        assert_eq!(log(stack, lv), (Some(Level::Error), vec![(stack, T::Tinted(Level::Error))]));
        assert_eq!(log("plain text", lv).0, None);
    }

    #[test]
    fn tracing_and_env_logger() {
        use LogTone as T;
        let (lv, spans) = log("2026-10-12T10:00:00.123Z  INFO hangar::net: listening on 0.0.0.0:8080", None);
        assert_eq!(lv, Some(Level::Info));
        assert_eq!(
            spans,
            vec![
                ("2026-10-12T10:00:00.123Z", T::Time),
                ("INFO", T::Level(Level::Info)),
                ("hangar::net", T::Group),
                (": ", T::Plain),
                ("listening on 0.0.0.0:8080", T::Plain),
            ]
        );
        let (lv, spans) = log("[2026-10-12T10:00:00Z WARN  hangar] fuel is low", None);
        assert_eq!(lv, Some(Level::Warn));
        assert_eq!(
            spans,
            vec![
                ("[2026-10-12T10:00:00Z", T::Time),
                ("WARN", T::Level(Level::Warn)),
                ("hangar]", T::Group),
                ("fuel is low", T::Tinted(Level::Warn)),
            ]
        );
        let (lv, spans) = log("2026-10-12 13:36:24,512 ERROR Engine stalled", None);
        assert_eq!(lv, Some(Level::Error));
        assert_eq!(spans[0], ("2026-10-12 13:36:24,512", T::Time));
        assert_eq!(spans.last(), Some(&("Engine stalled", T::Tinted(Level::Error))));
        assert_eq!(log("13:36:24.123 DEBUG cache warmed", None).1[1], ("DEBUG", T::Level(Level::Debug)));
        assert_eq!(log("FATAL: out of memory", None).0, Some(Level::Error));
        assert_eq!(log("TRACE enter", None).0, Some(Level::Trace));
        // Слово уровня — только целиком и заглавными; 12:30 без секунд — не время.
        assert_eq!(log("INFORMATION desk", None).0, None);
        assert_eq!(log("info: lower case", None).0, None);
        assert_eq!(log("12:30 lunch", None).1, vec![("12:30 lunch", T::Plain)]);
    }

    #[test]
    fn collapse_threshold() {
        let long = (1..=20).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        assert_eq!(collapse_at(&cb("console", &long)), Some(DEFAULT_LINES));
        assert_eq!(collapse_at(&cb("log", &long)), Some(DEFAULT_LINES));
        assert_eq!(collapse_at(&cb("rust", &long)), None, "обычный код — только с lines=N");
        assert_eq!(collapse_at(&CodeBlock { lines: Some(5), ..cb("rust", &long) }), Some(5));
        assert_eq!(collapse_at(&CodeBlock { lines: Some(30), ..cb("log", &long) }), None);
        assert_eq!(collapse_at(&CodeBlock { open: true, ..cb("log", &long) }), None);
        assert_eq!(collapse_at(&CodeBlock { lines: Some(0), ..cb("log", &long) }), Some(1));
        let twelve = (1..=12).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        assert_eq!(collapse_at(&cb("console", &twelve)), None, "ровно 12 строк не сворачиваем");
        assert_eq!(head(&long, 3), "1\n2\n3");
        assert_eq!(head("a", 3), "a");
        assert_eq!(head("a\nb", 0), "");
        assert_eq!(line_count(""), 1);
    }
}
