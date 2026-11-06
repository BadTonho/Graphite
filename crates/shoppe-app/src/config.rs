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
#[derive(Debug, Clone)]
pub struct Snippet {
    pub prefix: String,
    pub body: String,
}
fn expand_snippet(body: &str) -> String {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && matches!(chars[i + 1], '$' | '`' | '\\') {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if chars[i] == '$' {
            if chars.get(i + 1) == Some(&'{') {
                if let Some(close) = (i + 2..chars.len()).find(|j| chars[*j] == '}') {
                    let content: String = chars[i + 2..close].iter().collect();
                    if let Some((_, default)) = content.split_once(':') {
                        out.push_str(default);
                    }
                    i = close + 1;
                    continue;
                }
            }
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 {
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}
pub fn load_snippets() -> Vec<Snippet> {
    let mut found = Vec::new();
    let dir = app_config_dir().join("imported/snippets");
    let Ok(files) = fs::read_dir(dir) else {
        return found;
    };
    for file in files.flatten() {
        let Ok(data) = fs::read(file.path()) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&data) else {
            continue;
        };
        let Some(entries) = value.as_object() else {
            continue;
        };
        for item in entries.values() {
            let Some(prefix) = item.get("prefix") else {
                continue;
            };
            let prefixes: Vec<String> = if let Some(s) = prefix.as_str() {
                vec![s.to_owned()]
            } else {
                prefix
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let body = item
                .get("body")
                .map(|v| {
                    if let Some(s) = v.as_str() {
                        s.to_owned()
                    } else {
                        v.as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .unwrap_or_default()
                    }
                })
                .unwrap_or_default();
            let body = expand_snippet(&body);
            for prefix in prefixes {
                found.push(Snippet {
                    prefix,
                    body: body.clone(),
                });
            }
        }
    }
    found.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    found
}
pub fn project_settings(value: &Value) -> (Value, Vec<String>) {
    let keys = [
        "editor.fontSize",
        "editor.tabSize",
        "editor.insertSpaces",
        "workbench.colorTheme",
        "workbench.colorCustomizations",
    ];
    let mut supported = Map::new();
    let mut warnings = Vec::new();
    for key in keys {
        if let Some(v) = value.get(key) {
            supported.insert(key.into(), v.clone());
        }
    }
    for key in value.as_object().into_iter().flat_map(|o| o.keys()) {
        if !keys.contains(&key.as_str()) {
            warnings.push(format!("Unsupported setting ignored: {key}"));
        }
    }
    (Value::Object(supported), warnings)
}

fn parse_jsonc(text: &str) -> Result<Value, serde_json::Error> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let (mut i, mut quoted, mut escape) = (0, false, false);
    while i < chars.len() {
        let c = chars[i];
        if quoted {
            out.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quoted = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            quoted = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }
        if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if chars.get(j) == Some(&'}') || chars.get(j) == Some(&']') {
                i += 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    serde_json::from_str(&out)
}
pub fn editor_prefs() -> EditorPrefs {
    let parsed = fs::read_to_string(app_config_dir().join("imported/settings.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());
    let font = parsed
        .as_ref()
        .and_then(|v| v.get("editor.fontSize"))
        .and_then(Value::as_f64)
        .unwrap_or(15.0)
        .clamp(8.0, 40.0) as f32;
    let tab = parsed
        .as_ref()
        .and_then(|v| v.get("editor.tabSize"))
        .and_then(Value::as_u64)
        .unwrap_or(4)
        .clamp(1, 8) as usize;
    let spaces = parsed
        .as_ref()
        .and_then(|v| v.get("editor.insertSpaces"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    EditorPrefs {
        font_size: font,
        tab_size: tab,
        insert_spaces: spaces,
    }
}

pub fn imported_theme_colors() -> std::collections::HashMap<String, String> {
    let settings_path = app_config_dir().join("imported/settings.json");
    let Ok(settings) = fs::read_to_string(settings_path) else {
        return Default::default();
    };
    let Ok(settings) = serde_json::from_str::<Value>(&settings) else {
        return Default::default();
    };
    let mut colors = theme_colors_from(&settings, &app_config_dir().join("imported/themes"));
    if let Some(custom) = settings
        .get("workbench.colorCustomizations")
        .and_then(Value::as_object)
    {
        colors.extend(
            custom
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned()))),
        );
    }
    colors
}

fn theme_colors_from(
    settings: &Value,
    theme_dir: &Path,
) -> std::collections::HashMap<String, String> {
    let mut colors = std::collections::HashMap::new();
    let selected = settings.get("workbench.colorTheme").and_then(Value::as_str);
    if let (Some(selected), Ok(files)) = (selected, fs::read_dir(theme_dir)) {
        for file in files.flatten() {
            let Ok(theme) = fs::read(file.path()) else {
                continue;
            };
            let Ok(theme) = serde_json::from_slice::<Value>(&theme) else {
                continue;
            };
            if theme.get("name").and_then(Value::as_str) == Some(selected) {
                if let Some(values) = theme.get("colors").and_then(Value::as_object) {
                    colors.extend(
                        values
                            .iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned()))),
                    );
                }
                if let Some(tokens) = theme.get("tokenColors").and_then(Value::as_array) {
                    for token in tokens {
                        let scopes: Vec<String> = match token.get("scope") {
                            Some(Value::String(s)) => {
                                s.split(',').map(|v| v.trim().to_owned()).collect()
                            }
                            Some(Value::Array(a)) => a
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect(),
                            _ => Vec::new(),
                        };
                        let Some(color) = token
                            .pointer("/settings/foreground")
                            .and_then(Value::as_str)
                        else {
                            continue;
                        };
                        for scope in scopes {
                            let role = if scope.contains("keyword") {
                                Some("syntax.keyword")
                            } else if scope.contains("function") {
                                Some("syntax.function")
                            } else if scope.contains("type") || scope.contains("class") {
                                Some("syntax.type")
                            } else if scope.contains("string") {
                                Some("syntax.string")
                            } else if scope.contains("comment") {
                                Some("syntax.comment")
                            } else if scope.contains("number") || scope.contains("constant") {
                                Some("syntax.number")
                            } else if scope.contains("variable") || scope.contains("property") {
                                Some("syntax.variable")
                            } else {
                                None
                            };
                            if let Some(role) = role {
                                colors
                                    .entry(role.to_owned())
                                    .or_insert_with(|| color.to_owned());
                            }
                        }
                    }
                }
                break;
            }
        }
    }
    colors
}

pub fn app_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("VsCodeShoppe")
}

pub fn vscode_user_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("Code/User"))
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("Code/User"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

pub fn load_dark_theme() -> bool {
    fs::read_to_string(app_config_dir().join("theme.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("dark").and_then(Value::as_bool))
        .unwrap_or(true)
}
pub fn save_dark_theme(dark: bool) {
    let dir = app_config_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join("theme.json"), format!("{{\"dark\":{dark}}}"));
}

pub fn import_vscode() -> ImportReport {
    let Some(user_dir) = vscode_user_dir() else {
        return ImportReport {
            imported: 0,
            warnings: vec!["VS Code user directory was not found".into()],
        };
    };
    import_from(&user_dir, &app_config_dir().join("imported"))
}

pub fn import_from(user_dir: &Path, target: &Path) -> ImportReport {
    let mut report = ImportReport::default();
    if let Err(e) = fs::create_dir_all(&target) {
        report
            .warnings
            .push(format!("Cannot create import directory: {e}"));
        return report;
    }
    let settings = user_dir.join("settings.json");
    match fs::read_to_string(settings) {
        Ok(text) => match parse_jsonc(&text) {
            Ok(value) => {
                let (supported, warnings) = project_settings(&value);
                report.warnings.extend(warnings);
                if let Some(name) = value.get("workbench.colorTheme").and_then(Value::as_str) {
                    save_dark_theme(!name.to_ascii_lowercase().contains("light"));
                }
                if fs::write(
                    target.join("settings.json"),
                    serde_json::to_vec_pretty(&supported).unwrap_or_default(),
                )
                .is_ok()
                {
                    report.imported += 1;
                }
            }
            Err(e) => report
                .warnings
                .push(format!("Invalid VS Code settings.json: {e}")),
        },
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => report
            .warnings
            .push(format!("Cannot read VS Code settings.json: {e}")),
        _ => {}
    }
    for folder in ["snippets", "themes"] {
        let source = user_dir.join(folder);
        if !source.is_dir() {
            continue;
        }
        let dest = target.join(folder);
        if let Err(e) = fs::create_dir_all(&dest) {
            report
                .warnings
                .push(format!("Cannot create {folder} import folder: {e}"));
            continue;
        }
        match fs::read_dir(source) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && (folder == "snippets" || path.extension().is_some_and(|e| e == "json"))
                    {
                        if let Err(e) =
                            serde_json::from_slice::<Value>(&fs::read(&path).unwrap_or_default())
                        {
                            report.warnings.push(format!(
                                "Invalid {} file {}: {e}",
                                folder,
                                path.display()
                            ));
                            continue;
                        }
                        if let Some(name) = path.file_name() {
                            match fs::copy(&path, dest.join(name)) {
                                Ok(_) => report.imported += 1,
                                Err(e) => report
                                    .warnings
                                    .push(format!("Cannot import {}: {e}", path.display())),
                            }
                        }
                    }
                }
            }
            Err(e) => report.warnings.push(format!("Cannot read {folder}: {e}")),
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let extensions = PathBuf::from(home).join(".vscode/extensions");
        if let Ok(entries) = fs::read_dir(extensions) {
            for ext in entries.flatten() {
                let themes = ext.path().join("themes");
                if let Ok(files) = fs::read_dir(themes) {
                    for file in files.flatten() {
                        let path = file.path();
                        if path.extension().is_some_and(|e| e == "json") {
                            if serde_json::from_slice::<Value>(&fs::read(&path).unwrap_or_default())
                                .is_err()
                            {
                                report
                                    .warnings
                                    .push(format!("Invalid theme file: {}", path.display()));
                                continue;
                            }
                            let dest = target.join("themes");
                            let _ = fs::create_dir_all(&dest);
                            match fs::copy(&path, dest.join(path.file_name().unwrap_or_default())) {
                                Ok(_) => report.imported += 1,
                                Err(e) => report
                                    .warnings
                                    .push(format!("Cannot import theme {}: {e}", path.display())),
                            }
                        }
                    }
                }
            }
        }
    }
    report
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
