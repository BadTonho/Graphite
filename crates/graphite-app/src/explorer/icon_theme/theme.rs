//! Parsing a file icon theme document and choosing the definition for a file or folder.
//!
//! A theme maps file names, extensions, language ids and folder names to entries of
//! `iconDefinitions`. The `light` and `highContrast` sections override the base mapping while a
//! light or high-contrast color theme is active.

use crate::config::parse_jsonc;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Which section of the theme applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Variant {
    Dark,
    Light,
    HighContrast,
}

/// What an icon definition draws.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Definition {
    /// An SVG or raster image, as a path resolved against the theme file's folder.
    Image(PathBuf),
    /// A glyph from one of the theme's fonts (`fontId`, or the first font when absent).
    Glyph {
        glyph: char,
        color: Option<u32>,
        font: Option<String>,
        /// Size relative to the row's icon box (`fontSize`, or the font's `size`).
        scale: f32,
    },
}

/// A font declared in `fonts`, with its first source resolved against the theme file's folder.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FontSpec {
    pub path: PathBuf,
    pub scale: f32,
}

/// One section of associations (the base, `light` or `highContrast`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Associations {
    file: Option<String>,
    folder: Option<String>,
    folder_expanded: Option<String>,
    root_folder: Option<String>,
    root_folder_expanded: Option<String>,
    file_extensions: HashMap<String, String>,
    file_names: HashMap<String, String>,
    folder_names: HashMap<String, String>,
    folder_names_expanded: HashMap<String, String>,
    root_folder_names: HashMap<String, String>,
    root_folder_names_expanded: HashMap<String, String>,
    language_ids: HashMap<String, String>,
}

impl Associations {
    fn parse(value: &Value) -> Self {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let map = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(name, id)| Some((name.to_lowercase(), id.as_str()?.to_owned())))
                .collect::<HashMap<_, _>>()
        };
        let language_ids = value
            .get("languageIds")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(name, id)| Some((name.clone(), id.as_str()?.to_owned())))
            .collect();
        Self {
            file: text("file"),
            folder: text("folder"),
            folder_expanded: text("folderExpanded"),
            root_folder: text("rootFolder"),
            root_folder_expanded: text("rootFolderExpanded"),
            file_extensions: map("fileExtensions"),
            file_names: map("fileNames"),
            folder_names: map("folderNames"),
            folder_names_expanded: map("folderNamesExpanded"),
            root_folder_names: map("rootFolderNames"),
            root_folder_names_expanded: map("rootFolderNamesExpanded"),
            language_ids,
        }
    }
}

/// What is being looked up.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry<'a> {
    pub name: &'a str,
    pub is_dir: bool,
    pub expanded: bool,
    /// A workspace folder shown at the top of the tree.
    pub is_root: bool,
    /// The language of a file, when known.
    pub language: Option<&'a str>,
}

/// A parsed file icon theme.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IconThemeData {
    pub definitions: HashMap<String, Definition>,
    pub fonts: HashMap<String, FontSpec>,
    /// The font used by glyph definitions without `fontId`.
    pub default_font: Option<String>,
    pub hides_explorer_arrows: bool,
    base: Associations,
    light: Associations,
    high_contrast: Associations,
}

/// Parses a CSS-like size (`"110%"`, `"1.2"`) into a scale factor, kept between 0.5 and 2.
fn parse_scale(value: Option<&Value>) -> Option<f32> {
    let text = value?.as_str()?.trim();
    let scale = match text.strip_suffix('%') {
        Some(percent) => percent.trim().parse::<f32>().ok()? / 100.0,
        None => text.parse::<f32>().ok()?,
    };
    (scale.is_finite() && scale > 0.0).then(|| scale.clamp(0.5, 2.0))
}

/// Parses `#rgb`, `#rrggbb` or `#rrggbbaa` (the alpha is dropped).
pub(crate) fn parse_color(text: &str) -> Option<u32> {
    let hex = text.trim().strip_prefix('#')?;
    let expanded: String = match hex.len() {
        3 | 4 => hex.chars().take(3).flat_map(|ch| [ch, ch]).collect(),
        6 | 8 => hex[..6].to_owned(),
        _ => return None,
    };
    u32::from_str_radix(&expanded, 16).ok()
}

/// Reads `fontCharacter`: an escape such as `\E001` or the character itself.
fn parse_glyph(text: &str) -> Option<char> {
    match text.strip_prefix('\\') {
        Some(hex) if !hex.is_empty() => char::from_u32(u32::from_str_radix(hex, 16).ok()?),
        _ => text.chars().next(),
    }
}

/// Joins a theme-relative path onto the theme file's folder.
fn resolve(dir: &Path, relative: &str) -> PathBuf {
    let relative = relative.strip_prefix("./").unwrap_or(relative);
    dir.join(relative)
}

impl IconThemeData {
    /// Parses a theme document; `dir` is the folder of the theme file.
    pub(crate) fn parse(text: &str, dir: &Path) -> Result<Self, String> {
        let value = parse_jsonc(text).map_err(|error| error.to_string())?;
        if !value.is_object() {
            return Err("the icon theme is not a JSON object".into());
        }
        let mut fonts = HashMap::new();
        let mut default_font = None;
        for font in value
            .get("fonts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(id) = font.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(path) = font
                .get("src")
                .and_then(Value::as_array)
                .and_then(|sources| {
                    sources
                        .iter()
                        .find_map(|source| source.get("path").and_then(Value::as_str))
                })
            else {
                continue;
            };
            default_font.get_or_insert_with(|| id.to_owned());
            fonts.insert(
                id.to_owned(),
                FontSpec {
                    path: resolve(dir, path),
                    scale: parse_scale(font.get("size")).unwrap_or(1.0),
                },
            );
        }
        let mut definitions = HashMap::new();
        for (id, definition) in value
            .get("iconDefinitions")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let parsed = if let Some(path) = definition.get("iconPath").and_then(Value::as_str) {
                Definition::Image(resolve(dir, path))
            } else if let Some(glyph) = definition
                .get("fontCharacter")
                .and_then(Value::as_str)
                .and_then(parse_glyph)
            {
                let font = definition
                    .get("fontId")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let font_scale = font
                    .as_ref()
                    .or(default_font.as_ref())
                    .and_then(|font| fonts.get(font))
                    .map_or(1.0, |font: &FontSpec| font.scale);
                Definition::Glyph {
                    glyph,
                    color: definition
                        .get("fontColor")
                        .and_then(Value::as_str)
                        .and_then(parse_color),
                    font,
                    scale: parse_scale(definition.get("fontSize")).unwrap_or(font_scale),
                }
            } else {
                continue;
            };
            definitions.insert(id.clone(), parsed);
        }
        let section = |key: &str| value.get(key).map(Associations::parse).unwrap_or_default();
        Ok(Self {
            definitions,
            fonts,
            default_font,
            hides_explorer_arrows: value
                .get("hidesExplorerArrows")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            base: Associations::parse(&value),
            light: section("light"),
            high_contrast: section("highContrast"),
        })
    }

    /// The id of the definition to draw for `entry`, if the theme has one.
    pub(crate) fn definition_id(&self, entry: Entry<'_>, variant: Variant) -> Option<&str> {
        let sections: &[&Associations] = match variant {
            Variant::Dark => &[&self.base],
            Variant::Light => &[&self.light, &self.base],
            Variant::HighContrast => &[&self.high_contrast, &self.base],
        };
        // An association to a definition the theme does not have is skipped.
        let lookup = |pick: &dyn Fn(&Associations) -> Option<&String>| {
            sections.iter().find_map(|section| {
                pick(section).filter(|id| self.definitions.contains_key(id.as_str()))
            })
        };
        let name = entry.name.to_lowercase();
        let id = if entry.is_dir {
            let by_name = |names: fn(&Associations) -> &HashMap<String, String>| {
                lookup(&|section: &Associations| names(section).get(&name))
            };
            let root = || {
                if !entry.is_root {
                    return None;
                }
                if entry.expanded {
                    by_name(|s| &s.root_folder_names_expanded)
                        .or_else(|| by_name(|s| &s.root_folder_names))
                        .or_else(|| lookup(&|s| s.root_folder_expanded.as_ref()))
                        .or_else(|| lookup(&|s| s.root_folder.as_ref()))
                } else {
                    by_name(|s| &s.root_folder_names)
                        .or_else(|| lookup(&|s| s.root_folder.as_ref()))
                }
            };
            let folder = || {
                if entry.expanded {
                    by_name(|s| &s.folder_names_expanded)
                        .or_else(|| by_name(|s| &s.folder_names))
                        .or_else(|| lookup(&|s| s.folder_expanded.as_ref()))
                        .or_else(|| lookup(&|s| s.folder.as_ref()))
                } else {
                    by_name(|s| &s.folder_names).or_else(|| lookup(&|s| s.folder.as_ref()))
                }
            };
            root().or_else(folder)
        } else {
            lookup(&|section| section.file_names.get(&name))
                .or_else(|| {
                    // `a.spec.ts` tries `spec.ts`, then `ts`; `.gitignore` tries `gitignore`.
                    name.match_indices('.').find_map(|(index, _)| {
                        let extension = &name[index + 1..];
                        lookup(&|section| section.file_extensions.get(extension))
                    })
                })
                .or_else(|| {
                    let language = entry.language?;
                    lookup(&|section| section.language_ids.get(language))
                })
                .or_else(|| lookup(&|section| section.file.as_ref()))
        };
        id.map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file<'a>(name: &'a str, language: Option<&'a str>) -> Entry<'a> {
        Entry {
            name,
            is_dir: false,
            expanded: false,
            is_root: false,
            language,
        }
    }

    fn folder(name: &str, expanded: bool, is_root: bool) -> Entry<'_> {
        Entry {
            name,
            is_dir: true,
            expanded,
            is_root,
            language: None,
        }
    }

    const THEME: &str = r##"{
        // Comments are allowed, as in every settings-style file.
        "iconDefinitions": {
            "file": {"iconPath": "./icons/file.svg"},
            "folder": {"iconPath": "icons/folder.svg"},
            "folder-open": {"iconPath": "icons/folder-open.svg"},
            "root": {"iconPath": "icons/root.svg"},
            "src": {"iconPath": "icons/src.svg"},
            "src-open": {"iconPath": "icons/src-open.svg"},
            "ts": {"iconPath": "icons/ts.svg"},
            "spec": {"iconPath": "icons/spec.svg"},
            "git": {"iconPath": "icons/git.svg"},
            "license": {"iconPath": "icons/license.svg"},
            "rust": {"fontCharacter": "\\E001", "fontColor": "#ce422b", "fontSize": "120%"},
            "rust-light": {"fontCharacter": "R", "fontColor": "#abc", "fontId": "alt"}
        },
        "fonts": [
            {"id": "main", "src": [{"path": "./fonts/main.woff", "format": "woff"}], "size": "150%"},
            {"id": "alt", "src": [{"path": "fonts/alt.ttf", "format": "truetype"}]}
        ],
        "file": "file",
        "folder": "folder",
        "folderExpanded": "folder-open",
        "rootFolder": "root",
        "folderNames": {"SRC": "src"},
        "folderNamesExpanded": {"src": "src-open"},
        "fileExtensions": {"ts": "ts", "spec.ts": "spec", "gitignore": "git", "missing": "nowhere"},
        "fileNames": {"LICENSE": "license"},
        "languageIds": {"rust": "rust"},
        "hidesExplorerArrows": true,
        "light": {"languageIds": {"rust": "rust-light"}}
    }"##;

    fn theme() -> IconThemeData {
        IconThemeData::parse(THEME, Path::new("/ext/theme")).unwrap()
    }

    #[test]
    fn files_match_names_then_longest_extension_then_language_then_default() {
        let theme = theme();
        let id = |name, language| theme.definition_id(file(name, language), Variant::Dark);
        assert_eq!(id("license", None), Some("license"));
        assert_eq!(id("app.spec.ts", None), Some("spec"));
        assert_eq!(id("App.TS", None), Some("ts"));
        assert_eq!(id(".gitignore", None), Some("git"));
        assert_eq!(id("main.rs", Some("rust")), Some("rust"));
        assert_eq!(id("notes.txt", None), Some("file"));
        // An association to a definition that does not exist is ignored.
        assert_eq!(id("data.missing", None), Some("file"));
    }

    #[test]
    fn folders_use_names_expanded_variants_and_root_icons() {
        let theme = theme();
        let id = |entry| theme.definition_id(entry, Variant::Dark);
        assert_eq!(id(folder("src", false, false)), Some("src"));
        assert_eq!(id(folder("Src", true, false)), Some("src-open"));
        assert_eq!(id(folder("lib", false, false)), Some("folder"));
        assert_eq!(id(folder("lib", true, false)), Some("folder-open"));
        assert_eq!(id(folder("project", false, true)), Some("root"));
        // Without `rootFolderExpanded`, an open root keeps the root icon.
        assert_eq!(id(folder("project", true, true)), Some("root"));
    }

    #[test]
    fn light_and_high_contrast_sections_override_the_base() {
        let theme = theme();
        let rust = file("main.rs", Some("rust"));
        assert_eq!(
            theme.definition_id(rust, Variant::Light),
            Some("rust-light")
        );
        assert_eq!(
            theme.definition_id(rust, Variant::HighContrast),
            Some("rust")
        );
        assert_eq!(
            theme.definition_id(file("a.ts", None), Variant::Light),
            Some("ts")
        );
    }

    #[test]
    fn definitions_fonts_and_options_are_read() {
        let theme = theme();
        assert!(theme.hides_explorer_arrows);
        assert_eq!(
            theme.definitions["file"],
            Definition::Image(Path::new("/ext/theme").join("icons/file.svg"))
        );
        assert_eq!(
            theme.definitions["rust"],
            Definition::Glyph {
                glyph: '\u{e001}',
                color: Some(0xce422b),
                font: None,
                scale: 1.2,
            }
        );
        // Without `fontSize`, the font's own size applies.
        assert_eq!(
            theme.definitions["rust-light"],
            Definition::Glyph {
                glyph: 'R',
                color: Some(0xaabbcc),
                font: Some("alt".into()),
                scale: 1.0,
            }
        );
        assert_eq!(theme.default_font.as_deref(), Some("main"));
        assert_eq!(
            theme.fonts["main"],
            FontSpec {
                path: Path::new("/ext/theme").join("fonts/main.woff"),
                scale: 1.5,
            }
        );
    }

    #[test]
    fn malformed_documents_and_values_are_rejected() {
        assert!(IconThemeData::parse("[1]", Path::new("/")).is_err());
        assert!(IconThemeData::parse("{", Path::new("/")).is_err());
        assert_eq!(parse_scale(Some(&Value::from("900%"))), Some(2.0));
        assert_eq!(parse_scale(Some(&Value::from("-1"))), None);
        assert_eq!(parse_color("#fff"), Some(0xffffff));
        assert_eq!(parse_color("#11223344"), Some(0x112233));
        assert_eq!(parse_color("red"), None);
        assert_eq!(parse_glyph("\\zz"), None);
        assert_eq!(parse_glyph("x"), Some('x'));
    }
}
