//! Resolve VS Code JSON/JSONC theme inheritance without reading outside its theme root.
use super::*;
use std::collections::HashSet;

pub(super) fn load(path: &Path, root: &Path) -> Result<Value, String> {
    let root = fs::canonicalize(root).map_err(|e| format!("Cannot open theme directory: {e}"))?;
    load_inner(path, &root, &mut HashSet::new())
}

fn load_inner(path: &Path, root: &Path, active: &mut HashSet<PathBuf>) -> Result<Value, String> {
    let path =
        fs::canonicalize(path).map_err(|e| format!("Cannot open theme {}: {e}", path.display()))?;
    if !path.starts_with(root) {
        return Err(format!(
            "Theme include is outside theme directory: {}",
            path.display()
        ));
    }
    if !active.insert(path.clone()) {
        return Err(format!("Circular theme include: {}", path.display()));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read theme {}: {e}", path.display()))?;
    let mut child =
        parse_jsonc(&raw).map_err(|e| format!("Invalid theme {}: {e}", path.display()))?;
    if !child.is_object() {
        return Err(format!("Theme must be an object: {}", path.display()));
    }
    let include = match child.get("include") {
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => return Err(format!("Invalid theme include: {}", path.display())),
        None => None,
    };
    if let Some(include) = include {
        let parent_path = path.parent().unwrap_or(root).join(include);
        let base = load_inner(&parent_path, root, active)?;
        merge_theme(&mut child, &base);
    }
    if child.get("tokenColors").is_some_and(|v| !v.is_array()) {
        return Err(format!("Unsupported tokenColors in {}", path.display()));
    }
    active.remove(&path);
    Ok(child)
}

fn merge_theme(child: &mut Value, base: &Value) {
    if let Some(base_colors) = base.get("colors").and_then(Value::as_object) {
        let Some(object) = child.as_object_mut() else {
            return;
        };
        let colors = object
            .entry("colors")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(colors) = colors.as_object_mut() {
            for (key, value) in base_colors {
                colors.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
    }
    if let Some(base_tokens) = base.get("tokenColors").and_then(Value::as_array) {
        let Some(object) = child.as_object_mut() else {
            return;
        };
        let tokens = object
            .entry("tokenColors")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(tokens) = tokens.as_array_mut() {
            tokens.extend(base_tokens.iter().cloned());
        }
    }
}
