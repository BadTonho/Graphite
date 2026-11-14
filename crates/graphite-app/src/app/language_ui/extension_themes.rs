//! Color themes contributed by enabled VSIX extensions (`contributes.themes`). Each enabled
//! extension's themes are imported into the profile, where `workbench.colorTheme` finds them
//! by label or id like any imported VS Code theme. Disabling or removing an extension removes
//! its themes again.

use super::*;
use graphite_plugins::manifest::Metadata;

/// Folder inside the profile's `imported/themes` that holds extension themes.
const THEMES_DIR: &str = "imported/themes/vsix";

/// An extension directory name that is safe on every platform.
fn directory_name(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

impl App {
    /// Brings the imported extension themes in line with the enabled extensions. When something
    /// changed, the selected theme is resolved again so a newly available theme applies.
    pub(crate) fn sync_extension_themes(&mut self) {
        let Some(root) = self.plugins.store.as_ref().map(|store| store.root.clone()) else {
            return;
        };
        let target = self.profile_root.join(THEMES_DIR);
        let mut wanted = std::collections::BTreeSet::new();
        let mut changed = false;
        for record in self
            .plugins
            .installed
            .iter()
            .filter(|record| record.enabled)
        {
            let (name, is_native) = match &record.metadata {
                Metadata::Vsix(vsix) if vsix.contributions.iter().any(|kind| kind == "themes") => {
                    (directory_name(&vsix.id), false)
                }
                Metadata::Native(native) if !native.themes.is_empty() => {
                    (format!("native-{}", directory_name(&native.id)), true)
                }
                _ => continue,
            };
            let destination = target.join(&name);
            let marker = destination.join(".graphite-extension-version");
            wanted.insert(name);
            if record.dev_path.is_none()
                && std::fs::read_to_string(&marker).is_ok_and(|version| version == record.directory)
            {
                continue;
            }
            let _ = std::fs::remove_dir_all(&destination);
            let report = if is_native {
                let Metadata::Native(native) = &record.metadata else {
                    continue;
                };
                let package = record.install_dir(&root);
                let entries = native
                    .themes
                    .iter()
                    .map(|theme| {
                        (
                            theme.id.as_deref(),
                            Some(theme.label.as_str()),
                            theme.path.as_str(),
                        )
                    })
                    .collect::<Vec<_>>();
                config::import_native_plugin_themes(
                    &package,
                    &destination,
                    &entries,
                    &self.translator,
                )
            } else {
                let package = root.join(&record.directory).join("extension");
                config::import_extension_themes(&package, &destination, &self.translator)
            };
            if let Some(warning) = report.warnings.first() {
                self.status = warning.clone();
            }
            let _ = std::fs::create_dir_all(&destination)
                .and_then(|_| std::fs::write(&marker, &record.directory));
            changed = true;
        }
        if let Ok(entries) = std::fs::read_dir(&target) {
            for entry in entries.flatten() {
                if !wanted.contains(&entry.file_name().to_string_lossy().into_owned()) {
                    let _ = std::fs::remove_dir_all(entry.path());
                    changed = true;
                }
            }
        }
        if changed {
            self.apply_selected_theme();
        }
        self.sync_icon_theme();
    }

    /// Resolves `workbench.colorTheme` again after the available themes changed. A selection
    /// that is not available (yet) leaves the current colors alone.
    fn apply_selected_theme(&mut self) {
        let workspace = self.tree.root().to_path_buf();
        if let Ok(colors) =
            config::imported_theme_colors_in(&self.profile_root, &workspace, &self.translator)
        {
            Theme::set_imported_colors(colors);
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn theme_package(path: &Path, background: &str) {
        let manifest = serde_json::json!({
            "publisher": "test", "name": "themes", "version": "1.0.0",
            "engines": {"vscode": "^1.100.0"},
            "contributes": {"themes": [
                {"label": "Ext Theme", "uiTheme": "vs-dark", "path": "./themes/ext.json"},
            ]},
        });
        let mut archive = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        archive
            .start_file("extension/package.json", options)
            .unwrap();
        serde_json::to_writer(&mut archive, &manifest).unwrap();
        archive
            .start_file("extension/themes/ext.json", options)
            .unwrap();
        write!(
            archive,
            r#"{{"name": "Ext Theme", "colors": {{"editor.background": "{background}"}}}}"#
        )
        .unwrap();
        archive.finish().unwrap();
    }

    #[test]
    fn enabled_extension_themes_resolve_by_label_and_vanish_when_disabled() {
        let dir = crate::test_support::TempDir::new();
        let package = dir.path().join("themes.vsix");
        theme_package(&package, "#123456");
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.profile_root = dir.path().join("profile");
        std::fs::create_dir_all(&app.profile_root).unwrap();
        std::fs::write(
            app.profile_root.join("settings.json"),
            r#"{"workbench.colorTheme": "Ext Theme"}"#,
        )
        .unwrap();
        let store = graphite_plugins::store::Store::new(app.profile_root.join("plugins"));
        store.install(&package).unwrap();
        app.plugins.store = Some(store.clone());
        let resolve = |app: &App| {
            config::imported_theme_colors_in(&app.profile_root, dir.path(), &app.translator)
        };

        app.plugins.installed = store.list().unwrap();
        app.sync_extension_themes();
        assert!(
            resolve(&app).is_err(),
            "a disabled extension provides no theme"
        );

        store.enable("test.themes", true).unwrap();
        app.plugins.installed = store.list().unwrap();
        app.sync_extension_themes();
        assert_eq!(resolve(&app).unwrap()["editor.background"], "#123456");

        store.disable("test.themes").unwrap();
        app.plugins.installed = store.list().unwrap();
        app.sync_extension_themes();
        assert!(resolve(&app).is_err());
    }

    #[test]
    fn directory_names_replace_unsafe_characters() {
        assert_eq!(directory_name("pub.name-1"), "pub.name-1");
        assert_eq!(directory_name("a/b:c"), "a_b_c");
    }
}
