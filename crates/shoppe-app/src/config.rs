mod import;
mod paths;
mod settings;
mod snippets;
mod themes;
#[cfg(test)]
use import::import_from_with;
pub use import::import_vscode;
use paths::vscode_user_dir;
pub use paths::{app_config_dir, lsp_config_path, terminal_shell};
pub use settings::editor_prefs_for_workspace;
use settings::{parse_jsonc, project_settings};
#[cfg(test)]
use snippets::expand_snippet;
pub use snippets::{load_snippets, Snippet};
#[cfg(test)]
use themes::theme_colors_from;
pub use themes::{imported_theme_colors, load_dark_theme, save_dark_theme};

use serde_json::{Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Default)]
pub struct ImportReport {
    pub imported: usize,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct EditorPrefs {
    pub font_size: f32,
    pub tab_size: usize,
    pub insert_spaces: bool,
    pub read_only_patterns: Vec<String>,
    pub detect_indentation: bool,
    pub trim_trailing_whitespace: bool,
    pub rulers: Vec<usize>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expands_common_vscode_snippet_placeholders() {
        assert_eq!(expand_snippet("fn ${1:name}() { $0 }"), "fn name() {  }");
    }
    #[test]
    fn projects_supported_settings_and_reports_unknown_keys() {
        let source: Value =
            serde_json::from_str(r#"{"editor.fontSize":18,"other.setting":true}"#).unwrap();
        let (result, warnings) = project_settings(&source);
        assert_eq!(result["editor.fontSize"], 18);
        assert!(result.get("other.setting").is_none());
        assert_eq!(warnings.len(), 1);
    }
    #[test]
    fn malformed_settings_are_rejected() {
        assert!(parse_jsonc("not json").is_err());
        assert_eq!(
            parse_jsonc("{ // settings\n \"editor.tabSize\": 2, } ").unwrap()["editor.tabSize"],
            2
        );
    }

    #[test]
    fn settings_layers_validate_and_override_in_order() {
        let temp = crate::test_support::TempDir::new();
        let imported = temp.path().join("imported.json");
        let user = temp.path().join("user.json");
        let workspace = temp.path().join("workspace.json");
        fs::write(&imported, r#"{"editor.fontSize": 13, "editor.tabSize": 2}"#).unwrap();
        fs::write(
            &user,
            "{ // user\n \"editor.fontSize\": 17, \"editor.insertSpaces\": true, }",
        )
        .unwrap();
        fs::write(&workspace, r#"{"editor.tabSize": 8, "editor.rulers": [100, {"column": 80}, 100], "files.readonlyInclude": {"generated/**": true, "src/**": false}}"#).unwrap();
        let paths = [imported, user, workspace.clone()];
        let prefs = settings::read_editor_prefs(&paths).unwrap();
        assert_eq!(
            (prefs.font_size, prefs.tab_size, prefs.insert_spaces),
            (17.0, 8, true)
        );
        assert_eq!(prefs.read_only_patterns, ["generated/**"]);
        assert_eq!(prefs.rulers, [80, 100]);
        fs::write(&workspace, r#"{"editor.tabSize": 0}"#).unwrap();
        assert!(settings::read_editor_prefs(&paths)
            .unwrap_err()
            .contains("editor.tabSize"));
        fs::write(&workspace, "{").unwrap();
        assert!(settings::read_editor_prefs(&paths).is_err());
    }
    #[test]
    fn import_copies_valid_files_and_preserves_sources() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        let source = root.join("vscode");
        let target = root.join("app");
        fs::create_dir_all(source.join("snippets")).unwrap();
        fs::create_dir_all(source.join("themes")).unwrap();
        let settings = br#"{"editor.fontSize":18,"unsupported":true}"#;
        fs::write(source.join("settings.json"), settings).unwrap();
        fs::write(
            source.join("snippets/code.json"),
            br#"{"sample":{"prefix":"s","body":"hello"}}"#,
        )
        .unwrap();
        fs::write(source.join("snippets/bad.json"), "{").unwrap();
        fs::write(
            source.join("themes/theme.json"),
            br#"{"name":"sample","type":"dark"}"#,
        )
        .unwrap();
        let report = import_from_with(&source, &target, None, |_| {});
        assert_eq!(fs::read(source.join("settings.json")).unwrap(), settings);
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(target.join("settings.json")).unwrap())
                .unwrap()["editor.fontSize"],
            18
        );
        assert!(target.join("snippets/code.json").exists());
        assert!(target.join("themes/theme.json").exists());
        assert!(report
            .warnings
            .iter()
            .any(|w| w.contains("Invalid snippets")));
        assert!(report
            .warnings
            .iter()
            .any(|w| w.contains("Unsupported setting")));
    }

    #[test]
    fn imported_theme_tokens_map_to_editor_color_roles() {
        let temp = crate::test_support::TempDir::new();
        let dir = temp.path();
        fs::write(dir.join("theme.json"),r##"{"name":"Sample","colors":{"editor.background":"#101010"},"tokenColors":[{"scope":["keyword.control"],"settings":{"foreground":"#ff0000"}}]}"##).unwrap();
        let settings: Value = serde_json::from_str(r#"{"workbench.colorTheme":"Sample"}"#).unwrap();
        let colors = theme_colors_from(&settings, &dir);
        assert_eq!(
            colors.get("editor.background").map(String::as_str),
            Some("#101010")
        );
        assert_eq!(
            colors.get("syntax.keyword").map(String::as_str),
            Some("#ff0000")
        );
    }

    #[test]
    fn malformed_import_is_reported_without_overwriting_existing_settings() {
        let temp = crate::test_support::TempDir::new();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("settings.json"), "{").unwrap();
        fs::write(target.join("settings.json"), "original").unwrap();
        let report = import_from_with(&source, &target, None, |_| {
            panic!("unexpected theme change")
        });
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("Invalid VS Code settings")));
        assert_eq!(
            fs::read_to_string(target.join("settings.json")).unwrap(),
            "original"
        );
    }
}
