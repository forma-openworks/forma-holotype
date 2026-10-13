//! Экспорт в HTML: одна заметка (самодостаточный файл) или всё хранилище как сайт документации.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::diagram;
use crate::highlight;
use crate::markdown::{self, Align, Block, CodeBlock, Doc, Inline, LinkKind, Style};
use crate::term;
use crate::theme;
use crate::vault::Vault;
use forma_i18n::t;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Отдельный файл: стили внутри, картинки встроены как data:-URI.
    Single,
    /// Сайт: общие стили, картинки копируются рядом.
    Site,
}

struct Ctx<'a> {
    vault: &'a Vault,
    rel: &'a Path,
    doc: &'a Doc,
    mode: Mode,
    /// Префикс до корня сайта, например `../`.
    root: String,
    assets: &'a mut Vec<PathBuf>,
    depth: usize,
}

/// Экспорт одной заметки в самодостаточный HTML-файл.
pub fn export_note(vault: &Vault, idx: usize, out: &Path) -> io::Result<()> {
    let note = &vault.notes[idx];
    let doc = markdown::parse(&note.content);
    let mut assets = Vec::new();
    let mut ctx = Ctx { vault, rel: &note.rel, doc: &doc, mode: Mode::Single, root: String::new(), assets: &mut assets, depth: 0 };
    let body = render_note_body(&mut ctx, &note.name);
    let html = page(&note.name, &vault.name(), &format!("<style>{}</style>", stylesheet()), "", &body, doc.has_math, false);
    std::fs::write(out, html)
}

/// Экспорт всего хранилища в папку как статический сайт. Возвращает число страниц.
pub fn export_site(vault: &Vault, out_dir: &Path) -> io::Result<usize> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(out_dir.join("style.css"), stylesheet())?;
    let mut assets: Vec<PathBuf> = Vec::new();
    let any_math = vault.notes.iter().any(|n| n.content.contains('$'));

    for (i, note) in vault.notes.iter().enumerate() {
        let doc = markdown::parse(&note.content);
        let depth = note.rel.components().count().saturating_sub(1);
        let root = "../".repeat(depth);
        let mut ctx = Ctx { vault, rel: &note.rel, doc: &doc, mode: Mode::Site, root: root.clone(), assets: &mut assets, depth: 0 };
        let mut body = render_note_body(&mut ctx, &note.name);

        let backlinks = vault.backlinks(i);
        if !backlinks.is_empty() {
            body.push_str(&format!("<section class=\"backlinks\"><h2>{}</h2><ul>", esc(&t!("export-backlinks"))));
            for (j, _) in backlinks {
                let other = &vault.notes[j];
                let _ = write!(body, "<li><a href=\"{}\">{}</a></li>", href_for(&root, &other.rel), esc(&other.name));
            }
            body.push_str("</ul></section>");
        }

        let nav = nav_html(vault, &root, Some(&note.rel));
        let head = format!("<link rel=\"stylesheet\" href=\"{root}style.css\">");
        let html = page(&note.name, &vault.name(), &head, &nav, &body, any_math, true);
        let out = out_dir.join(note.rel.with_extension("html"));
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(out, html)?;
    }

    // Главная страница со списком всех заметок.
    let mut index = String::new();
    let _ = write!(index, "<h1>{}</h1><p class=\"muted\">{}</p><div class=\"cards\">", esc(&vault.name()), esc(&t!("export-notes-count", count = vault.notes.len())));
    for note in &vault.notes {
        let folder = note.rel.parent().map(|p| p.to_string_lossy().replace('\\', " / ")).unwrap_or_default();
        let preview = preview_text(&note.content);
        let _ = write!(
            index,
            "<a class=\"card\" href=\"{}\"><div class=\"card-folder\">{}</div><div class=\"card-title\">{}</div><div class=\"card-text\">{}</div></a>",
            href_for("", &note.rel),
            esc(&folder),
            esc(&note.name),
            esc(&strip_md(&preview))
        );
    }
    index.push_str("</div>");
    let nav = nav_html(vault, "", None);
    let html = page(&vault.name(), &vault.name(), "<link rel=\"stylesheet\" href=\"style.css\">", &nav, &index, false, true);
    std::fs::write(out_dir.join("index.html"), html)?;

    assets.sort();
    assets.dedup();
    for rel in assets {
        let dst = out_dir.join(&rel);
        if let Some(dir) = dst.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let _ = std::fs::copy(vault.abs(&rel), dst);
    }
    Ok(vault.notes.len() + 1)
}

fn render_note_body(ctx: &mut Ctx<'_>, title: &str) -> String {
    let mut out = String::new();
    let doc = ctx.doc;
    // Заголовок страницы: собственный H1 заметки или имя файла. Свойства — сразу под ним.
    let rest = match doc.blocks.split_first() {
        Some((first @ Block::Heading { level: 1, .. }, rest)) => {
            block(ctx, &mut out, first);
            rest
        }
        _ => {
            let _ = write!(out, "<h1 class=\"note-title\">{}</h1>", esc(title));
            &doc.blocks[..]
        }
    };
    if !doc.properties.is_empty() {
        out.push_str("<table class=\"properties\">");
        for (k, v) in &doc.properties {
            let _ = write!(out, "<tr><th>{}</th><td>{}</td></tr>", esc(k), esc(v));
        }
        out.push_str("</table>");
    }
    blocks(ctx, &mut out, rest);
    out
}

fn page(title: &str, vault: &str, head: &str, nav: &str, body: &str, math: bool, sidebar: bool) -> String {
    let katex = if math {
        r#"<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css">
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/contrib/auto-render.min.js" onload="renderMathInElement(document.body,{delimiters:[{left:'\\[',right:'\\]',display:true},{left:'\\(',right:'\\)',display:false}],throwOnError:false})"></script>"#
    } else {
        ""
    };
    let (layout_class, aside) = if sidebar {
        ("with-sidebar", format!("<aside class=\"sidebar\">{nav}</aside>"))
    } else {
        ("single", String::new())
    };
    let lang = forma_i18n::language();
    format!(
        r#"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — {vault}</title>
{head}
{katex}
</head>
<body class="{layout_class}">
{aside}
<main><article class="note">{body}</article></main>
<script>
const f=document.getElementById('nav-filter');
if(f){{f.addEventListener('input',()=>{{const q=f.value.toLowerCase();document.querySelectorAll('.nav a').forEach(a=>{{a.parentElement.style.display=a.textContent.toLowerCase().includes(q)?'':'none';}});}});}}
document.querySelectorAll('.code').forEach(block=>{{
  const tabs=[...block.querySelectorAll('.code-tabs .code-tab')];
  const bodies=[...block.querySelectorAll('.code-body')];
  const title=block.querySelector('.code-title');
  const copy=block.querySelector('.code-copy');
  tabs.forEach((t,i)=>t.addEventListener('click',()=>{{
    tabs.forEach(x=>x.classList.toggle('active',x===t));
    bodies.forEach((b,j)=>b.hidden=j!==i);
    block.querySelectorAll('.code-head [data-tab]').forEach(e=>e.hidden=e.dataset.tab!=i);
    if(title)title.textContent=t.dataset.title||'';
    if(copy&&bodies[i])copy.textContent=bodies[i].dataset.label;
  }}));
  const views=[...block.querySelectorAll('.view-tab')];
  views.forEach((v,i)=>v.addEventListener('click',()=>{{
    views.forEach(x=>x.classList.toggle('active',x===v));
    block.querySelectorAll('.diagram').forEach(d=>d.hidden=i===1);
    block.querySelectorAll('.diagram-src').forEach(d=>d.hidden=i===0);
  }}));
  if(copy)copy.addEventListener('click',()=>{{
    const body=bodies.find(b=>!b.hidden)||bodies[0];
    navigator.clipboard.writeText(body.dataset.copy??body.querySelector('.src').innerText);
    copy.textContent=copy.dataset.done;
    setTimeout(()=>copy.textContent=body.dataset.label,1500);
  }});
  // Длинные блоки сворачиваем только со скриптом: без него всё видно целиком.
  block.querySelectorAll('.code-more').forEach(btn=>{{
    const body=btn.parentElement;
    const label=btn.querySelector('span');
    body.classList.add('collapsed');
    btn.hidden=false;
    btn.addEventListener('click',()=>{{
      const collapsed=body.classList.toggle('collapsed');
      label.textContent=collapsed?btn.dataset.more:btn.dataset.less;
    }});
  }});
}});
</script>
</body>
</html>"#,
        title = esc(title),
        vault = esc(vault),
    )
}

fn nav_html(vault: &Vault, root: &str, current: Option<&Path>) -> String {
    // Строим дерево папок.
    #[derive(Default)]
    struct Dir {
        dirs: BTreeMap<String, Dir>,
        notes: Vec<(String, PathBuf)>,
    }
    let mut tree = Dir::default();
    for n in &vault.notes {
        let mut d = &mut tree;
        if let Some(parent) = n.rel.parent() {
            for c in parent.components() {
                d = d.dirs.entry(c.as_os_str().to_string_lossy().into_owned()).or_default();
            }
        }
        d.notes.push((n.name.clone(), n.rel.clone()));
    }
    fn render(d: &Dir, root: &str, current: Option<&Path>, out: &mut String) {
        out.push_str("<ul>");
        for (name, sub) in &d.dirs {
            let open = current.is_some_and(|c| c.to_string_lossy().contains(name.as_str()));
            let _ = write!(out, "<li><details{}><summary>{}</summary>", if open { " open" } else { "" }, esc(name));
            render(sub, root, current, out);
            out.push_str("</details></li>");
        }
        for (name, rel) in &d.notes {
            let active = current == Some(rel.as_path());
            let _ = write!(
                out,
                "<li><a href=\"{}\"{}>{}</a></li>",
                href_for(root, rel),
                if active { " class=\"active\"" } else { "" },
                esc(name)
            );
        }
        out.push_str("</ul>");
    }
    let mut out = String::new();
    let _ = write!(
        out,
        "<a class=\"brand\" href=\"{root}index.html\">{}</a><input id=\"nav-filter\" placeholder=\"{}\"><nav class=\"nav\">",
        esc(&vault.name()),
        esc(&t!("export-filter"))
    );
    render(&tree, root, current, &mut out);
    out.push_str("</nav>");
    out
}

fn href_for(root: &str, rel: &Path) -> String {
    let p = rel.with_extension("html").to_string_lossy().replace('\\', "/");
    format!("{root}{}", url_encode_path(&p))
}

fn url_encode_path(p: &str) -> String {
    let mut out = String::new();
    for b in p.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// Пара первых строк обычного текста заметки (без шапки, заголовков и кода) для карточки.
fn preview_text(content: &str) -> String {
    let mut lines = content.lines().peekable();
    if lines.peek() == Some(&"---") {
        lines.next();
        for l in lines.by_ref() {
            if l == "---" {
                break;
            }
        }
    }
    let mut in_code = false;
    let text: Vec<&str> = lines
        .filter(|l| {
            let t = l.trim();
            if t.starts_with("```") {
                in_code = !in_code;
                return false;
            }
            !in_code && !t.is_empty() && !t.starts_with('#') && !t.starts_with('|') && !t.starts_with("$$") && !t.starts_with("![")
        })
        .take(2)
        .collect();
    text.join(" ").chars().take(140).collect()
}

fn strip_md(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, '*' | '_' | '`' | '[' | ']' | '#' | '>' | '=')).collect()
}

fn blocks(ctx: &mut Ctx<'_>, out: &mut String, list: &[Block]) {
    for b in list {
        block(ctx, out, b);
    }
}

fn block(ctx: &mut Ctx<'_>, out: &mut String, b: &Block) {
    match b {
        Block::Heading { level, inlines: items, text } => {
            let _ = write!(out, "<h{level} id=\"{}\">", esc(&markdown::slug(text)));
            inlines(ctx, out, items);
            let _ = write!(out, "</h{level}>");
        }
        Block::Paragraph(items) => {
            // Абзац из одних картинок не оборачиваем в <p>.
            out.push_str("<p>");
            inlines(ctx, out, items);
            out.push_str("</p>");
        }
        Block::Code(cb) => code_html(out, std::slice::from_ref(cb)),
        Block::CodeGroup(group) => code_html(out, group),
        Block::Quote { blocks: list, author } => {
            out.push_str("<blockquote class=\"quote\"><div class=\"quote-body\">");
            blocks(ctx, out, list);
            out.push_str("</div>");
            if let Some(author) = author {
                out.push_str("<footer>— ");
                inlines(ctx, out, author);
                out.push_str("</footer>");
            }
            out.push_str("</blockquote>");
        }
        Block::Callout { kind, title, folded, body } => {
            let st = theme::callout(kind);
            let rgb = format!("{}, {}, {}", st.color.r(), st.color.g(), st.color.b());
            let mut title_html = String::new();
            if title.is_empty() {
                title_html.push_str(&esc(&st.title));
            } else {
                inlines(ctx, &mut title_html, title);
            }
            let head = format!("<span class=\"callout-icon\">{}</span><span>{}</span>", st.icon, title_html);
            match folded {
                Some(f) => {
                    let _ = write!(
                        out,
                        "<details class=\"callout\" style=\"--c: {rgb}\"{}><summary class=\"callout-title\">{head}</summary><div class=\"callout-body\">",
                        if *f { "" } else { " open" }
                    );
                    blocks(ctx, out, body);
                    out.push_str("</div></details>");
                }
                None => {
                    let _ = write!(out, "<div class=\"callout\" style=\"--c: {rgb}\"><div class=\"callout-title\">{head}</div><div class=\"callout-body\">");
                    blocks(ctx, out, body);
                    out.push_str("</div></div>");
                }
            }
        }
        Block::List { start, items } => {
            let tag = if start.is_some() { "ol" } else { "ul" };
            match start {
                Some(s) if *s != 1 => {
                    let _ = write!(out, "<ol start=\"{s}\">");
                }
                _ => {
                    let _ = write!(out, "<{tag}>");
                }
            }
            for item in items {
                match &item.task {
                    Some(t) => {
                        let _ = write!(
                            out,
                            "<li class=\"task{}\"><input type=\"checkbox\" disabled{}> ",
                            if t.checked { " done" } else { "" },
                            if t.checked { " checked" } else { "" }
                        );
                    }
                    None => out.push_str("<li>"),
                }
                // В «плотных» пунктах не нужен лишний <p>.
                if let [Block::Paragraph(items)] = item.blocks.as_slice() {
                    inlines(ctx, out, items);
                } else {
                    blocks(ctx, out, &item.blocks);
                }
                out.push_str("</li>");
            }
            let _ = write!(out, "</{tag}>");
        }
        Block::Table { aligns, head, rows, cell_aligns } => {
            // Выравнивание ячейки важнее выравнивания колонки.
            let align_attr = |ri: usize, j: usize| {
                let own = cell_aligns.get(ri).and_then(|r| r.get(j)).copied().unwrap_or(Align::None);
                let a = if own == Align::None { aligns.get(j).copied().unwrap_or(Align::None) } else { own };
                match a {
                    Align::Left => " style=\"text-align:left\"",
                    Align::Center => " style=\"text-align:center\"",
                    Align::Right => " style=\"text-align:right\"",
                    Align::None => "",
                }
            };
            out.push_str("<div class=\"table-wrap\"><table><thead><tr>");
            for (j, cell) in head.iter().enumerate() {
                let _ = write!(out, "<th{}>", align_attr(0, j));
                inlines(ctx, out, cell);
                out.push_str("</th>");
            }
            out.push_str("</tr></thead><tbody>");
            for (ri, row) in rows.iter().enumerate() {
                out.push_str("<tr>");
                for (j, cell) in row.iter().enumerate() {
                    let _ = write!(out, "<td{}>", align_attr(ri + 1, j));
                    inlines(ctx, out, cell);
                    out.push_str("</td>");
                }
                out.push_str("</tr>");
            }
            out.push_str("</tbody></table></div>");
        }
        Block::Rule => out.push_str("<hr>"),
        Block::Html(html) => out.push_str(html),
        Block::Math(tex) => {
            let _ = write!(out, "<div class=\"math\">\\[{}\\]</div>", esc(tex));
        }
        Block::Footnote { label, number, blocks: list } => {
            let _ = write!(
                out,
                "<div class=\"footnote\" id=\"fn-{}\"><span class=\"footnote-num\">{number}.</span><div>",
                esc(&markdown::slug(label))
            );
            blocks(ctx, out, list);
            out.push_str("</div></div>");
        }
    }
}

fn inlines(ctx: &mut Ctx<'_>, out: &mut String, items: &[Inline]) {
    let mut open_link: Option<u32> = None;
    for item in items {
        let link = match item {
            Inline::Text(_, st) => st.link,
            _ => None,
        };
        if open_link != link {
            if open_link.is_some() {
                out.push_str("</a>");
            }
            if let Some(li) = link {
                out.push_str(&link_open(ctx, li));
            }
            open_link = link;
        }
        match item {
            Inline::Text(t, st) => text(out, t, st),
            Inline::SoftBreak => out.push(' '),
            Inline::HardBreak => out.push_str("<br>"),
            Inline::Image { src, alt, width, wiki } => image(ctx, out, src, alt, *width, *wiki),
        }
    }
    if open_link.is_some() {
        out.push_str("</a>");
    }
}

fn link_open(ctx: &Ctx<'_>, li: u32) -> String {
    let Some(link) = ctx.doc.links.get(li as usize) else { return "<a>".into() };
    match link.kind {
        LinkKind::Url => format!("<a href=\"{}\" target=\"_blank\" rel=\"noopener\">", esc(&link.target)),
        LinkKind::Tag => "<a class=\"tag\">".into(),
        LinkKind::Footnote => format!("<a class=\"footnote-ref\" href=\"#fn-{}\">", esc(&markdown::slug(&link.target))),
        LinkKind::Wiki => {
            let heading = markdown::link_heading_part(&link.target).map(|h| format!("#{}", markdown::slug(h))).unwrap_or_default();
            if link.target.starts_with('#') {
                return format!("<a href=\"{}\">", esc(&heading));
            }
            match ctx.vault.resolve(&link.target, Some(ctx.rel)) {
                Some(j) => {
                    let target = &ctx.vault.notes[j].rel;
                    let href = match ctx.mode {
                        Mode::Site => href_for(&ctx.root, target),
                        Mode::Single => url_encode_path(&target.with_extension("html").to_string_lossy().replace('\\', "/")),
                    };
                    format!("<a class=\"internal\" href=\"{href}{}\">", esc(&heading))
                }
                None => "<a class=\"internal unresolved\">".into(),
            }
        }
    }
}

fn text(out: &mut String, t: &str, st: &Style) {
    if st.math {
        let _ = write!(out, "<span class=\"math-inline\">\\({}\\)</span>", esc(t));
        return;
    }
    if st.footnote {
        let _ = write!(out, "<sup>{}</sup>", esc(t));
        return;
    }
    if st.kbd {
        let _ = write!(out, "<kbd>{}</kbd>", esc(t));
        return;
    }
    let tint = st.tint.map(|c| format!(" style=\"--tint: {}, {}, {}\"", c[0], c[1], c[2])).unwrap_or_default();
    if st.code && st.badge {
        let _ = write!(out, "<span class=\"badge\"{tint}>{}</span>", esc(t));
        return;
    }
    let mut open: Vec<String> = Vec::new();
    if st.code {
        open.push(format!("code class=\"ic\"{tint}"));
    }
    if st.bold {
        open.push("strong".into());
    }
    if st.italic {
        open.push("em".into());
    }
    if st.strike {
        open.push("del".into());
    }
    if st.mark {
        open.push(format!("mark{tint}"));
    }
    for tag in &open {
        let _ = write!(out, "<{tag}>");
    }
    out.push_str(&esc(t));
    for tag in open.iter().rev() {
        let name = tag.split(' ').next().unwrap_or(tag);
        let _ = write!(out, "</{name}>");
    }
}

/// Значки шапки консоли и лога (Lucide, ISC) — те же, что в программе.
const CONSOLE_ICON: &str = include_str!("../icons/lucide/square-terminal.svg");
const LOG_ICON: &str = include_str!("../icons/lucide/logs.svg");
const CHEVRON: &str = r#"<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>"#;

/// Подпись блока в шапке: `title=…`, у лога — «log», у диаграммы — её вид.
fn code_title(cb: &CodeBlock, diagram: Option<diagram::Kind>, tabs: bool) -> String {
    if let Some(t) = &cb.title {
        return t.clone();
    }
    match term::kind(&cb.lang) {
        term::Kind::Log => "log".to_owned(),
        term::Kind::Console => String::new(),
        term::Kind::Code => match diagram {
            Some(k) => k.title().to_owned(),
            None if tabs => String::new(),
            None => highlight::display_name(&cb.lang),
        },
    }
}

/// Блок кода или группа блоков с вкладками: шапка, номера строк, подсветка. Консоль
/// и лог — со своей шапкой и раскраской. Длинный блок сворачивает скрипт страницы,
/// так что без JS он виден целиком.
fn code_html(out: &mut String, group: &[CodeBlock]) {
    if group.is_empty() {
        return;
    }
    let tabs = group.len() > 1 || group[0].tab.is_some();
    let hidden = |i: usize| if i == 0 { "" } else { " hidden" };
    out.push_str("<div class=\"code\"><div class=\"code-head\">");
    if tabs {
        out.push_str("<div class=\"code-tabs\">");
        for (i, cb) in group.iter().enumerate() {
            let label = cb.tab.clone().unwrap_or_else(|| highlight::display_name(&cb.lang));
            let title = if term::kind(&cb.lang) == term::Kind::Code { cb.title.clone().unwrap_or_default() } else { code_title(cb, None, true) };
            let _ = write!(
                out,
                "<button class=\"code-tab{}\" data-title=\"{}\">{}</button>",
                if i == 0 { " active" } else { "" },
                esc(&title),
                esc(&label)
            );
        }
        out.push_str("</div>");
    }
    // Диаграммы рисуем сразу в SVG; исходный код остаётся доступен по кнопке «Код».
    let diagrams: Vec<Option<(diagram::Kind, String)>> = group
        .iter()
        .map(|cb| {
            if !diagram::is_diagram(&cb.lang) {
                return None;
            }
            diagram::render(&cb.lang, &cb.code, &diagram::Approx).ok().map(|(k, scene)| (k, scene.to_svg()))
        })
        .collect();
    // Значок и плашка у консоли и лога — свои у каждой вкладки.
    for (i, cb) in group.iter().enumerate() {
        let icon = match term::kind(&cb.lang) {
            term::Kind::Console => CONSOLE_ICON,
            term::Kind::Log => LOG_ICON,
            term::Kind::Code => continue,
        };
        // Без строки лицензии и лишних пробелов: значок встраивается в каждый блок.
        let icon = icon[icon.find("<svg").unwrap_or(0)..].split_whitespace().collect::<Vec<_>>().join(" ");
        let _ = write!(out, "<span class=\"code-icon\" data-tab=\"{i}\"{}>{icon}</span>", hidden(i));
    }
    let first_title = code_title(&group[0], diagrams[0].as_ref().map(|(k, _)| *k), tabs);
    let _ = write!(out, "<span class=\"code-title\">{}</span>", esc(&first_title));
    for (i, cb) in group.iter().enumerate() {
        let badge = match term::kind(&cb.lang) {
            term::Kind::Console => "bash".to_owned(),
            term::Kind::Log => t!("code-lines", count = term::line_count(&cb.code)),
            term::Kind::Code => continue,
        };
        let _ = write!(out, "<span class=\"code-badge\" data-tab=\"{i}\"{}>{}</span>", hidden(i), esc(&badge));
    }
    if diagrams.iter().any(Option::is_some) {
        let _ = write!(out, "<span class=\"view-tabs\"><button class=\"code-tab view-tab active\">{}</button><button class=\"code-tab view-tab\">{}</button></span>", esc(&t!("code-view-diagram")), esc(&t!("code-view-code")));
    }
    let copy_label = |cb: &CodeBlock| {
        let label = match term::kind(&cb.lang) {
            term::Kind::Console => t!("code-copy-commands"),
            term::Kind::Log => t!("code-copy-log"),
            term::Kind::Code => t!("code-copy"),
        };
        format!("⧉ {label}")
    };
    let _ = write!(
        out,
        "<button class=\"code-copy\" data-done=\"✔ {}\">{}</button></div>",
        esc(&t!("code-copied")),
        esc(&copy_label(&group[0]))
    );
    for (i, cb) in group.iter().enumerate() {
        let label = esc(&copy_label(cb));
        if let Some((_, svg)) = &diagrams[i] {
            let _ = write!(out, "<div class=\"code-body\" data-label=\"{label}\"{}><div class=\"diagram\">{svg}</div>", hidden(i));
            let _ = write!(out, "<pre class=\"src diagram-src\" hidden><code>{}</code></pre></div>", esc(&cb.code));
            continue;
        }
        let kind = term::kind(&cb.lang);
        let total = term::line_count(&cb.code);
        let mut style = String::new();
        let mut attrs = String::new();
        if let Some(n) = term::collapse_at(cb) {
            let _ = write!(attrs, " data-shown=\"{n}\"");
            let _ = write!(style, "--shown:{n};");
        }
        let class = match kind {
            term::Kind::Code => "code-body",
            term::Kind::Console => {
                let _ = write!(attrs, " data-copy=\"{}\"", esc(&term::commands_text(&cb.code)));
                "code-body term-console"
            }
            term::Kind::Log => {
                let _ = write!(attrs, " data-copy=\"{}\"", esc(&cb.code));
                let _ = write!(style, "--gw:{}ch;", total.to_string().len());
                "code-body term-log"
            }
        };
        if !style.is_empty() {
            let _ = write!(attrs, " style=\"{style}\"");
        }
        let _ = write!(out, "<div class=\"{class}\" data-label=\"{label}\"{attrs}{}>", hidden(i));
        match kind {
            term::Kind::Code => {
                if total >= 2 {
                    let numbers: Vec<String> = (1..=total).map(|n| n.to_string()).collect();
                    let _ = write!(out, "<pre class=\"gutter\" aria-hidden=\"true\">{}</pre>", numbers.join("\n"));
                }
                out.push_str("<pre class=\"src\"><code>");
                match highlight::html(&cb.code, &cb.lang) {
                    Some(html) => out.push_str(&html),
                    None => out.push_str(&esc(&cb.code)),
                }
                out.push_str("</code></pre>");
            }
            term::Kind::Console => console_html(out, cb),
            term::Kind::Log => log_html(out, &cb.code),
        }
        if let Some(n) = term::collapse_at(cb) {
            let more = esc(&t!("code-show-more", count = total - n));
            let less = esc(&t!("code-collapse"));
            let _ = write!(out, "<button class=\"code-more\" hidden data-more=\"{more}\" data-less=\"{less}\">{CHEVRON}<span>{more}</span></button>");
        }
        out.push_str("</div>");
    }
    out.push_str("</div>");
}

/// Сеанс консоли: приглашение — пустой `<span class="pr">`, его рисует CSS, поэтому при
/// копировании текста страницы `$` не попадает в буфер.
fn console_html(out: &mut String, cb: &CodeBlock) {
    let lines = term::console_lines(&cb.code);
    let commands = term::commands(&lines);
    let mut next_cmd = commands.iter();
    let cwd = cb.cwd.as_deref().map(|c| format!(" data-cwd=\"{}\"", esc(c))).unwrap_or_default();
    let mut level = None;
    let mut i = 0;
    out.push_str("<pre class=\"src\"><code>");
    while i < lines.len() {
        if i > 0 {
            out.push('\n');
        }
        let line = lines[i];
        match line.kind {
            term::LineKind::Command => {
                let Some(c) = next_cmd.next() else { break };
                level = None;
                let code = highlight::html(&c.text, "bash").unwrap_or_else(|| esc(&c.text));
                let _ = write!(out, "<span class=\"cmd\"><span class=\"pr\"{cwd}></span>{code}</span>");
                i = c.lines.end;
                continue;
            }
            term::LineKind::Comment => {
                let _ = write!(out, "<span class=\"cmt\">{}</span>", esc(line.text));
            }
            term::LineKind::Continuation | term::LineKind::Output => {
                out.push_str("<span class=\"out\">");
                let mut at = 0;
                for (r, tone) in term::output_spans(line.text, &mut level) {
                    let class = match tone {
                        term::Tone::Status => "t-st",
                        term::Tone::Error => "t-err",
                        term::Tone::Warning => "t-warn",
                        term::Tone::Hint => "t-hint",
                        term::Tone::Message => "t-msg",
                        term::Tone::Gutter => "t-gut",
                        term::Tone::Added => "t-add",
                    };
                    let _ = write!(out, "{}<span class=\"{class}\">{}</span>", esc(&line.text[at..r.start]), esc(&line.text[r.clone()]));
                    at = r.end;
                }
                out.push_str(&esc(&line.text[at..]));
                out.push_str("</span>");
            }
        }
        i += 1;
    }
    out.push_str("</code></pre>");
}

/// Лог: каждая строка — блок с номером в `::before`; длинные строки переносятся под текст.
fn log_html(out: &mut String, code: &str) {
    out.push_str("<pre class=\"src\"><code>");
    let mut prev = None;
    for (i, line) in term::lines(code).enumerate() {
        let parsed = term::log_line(line, prev);
        prev = parsed.level;
        let _ = write!(out, "<span class=\"ll\" data-n=\"{}\">", i + 1);
        for (r, tone) in parsed.spans {
            let class = match tone {
                term::LogTone::Plain => None,
                term::LogTone::Time => Some("l-time"),
                term::LogTone::Group => Some("l-grp"),
                term::LogTone::Level(term::Level::Trace) => Some("l-lv l-trace"),
                term::LogTone::Level(term::Level::Debug) => Some("l-lv l-debug"),
                term::LogTone::Level(term::Level::Info) => Some("l-lv l-info"),
                term::LogTone::Level(term::Level::Warn) => Some("l-lv l-warn"),
                term::LogTone::Level(term::Level::Error) => Some("l-lv l-err"),
                term::LogTone::Tinted(term::Level::Error) => Some("l-te"),
                term::LogTone::Tinted(_) => Some("l-tw"),
            };
            match class {
                Some(c) => {
                    let _ = write!(out, "<span class=\"{c}\">{}</span>", esc(&line[r]));
                }
                None => out.push_str(&esc(&line[r])),
            }
        }
        out.push_str("</span>");
    }
    out.push_str("</code></pre>");
}

fn image(ctx: &mut Ctx<'_>, out: &mut String, src: &str, alt: &str, width: Option<f32>, wiki: bool) {
    let width_attr = width.map(|w| format!(" width=\"{w}\"")).unwrap_or_default();
    if src.starts_with("http://") || src.starts_with("https://") {
        let _ = write!(out, "<img src=\"{}\" alt=\"{}\"{width_attr}>", esc(src), esc(alt));
        return;
    }
    if wiki && !markdown::is_image_path(src) {
        // Встроенная заметка.
        if let Some(j) = ctx.vault.resolve(src, Some(ctx.rel)) {
            let note = &ctx.vault.notes[j];
            let _ = write!(out, "<div class=\"embed\"><div class=\"embed-title\">↪ {}</div>", esc(&note.name));
            if ctx.depth < 1 && note.rel != ctx.rel {
                let doc = markdown::parse(&note.content);
                let mut inner = Ctx {
                    vault: ctx.vault,
                    rel: &note.rel,
                    doc: &doc,
                    mode: ctx.mode,
                    root: ctx.root.clone(),
                    assets: ctx.assets,
                    depth: ctx.depth + 1,
                };
                blocks(&mut inner, out, &doc.blocks);
            }
            out.push_str("</div>");
        }
        return;
    }
    let Some(abs) = ctx.vault.resolve_attachment(src, ctx.rel) else {
        let _ = write!(out, "<span class=\"missing\">⚠ {}</span>", esc(src));
        return;
    };
    let src_attr = match ctx.mode {
        Mode::Single => match std::fs::read(&abs) {
            Ok(bytes) => format!("data:{};base64,{}", mime_for(&abs), base64(&bytes)),
            Err(_) => String::new(),
        },
        Mode::Site => match abs.strip_prefix(&ctx.vault.root) {
            Ok(rel) => {
                ctx.assets.push(rel.to_path_buf());
                format!("{}{}", ctx.root, url_encode_path(&rel.to_string_lossy().replace('\\', "/")))
            }
            Err(_) => String::new(),
        },
    };
    let _ = write!(out, "<img src=\"{src_attr}\" alt=\"{}\"{width_attr}>", esc(alt));
}

fn mime_for(p: &Path) -> &'static str {
    match p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn stylesheet() -> String {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        format!(
            "{BASE_CSS}\n/* Подсветка кода */\n{}\n.code pre, .code code {{ background: transparent !important; }}\n",
            highlight::css()
        )
    })
    .clone()
}

const BASE_CSS: &str = r#"
:root {
  --bg: #ffffff; --bg-side: #f6f7f9; --text: #282c34; --muted: #5c6370; --faint: #969ca8;
  --heading: #111318; --border: #dee1e7; --accent: #6c54e6; --code-bg: #f8f9fb; --code-head: #f0f2f6;
  --code-border: #dce0e7; --gutter: #a0a6b2; --kbd-bg: #fcfcfd; --kbd-border: #ced3db; --kbd-shadow: #ced3db;
  --badge-bg: #e5e7ec; --badge-text: #373f4e; --table-bg: #ffffff; --table-head: #f4f5f8; --table-line: #e2e5eb;
  --mono: "Cascadia Code", "JetBrains Mono", Consolas, monospace;
  --dg-node-fill: #f2f0fd; --dg-node-stroke: #6c54e6; --dg-text: #282c34; --dg-muted: #5c6370; --dg-edge: #6e747f;
  --dg-bg: #f8f9fb; --dg-cluster-fill: #f6f5fe; --dg-cluster-stroke: #c1bcf0; --dg-note-fill: #fcf1d9;
  --dg-note-stroke: #f7c353; --dg-accent: #6c54e6; --dg-strong: #111318; --dg-actor-fill: #e9e6fb;
  --t-ok: #16a34a; --t-warn: #ca8a04; --t-err: #dc2626; --t-info: #2563eb; --t-hint: #0e7490;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1c1e24; --bg-side: #16181d; --text: #dcdfe4; --muted: #a0a6b2; --faint: #6c7380;
    --heading: #f4f5f8; --border: #323640; --accent: #9684ff; --code-bg: #0f1117; --code-head: #1d2029;
    --code-border: #333744; --gutter: #565e72; --kbd-bg: #30343e; --kbd-border: #4e5463; --kbd-shadow: #14161b;
    --badge-bg: #3a3e4a; --badge-text: #e2e5eb; --table-bg: #21242c; --table-head: #191b22; --table-line: #343844;
    --dg-node-fill: #25233c; --dg-node-stroke: #8273dc; --dg-text: #dcdfe4; --dg-muted: #a0a6b2; --dg-edge: #8f949f;
    --dg-bg: #0f1117; --dg-cluster-fill: #161723; --dg-cluster-stroke: #4b4a70; --dg-note-fill: #342b1a;
    --dg-note-stroke: #a57b22; --dg-accent: #9684ff; --dg-strong: #f4f5f8; --dg-actor-fill: #2d2a4a;
    --t-ok: #4ade80; --t-warn: #fbbf24; --t-err: #f87171; --t-info: #7aa2f7; --t-hint: #7dcfff;
  }
}
* { box-sizing: border-box; }
html { font-size: 16.5px; }
body { margin: 0; background: var(--bg); color: var(--text); line-height: 1.68;
  font-family: "Segoe UI", system-ui, -apple-system, "Noto Sans", sans-serif; }
body.with-sidebar { display: grid; grid-template-columns: 280px 1fr; min-height: 100vh; }
.sidebar { background: var(--bg-side); border-right: 1px solid var(--border); padding: 20px 14px;
  position: sticky; top: 0; height: 100vh; overflow-y: auto; font-size: .93rem; }
.brand { display: block; font-weight: 700; font-size: 1.1rem; color: var(--heading); text-decoration: none; margin: 0 6px 12px; }
#nav-filter { width: 100%; padding: 7px 10px; border-radius: 8px; border: 1px solid var(--border);
  background: var(--bg); color: var(--text); margin-bottom: 10px; font: inherit; }
.nav ul { list-style: none; margin: 0; padding-left: 12px; }
.nav > ul { padding-left: 0; }
.nav a { display: block; padding: 3px 8px; border-radius: 6px; color: var(--muted); text-decoration: none; }
.nav a:hover { background: var(--border); color: var(--text); }
.nav a.active { background: color-mix(in srgb, var(--accent) 18%, transparent); color: var(--heading); font-weight: 600; }
.nav summary { cursor: pointer; padding: 3px 8px; color: var(--faint); font-weight: 600; }
main { padding: 48px 32px 96px; }
.note { max-width: 780px; margin: 0 auto; }
h1, h2, h3, h4, h5, h6 { color: var(--heading); line-height: 1.3; margin: 1.6em 0 .5em; }
h1 { font-size: 2.1rem; padding-bottom: .3em; border-bottom: 1px solid var(--border); }
h2 { font-size: 1.6rem; padding-bottom: .25em; border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent); }
h3 { font-size: 1.3rem; } h6 { color: var(--muted); }
.note > :first-child { margin-top: 0; }
a { color: var(--accent); }
a.internal { text-decoration: none; } a.internal:hover { text-decoration: underline; }
a.unresolved { opacity: .6; }
a.tag { background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent);
  padding: 1px 7px; border-radius: 999px; font-size: .9em; text-decoration: none; }
code, pre { font-family: "Cascadia Code", "JetBrains Mono", Consolas, monospace; }
code.ic { --tint: 176, 163, 255; font-size: .86em; color: rgb(var(--tint)); background: rgba(var(--tint), .1);
  border: 1px solid rgba(var(--tint), .32); padding: .08em .4em; border-radius: 5px; white-space: nowrap; }
@media (prefers-color-scheme: light) { code.ic { --tint: 108, 84, 230; } }
kbd { font: 600 .8em "Segoe UI", system-ui, sans-serif; color: var(--heading); background: var(--kbd-bg);
  border: 1px solid var(--kbd-border); border-radius: 5px; box-shadow: 0 1.5px 0 var(--kbd-shadow); padding: .12em .5em; }
.badge { --tint: 156, 163, 175; font-size: .78em; font-weight: 600; padding: .2em .65em; border-radius: 6px;
  background: var(--badge-bg); color: var(--badge-text); white-space: nowrap; }
.badge[style] { background: rgba(var(--tint), .22); color: rgb(var(--tint)); }
.code { background: var(--code-bg); border: 1px solid var(--code-border); border-radius: 12px; margin: 1.2em 0; overflow: hidden; }
.code-head { display: flex; align-items: center; gap: 8px; padding: 9px 14px 9px 12px; background: var(--code-head);
  border-bottom: 1px solid var(--code-border); }
.code-tabs { display: flex; gap: 6px; margin-right: 6px; }
.code-tab { font: 600 .82rem "Segoe UI", system-ui, sans-serif; color: var(--muted); background: transparent;
  border: 1px solid var(--code-border); border-radius: 8px; padding: 4px 12px; cursor: pointer; transition: all .15s; }
.code-tab:hover { color: var(--heading); background: color-mix(in srgb, var(--border) 50%, transparent); }
.code-tab.active { color: var(--heading); border-color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); }
.code-title { font: .8rem "Cascadia Code", Consolas, monospace; color: var(--muted); }
.code-copy { margin-left: auto; font: 600 .8rem "Segoe UI", system-ui, sans-serif; color: var(--muted); background: transparent;
  border: none; border-radius: 6px; padding: 4px 8px; cursor: pointer; }
.code-copy:hover { color: var(--heading); background: color-mix(in srgb, var(--border) 50%, transparent); }
.code-body { display: flex; flex-wrap: wrap; padding: 14px 16px 14px 14px; --lh: 1.7; }
.code-body[hidden] { display: none; }
.code-body pre { margin: 0; line-height: var(--lh); font-size: .86rem; }
.code-body .gutter { color: var(--gutter); text-align: right; padding-right: 1.1em; user-select: none; }
.code-body .src { overflow-x: auto; flex: 1; }
.view-tabs { display: flex; gap: 6px; margin-left: auto; }
.code-head [hidden] { display: none; }
.code-icon { display: inline-flex; color: var(--muted); }
.code-icon svg { width: 15px; height: 15px; }
.code-badge { font: 600 .72rem "Segoe UI", system-ui, sans-serif; color: var(--muted); border-radius: 5px; padding: 1px 7px;
  background: color-mix(in srgb, var(--code-border) 80%, var(--code-head)); }
/* Консоль: приглашение рисует CSS — при копировании текста страницы его нет. */
.term-console { --lh: 1.6; }
.term-console .pr { user-select: none; }
.term-console .pr[data-cwd]::before { content: attr(data-cwd) " "; color: color-mix(in srgb, var(--t-info) 65%, var(--muted)); }
.term-console .pr::after { content: "$ "; color: var(--t-ok); font-weight: 700; }
.term-console .out { color: color-mix(in srgb, var(--text) 82%, var(--code-bg)); }
.term-console .cmt { color: var(--faint); }
.t-st, .t-err, .t-warn, .t-hint, .t-msg, .t-gut { font-weight: 700; }
.t-st, .t-add { color: var(--t-ok); } .t-err { color: var(--t-err); } .t-warn { color: var(--t-warn); }
.t-hint { color: var(--t-hint); } .t-msg { color: var(--heading); } .t-gut { color: var(--t-info); }
/* Лог: номер строки в ::before, длинные строки переносятся под начало текста. */
.term-log { --lh: 1.5; }
.term-log .src { white-space: pre-wrap; overflow-wrap: anywhere; }
.term-log .ll { display: block; position: relative; min-height: calc(var(--lh) * 1em); padding-left: calc(var(--gw) + 1.1em); }
.term-log .ll::before { content: attr(data-n); position: absolute; left: 0; width: var(--gw); text-align: right; color: var(--gutter); user-select: none; }
.l-time { color: var(--faint); } .l-grp { color: color-mix(in srgb, var(--accent) 55%, var(--muted)); }
.l-lv { font-weight: 700; } .l-trace { color: var(--faint); } .l-debug { color: color-mix(in srgb, var(--accent) 65%, var(--faint)); }
.l-info { color: var(--t-info); } .l-warn { color: var(--t-warn); } .l-err { color: var(--t-err); }
.l-tw { color: color-mix(in srgb, var(--t-warn) 80%, var(--text)); } .l-te { color: color-mix(in srgb, var(--t-err) 85%, var(--text)); }
/* Свёрнутый блок: первые строки, низ растворяется в фоне. */
.code-body.collapsed pre { max-height: calc(var(--shown) * var(--lh) * 1em); overflow-y: hidden;
  mask-image: linear-gradient(to bottom, #000 calc(100% - 3.2em), transparent); }
.code-more { flex: 0 0 calc(100% + 30px); margin: 12px -16px -14px -14px; display: flex; justify-content: center; align-items: center;
  gap: 6px; padding: 7px; border: none; border-top: 1px solid var(--code-border); background: transparent; color: var(--muted);
  font: 600 .8rem "Segoe UI", system-ui, sans-serif; cursor: pointer; }
.code-more[hidden] { display: none; }
.code-more:hover { color: var(--heading); background: color-mix(in srgb, var(--border) 35%, transparent); }
.code-body:not(.collapsed) .code-more svg { transform: rotate(180deg); }
.view-tabs + .code-copy { margin-left: 4px; }
.diagram { flex: 1; overflow-x: auto; text-align: center; }
.diagram[hidden], .diagram-src[hidden] { display: none; }
.diagram svg { max-width: 100%; height: auto; font-family: "Segoe UI", system-ui, sans-serif; }
mark { --tint: 250, 204, 21; background: rgba(var(--tint), .32); color: var(--heading); border-radius: 4px; padding: 0 .14em; }
blockquote { margin: 1.2em 0; padding: 2px 18px; border-left: 3px solid var(--accent); color: var(--muted); }
blockquote.quote { position: relative; color: var(--text); background: color-mix(in srgb, var(--accent) 7%, transparent);
  border-radius: 4px 14px 14px 4px; padding: 16px 20px 16px 3em; font-family: Cambria, Georgia, serif; font-style: italic; font-size: 1.06em; }
blockquote.quote::before { content: "“"; position: absolute; left: .35em; top: -.05em; font-size: 3em; color: var(--accent); opacity: .55; }
blockquote.quote footer { text-align: right; font: 600 .85em "Segoe UI", system-ui, sans-serif; font-style: normal; color: var(--muted); margin-top: .5em; }
.quote-body > :first-child { margin-top: 0; } .quote-body > :last-child { margin-bottom: 0; }
.callout { --c: 68, 138, 255; background: rgba(var(--c), .1); border: 1px solid rgba(var(--c), .25);
  border-radius: 12px; padding: 10px 16px; margin: 1.1em 0; }
.callout-title { display: flex; gap: 8px; align-items: center; font-weight: 600; color: rgb(var(--c)); cursor: default; }
details.callout > summary { cursor: pointer; list-style: none; }
.callout-body > :first-child { margin-top: .4em; } .callout-body > :last-child { margin-bottom: .2em; }
ul, ol { padding-left: 1.6em; } li { margin: .2em 0; } li > p { margin: .2em 0; }
li.task { list-style: none; margin-left: -1.3em; }
li.task input { accent-color: var(--accent); margin-right: .4em; }
li.task.done { color: var(--muted); text-decoration: line-through; }
.table-wrap { overflow-x: auto; border: 1px solid var(--table-line); border-radius: 12px; margin: 1.2em 0; background: var(--table-bg); }
table { border-collapse: collapse; width: 100%; }
th, td { padding: .7em 1em; text-align: left; border-bottom: 1px solid var(--table-line); border-right: 1px solid var(--table-line); }
th:last-child, td:last-child { border-right: none; }
tr:last-child td { border-bottom: none; }
thead th { background: var(--table-head); color: var(--heading); font-weight: 600; font-size: .92em; }
tbody tr { transition: background .12s; }
tbody tr:hover { background: color-mix(in srgb, var(--border) 35%, transparent); }
table.properties { width: 100%; border: 1px solid var(--border); border-radius: 12px; margin-bottom: 1.6em; font-size: .92rem;
  background: var(--bg-side); border-collapse: separate; overflow: hidden; }
table.properties th { color: var(--faint); font-weight: 500; width: 30%; background: transparent; border-right: none; }
table.properties td { border-right: none; }
img { max-width: 100%; border-radius: 10px; }
hr { border: none; border-top: 1px solid var(--border); margin: 2em 0; }
.math { overflow-x: auto; text-align: center; margin: 1em 0; }
.footnote { display: flex; gap: 8px; font-size: .88rem; color: var(--muted); }
.footnote-num { color: var(--accent); }
.embed { border: 1px solid var(--border); border-left: 3px solid var(--accent); border-radius: 10px; padding: 8px 16px; margin: 1em 0; background: var(--bg-side); }
.embed-title { font-size: .85rem; color: var(--accent); }
.missing { color: #f97316; }
.backlinks { margin-top: 3em; padding-top: 1em; border-top: 1px solid var(--border); font-size: .93rem; }
.backlinks h2 { font-size: 1rem; border: none; color: var(--muted); }
.muted { color: var(--muted); }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 14px; margin-top: 1.5em; }
.card { display: block; padding: 14px 16px; border: 1px solid var(--border); border-radius: 12px; text-decoration: none; color: var(--text); background: var(--bg-side); transition: transform .12s, border-color .12s; }
.card:hover { transform: translateY(-2px); border-color: var(--accent); }
.card-folder { font-size: .75rem; color: var(--faint); }
.card-title { font-weight: 600; color: var(--heading); margin: 2px 0 6px; }
.card-text { font-size: .85rem; color: var(--muted); }
@media (max-width: 800px) {
  body.with-sidebar { display: block; }
  .sidebar { position: static; height: auto; border-right: none; border-bottom: 1px solid var(--border); }
  main { padding: 28px 16px 64px; }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_works() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }

    #[test]
    fn exports_demo_site() {
        // DOCAPP_TEST_OUT позволяет сохранить результат, чтобы посмотреть сайт глазами.
        let keep = std::env::var_os("DOCAPP_TEST_OUT").map(PathBuf::from);
        let base = keep.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("holotype-test-{}", std::process::id())));
        let vault_dir = base.join("vault");
        let out = base.join("site");
        crate::sample::create_demo(&vault_dir).unwrap();
        let vault = Vault::open(&vault_dir).unwrap();
        let pages = export_site(&vault, &out).unwrap();
        assert!(pages >= 8);
        let index = std::fs::read_to_string(out.join("index.html")).unwrap();
        assert!(index.contains("Добро пожаловать"));
        let md = std::fs::read_to_string(out.join("Справка").join("Возможности Markdown.html")).unwrap();
        assert!(md.contains("class=\"callout\""));
        assert!(md.contains("class=\"internal\""));
        assert!(md.contains("code-body term-console") && md.contains("code-body term-log"));
        assert!(out.join("Вложения").join("обложка.png").exists());
        let dg = std::fs::read_to_string(out.join("Справка").join("Диаграммы.html")).unwrap();
        assert_eq!(dg.matches("<svg class=\"dg\"").count(), 7, "все диаграммы справки должны стать SVG");
        let single = base.join("note.html");
        export_note(&vault, vault.notes.iter().position(|n| n.name == "Формулы").unwrap(), &single).unwrap();
        assert!(std::fs::read_to_string(&single).unwrap().contains("katex"));
        if keep.is_none() {
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    fn html_of(src: &str) -> String {
        crate::i18n::init();
        let doc = markdown::parse(src);
        let mut out = String::new();
        for b in &doc.blocks {
            match b {
                Block::Code(cb) => code_html(&mut out, std::slice::from_ref(cb)),
                Block::CodeGroup(g) => code_html(&mut out, g),
                _ => {}
            }
        }
        out
    }

    #[test]
    fn console_block_html() {
        let mut src = String::from("```console title=\"Сборка\" cwd=\"~/hangar\"\n# Собираем\n$ cargo build \\\n    --release\n");
        for i in 0..12 {
            src.push_str(&format!("   Compiling crate{i} v0.1.0\n"));
        }
        src.push_str("error[E0425]: cannot find value `x`\n$ cargo run\n```");
        let html = html_of(&src);
        assert!(html.contains("class=\"code-body term-console\""));
        assert!(html.contains("<span class=\"code-title\">Сборка</span>"));
        assert!(html.contains("<span class=\"code-badge\" data-tab=\"0\">bash</span>"));
        assert!(html.contains("⧉ Копировать команды"));
        // Приглашение — пустой span (его рисует CSS), в тексте блока `$` нет.
        assert!(html.contains("<span class=\"pr\" data-cwd=\"~/hangar\"></span>"));
        assert!(!html.contains("$ cargo"));
        assert!(html.contains("data-copy=\"cargo build \\\n    --release\ncargo run\""));
        assert!(html.contains("<span class=\"cmt\"># Собираем</span>"));
        assert!(html.contains("<span class=\"t-st\">Compiling</span>"));
        assert!(html.contains("<span class=\"t-err\">error[E0425]</span>"));
        // 17 строк > 12: сворачивает скрипт, а без него всё видно.
        assert!(html.contains("data-shown=\"12\""));
        assert!(html.contains("data-more=\"Показать ещё 5 строк\""));
        assert!(html.contains("data-less=\"Свернуть\""));
        assert!(html.contains("<button class=\"code-more\" hidden"));
        assert!(!html.contains("collapsed"), "без JS блок развёрнут");
        // Флаг open — не сворачивать.
        let open = html_of(&src.replace("cwd=\"~/hangar\"", "open"));
        assert!(!open.contains("data-shown") && !open.contains("code-more"));
    }

    #[test]
    fn log_block_html() {
        let html = html_of("```log lines=1\n[13:36:24] [main/WARN]: Reference map could not be read\n[13:36:25] [main/INFO]: Done & ok\n```");
        assert!(html.contains("class=\"code-body term-log\""));
        assert!(html.contains("<span class=\"code-title\">log</span>"));
        assert!(html.contains(">2 строки</span>"));
        assert!(html.contains("⧉ Копировать лог"));
        assert!(html.contains("style=\"--shown:1;--gw:1ch;\""));
        assert!(html.contains("<span class=\"ll\" data-n=\"1\"><span class=\"l-time\">[13:36:24]</span>"));
        assert!(html.contains("<span class=\"l-lv l-warn\">WARN</span>"));
        assert!(html.contains("<span class=\"l-tw\">Reference map could not be read</span>"));
        assert!(html.contains("<span class=\"l-lv l-info\">INFO</span>"));
        assert!(html.contains("Done &amp; ok"));
        assert!(html.contains("data-more=\"Показать ещё 1 строку\""));
        // Копируется весь лог как есть.
        assert!(html.contains("data-copy=\"[13:36:24] [main/WARN]: Reference map could not be read\n[13:36:25] [main/INFO]: Done &amp; ok\""));
    }

    #[test]
    fn plain_code_collapses_only_with_lines() {
        let code: String = (1..=20).map(|i| format!("let x{i} = {i};\n")).collect();
        assert!(!html_of(&format!("```rust\n{code}```")).contains("data-shown"));
        let html = html_of(&format!("```rust lines=5\n{code}```"));
        assert!(html.contains("data-shown=\"5\""));
        assert!(html.contains("Показать ещё 15 строк"));
        // Вкладки: у каждой свой ярлык кнопки копирования, значок и плашка — только у консоли.
        let html = html_of("```rust tab=\"Код\"\nfn main() {}\n```\n\n```console tab=\"Запуск\"\n$ cargo run\n```");
        assert!(html.contains("data-label=\"⧉ Копировать\""));
        assert!(html.contains("data-label=\"⧉ Копировать команды\""));
        assert!(html.contains("<span class=\"code-icon\" data-tab=\"1\" hidden>"));
    }

    #[test]
    fn url_encoding() {
        assert_eq!(url_encode_path("Папка/My note.html"), "%D0%9F%D0%B0%D0%BF%D0%BA%D0%B0/My%20note.html");
    }
}
