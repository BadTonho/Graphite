mod import;
mod paths;
mod settings;
mod snippets;
mod themes;
#[cfg(test)]
use import::import_from;
pub use import::import_vscode;
use paths::vscode_user_dir;
pub use paths::{app_config_dir, lsp_config_path, terminal_shell};
pub use settings::editor_prefs;
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
#[derive(Debug, Clone, Copy)]
pub struct EditorPrefs {
    pub font_size: f32,
    pub tab_size: usize,
    pub insert_spaces: bool,
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
    fn import_copies_valid_files_and_preserves_sources() {
        let root = std::env::temp_dir().join("shoppe_import_test");
        let _ = fs::remove_dir_all(&root);
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
        let report = import_from(&source, &target);
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
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn imported_theme_tokens_map_to_editor_color_roles() {
        let dir = std::env::temp_dir().join("shoppe_theme_tokens_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
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
        let _ = fs::remove_dir_all(dir);
    }
}
