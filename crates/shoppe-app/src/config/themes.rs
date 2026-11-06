use super::*;

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

pub(super) fn theme_colors_from(
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
