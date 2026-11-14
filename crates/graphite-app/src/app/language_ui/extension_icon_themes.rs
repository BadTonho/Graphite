//! File icon themes contributed by enabled VSIX extensions (`contributes.iconThemes`).
//! `workbench.iconTheme` picks one by id; the picker writes that setting to the profile.

use super::*;
use crate::app::go_menu::{GoPicker, GoPickerKind, GoSymbolItem};
use crate::icon_theme::{IconTheme, Selection};
use graphite_plugins::manifest::Metadata;
use serde_json::Value;

const SETTING: &str = "workbench.iconTheme";

/// An icon theme an enabled extension offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AvailableIconTheme {
    pub id: String,
    pub label: String,
    pub extension: String,
    /// The extension folder and the theme file inside it.
    pub root: PathBuf,
    pub path: PathBuf,
}

/// What `workbench.iconTheme` asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Wanted {
    /// Not set: the built-in icons.
    Default,
    /// `null`: no file icons.
    Disabled,
    Theme(String),
}

fn wanted(settings: &Value) -> Wanted {
    match settings.get(SETTING) {
        None => Wanted::Default,
        Some(Value::Null) => Wanted::Disabled,
        Some(Value::String(id)) => Wanted::Theme(id.clone()),
        Some(_) => Wanted::Default,
    }
}

impl App {
    /// The icon themes of the enabled extensions, in installation order.
    pub(crate) fn available_icon_themes(&self) -> Vec<AvailableIconTheme> {
        let Some(store_root) = self.plugins.store.as_ref().map(|store| store.root.clone()) else {
            return Vec::new();
        };
        let mut themes = Vec::new();
        for record in self
            .plugins
            .installed
            .iter()
            .filter(|record| record.enabled)
        {
            let Metadata::Vsix(vsix) = &record.metadata else {
                continue;
            };
            if !vsix.contributions.iter().any(|kind| kind == "iconThemes") {
                continue;
            }
            let root = store_root.join(&record.directory).join("extension");
            let Some(manifest) = std::fs::read_to_string(root.join("package.json"))
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            else {
                continue;
            };
            for theme in graphite_plugins::manifest::vsix_icon_themes(&manifest) {
                themes.push(AvailableIconTheme {
                    path: root.join(&theme.path),
                    root: root.clone(),
                    id: theme.id,
                    label: theme.label,
                    extension: vsix.id.clone(),
                });
            }
        }
        themes
    }

    /// Language ids of the languages enabled extensions contribute, by lowercase file name
    /// and by extension with its dot.
    fn contributed_languages(&self) -> HashMap<String, String> {
        let Some(store_root) = self.plugins.store.as_ref().map(|store| store.root.clone()) else {
            return HashMap::new();
        };
        let mut languages = HashMap::new();
        for record in self
            .plugins
            .installed
            .iter()
            .filter(|record| record.enabled)
        {
            let Metadata::Vsix(vsix) = &record.metadata else {
                continue;
            };
            if !vsix.contributions.iter().any(|kind| kind == "languages") {
                continue;
            }
            let path = store_root
                .join(&record.directory)
                .join("extension/package.json");
            let Some(manifest) = std::fs::read_to_string(path)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            else {
                continue;
            };
            for language in manifest
                .pointer("/contributes/languages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(id) = language.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let names = |key: &str| {
                    language
                        .get(key)
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_lowercase)
                        .filter(|name| !name.is_empty())
                        .collect::<Vec<_>>()
                };
                let extensions = names("extensions")
                    .into_iter()
                    .map(|extension| format!(".{}", extension.trim_start_matches('.')));
                for key in names("filenames").into_iter().chain(extensions) {
                    languages.entry(key).or_insert_with(|| id.to_owned());
                }
            }
        }
        languages
    }

    /// Applies `workbench.iconTheme` with the themes the enabled extensions offer now. A theme
    /// that is not available (yet) leaves the built-in icons in place; one that cannot be loaded
    /// is reported in the status bar.
    pub(crate) fn sync_icon_theme(&mut self) {
        if let Err(error) = self.apply_icon_theme() {
            self.status = error;
        }
    }

    fn apply_icon_theme(&mut self) -> Result<(), String> {
        let settings = config::workspace_configuration_in(
            &self.profile_root,
            self.tree.root(),
            &self.translator,
        )
        .unwrap_or_default();
        let selection = match wanted(&settings) {
            Wanted::Default => Selection::Builtin,
            Wanted::Disabled => Selection::Disabled,
            Wanted::Theme(id) => match self
                .available_icon_themes()
                .into_iter()
                .find(|theme| theme.id == id)
            {
                None => Selection::Builtin,
                Some(available) => {
                    if let Selection::Theme(current) = crate::icon_theme::selection() {
                        if current.id == available.id && current.path == available.path {
                            return Ok(());
                        }
                    }
                    match IconTheme::load(
                        &available.id,
                        &available.path,
                        &available.root,
                        self.contributed_languages(),
                    ) {
                        Ok(theme) => Selection::Theme(Arc::new(theme)),
                        Err(error) => {
                            self.show_icon_selection(Selection::Builtin);
                            return Err(self.tf(
                                "icon_theme.error",
                                &[("name", &available.label), ("error", &error)],
                            ));
                        }
                    }
                }
            },
        };
        self.show_icon_selection(selection);
        Ok(())
    }

    fn show_icon_selection(&mut self, selection: Selection) {
        crate::icon_theme::set_selection(selection);
        self.tree.refresh_icons();
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    /// Opens the list of file icon themes; the current one is preselected.
    pub(crate) fn open_icon_theme_picker(&mut self) {
        let themes = self.available_icon_themes();
        let mut items = vec![
            GoSymbolItem {
                path: PathBuf::new(),
                line: 0,
                name: self.t("icon_theme.none").into(),
                scope: String::new(),
            },
            GoSymbolItem {
                path: PathBuf::new(),
                line: 1,
                name: self.t("icon_theme.builtin").into(),
                scope: String::new(),
            },
        ];
        items.extend(
            themes
                .iter()
                .enumerate()
                .map(|(index, theme)| GoSymbolItem {
                    path: PathBuf::new(),
                    line: index + 2,
                    name: theme.label.clone(),
                    scope: theme.extension.clone(),
                }),
        );
        let selected = match crate::icon_theme::selection() {
            Selection::Disabled => 0,
            Selection::Builtin => 1,
            Selection::Theme(current) => themes
                .iter()
                .position(|theme| theme.id == current.id)
                .map_or(1, |index| index + 2),
        };
        self.command_palette = None;
        self.quick_open = None;
        self.local_search = None;
        self.go_picker_generation = self.go_picker_generation.wrapping_add(1);
        self.go_picker = Some(GoPicker {
            kind: GoPickerKind::IconTheme,
            query: String::new(),
            items,
            selected,
            loading: false,
            generation: self.go_picker_generation,
            message: None,
        });
    }

    /// Applies the picked entry: 0 turns icons off, 1 restores the built-in icons, the rest
    /// are the available themes.
    pub(crate) fn select_icon_theme(&mut self, index: usize) {
        let themes = self.available_icon_themes();
        let (value, name) = match index {
            0 => (Some(Value::Null), self.t("icon_theme.none").to_owned()),
            1 => (None, self.t("icon_theme.builtin").to_owned()),
            index => match themes.get(index - 2) {
                Some(theme) => (Some(Value::from(theme.id.clone())), theme.label.clone()),
                None => return,
            },
        };
        if let Err(error) = config::set_user_setting(&self.profile_root, SETTING, value.as_ref()) {
            self.status = self.tf("icon_theme.save_error", &[("error", &error)]);
            return;
        }
        self.status = match self.apply_icon_theme() {
            Ok(()) => self.tf("icon_theme.selected", &[("name", &name)]),
            Err(error) => error,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="#00f"/></svg>"##;

    fn icon_package(path: &Path) {
        let manifest = serde_json::json!({
            "publisher": "test", "name": "icons", "version": "1.0.0",
            "engines": {"vscode": "^1.100.0"},
            "contributes": {
                "iconThemes": [{"id": "test-icons", "label": "Test Icons", "path": "./theme/icons.json"}],
                "languages": [{"id": "fancy", "extensions": [".fancy"]}],
            },
        });
        let mut archive = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        archive
            .start_file("extension/package.json", options)
            .unwrap();
        serde_json::to_writer(&mut archive, &manifest).unwrap();
        archive
            .start_file("extension/theme/icons.json", options)
            .unwrap();
        write!(
            archive,
            r#"{{"iconDefinitions": {{"f": {{"iconPath": "./f.svg"}}, "x": {{"iconPath": "./f.svg"}}}},
                "file": "f", "languageIds": {{"fancy": "x"}}, "hidesExplorerArrows": true}}"#
        )
        .unwrap();
        archive
            .start_file("extension/theme/f.svg", options)
            .unwrap();
        archive.write_all(SVG.as_bytes()).unwrap();
        archive.finish().unwrap();
    }

    fn app_with_package(dir: &Path) -> (App, graphite_plugins::store::Store) {
        let package = dir.join("icons.vsix");
        icon_package(&package);
        let mut app = App::new_with_options(dir.to_path_buf(), None, true);
        app.profile_root = dir.join("profile");
        std::fs::create_dir_all(&app.profile_root).unwrap();
        let store = graphite_plugins::store::Store::new(app.profile_root.join("plugins"));
        store.install(&package).unwrap();
        app.plugins.store = Some(store.clone());
        (app, store)
    }

    #[test]
    fn the_setting_selects_an_enabled_extension_theme() {
        let dir = crate::test_support::TempDir::new();
        let (mut app, store) = app_with_package(dir.path());
        std::fs::write(
            app.profile_root.join("settings.json"),
            r#"{"workbench.iconTheme": "test-icons"}"#,
        )
        .unwrap();

        app.plugins.installed = store.list().unwrap();
        app.sync_icon_theme();
        assert!(app.available_icon_themes().is_empty());
        assert!(matches!(crate::icon_theme::selection(), Selection::Builtin));

        store.enable("test.icons", true).unwrap();
        app.plugins.installed = store.list().unwrap();
        app.sync_icon_theme();
        let Selection::Theme(theme) = crate::icon_theme::selection() else {
            panic!("the enabled theme applies");
        };
        assert_eq!(theme.id, "test-icons");
        assert!(crate::icon_theme::hides_explorer_arrows());
        assert!(matches!(
            crate::icon_theme::for_path(Path::new("a.fancy")),
            crate::icon_theme::Icon::Image(_)
        ));

        store.disable("test.icons").unwrap();
        app.plugins.installed = store.list().unwrap();
        app.sync_icon_theme();
        assert!(matches!(crate::icon_theme::selection(), Selection::Builtin));
    }

    #[test]
    fn the_picker_lists_themes_and_writes_the_setting() {
        let dir = crate::test_support::TempDir::new();
        let (mut app, store) = app_with_package(dir.path());
        store.enable("test.icons", true).unwrap();
        app.plugins.installed = store.list().unwrap();
        crate::icon_theme::set_selection(Selection::Builtin);

        app.open_icon_theme_picker();
        let picker = app.go_picker.as_ref().unwrap();
        assert_eq!(picker.kind, GoPickerKind::IconTheme);
        assert_eq!(picker.selected, 1);
        let names: Vec<_> = picker.items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names[2], "Test Icons");
        assert_eq!(picker.items[2].scope, "test.icons");

        let settings = |app: &App| {
            config::parse_jsonc(
                &std::fs::read_to_string(app.profile_root.join("settings.json")).unwrap(),
            )
            .unwrap()
        };
        if let Some(picker) = app.go_picker.as_mut() {
            picker.selected = 2;
        }
        app.accept_go_picker();
        assert_eq!(settings(&app)[SETTING], "test-icons");
        assert!(matches!(
            crate::icon_theme::selection(),
            Selection::Theme(_)
        ));

        app.select_icon_theme(0);
        assert_eq!(settings(&app)[SETTING], Value::Null);
        assert!(matches!(
            crate::icon_theme::selection(),
            Selection::Disabled
        ));

        app.select_icon_theme(1);
        assert!(settings(&app).get(SETTING).is_none());
        assert!(matches!(crate::icon_theme::selection(), Selection::Builtin));
    }

    #[test]
    fn the_setting_value_is_read_as_default_disabled_or_an_id() {
        assert_eq!(wanted(&serde_json::json!({})), Wanted::Default);
        assert_eq!(
            wanted(&serde_json::json!({SETTING: null})),
            Wanted::Disabled
        );
        assert_eq!(
            wanted(&serde_json::json!({SETTING: "x"})),
            Wanted::Theme("x".into())
        );
        assert_eq!(wanted(&serde_json::json!({SETTING: 3})), Wanted::Default);
    }
}
