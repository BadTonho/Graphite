use super::*;
use std::collections::HashSet;

/// Copy theme JSON files and their inherited dependencies, preserving relative paths.
pub(super) fn import_directory(
    scan: &Path,
    package: &Path,
    destination: &Path,
    report: &mut ImportReport,
    translator: &Translator,
) {
    let Ok(package) = fs::canonicalize(package) else {
        return;
    };
    let mut files = Vec::new();
    collect(scan, &mut files);
    let mut seen = HashSet::new();
    for file in files {
        copy_with_dependencies(
            &file,
            &package,
            destination,
            &mut seen,
            &mut HashSet::new(),
            report,
            translator,
        );
    }
}

/// Preserve extension theme labels even when the referenced JSON has no `name` field.
pub(super) fn import_extension(
    package: &Path,
    destination: &Path,
    report: &mut ImportReport,
    translator: &Translator,
) {
    let Ok(root) = fs::canonicalize(package) else {
        return;
    };
    let themes = root.join("themes");
    let mut files = Vec::new();
    collect(&themes, &mut files);
    let mut seen = HashSet::new();
    for file in files {
        copy_with_dependencies(
            &file,
            &root,
            destination,
            &mut seen,
            &mut HashSet::new(),
            report,
            translator,
        );
    }
    let manifest_path = root.join("package.json");
    let manifest = match fs::read_to_string(&manifest_path) {
        Ok(raw) => match parse_jsonc(&raw) {
            Ok(value) => Some(value),
            Err(error) => {
                report.warnings.push(translator.format(
                    "config.theme.invalid_manifest",
                    &[("path", &manifest_path.display()), ("error", &error)],
                ));
                None
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            report.warnings.push(translator.format(
                "config.theme.cannot_read_manifest",
                &[("path", &manifest_path.display()), ("error", &error)],
            ));
            None
        }
    };
    let Some(entries) = manifest
        .as_ref()
        .and_then(|value| value.pointer("/contributes/themes"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let mut specs = Vec::new();
    for entry in entries {
        let Some(relative) = entry.get("path").and_then(Value::as_str) else {
            report.warnings.push(translator.format(
                "config.theme.invalid_path",
                &[("path", &manifest_path.display())],
            ));
            continue;
        };
        specs.push((
            entry.get("id").and_then(Value::as_str),
            entry.get("label").and_then(Value::as_str),
            relative,
        ));
    }
    import_entries(
        &root,
        &manifest_path,
        destination,
        &specs,
        report,
        translator,
    );
}

/// Import themes declared explicitly by a native plugin manifest (`graphite-plugin.json`).
pub(super) fn import_native(
    package: &Path,
    destination: &Path,
    entries: &[(Option<&str>, Option<&str>, &str)],
    report: &mut ImportReport,
    translator: &Translator,
) {
    let Ok(root) = fs::canonicalize(package) else {
        return;
    };
    let manifest_path = root.join("graphite-plugin.json");
    import_entries(
        &root,
        &manifest_path,
        destination,
        entries,
        report,
        translator,
    );
}

fn import_entries(
    root: &Path,
    manifest_path: &Path,
    destination: &Path,
    entries: &[(Option<&str>, Option<&str>, &str)],
    report: &mut ImportReport,
    translator: &Translator,
) {
    let mut seen = HashSet::new();
    let mut catalog = Map::new();
    let mut labels: Vec<String> = Vec::new();
    for (id, label, relative) in entries {
        let source = root.join(relative);
        let path = match fs::canonicalize(&source) {
            Ok(path) if path.starts_with(root) => path,
            _ => {
                report.warnings.push(translator.format(
                    "config.theme.invalid_path_relative",
                    &[("path", &manifest_path.display()), ("relative", relative)],
                ));
                continue;
            }
        };
        copy_with_dependencies(
            &path,
            root,
            destination,
            &mut seen,
            &mut HashSet::new(),
            report,
            translator,
        );
        let relative = path
            .strip_prefix(root)
            .expect("theme path verified inside root")
            .to_string_lossy()
            .replace('\\', "/");
        for name in [id, label].into_iter().flatten() {
            catalog.insert((*name).to_owned(), Value::String(relative.clone()));
        }
        // The label is what the Themes menu shows; the id is only an alias.
        if let Some(name) = label.or(*id) {
            if !labels.iter().any(|known| known == name) {
                labels.push(name.to_owned());
            }
        }
    }
    if !catalog.is_empty() {
        let result = fs::create_dir_all(destination).and_then(|_| {
            fs::write(
                destination.join(".graphite-theme-catalog.json"),
                serde_json::json!({"graphiteThemes":catalog,"graphiteThemeLabels":labels}).to_string(),
            )
        });
        if let Err(error) = result {
            report
                .warnings
                .push(translator.format("config.theme.cannot_write_catalog", &[("error", &error)]));
        }
    }
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect(&entry.path(), files);
        } else if kind.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            files.push(entry.path());
        }
    }
}

fn copy_with_dependencies(
    path: &Path,
    root: &Path,
    destination: &Path,
    seen: &mut HashSet<PathBuf>,
    active: &mut HashSet<PathBuf>,
    report: &mut ImportReport,
    translator: &Translator,
) {
    let path = match fs::canonicalize(path) {
        Ok(path) if path.starts_with(root) => path,
        Ok(path) => {
            report.warnings.push(translator.format(
                "config.theme.include_outside_package",
                &[("path", &path.display())],
            ));
            return;
        }
        Err(error) => {
            report.warnings.push(translator.format(
                "config.theme.cannot_read",
                &[("path", &path.display()), ("error", &error)],
            ));
            return;
        }
    };
    if active.contains(&path) {
        report.warnings.push(translator.format(
            "config.theme.circular_include",
            &[("path", &path.display())],
        ));
        return;
    }
    if !seen.insert(path.clone()) {
        return;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) => {
            report.warnings.push(translator.format(
                "config.theme.cannot_read",
                &[("path", &path.display()), ("error", &error)],
            ));
            return;
        }
    };
    let value = match parse_jsonc(&raw) {
        Ok(value) => value,
        Err(error) => {
            report.warnings.push(translator.format(
                "config.theme.invalid_file",
                &[("path", &path.display()), ("error", &error)],
            ));
            return;
        }
    };
    let target = destination.join(
        path.strip_prefix(root)
            .expect("theme path verified inside root"),
    );
    let result = fs::create_dir_all(target.parent().unwrap_or(destination))
        .and_then(|_| fs::copy(&path, &target));
    match result {
        Ok(_) => report.imported += 1,
        Err(error) => {
            report.warnings.push(translator.format(
                "config.theme.cannot_import",
                &[("path", &path.display()), ("error", &error)],
            ));
            return;
        }
    }
    if let Some(include) = value.get("include") {
        active.insert(path.clone());
        match include.as_str() {
            Some(include) => copy_with_dependencies(
                &path.parent().unwrap_or(root).join(include),
                root,
                destination,
                seen,
                active,
                report,
                translator,
            ),
            None => report.warnings.push(
                translator.format("config.theme.invalid_include", &[("path", &path.display())]),
            ),
        }
        active.remove(&path);
    }
}
