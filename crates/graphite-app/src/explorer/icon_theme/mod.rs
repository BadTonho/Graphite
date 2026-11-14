//! File icon themes contributed by extensions (`contributes.iconThemes`) and the icon that the
//! explorer, tabs, quick open and source control draw for a file or folder.
//!
//! `workbench.iconTheme` selects a theme by id; without the setting the built-in icons apply,
//! and `null` turns file icons off. Image and font files are loaded the first time an icon
//! needs them.

mod assets;
mod theme;

use crate::file_icons::{self, FileIcon};
use graphite_render::{rgb, IconFont, Image, Renderer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
#[cfg(not(test))]
use std::sync::RwLock;
use std::sync::{Arc, Mutex};
pub(crate) use theme::{Definition, Entry, IconThemeData, Variant};

/// What to draw for one row.
#[derive(Clone, Debug)]
pub(crate) enum Icon {
    /// A glyph of the embedded icon font.
    Builtin(FileIcon),
    /// Pixels from a theme image.
    Image(Arc<Image>),
    /// A glyph from a theme font; without a color the text color applies.
    Glyph {
        font: IconFont,
        glyph: char,
        color: Option<u32>,
        scale: f32,
    },
    /// The theme has no icon for this entry, or file icons are off.
    None,
}

impl Icon {
    /// The built-in icon, when that is what this is.
    #[cfg(test)]
    pub(crate) fn builtin(&self) -> Option<FileIcon> {
        match self {
            Self::Builtin(icon) => Some(*icon),
            _ => None,
        }
    }
}

/// A loaded icon theme with the files it needs, cached as they are used.
pub(crate) struct IconTheme {
    pub id: String,
    /// The theme file, as given to [`IconTheme::load`].
    pub path: PathBuf,
    data: IconThemeData,
    /// The extension folder, canonical; every file the theme reads stays inside it.
    root: PathBuf,
    /// Language ids contributed by extensions: keys are lowercase file names (`jenkinsfile`)
    /// or extensions with their dot (`.d.ts`).
    languages: HashMap<String, String>,
    icons: Mutex<HashMap<String, Icon>>,
    fonts: Mutex<HashMap<String, Option<IconFont>>>,
}

impl std::fmt::Debug for IconTheme {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IconTheme")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl IconTheme {
    /// Loads the theme file at `path` of the extension in `root`.
    pub(crate) fn load(
        id: &str,
        path: &Path,
        root: &Path,
        languages: HashMap<String, String>,
    ) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let path = assets::confined(path, &root)
            .ok_or_else(|| format!("{} is not inside the extension", path.display()))?;
        let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let dir = path.parent().unwrap_or(&root);
        Ok(Self {
            id: id.to_owned(),
            path: path.clone(),
            data: IconThemeData::parse(&text, dir)?,
            root,
            languages,
            icons: Mutex::default(),
            fonts: Mutex::default(),
        })
    }

    pub(crate) fn hides_explorer_arrows(&self) -> bool {
        self.data.hides_explorer_arrows
    }

    fn language_id(&self, name: &str) -> Option<String> {
        let lower = name.to_lowercase();
        let contributed = self.languages.get(&lower).or_else(|| {
            lower
                .match_indices('.')
                .find_map(|(index, _)| self.languages.get(&lower[index..]))
        });
        if let Some(id) = contributed {
            return Some(id.clone());
        }
        if let Some((_, extension)) = lower.rsplit_once('.') {
            match extension {
                "jsx" => return Some("javascriptreact".into()),
                "txt" | "text" => return Some("plaintext".into()),
                _ => {}
            }
        }
        let language = graphite_syntax::Language::from_path(Path::new(&lower))?;
        Some(
            match language.name() {
                "bash" => "shellscript",
                "tsx" => "typescriptreact",
                other => other,
            }
            .to_owned(),
        )
    }

    fn font(&self, id: Option<&str>) -> Option<IconFont> {
        let id = id.or(self.data.default_font.as_deref())?;
        let mut fonts = self.fonts.lock().ok()?;
        fonts
            .entry(id.to_owned())
            .or_insert_with(|| {
                let spec = self.data.fonts.get(id)?;
                let data = assets::read_font(&spec.path, &self.root)?;
                Some(IconFont {
                    id: assets::next_font_id(),
                    data: data.into(),
                })
            })
            .clone()
    }

    fn icon(&self, definition_id: &str) -> Icon {
        if let Some(icon) = self
            .icons
            .lock()
            .ok()
            .and_then(|icons| icons.get(definition_id).cloned())
        {
            return icon;
        }
        let icon = match self.data.definitions.get(definition_id) {
            Some(Definition::Image(path)) => assets::load_image(path, &self.root)
                .map(|image| Icon::Image(Arc::new(image)))
                .unwrap_or(Icon::None),
            Some(Definition::Glyph {
                glyph,
                color,
                font,
                scale,
            }) => self
                .font(font.as_deref())
                .map(|font| Icon::Glyph {
                    font,
                    glyph: *glyph,
                    color: *color,
                    scale: *scale,
                })
                .unwrap_or(Icon::None),
            None => Icon::None,
        };
        if let Ok(mut icons) = self.icons.lock() {
            icons.insert(definition_id.to_owned(), icon.clone());
        }
        icon
    }

    /// The icon for a tree row.
    pub(crate) fn resolve(
        &self,
        name: &str,
        is_dir: bool,
        expanded: bool,
        is_root: bool,
        variant: Variant,
    ) -> Icon {
        let language = (!is_dir).then(|| self.language_id(name)).flatten();
        let entry = Entry {
            name,
            is_dir,
            expanded,
            is_root,
            language: language.as_deref(),
        };
        match self.data.definition_id(entry, variant) {
            Some(id) => self.icon(id),
            None => Icon::None,
        }
    }
}

/// Which icons are drawn.
#[derive(Clone, Debug, Default)]
pub(crate) enum Selection {
    #[default]
    Builtin,
    /// `workbench.iconTheme` is `null`: no file icons.
    Disabled,
    Theme(Arc<IconTheme>),
}

#[cfg(not(test))]
static ACTIVE: RwLock<Selection> = RwLock::new(Selection::Builtin);

#[cfg(test)]
thread_local! {
    /// Tests run in parallel threads; each keeps its own selection.
    static ACTIVE: std::cell::RefCell<Selection> = std::cell::RefCell::default();
}

/// Makes `selection` the icons drawn from now on.
pub(crate) fn set_selection(selection: Selection) {
    #[cfg(not(test))]
    if let Ok(mut active) = ACTIVE.write() {
        *active = selection;
    }
    #[cfg(test)]
    ACTIVE.with(|active| *active.borrow_mut() = selection);
}

/// The active selection.
pub(crate) fn selection() -> Selection {
    #[cfg(not(test))]
    return ACTIVE
        .read()
        .map(|active| active.clone())
        .unwrap_or_default();
    #[cfg(test)]
    ACTIVE.with(|active| active.borrow().clone())
}

fn variant() -> Variant {
    if crate::app::theme::HIGH_CONTRAST.load(Ordering::Relaxed) {
        Variant::HighContrast
    } else if crate::app::theme::DARK_THEME.load(Ordering::Relaxed) {
        Variant::Dark
    } else {
        Variant::Light
    }
}

/// The icon for a tree row (`is_root` marks a workspace folder at the top of the tree).
pub(crate) fn for_entry(name: &str, is_dir: bool, expanded: bool, is_root: bool) -> Icon {
    match selection() {
        Selection::Builtin => Icon::Builtin(file_icons::for_entry(name, is_dir, expanded)),
        Selection::Disabled => Icon::None,
        Selection::Theme(theme) => theme.resolve(name, is_dir, expanded, is_root, variant()),
    }
}

/// The icon for a file path (tabs, quick open, source control).
pub(crate) fn for_path(path: &Path) -> Icon {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    for_entry(&name, false, false, false)
}

/// Whether the active theme asks the explorer not to draw folder arrows.
pub(crate) fn hides_explorer_arrows() -> bool {
    matches!(selection(), Selection::Theme(theme) if theme.hides_explorer_arrows())
}

/// Draws `icon` in the `size` by `size` box at (`x`, `y`). `tint` adjusts glyph colors (for
/// example to dim them); images are drawn as they are.
pub(crate) fn draw(
    renderer: &mut Renderer,
    icon: &Icon,
    x: f32,
    y: f32,
    size: f32,
    tint: impl Fn(u32) -> u32,
) {
    match icon {
        Icon::Builtin(builtin) => {
            renderer.draw_icon(builtin.glyph, x, y, size, rgb(tint(builtin.color)));
        }
        Icon::Image(image) => renderer.draw_image(image, x, y, size, size),
        Icon::Glyph {
            font,
            glyph,
            color,
            scale,
        } => {
            let color = color.unwrap_or_else(|| {
                crate::app::theme::Theme::color(crate::app::theme::Theme::TEXT, 0x1e1e1e)
            });
            let scaled = (size * scale).round();
            let offset = (size - scaled) * 0.5;
            renderer.draw_font_icon(
                font,
                *glyph,
                x + offset,
                y + offset,
                scaled,
                rgb(tint(color)),
            );
        }
        Icon::None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
        <rect width="4" height="4" fill="#00ff00"/></svg>"##;

    fn extension(dir: &Path) -> PathBuf {
        let root = dir.join("ext");
        std::fs::create_dir_all(root.join("theme/icons")).unwrap();
        std::fs::write(root.join("theme/icons/rust.svg"), SVG).unwrap();
        std::fs::write(
            root.join("theme/icons.json"),
            r##"{
                "iconDefinitions": {
                    "rust": {"iconPath": "./icons/rust.svg"},
                    "broken": {"iconPath": "./icons/missing.svg"},
                    "glyph": {"fontCharacter": "\\E001", "fontColor": "#123456"},
                    "custom": {"iconPath": "../../outside.svg"}
                },
                "fonts": [{"id": "f", "src": [{"path": "./icons/rust.svg"}]}],
                "file": "broken",
                "languageIds": {"rust": "rust", "shellscript": "glyph", "mylang": "custom"}
            }"##,
        )
        .unwrap();
        std::fs::write(dir.join("outside.svg"), SVG).unwrap();
        root
    }

    #[test]
    fn theme_icons_load_images_fonts_and_languages_lazily() {
        let dir = crate::test_support::TempDir::new();
        let root = extension(dir.path());
        let languages = HashMap::from([(".my".to_owned(), "mylang".to_owned())]);
        let theme =
            IconTheme::load("pack", &root.join("theme/icons.json"), &root, languages).unwrap();
        let resolve = |name: &str| theme.resolve(name, false, false, false, Variant::Dark);

        let Icon::Image(image) = resolve("main.rs") else {
            panic!("a Rust file uses the theme image");
        };
        assert_eq!(image.width, assets::RASTER_SIZE);
        // The same definition is decoded once.
        let Icon::Image(again) = resolve("lib.rs") else {
            panic!("cached image");
        };
        assert_eq!(image.id, again.id);

        let Icon::Glyph { glyph, color, .. } = resolve("run.sh") else {
            panic!("shell scripts use the font glyph");
        };
        assert_eq!((glyph, color), ('\u{e001}', Some(0x123456)));

        // Missing files and files outside the extension draw nothing.
        assert!(matches!(resolve("notes.zzz"), Icon::None));
        assert!(matches!(resolve("thing.my"), Icon::None));
    }

    #[test]
    fn a_theme_outside_its_extension_is_refused() {
        let dir = crate::test_support::TempDir::new();
        let root = extension(dir.path());
        assert!(
            IconTheme::load("x", &root.join("../outside.svg"), &root, HashMap::default()).is_err()
        );
    }

    #[test]
    fn the_selection_switches_between_builtin_disabled_and_theme_icons() {
        let dir = crate::test_support::TempDir::new();
        let root = extension(dir.path());
        let theme = Arc::new(
            IconTheme::load(
                "pack",
                &root.join("theme/icons.json"),
                &root,
                HashMap::default(),
            )
            .unwrap(),
        );
        set_selection(Selection::Builtin);
        assert_eq!(
            for_path(Path::new("src/main.rs")).builtin(),
            Some(file_icons::RUST)
        );
        set_selection(Selection::Disabled);
        assert!(matches!(for_entry("src", true, false, false), Icon::None));
        set_selection(Selection::Theme(theme));
        assert!(matches!(for_path(Path::new("main.rs")), Icon::Image(_)));
        assert!(!hides_explorer_arrows());
        set_selection(Selection::Builtin);
    }
}
