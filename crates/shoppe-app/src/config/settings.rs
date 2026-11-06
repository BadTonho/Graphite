use super::*;

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

pub(super) fn parse_jsonc(text: &str) -> Result<Value, serde_json::Error> {
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
