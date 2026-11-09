//! Localization catalogs embedded at compile time.
//!
//! Translation sources live in the repository-level `locales/` directory.
//! `graphite-i18n/build.rs` validates them and embeds every locale so the
//! editor never depends on files next to the executable.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;

mod interpolation;

pub use interpolation::interpolate;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));
}

use generated::{CatalogData, CATALOGS, FALLBACK_CODE};

/// A compiled-in interface language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Language(&'static str);

impl Language {
    /// Returns the fallback language (English).
    pub fn fallback() -> Self {
        Self(FALLBACK_CODE)
    }

    /// Returns the BCP 47-like language code, such as `en` or `pt-BR`.
    pub fn code(self) -> &'static str {
        self.0
    }

    /// Returns the English display name.
    pub fn english_name(self) -> &'static str {
        catalog(self).map_or(self.0, |catalog| catalog.name)
    }

    /// Returns the language's own display name.
    pub fn native_name(self) -> &'static str {
        catalog(self).map_or(self.0, |catalog| catalog.native_name)
    }

    /// Returns every compiled-in language, in `locales/languages.json` order.
    pub fn available() -> impl Iterator<Item = Self> {
        CATALOGS.iter().map(|catalog| Self(catalog.code))
    }

    /// Parses a language tag or alias, accepting `_` or `-` separators.
    ///
    /// Exact tags win. A base-language tag such as `pt` matches a unique
    /// regional catalog such as `pt-BR`, and `en-US` falls back to `en`.
    pub fn parse(tag: &str) -> Option<Self> {
        let normalized = normalize_tag(tag);
        if normalized.is_empty() {
            return None;
        }
        if let Some(catalog) = CATALOGS
            .iter()
            .find(|catalog| normalize_tag(catalog.code) == normalized)
        {
            return Some(Self(catalog.code));
        }
        let base = normalized.split('-').next().unwrap_or(&normalized);
        let mut matches = CATALOGS
            .iter()
            .filter(|catalog| normalize_tag(catalog.code).split('-').next() == Some(base));
        let first = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(Self(first.code))
    }

    /// Detects the operating-system language, falling back to English.
    pub fn detect() -> Self {
        sys_locale::get_locale()
            .and_then(|tag| Self::parse(&tag))
            .unwrap_or_else(Self::fallback)
    }
}

fn normalize_tag(tag: &str) -> String {
    tag.trim().replace('_', "-").to_ascii_lowercase()
}

fn catalog(language: Language) -> Option<&'static CatalogData> {
    CATALOGS.iter().find(|catalog| catalog.code == language.0)
}

fn catalog_index(language: Language) -> Option<usize> {
    CATALOGS
        .iter()
        .position(|catalog| catalog.code == language.0)
}

fn lookup(language: Language, key: &str) -> Option<&'static str> {
    static MAPS: OnceLock<Vec<HashMap<&'static str, &'static str>>> = OnceLock::new();
    let maps = MAPS.get_or_init(|| {
        CATALOGS
            .iter()
            .map(|catalog| catalog.entries.iter().copied().collect())
            .collect()
    });
    maps.get(catalog_index(language)?)?.get(key).copied()
}

/// Resolves keys against a language, falling back to English and then the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Translator {
    language: Language,
}

impl Translator {
    pub fn new(language: Language) -> Self {
        Self { language }
    }

    pub fn english() -> Self {
        Self::new(Language::fallback())
    }

    pub fn language(self) -> Language {
        self.language
    }

    /// Returns the translation for `key`, or `key` itself when it is missing.
    pub fn text<'a>(self, key: &'a str) -> &'a str {
        if let Some(value) = lookup(self.language, key) {
            return value;
        }
        if let Some(value) = lookup(Language::fallback(), key) {
            return value;
        }
        key
    }

    /// Returns the translation for `key`, or `fallback` when it is missing.
    pub fn text_or<'a>(self, key: &str, fallback: &'a str) -> &'a str {
        lookup(self.language, key)
            .or_else(|| lookup(Language::fallback(), key))
            .unwrap_or(fallback)
    }

    /// Returns whether the active language or English defines `key`.
    pub fn has(self, key: &str) -> bool {
        lookup(self.language, key).is_some() || lookup(Language::fallback(), key).is_some()
    }

    /// Resolves `key` and replaces named `{placeholders}` with `args`.
    pub fn format(self, key: &str, args: &[(&str, &dyn Display)]) -> String {
        interpolate(self.text(key), args)
    }

    /// Resolves `key` with an English source fallback before interpolating.
    pub fn format_or(self, key: &str, fallback: &str, args: &[(&str, &dyn Display)]) -> String {
        interpolate(self.text_or(key, fallback), args)
    }

    /// Returns the translation keys defined for a language.
    pub fn keys(self) -> impl Iterator<Item = &'static str> {
        catalog(self.language)
            .into_iter()
            .flat_map(|catalog| catalog.entries.iter().map(|(key, _)| *key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_and_regional_language_tags() {
        let english = Language::fallback();
        assert_eq!(english.code(), "en");
        assert_eq!(Language::parse("en"), Some(english));
        assert_eq!(Language::parse("EN_us"), Some(english));
        assert_eq!(Language::parse("pt-br").unwrap().code(), "pt-BR");
        assert_eq!(Language::parse("pt_BR").unwrap().code(), "pt-BR");
        assert_eq!(Language::parse("pt").unwrap().code(), "pt-BR");
        assert_eq!(Language::parse("pt-BR-extra").unwrap().code(), "pt-BR");
        assert_eq!(Language::parse("fr"), None);
        assert_eq!(Language::parse(""), None);
    }

    #[test]
    fn every_language_has_a_catalog_and_fallback_is_english() {
        let mut languages: Vec<_> = Language::available().collect();
        languages.sort();
        assert!(languages.contains(&Language::fallback()));
        for language in languages {
            assert!(!language.english_name().is_empty());
            assert!(!language.native_name().is_empty());
            assert!(catalog(language).is_some());
        }
    }

    #[test]
    fn translation_falls_back_to_english_then_the_key() {
        let portuguese = Translator::new(Language::parse("pt-BR").unwrap());
        assert_eq!(portuguese.text("common.cancel"), "Cancelar");
        assert_eq!(portuguese.text("missing.key"), "missing.key");
    }

    #[test]
    fn formats_named_placeholders_and_preserves_unknown_ones() {
        let translator = Translator::english();
        assert_eq!(
            translator.format("status.cannot_open_folder", &[("path", &"/tmp")]),
            "Cannot open folder: /tmp"
        );
        assert_eq!(
            interpolate("{{literal}} {name}", &[("name", &2)]),
            "{literal} 2"
        );
    }

    #[test]
    fn catalogs_define_the_same_keys_in_every_language() {
        let english: std::collections::BTreeSet<_> = Translator::english().keys().collect();
        for language in Language::available() {
            let keys: std::collections::BTreeSet<_> = Translator::new(language).keys().collect();
            assert_eq!(keys, english, "locale {}", language.code());
        }
    }
}
