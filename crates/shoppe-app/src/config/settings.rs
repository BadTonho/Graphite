use super::*;

pub fn project_settings(value: &Value) -> (Value, Vec<String>) {
    let keys = [
        "editor.fontSize",
        "editor.tabSize",
        "editor.insertSpaces",
        "workbench.colorTheme",
        "workbench.colorCustomizations",
        "files.readonlyInclude",
        "editor.detectIndentation",
        "files.trimTrailingWhitespace",
        "editor.rulers",
        "editor.wordWrap",
        "editor.wrappingIndent",
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

pub(crate) fn parse_jsonc(text: &str) -> Result<Value, serde_json::Error> {
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
    // Remove trailing commas after comments have been stripped, including
    // `value, // comment\n}`. Preserve commas inside quoted strings.
    let chars: Vec<char> = out.chars().collect();
    let mut cleaned = String::with_capacity(out.len());
    let (mut i, mut quoted, mut escape) = (0, false, false);
    while i < chars.len() {
        let c = chars[i];
        if quoted {
            cleaned.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quoted = false;
            }
        } else if c == '"' {
            quoted = true;
            cleaned.push(c);
        } else if c == ',' {
            let mut next = i + 1;
            while next < chars.len() && chars[next].is_whitespace() {
                next += 1;
            }
            if !matches!(chars.get(next), Some('}' | ']')) {
                cleaned.push(c);
            }
        } else {
            cleaned.push(c);
        }
        i += 1;
    }
    serde_json::from_str(&cleaned)
}
pub fn editor_prefs_for_workspace(workspace: &Path) -> Result<EditorPrefs, String> {
    read_editor_prefs(&settings_paths(workspace))
}

pub(super) fn settings_paths(workspace: &Path) -> [PathBuf; 3] {
    let config = app_config_dir();
    [
        config.join("imported/settings.json"),
        config.join("settings.json"),
        workspace.join(".vscode/settings.json"),
    ]
}

pub(super) fn merge_settings(paths: &[PathBuf]) -> Result<Value, String> {
    let mut merged = Map::new();
    for path in paths {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("Cannot read {}: {e}", path.display())),
        };
        let value =
            parse_jsonc(&raw).map_err(|e| format!("Invalid settings {}: {e}", path.display()))?;
        let values = value
            .as_object()
            .ok_or_else(|| format!("Settings {} must be an object", path.display()))?;
        for (key, value) in values {
            if key == "workbench.colorCustomizations" {
                let custom = value.as_object().ok_or_else(|| {
                    format!(
                        "Invalid workbench.colorCustomizations in {}",
                        path.display()
                    )
                })?;
                let target = merged
                    .entry(key.clone())
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(target) = target.as_object_mut() {
                    target.extend(custom.clone());
                }
            } else {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(Value::Object(merged))
}

pub(super) fn read_editor_prefs(paths: &[PathBuf]) -> Result<EditorPrefs, String> {
    let mut prefs = EditorPrefs {
        font_size: 15.0,
        tab_size: 4,
        insert_spaces: false,
        read_only_patterns: Vec::new(),
        detect_indentation: true,
        trim_trailing_whitespace: false,
        rulers: Vec::new(),
        word_wrap: false,
        wrapping_indent: WrapIndent::Same,
    };
    for path in paths {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("Cannot read {}: {e}", path.display())),
        };
        let value =
            parse_jsonc(&raw).map_err(|e| format!("Invalid settings {}: {e}", path.display()))?;
        if !value.is_object() {
            return Err(format!("Settings {} must be an object", path.display()));
        }
        if let Some(value) = value.get("editor.fontSize") {
            prefs.font_size = value
                .as_f64()
                .filter(|v| (8.0..=40.0).contains(v))
                .ok_or_else(|| format!("Invalid editor.fontSize in {}", path.display()))?
                as f32;
        }
        if let Some(value) = value.get("editor.tabSize") {
            prefs.tab_size = value
                .as_u64()
                .filter(|v| (1..=8).contains(v))
                .ok_or_else(|| format!("Invalid editor.tabSize in {}", path.display()))?
                as usize;
        }
        if let Some(value) = value.get("editor.insertSpaces") {
            prefs.insert_spaces = value
                .as_bool()
                .ok_or_else(|| format!("Invalid editor.insertSpaces in {}", path.display()))?;
        }
        if let Some(value) = value.get("editor.wordWrap") {
            prefs.word_wrap = match value.as_str() {
                Some("on") => true,
                Some("off") => false,
                _ => {
                    return Err(format!(
                        "Invalid or unsupported editor.wordWrap in {}",
                        path.display()
                    ))
                }
            };
        }
        if let Some(value) = value.get("editor.wrappingIndent") {
            prefs.wrapping_indent = match value.as_str() {
                Some("none") => WrapIndent::None,
                Some("same") => WrapIndent::Same,
                Some("indent") => WrapIndent::Indent,
                Some("deepIndent") => WrapIndent::DeepIndent,
                _ => {
                    return Err(format!(
                        "Invalid editor.wrappingIndent in {}",
                        path.display()
                    ))
                }
            };
        }
        if let Some(value) = value.get("files.readonlyInclude") {
            let mut result = Vec::new();
            let patterns: Vec<&str> = if let Some(items) = value.as_array() {
                items
                    .iter()
                    .map(|v| {
                        v.as_str().ok_or_else(|| {
                            format!("Invalid files.readonlyInclude in {}", path.display())
                        })
                    })
                    .collect::<Result<_, _>>()?
            } else if let Some(items) = value.as_object() {
                let mut patterns = Vec::new();
                for (pattern, enabled) in items {
                    match enabled.as_bool() {
                        Some(true) => patterns.push(pattern.as_str()),
                        Some(false) => {}
                        None => {
                            return Err(format!(
                                "Invalid files.readonlyInclude in {}",
                                path.display()
                            ))
                        }
                    }
                }
                patterns
            } else {
                return Err(format!(
                    "Invalid files.readonlyInclude in {}",
                    path.display()
                ));
            };
            for pattern in patterns {
                globset::Glob::new(pattern)
                    .map_err(|e| format!("Invalid read-only pattern in {}: {e}", path.display()))?;
                result.push(pattern.to_owned());
            }
            prefs.read_only_patterns = result;
        }
        if let Some(value) = value.get("editor.detectIndentation") {
            prefs.detect_indentation = value
                .as_bool()
                .ok_or_else(|| format!("Invalid editor.detectIndentation in {}", path.display()))?;
        }
        if let Some(value) = value.get("files.trimTrailingWhitespace") {
            prefs.trim_trailing_whitespace = value.as_bool().ok_or_else(|| {
                format!("Invalid files.trimTrailingWhitespace in {}", path.display())
            })?;
        }
        if let Some(value) = value.get("editor.rulers") {
            let values = value
                .as_array()
                .ok_or_else(|| format!("Invalid editor.rulers in {}", path.display()))?;
            let mut rulers = Vec::new();
            for ruler in values {
                let column = ruler
                    .as_u64()
                    .or_else(|| ruler.get("column").and_then(Value::as_u64))
                    .filter(|column| (1..=1000).contains(column))
                    .ok_or_else(|| format!("Invalid editor.rulers in {}", path.display()))?;
                rulers.push(column as usize);
            }
            rulers.sort_unstable();
            rulers.dedup();
            prefs.rulers = rulers;
        }
    }
    Ok(prefs)
}

pub fn workspace_configuration(workspace: &Path) -> Result<Value, String> {
    merge_settings(&settings_paths(workspace))
}
