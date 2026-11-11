use super::*;

pub fn imported_theme_colors(
    workspace: &Path,
    translator: &Translator,
) -> Result<std::collections::HashMap<String, String>, String> {
    imported_theme_colors_in(&app_config_dir(), workspace, translator)
}

pub fn imported_theme_colors_in(
    config_root: &Path,
    workspace: &Path,
    translator: &Translator,
) -> Result<std::collections::HashMap<String, String>, String> {
    let settings = super::settings::merge_settings(
        &super::settings::settings_paths_in(config_root, workspace),
        translator,
    )?;
    let mut colors =
        resolve_theme_colors(&settings, &config_root.join("imported/themes"), translator)?;
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
    Ok(colors)
}

#[cfg(test)]
pub(super) fn theme_colors_from(
    settings: &Value,
    theme_dir: &Path,
    translator: &Translator,
) -> std::collections::HashMap<String, String> {
    resolve_theme_colors(settings, theme_dir, translator).unwrap_or_default()
}

pub(super) fn resolve_theme_colors(
    settings: &Value,
    theme_dir: &Path,
    translator: &Translator,
) -> Result<std::collections::HashMap<String, String>, String> {
    let mut colors = std::collections::HashMap::new();
    let mut found = false;
    let selected = match settings.get("workbench.colorTheme") {
        Some(Value::String(name)) => Some(name.as_str()),
        Some(_) => return Err(translator.text("config.theme.invalid_selection").into()),
        None => None,
    };
    if let Some(selected) = selected {
        let mut files = Vec::new();
        collect_theme_files(theme_dir, &mut files);
        files.sort();
        for file in files {
            let Ok(theme) = fs::read_to_string(&file) else {
                continue;
            };
            let Ok(theme) = parse_jsonc(&theme) else {
                continue;
            };
            let path = if theme.get("name").and_then(Value::as_str) == Some(selected) {
                Some(file.clone())
            } else {
                theme
                    .get("graphiteThemes")
                    .and_then(|catalog| catalog.get(selected))
                    .and_then(Value::as_str)
                    .map(|relative| file.parent().unwrap_or(theme_dir).join(relative))
            };
            if let Some(path) = path {
                found = true;
                let theme = super::theme_includes::load(&path, theme_dir, translator)?;
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
    if let Some(selected) = selected {
        if !found {
            return Err(translator.format("config.theme.not_found", &[("name", &selected)]));
        }
    }
    Ok(colors)
}

fn collect_theme_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect_theme_files(&entry.path(), files);
        } else if kind.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            files.push(entry.path());
        }
    }
}

pub fn load_dark_theme() -> bool {
    load_dark_theme_from(&app_config_dir())
}
pub fn load_dark_theme_from(config_root: &Path) -> bool {
    fs::read_to_string(config_root.join("theme.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("dark").and_then(Value::as_bool))
        .unwrap_or(true)
}
pub fn save_dark_theme(dark: bool) {
    save_dark_theme_in(&app_config_dir(), dark)
}
pub fn save_dark_theme_in(dir: &Path, dark: bool) {
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join("theme.json"), format!("{{\"dark\":{dark}}}"));
}

/// Whether high contrast was enabled in this profile (`high-contrast.json`).
pub fn load_high_contrast_from(config_root: &Path) -> bool {
    fs::read_to_string(config_root.join("high-contrast.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("enabled").and_then(Value::as_bool))
        .unwrap_or(false)
}

pub fn save_high_contrast_in(dir: &Path, enabled: bool) {
    let _ = fs::create_dir_all(dir);
    let _ = fs::write(
        dir.join("high-contrast.json"),
        format!("{{\"enabled\":{enabled}}}"),
    );
}

#[cfg(test)]
mod high_contrast_storage_tests {
    use super::*;

    #[test]
    fn high_contrast_choice_round_trips_and_defaults_to_off() {
        let dir = crate::test_support::TempDir::new();
        assert!(!load_high_contrast_from(dir.path()));
        save_high_contrast_in(dir.path(), true);
        assert!(load_high_contrast_from(dir.path()));
        save_high_contrast_in(dir.path(), false);
        assert!(!load_high_contrast_from(dir.path()));
    }
}
