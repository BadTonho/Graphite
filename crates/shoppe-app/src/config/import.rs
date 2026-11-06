use super::*;

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
    let extensions = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join(".vscode/extensions"));
    import_from_with(user_dir, target, extensions.as_deref(), save_dark_theme)
}

pub(super) fn import_from_with(
    user_dir: &Path,
    target: &Path,
    extensions: Option<&Path>,
    set_theme: impl Fn(bool),
) -> ImportReport {
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
                    set_theme(!name.to_ascii_lowercase().contains("light"));
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
    if let Some(extensions) = extensions {
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
