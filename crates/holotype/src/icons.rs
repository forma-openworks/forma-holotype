//! Значки Holotype поверх значков платформы Forma: рисунки из набора Lucide (лицензия ISC,
//! файлы и лицензия — в `icons/lucide/`), названные по смыслу. Тема может заменить любой.

macro_rules! lucide {
    ($($name:literal => $file:literal),* $(,)?) => {
        /// Значки заметок: имя по смыслу и SVG.
        const ICONS: &[(&str, &[u8])] = &[$(($name, include_bytes!(concat!("../icons/lucide/", $file, ".svg")))),*];
    };
}

lucide! {
    "note" => "file-text",
    "note-new" => "file-plus",
    "folder-new" => "folder-plus",
    "tag" => "tag",
    "tags" => "hash",
    "link" => "link",
    "outline" => "list-tree",
    "graph" => "waypoints",
    "daily" => "calendar-days",
    "read" => "book-open",
    "write" => "pencil-line",
    "split" => "columns-2",
    "back" => "arrow-left",
    "forward" => "arrow-right",
    "vault" => "book-marked",
    "export" => "file-down",
    "site" => "globe",
    "demo" => "sparkles",
    "info-panel" => "panel-right",
    "snippet" => "text-cursor-input",
}

/// Добавляет значки заметок к значкам платформы. Вызывать при запуске.
pub fn register() {
    forma_ui::icons::register(ICONS);
}
