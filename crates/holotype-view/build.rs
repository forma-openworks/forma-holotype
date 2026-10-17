//! Сборочный скрипт: вшивает строки рендера из `locales/` (`pub static LOCALES`). Программы,
//! которые рисуют заметки, добавляют эти строки к своим (см. `build.rs` у Holotype).

fn main() {
    forma_build::locales("locales", "LOCALES");
}
