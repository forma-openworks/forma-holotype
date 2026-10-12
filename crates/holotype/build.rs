//! Сборочный скрипт: вшивает строки интерфейса из `locales/` и иконку (тот же код, что в
//! `src/icon.rs`, из рисунка `assets/logo/holotype.webp`) со сведениями о продукте в .exe.

#[path = "src/icon.rs"]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=../../assets/logo");
    forma_build::locales("locales", "LOCALES");
    forma_build::windows_resources(&forma_build::WindowsResources {
        product: "Forma: Holotype",
        internal_name: "forma-holotype",
        file_name: "forma-holotype.exe",
        icon: &icon::rgba,
    });
}
