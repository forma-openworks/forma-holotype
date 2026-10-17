//! Сборочный скрипт: вшивает строки интерфейса из `locales/` и строки рендера заметок
//! (`holotype-view/locales/`) и иконку (тот же код, что в `src/icon.rs`, из рисунка
//! `assets/logo/holotype.webp`) со сведениями о продукте в .exe.

use std::path::{Path, PathBuf};

#[path = "src/icon.rs"]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=../../assets/logo");
    // Строки программы и строки рендера — одним набором переводов: оболочка берёт у
    // программы один набор (`Spec::locales`).
    let merged = merge_locales(&["locales", "../holotype-view/locales"]);
    forma_build::locales(&merged, "LOCALES");
    forma_build::windows_resources(&forma_build::WindowsResources {
        product: "Forma: Holotype",
        internal_name: "forma-holotype",
        file_name: "forma-holotype.exe",
        icon: &icon::rgba,
    });
}

/// Складывает папки `<язык>/*.ftl` нескольких crate в одну (в `OUT_DIR`) и возвращает её.
fn merge_locales(dirs: &[&str]) -> PathBuf {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("locales");
    let _ = std::fs::remove_dir_all(&out);
    for dir in dirs {
        let src = manifest.join(dir);
        println!("cargo:rerun-if-changed={}", src.display());
        for lang in std::fs::read_dir(&src).unwrap_or_else(|e| panic!("нет папки переводов {}: {e}", src.display())).flatten() {
            if lang.path().is_dir() {
                copy_ftl(&lang.path(), &out.join(lang.file_name()));
            }
        }
    }
    out
}

fn copy_ftl(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("папка переводов");
    for f in std::fs::read_dir(from).expect("папка языка").flatten() {
        if f.path().extension().is_some_and(|e| e == "ftl") {
            std::fs::copy(f.path(), to.join(f.file_name())).expect("файл переводов");
        }
    }
}
