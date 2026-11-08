use super::*;

#[derive(Debug, Clone)]
pub struct Snippet {
    pub prefix: String,
    pub body: String,
    pub scope: Option<Vec<String>>,
}
pub(super) fn expand_snippet(body: &str) -> String {
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
pub fn load_snippets_for_workspace(workspace: &Path) -> (Vec<Snippet>, Vec<String>) {
    load_snippets_from_dirs(
        &app_config_dir().join("imported/snippets"),
        &workspace.join(".vscode"),
    )
}

pub(super) fn load_snippets_from_dirs(
    global_dir: &Path,
    project_dir: &Path,
) -> (Vec<Snippet>, Vec<String>) {
    let (mut snippets, mut warnings) = read_snippet_files(global_dir, false);
    let (project, project_warnings) = read_snippet_files(project_dir, true);
    warnings.extend(project_warnings);
    let project_keys = project
        .iter()
        .map(|snippet| (snippet.prefix.clone(), snippet.scope.clone()))
        .collect::<std::collections::HashSet<_>>();
    snippets
        .retain(|snippet| !project_keys.contains(&(snippet.prefix.clone(), snippet.scope.clone())));
    snippets.extend(project);
    snippets.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    (snippets, warnings)
}

fn read_snippet_files(dir: &Path, code_snippets_only: bool) -> (Vec<Snippet>, Vec<String>) {
    let mut found = Vec::new();
    let mut warnings = Vec::new();
    let files = match fs::read_dir(dir) {
        Ok(files) => files,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (found, warnings),
        Err(error) => {
            warnings.push(format!(
                "Cannot read snippets in {}: {error}",
                dir.display()
            ));
            return (found, warnings);
        }
    };
    let mut files = files
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    files.sort();
    for path in files {
        if !path.is_file() {
            continue;
        }
        if code_snippets_only {
            if !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".code-snippets"))
            {
                continue;
            }
        } else if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let data = match fs::read(&path) {
            Ok(data) => data,
            Err(error) => {
                warnings.push(format!(
                    "Cannot read snippet file {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        let value = match serde_json::from_slice::<Value>(&data) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!("Invalid snippets {}: {error}", path.display()));
                continue;
            }
        };
        let Some(entries) = value.as_object() else {
            warnings.push(format!(
                "Invalid snippets {}: expected an object",
                path.display()
            ));
            continue;
        };
        for (name, item) in entries {
            let Some(prefix) = item.get("prefix") else {
                warnings.push(format!(
                    "Snippet '{name}' in {} has no prefix",
                    path.display()
                ));
                continue;
            };
            let prefixes: Vec<String> = if let Some(s) = prefix.as_str() {
                vec![s.to_owned()]
            } else {
                prefix
                    .as_array()
                    .map(|array| {
                        array
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let body = item
                .get("body")
                .map(|value| {
                    if let Some(text) = value.as_str() {
                        text.to_owned()
                    } else {
                        value
                            .as_array()
                            .map(|array| {
                                array
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .unwrap_or_default()
                    }
                })
                .unwrap_or_default();
            if prefixes.is_empty() || body.is_empty() {
                warnings.push(format!(
                    "Snippet '{name}' in {} needs a prefix and body",
                    path.display()
                ));
                continue;
            }
            let scope = item.get("scope").and_then(|scope| {
                if let Some(scope) = scope.as_str() {
                    Some(
                        scope
                            .split(',')
                            .map(str::trim)
                            .filter(|part| !part.is_empty())
                            .map(str::to_owned)
                            .collect(),
                    )
                } else {
                    scope.as_array().map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                }
            });
            let body = expand_snippet(&body);
            for prefix in prefixes {
                found.push(Snippet {
                    prefix,
                    body: body.clone(),
                    scope: scope.clone(),
                });
            }
        }
    }
    found.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    (found, warnings)
}
