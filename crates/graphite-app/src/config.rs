mod color_themes;
mod import;
mod import_themes;
mod language;
mod paths;
mod settings;
mod settings_edit;
mod sidebar_layout;
mod snippets;
mod theme_includes;
mod themes;
mod view_layout;
pub(crate) use color_themes::{
    install_bundled_themes, list_color_themes, selected_color_theme, set_color_theme,
};
#[cfg(test)]
use import::import_from_with;
pub use import::import_vscode_to;
pub(crate) use language::{language_selection, language_selection_in};
pub(crate) use paths::terminal_settings_in;
use paths::vscode_user_dir;
pub use paths::{app_config_dir, lsp_config_path, TerminalProfile};
pub(crate) use settings::parse_jsonc;
use settings::project_settings;
pub use settings::{editor_prefs_for_workspace_in, workspace_configuration_in};
pub(crate) use settings_edit::set_user_setting;
pub(crate) use sidebar_layout::clamp_width as clamp_sidebar_width;
pub(crate) use sidebar_layout::MIN_SECONDARY_WIDTH as MIN_SECONDARY_SIDEBAR_WIDTH;
pub(crate) use sidebar_layout::{
    clear_secondary_sidebar_width, load_secondary_sidebar_width, save_secondary_sidebar_width,
};
pub(crate) use sidebar_layout::{
    load_activity_bar_preferences, save_activity_bar_preferences, ActivityBarPosition,
    ActivityBarPreferences, ActivityBarSize,
};
pub(crate) use sidebar_layout::{load_sidebar_width, save_sidebar_width};
pub(crate) use sidebar_layout::{
    DEFAULT_WIDTH as DEFAULT_SIDEBAR_WIDTH, MAX_PERSISTED_WIDTH as MAX_PERSISTED_SIDEBAR_WIDTH,
    MIN_WIDTH as MIN_SIDEBAR_WIDTH,
};
#[cfg(test)]
use snippets::expand_snippet;
pub(crate) use snippets::parse_snippet_file;
pub use snippets::{load_snippets_for_workspace_in, Snippet};
pub(crate) use view_layout::NotificationPosition;

/// Imports the themes a VSIX contributes (`contributes.themes`) from its extracted `package`
/// folder into `destination`, where `workbench.colorTheme` can select them by label or id.
pub(crate) fn import_extension_themes(
    package: &Path,
    destination: &Path,
    translator: &Translator,
) -> ImportReport {
    let mut report = ImportReport::default();
    import_themes::import_extension(package, destination, &mut report, translator);
    report
}

/// Imports themes declared by an enabled native Graphite plugin into `destination`.
pub(crate) fn import_native_plugin_themes(
    package: &Path,
    destination: &Path,
    entries: &[(Option<&str>, Option<&str>, &str)],
    translator: &Translator,
) -> ImportReport {
    let mut report = ImportReport::default();
    import_themes::import_native(package, destination, entries, &mut report, translator);
    report
}
#[cfg(test)]
use themes::theme_colors_from;
pub use themes::{
    imported_theme_colors_in, load_dark_theme_from, load_high_contrast_from, save_dark_theme_in,
    save_high_contrast_in,
};
pub(crate) use view_layout::{
    load_view_layout_preferences, save_view_layout_preferences, MultiCursorModifier, PanelPosition,
    ViewLayoutPreferences, WhitespaceRendering,
};

use graphite_i18n::Translator;
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
/// When dirty documents are saved automatically (`files.autoSave`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoSaveMode {
    Off,
    AfterDelay,
    OnFocusChange,
    OnWindowChange,
}

impl AutoSaveMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "afterDelay" => Some(Self::AfterDelay),
            "onFocusChange" => Some(Self::OnFocusChange),
            "onWindowChange" => Some(Self::OnWindowChange),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorPrefs {
    pub font_size: f32,
    pub tab_size: usize,
    pub insert_spaces: bool,
    pub read_only_patterns: Vec<String>,
    pub detect_indentation: bool,
    pub trim_trailing_whitespace: bool,
    pub format_on_save: bool,
    pub format_on_paste: bool,
    pub format_on_type: bool,
    /// `files.autoSave` when set in settings; otherwise the File menu toggle decides.
    pub auto_save: Option<AutoSaveMode>,
    pub auto_save_delay_ms: u64,
    pub rulers: Vec<usize>,
    /// `editor.wordWrap` when set; `None` leaves wrapping to the default of each file type.
    pub word_wrap: Option<bool>,
    pub wrapping_indent: WrapIndent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapIndent {
    None,
    Same,
    Indent,
    DeepIndent,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapping_settings_follow_layers_and_reject_unsupported_values() {
        let temp = crate::test_support::TempDir::new();
        let user = temp.path().join("user.json");
        let workspace = temp.path().join("workspace.json");
        fs::write(
            &user,
            r#"{"editor.wordWrap":"on","editor.wrappingIndent":"indent"}"#,
        )
        .unwrap();
        fs::write(&workspace, r#"{"editor.wrappingIndent":"deepIndent"}"#).unwrap();
        let prefs = settings::read_editor_prefs(&[user, workspace.clone()], &Translator::english())
            .unwrap();
        assert_eq!(prefs.word_wrap, Some(true));
        assert_eq!(prefs.wrapping_indent, WrapIndent::DeepIndent);
        fs::write(&workspace, r#"{"editor.wordWrap":"bounded"}"#).unwrap();
        assert!(settings::read_editor_prefs(&[workspace], &Translator::english()).is_err());
    }
    #[test]
    fn format_on_save_is_read_from_settings_and_rejects_non_boolean_values() {
        let temp = crate::test_support::TempDir::new();
        let workspace = temp.path().join("settings.json");
        fs::write(&workspace, r#"{"editor.formatOnSave":true}"#).unwrap();
        let prefs =
            settings::read_editor_prefs(std::slice::from_ref(&workspace), &Translator::english())
                .unwrap();
        assert!(prefs.format_on_save);
        assert!(!prefs.format_on_paste);
        fs::write(&workspace, r#"{"editor.formatOnSave":"yes"}"#).unwrap();
        assert!(settings::read_editor_prefs(
            std::slice::from_ref(&workspace),
            &Translator::english()
        )
        .is_err());
        fs::write(&workspace, r#"{"editor.formatOnPaste":1}"#).unwrap();
        assert!(settings::read_editor_prefs(&[workspace], &Translator::english()).is_err());
    }

    #[test]
    fn format_on_type_is_off_by_default_and_read_from_settings() {
        let temp = crate::test_support::TempDir::new();
        let workspace = temp.path().join("settings.json");
        fs::write(&workspace, r#"{"editor.formatOnSave":true}"#).unwrap();
        let prefs =
            settings::read_editor_prefs(std::slice::from_ref(&workspace), &Translator::english())
                .unwrap();
        assert!(!prefs.format_on_type);
        fs::write(&workspace, r#"{"editor.formatOnType":true}"#).unwrap();
        let prefs =
            settings::read_editor_prefs(std::slice::from_ref(&workspace), &Translator::english())
                .unwrap();
        assert!(prefs.format_on_type);
        fs::write(&workspace, r#"{"editor.formatOnType":"yes"}"#).unwrap();
        assert!(settings::read_editor_prefs(&[workspace], &Translator::english()).is_err());
    }

    #[test]
    fn auto_save_mode_and_delay_are_read_from_settings() {
        let temp = crate::test_support::TempDir::new();
        let workspace = temp.path().join("settings.json");
        fs::write(
            &workspace,
            r#"{"files.autoSave":"onFocusChange","files.autoSaveDelay":250}"#,
        )
        .unwrap();
        let prefs =
            settings::read_editor_prefs(std::slice::from_ref(&workspace), &Translator::english())
                .unwrap();
        assert_eq!(prefs.auto_save, Some(AutoSaveMode::OnFocusChange));
        assert_eq!(prefs.auto_save_delay_ms, 250);
        fs::write(&workspace, r#"{"files.autoSave":"always"}"#).unwrap();
        assert!(settings::read_editor_prefs(&[workspace], &Translator::english()).is_err());
    }

    #[test]
    fn expands_common_vscode_snippet_placeholders() {
        assert_eq!(expand_snippet("fn ${1:name}() { $0 }"), "fn name() {  }");
    }

    #[test]
    fn project_snippets_override_user_prefixes_respect_scopes_and_report_invalid_files() {
        let temp = crate::test_support::TempDir::new();
        let global = temp.path().join("global");
        let project = temp.path().join("workspace/.vscode");
        fs::create_dir_all(&global).unwrap();
        fs::create_dir_all(&project).unwrap();
        fs::write(
            global.join("user.json"),
            r#"{"global":{"prefix":"dup","body":"user"}}"#,
        )
        .unwrap();
        fs::write(
            project.join("project.code-snippets"),
            r#"{"project":{"prefix":"dup","body":["project","body"]},"rustOnly":{"prefix":"rs","body":"fn","scope":["rust"]}}"#,
        )
        .unwrap();
        fs::write(project.join("broken.code-snippets"), "{").unwrap();
        let (snippets, warnings) =
            snippets::load_snippets_from_dirs(&global, &project, &Translator::english());
        assert_eq!(snippets.iter().filter(|s| s.prefix == "dup").count(), 1);
        assert_eq!(
            snippets.iter().find(|s| s.prefix == "dup").unwrap().body,
            "project\nbody"
        );
        assert!(snippets.iter().any(|s| s.prefix == "rs"
            && s.scope
                .as_ref()
                .is_some_and(|scope| scope.len() == 1 && scope[0] == "rust")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("broken.code-snippets")));
    }
    #[test]
    fn projects_supported_settings_and_reports_unknown_keys() {
        let source: Value =
            serde_json::from_str(r#"{"editor.fontSize":18,"other.setting":true}"#).unwrap();
        let (result, warnings) = project_settings(&source, &Translator::english());
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
        assert_eq!(
            parse_jsonc("{\"text\":\"x, }\", // trailing\n}").unwrap()["text"],
            "x, }"
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
        let translator = Translator::english();
        let prefs = settings::read_editor_prefs(&paths, &translator).unwrap();
        assert_eq!(
            (prefs.font_size, prefs.tab_size, prefs.insert_spaces),
            (17.0, 8, true)
        );
        assert_eq!(prefs.read_only_patterns, ["generated/**"]);
        assert_eq!(prefs.rulers, [80, 100]);
        fs::write(&workspace, r#"{"editor.tabSize": 0}"#).unwrap();
        assert!(settings::read_editor_prefs(&paths, &translator)
            .unwrap_err()
            .contains("editor.tabSize"));
        fs::write(&workspace, "{").unwrap();
        assert!(settings::read_editor_prefs(&paths, &translator).is_err());
    }

    #[test]
    fn color_customizations_merge_across_settings_layers() {
        let temp = crate::test_support::TempDir::new();
        let user = temp.path().join("user.json");
        let workspace = temp.path().join("workspace.json");
        fs::write(&user, r##"{"workbench.colorTheme":"A","workbench.colorCustomizations":{"editor.background":"#111111","editor.foreground":"#222222"}}"##).unwrap();
        fs::write(&workspace, r##"{"workbench.colorTheme":"B","workbench.colorCustomizations":{"editor.background":"#333333"}}"##).unwrap();
        let merged = settings::merge_settings(&[user, workspace], &Translator::english()).unwrap();
        assert_eq!(merged["workbench.colorTheme"], "B");
        assert_eq!(
            merged["workbench.colorCustomizations"]["editor.background"],
            "#333333"
        );
        assert_eq!(
            merged["workbench.colorCustomizations"]["editor.foreground"],
            "#222222"
        );
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
        let report = import_from_with(&source, &target, None, |_| {}, &Translator::english());
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
        let colors = theme_colors_from(&settings, dir, &Translator::english());
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
    fn theme_jsonc_includes_inherit_and_override_colors_and_tokens() {
        let temp = crate::test_support::TempDir::new();
        fs::write(
            temp.path().join("base.json"),
            r##"{ // base
            "colors": {"editor.background": "#101010", "editor.foreground": "#cccccc"},
            "tokenColors": [{"scope": "keyword", "settings": {"foreground": "#112233"}}],
        }"##,
        )
        .unwrap();
        fs::write(
            temp.path().join("child.json"),
            r##"{
            "name": "Custom", "include": "./base.json",
            "colors": {"editor.background": "#202020"},
            "tokenColors": [{"scope": "keyword", "settings": {"foreground": "#aabbcc"}}]
        }"##,
        )
        .unwrap();
        let settings = serde_json::json!({"workbench.colorTheme": "Custom"});
        let colors =
            themes::resolve_theme_colors(&settings, temp.path(), &Translator::english()).unwrap();
        assert_eq!(
            colors.get("editor.background").map(String::as_str),
            Some("#202020")
        );
        assert_eq!(
            colors.get("editor.foreground").map(String::as_str),
            Some("#cccccc")
        );
        assert_eq!(
            colors.get("syntax.keyword").map(String::as_str),
            Some("#aabbcc")
        );
    }

    #[test]
    fn theme_include_cycles_and_missing_files_are_reported() {
        let temp = crate::test_support::TempDir::new();
        let child = temp.path().join("child.json");
        fs::write(&child, r#"{"name":"Loop","include":"base.json"}"#).unwrap();
        let settings = serde_json::json!({"workbench.colorTheme": "Loop"});
        let translator = Translator::english();
        assert!(
            themes::resolve_theme_colors(&settings, temp.path(), &translator)
                .unwrap_err()
                .contains("Cannot open theme")
        );
        fs::write(temp.path().join("base.json"), r#"{"include":"child.json"}"#).unwrap();
        assert!(
            themes::resolve_theme_colors(&settings, temp.path(), &translator)
                .unwrap_err()
                .contains("Circular")
        );
    }

    #[test]
    fn imports_nested_extension_theme_and_its_relative_dependency() {
        let temp = crate::test_support::TempDir::new();
        let source = temp.path().join("user");
        let target = temp.path().join("imported");
        let extensions = temp.path().join("extensions");
        let package = extensions.join("publisher.theme");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(package.join("themes")).unwrap();
        fs::create_dir_all(package.join("shared")).unwrap();
        let child =
            r##"{"include":"../shared/base.json", "colors":{"editor.background":"#123456"}}"##;
        fs::write(
            source.join("settings.json"),
            r#"{"workbench.colorTheme":"Custom"}"#,
        )
        .unwrap();
        fs::write(package.join("themes/custom.json"), child).unwrap();
        fs::write(
            package.join("package.json"),
            r#"{"contributes":{"themes":[{"label":"Custom","path":"./themes/custom.json"}]}}"#,
        )
        .unwrap();
        fs::write(
            package.join("shared/base.json"),
            r##"{// base
          "colors":{"editor.foreground":"#eeeeee"},
        }"##,
        )
        .unwrap();
        let report = import_from_with(
            &source,
            &target,
            Some(&extensions),
            |_| {},
            &Translator::english(),
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(
            fs::read_to_string(package.join("themes/custom.json")).unwrap(),
            child
        );
        assert!(target
            .join("themes/publisher.theme/shared/base.json")
            .exists());
        let settings = serde_json::json!({"workbench.colorTheme":"Custom"});
        let colors =
            themes::resolve_theme_colors(&settings, &target.join("themes"), &Translator::english())
                .unwrap();
        assert_eq!(
            colors.get("editor.background").map(String::as_str),
            Some("#123456")
        );
        assert_eq!(
            colors.get("editor.foreground").map(String::as_str),
            Some("#eeeeee")
        );
    }

    #[test]
    fn bundled_graphite_simple_theme_imports_and_resolves_the_dark_palette() {
        let bundled =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/themes/graphite-simple");
        let temp = crate::test_support::TempDir::new();
        let source = temp.path().join("user");
        let target = temp.path().join("imported");
        let extensions = temp.path().join("extensions");
        let package = extensions.join("graphite.graphite-simple");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(package.join("themes")).unwrap();
        fs::write(
            source.join("settings.json"),
            r#"{"workbench.colorTheme":"Graphite Simple"}"#,
        )
        .unwrap();
        for file in ["package.json", "themes/graphite-simple-color-theme.json"] {
            fs::copy(bundled.join(file), package.join(file)).unwrap();
        }
        let report = import_from_with(
            &source,
            &target,
            Some(&extensions),
            |_| {},
            &Translator::english(),
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let settings = serde_json::json!({"workbench.colorTheme":"Graphite Simple"});
        let colors =
            themes::resolve_theme_colors(&settings, &target.join("themes"), &Translator::english())
                .unwrap();
        let expected = [
            ("editor.background", "#1e1e1e"),
            ("editor.foreground", "#d4d4d4"),
            ("sideBar.background", "#252526"),
            ("activityBar.background", "#333333"),
            ("statusBar.background", "#007acc"),
            ("syntax.keyword", "#569cd6"),
            ("syntax.string", "#ce9178"),
            ("syntax.comment", "#6a9955"),
        ];
        for (key, value) in expected {
            assert_eq!(colors.get(key).map(String::as_str), Some(value), "{key}");
        }
    }

    #[test]
    fn theme_import_reports_missing_references_and_cycles() {
        let temp = crate::test_support::TempDir::new();
        let source = temp.path().join("user");
        let target = temp.path().join("target");
        fs::create_dir_all(source.join("themes")).unwrap();
        fs::write(
            source.join("themes/cycle.json"),
            r#"{"name":"cycle","include":"cycle.json"}"#,
        )
        .unwrap();
        fs::write(
            source.join("themes/missing.json"),
            r#"{"name":"missing","include":"no-such-file.json"}"#,
        )
        .unwrap();
        let report = import_from_with(&source, &target, None, |_| {}, &Translator::english());
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("Circular theme include")),
            "{:?}",
            report.warnings
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("Cannot read theme")),
            "{:?}",
            report.warnings
        );
        assert!(source.join("themes/cycle.json").exists());
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
        let report = import_from_with(
            &source,
            &target,
            None,
            |_| panic!("unexpected theme change"),
            &Translator::english(),
        );
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("Invalid settings.json")));
        assert_eq!(
            fs::read_to_string(target.join("settings.json")).unwrap(),
            "original"
        );
    }
}
