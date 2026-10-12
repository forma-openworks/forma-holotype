//! Строки Forma: Holotype: `locales/<язык>/*.ftl` этого крейта. Строки платформы и оболочки
//! Forma добавляются к ним автоматически.

// Встроенные переводы программы: `pub static LOCALES`.
include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// Переводы без папки пользователя на русском — для тестов (ожидаемые строки в них русские).
#[cfg(test)]
pub fn init() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        forma_i18n::init_with(&[forma_shell::LOCALES, LOCALES], None);
        forma_i18n::set_language("ru");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;

    #[test]
    fn all_languages_have_the_same_keys() {
        let reference = forma_i18n::message_ids(&[LOCALES], forma_i18n::FALLBACK);
        assert!(!reference.is_empty());
        for (code, _) in LOCALES {
            let ids = forma_i18n::message_ids(&[LOCALES], code);
            let missing: Vec<_> = reference.iter().filter(|k| !ids.contains(k)).collect();
            let extra: Vec<_> = ids.iter().filter(|k| !reference.contains(k)).collect();
            assert!(missing.is_empty(), "в языке «{code}» не хватает строк: {missing:?}");
            assert!(extra.is_empty(), "в языке «{code}» лишние строки: {extra:?}");
        }
    }

    #[test]
    fn keys_do_not_repeat_platform_shell_or_each_other() {
        for (code, _) in LOCALES {
            let ids = forma_i18n::message_ids(&[forma_i18n::PLATFORM, forma_shell::LOCALES, LOCALES], code);
            let mut seen = HashSet::new();
            let dups: Vec<_> = ids.iter().filter(|k| !seen.insert(k.as_str())).collect();
            assert!(dups.is_empty(), "в языке «{code}» повторяются ключи: {dups:?}");
        }
    }

    /// Каждый ключ макроса `t!` в крейтах Holotype переведён (у программы, оболочки или платформы).
    #[test]
    fn every_used_key_exists() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut used = Vec::new();
        for entry in std::fs::read_dir(&crates).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("holotype")) {
                used.extend(forma_i18n::used_keys(&path.join("src")));
            }
        }
        assert!(!used.is_empty());
        let ids = forma_i18n::message_ids(&[forma_i18n::PLATFORM, forma_shell::LOCALES, LOCALES], forma_i18n::FALLBACK);
        let missing: Vec<_> = used.iter().filter(|(k, _)| !ids.contains(k)).collect();
        assert!(missing.is_empty(), "ключи используются в коде, но не переведены: {missing:?}");
    }
}
