use super::*;
use std::collections::HashSet;

/// Copy theme JSON files and their inherited dependencies, preserving relative paths.
pub(super) fn import_directory(
    scan: &Path,
    package: &Path,
    destination: &Path,
    report: &mut ImportReport,
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
        );
    }
}

/// Preserve extension theme labels even when the referenced JSON has no `name` field.
pub(super) fn import_extension(package: &Path, destination: &Path, report: &mut ImportReport) {
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
        );
    }
    let manifest_path = root.join("package.json");
    let manifest = match fs::read_to_string(&manifest_path) {
        Ok(raw) => match parse_jsonc(&raw) {
            Ok(value) => Some(value),
            Err(error) => {
                report.warnings.push(format!(
                    "Invalid extension manifest {}: {error}",
                    manifest_path.display()
                ));
                None
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            report.warnings.push(format!(
                "Cannot read extension manifest {}: {error}",
                manifest_path.display()
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
    let mut catalog = Map::new();
    for entry in entries {
        let Some(relative) = entry.get("path").and_then(Value::as_str) else {
            report
                .warnings
                .push(format!("Invalid theme path in {}", manifest_path.display()));
            continue;
        };
        let source = root.join(relative);
        let path = match fs::canonicalize(&source) {
            Ok(path) if path.starts_with(&root) => path,
            _ => {
                report.warnings.push(format!(
                    "Invalid theme path in {}: {relative}",
                    manifest_path.display()
                ));
                continue;
            }
        };
        copy_with_dependencies(
            &path,
            &root,
            destination,
            &mut seen,
            &mut HashSet::new(),
            report,
        );
        let relative = path
            .strip_prefix(&root)
            .expect("theme path verified inside root")
            .to_string_lossy()
            .replace('\\', "/");
        for key in ["id", "label"] {
            if let Some(name) = entry.get(key).and_then(Value::as_str) {
                catalog.insert(name.to_owned(), Value::String(relative.clone()));
            }
        }
    }
    if !catalog.is_empty() {
        let result = fs::create_dir_all(destination).and_then(|_| {
            fs::write(
                destination.join(".shoppe-theme-catalog.json"),
                serde_json::json!({"shoppeThemes":catalog}).to_string(),
            )
        });
        if let Err(error) = result {
            report
                .warnings
                .push(format!("Cannot write theme catalog: {error}"));
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
) {
    let path = match fs::canonicalize(path) {
        Ok(path) if path.starts_with(root) => path,
        Ok(path) => {
            report.warnings.push(format!(
                "Theme include is outside package: {}",
                path.display()
            ));
            return;
        }
        Err(error) => {
            report
                .warnings
                .push(format!("Cannot read theme {}: {error}", path.display()));
            return;
        }
    };
    if active.contains(&path) {
        report
            .warnings
            .push(format!("Circular theme include: {}", path.display()));
        return;
    }
    if !seen.insert(path.clone()) {
        return;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) => {
            report
                .warnings
                .push(format!("Cannot read theme {}: {error}", path.display()));
            return;
        }
    };
    let value = match parse_jsonc(&raw) {
        Ok(value) => value,
        Err(error) => {
            report
                .warnings
                .push(format!("Invalid theme file {}: {error}", path.display()));
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
            report
                .warnings
                .push(format!("Cannot import theme {}: {error}", path.display()));
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
            ),
            None => report
                .warnings
                .push(format!("Invalid theme include: {}", path.display())),
        }
        active.remove(&path);
    }
}
