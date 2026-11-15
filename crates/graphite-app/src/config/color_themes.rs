//! Color themes the user can pick from the Themes menu: the themes bundled with Graphite plus
//! every theme imported into the profile (imported settings, extensions and native plugins).

use super::*;
use std::collections::{BTreeMap, HashSet};

/// Source of the bundled Graphite Simple theme (also published as a standalone package).
const GRAPHITE_SIMPLE: &str = include_str!(
    "../../../../assets/themes/graphite-simple/themes/graphite-simple-color-theme.json"
);

/// Folder inside `imported/themes` that holds the themes bundled with the application.
const BUNDLED_DIR: &str = "imported/themes/bundled";

/// A color theme that `workbench.colorTheme` can select by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ColorTheme {
    pub name: String,
    pub dark: bool,
}

/// Writes the bundled themes into the profile so they are selectable like imported ones.
/// Files are rewritten only when their content differs.
pub(crate) fn install_bundled_themes(config_root: &Path) {
    let dir = config_root.join(BUNDLED_DIR);
    let file = dir.join("graphite-simple.json");
    if fs::read_to_string(&file).is_ok_and(|current| current == GRAPHITE_SIMPLE) {
        return;
    }
    let _ = fs::create_dir_all(&dir).and_then(|_| fs::write(file, GRAPHITE_SIMPLE));
}

fn is_dark(theme: &Value) -> bool {
    !matches!(
        theme.get("type").and_then(Value::as_str),
        Some("light" | "hc-light")
    )
}

fn normalized(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Lists the selectable color themes of the profile, sorted by name without duplicates.
pub(crate) fn list_color_themes(config_root: &Path) -> Vec<ColorTheme> {
    let themes_dir = config_root.join("imported/themes");
    let mut files = Vec::new();
    themes::collect_theme_files(&themes_dir, &mut files);
    files.sort();
    let mut claimed = HashSet::new();
    let mut named: Vec<(PathBuf, ColorTheme)> = Vec::new();
    let mut found: BTreeMap<String, ColorTheme> = BTreeMap::new();
    for file in files {
        let Some(theme) = fs::read_to_string(&file)
            .ok()
            .and_then(|text| parse_jsonc(&text).ok())
        else {
            continue;
        };
        if let Some(catalog) = theme.get("graphiteThemes").and_then(Value::as_object) {
            let labels: Vec<&str> = match theme.get("graphiteThemeLabels") {
                Some(Value::Array(labels)) => labels.iter().filter_map(Value::as_str).collect(),
                _ => catalog.keys().map(String::as_str).collect(),
            };
            for label in labels {
                let Some(relative) = catalog.get(label).and_then(Value::as_str) else {
                    continue;
                };
                let path = normalized(&file.parent().unwrap_or(&themes_dir).join(relative));
                let dark = fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| parse_jsonc(&text).ok())
                    .is_none_or(|theme| is_dark(&theme));
                claimed.insert(path);
                found.insert(
                    label.to_lowercase(),
                    ColorTheme {
                        name: label.to_owned(),
                        dark,
                    },
                );
            }
        } else if ["colors", "tokenColors", "include"]
            .iter()
            .any(|key| theme.get(key).is_some())
        {
            if let Some(name) = theme.get("name").and_then(Value::as_str) {
                named.push((
                    normalized(&file),
                    ColorTheme {
                        name: name.to_owned(),
                        dark: is_dark(&theme),
                    },
                ));
            }
        }
    }
    for (path, theme) in named {
        if !claimed.contains(&path) {
            found.entry(theme.name.to_lowercase()).or_insert(theme);
        }
    }
    found.into_values().collect()
}

/// The effective `workbench.colorTheme` for the workspace, if any layer sets one.
pub(crate) fn selected_color_theme(
    config_root: &Path,
    workspace: &Path,
    translator: &Translator,
) -> Option<String> {
    let settings = settings::merge_settings(
        &settings::settings_paths_in(config_root, workspace),
        translator,
    )
    .ok()?;
    settings
        .get("workbench.colorTheme")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Stores (or clears, with `None`) the user-level `workbench.colorTheme`.
pub(crate) fn set_color_theme(config_root: &Path, name: Option<&str>) -> Result<(), String> {
    let value = name.map(|name| Value::String(name.to_owned()));
    settings_edit::set_user_setting(config_root, "workbench.colorTheme", value.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn bundled_graphite_simple_is_installed_listed_and_selectable() {
        let temp = crate::test_support::TempDir::new();
        assert!(list_color_themes(temp.path()).is_empty());
        install_bundled_themes(temp.path());
        install_bundled_themes(temp.path());
        assert_eq!(
            list_color_themes(temp.path()),
            [ColorTheme {
                name: "Graphite Simple".into(),
                dark: true
            }]
        );
        set_color_theme(temp.path(), Some("Graphite Simple")).unwrap();
        let translator = Translator::english();
        assert_eq!(
            selected_color_theme(temp.path(), temp.path(), &translator).as_deref(),
            Some("Graphite Simple")
        );
        let colors = imported_theme_colors_in(temp.path(), temp.path(), &translator).unwrap();
        assert_eq!(
            colors.get("editor.background").map(String::as_str),
            Some("#1e1e1e")
        );
        set_color_theme(temp.path(), None).unwrap();
        assert_eq!(
            selected_color_theme(temp.path(), temp.path(), &translator),
            None
        );
    }

    #[test]
    fn extension_themes_are_listed_once_by_label_with_their_brightness() {
        let temp = crate::test_support::TempDir::new();
        let dir = temp.path().join("imported/themes/vsix/pub.ext");
        write(
            &dir.join("themes/day.json"),
            r##"{"name":"Day File Name","type":"light","colors":{"editor.background":"#ffffff"}}"##,
        );
        write(
            &dir.join("themes/night.json"),
            r##"{"colors":{"editor.background":"#000000"}}"##,
        );
        write(
            &dir.join(".graphite-theme-catalog.json"),
            r#"{"graphiteThemes":{"day":"themes/day.json","Day":"themes/day.json","Night":"themes/night.json"},"graphiteThemeLabels":["Day","Night"]}"#,
        );
        write(&dir.join("package.json"), r#"{"name":"not-a-theme"}"#);
        write(
            &dir.join("base.json"),
            r##"{"colors":{"editor.foreground":"#eeeeee"}}"##,
        );
        assert_eq!(
            list_color_themes(temp.path()),
            [
                ColorTheme {
                    name: "Day".into(),
                    dark: false
                },
                ColorTheme {
                    name: "Night".into(),
                    dark: true
                },
            ]
        );
    }

    #[test]
    fn invalid_theme_files_are_ignored_and_names_are_not_duplicated() {
        let temp = crate::test_support::TempDir::new();
        let dir = temp.path().join("imported/themes");
        write(&dir.join("broken.json"), "{");
        write(&dir.join("a/one.json"), r##"{"name":"Same","colors":{}}"##);
        write(&dir.join("b/two.json"), r##"{"name":"same","colors":{}}"##);
        assert_eq!(list_color_themes(temp.path()).len(), 1);
    }
}
