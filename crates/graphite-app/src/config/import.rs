use super::*;

pub fn import_vscode_to(config_root: &Path, translator: &Translator) -> ImportReport {
    let Some(user_dir) = vscode_user_dir() else {
        return ImportReport {
            imported: 0,
            warnings: vec![translator.text("config.import.user_dir_missing").into()],
        };
    };
    import_from_with(
        &user_dir,
        &config_root.join("imported"),
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(|home| PathBuf::from(home).join(".vscode/extensions"))
            .as_deref(),
        |dark| save_dark_theme_in(config_root, dark),
        translator,
    )
}

pub(super) fn import_from_with(
    user_dir: &Path,
    target: &Path,
    extensions: Option<&Path>,
    set_theme: impl Fn(bool),
    translator: &Translator,
) -> ImportReport {
    let mut report = ImportReport::default();
    if let Err(e) = fs::create_dir_all(target) {
        report
            .warnings
            .push(translator.format("config.import.create_directory", &[("error", &e)]));
        return report;
    }
    let settings = user_dir.join("settings.json");
    match fs::read_to_string(settings) {
        Ok(text) => match parse_jsonc(&text) {
            Ok(value) => {
                let (supported, warnings) = project_settings(&value, translator);
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
                .push(translator.format("config.import.invalid_settings", &[("error", &e)])),
        },
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => report
            .warnings
            .push(translator.format("config.import.cannot_read_settings", &[("error", &e)])),
        _ => {}
    }
    for folder in ["snippets", "themes"] {
        let source = user_dir.join(folder);
        if !source.is_dir() {
            continue;
        }
        let dest = target.join(folder);
        if let Err(e) = fs::create_dir_all(&dest) {
            report.warnings.push(translator.format(
                "config.import.create_folder",
                &[("folder", &folder), ("error", &e)],
            ));
            continue;
        }
        if folder == "themes" {
            super::import_themes::import_directory(
                &source,
                &source,
                &dest,
                &mut report,
                translator,
            );
            continue;
        }
        match fs::read_dir(source) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && (folder == "snippets" || path.extension().is_some_and(|e| e == "json"))
                    {
                        if let Err(e) = fs::read_to_string(&path)
                            .map_err(|e| e.to_string())
                            .and_then(|raw| parse_jsonc(&raw).map_err(|e| e.to_string()))
                        {
                            report.warnings.push(translator.format(
                                "config.import.invalid_file",
                                &[
                                    ("folder", &folder),
                                    ("path", &path.display()),
                                    ("error", &e),
                                ],
                            ));
                            continue;
                        }
                        if let Some(name) = path.file_name() {
                            match fs::copy(&path, dest.join(name)) {
                                Ok(_) => report.imported += 1,
                                Err(e) => report.warnings.push(translator.format(
                                    "config.import.cannot_copy",
                                    &[("path", &path.display()), ("error", &e)],
                                )),
                            }
                        }
                    }
                }
            }
            Err(e) => report.warnings.push(translator.format(
                "config.import.cannot_read_folder",
                &[("folder", &folder), ("error", &e)],
            )),
        }
    }
    if let Some(extensions) = extensions {
        if let Ok(entries) = fs::read_dir(extensions) {
            for ext in entries.flatten() {
                let package = ext.path();
                let themes = package.join("themes");
                let destination = target.join("themes").join(ext.file_name());
                if themes.is_dir() {
                    super::import_themes::import_extension(
                        &package,
                        &destination,
                        &mut report,
                        translator,
                    );
                }
            }
        }
    }
    report
}
