use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
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

pub fn load_snippets_for_workspace_in(
    config_root: &Path,
    workspace: &Path,
    translator: &Translator,
) -> (Vec<Snippet>, Vec<String>) {
    load_snippets_from_dirs(
        &config_root.join("imported/snippets"),
        &workspace.join(".vscode"),
        translator,
    )
}

pub(super) fn load_snippets_from_dirs(
    global_dir: &Path,
    project_dir: &Path,
    translator: &Translator,
) -> (Vec<Snippet>, Vec<String>) {
    let (mut snippets, mut warnings) = read_snippet_files(global_dir, false, translator);
    let (project, project_warnings) = read_snippet_files(project_dir, true, translator);
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

/// Snippets and warnings from one snippet file (`{ "name": { "prefix", "body", "scope" } }`).
pub(crate) fn parse_snippet_file(
    path: &Path,
    translator: &Translator,
) -> (Vec<Snippet>, Vec<String>) {
    let mut found = Vec::new();
    let mut warnings = Vec::new();
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(error) => {
            warnings.push(translator.format(
                "config.snippets.cannot_read_file",
                &[("path", &path.display()), ("error", &error)],
            ));
            return (found, warnings);
        }
    };
    let value = match serde_json::from_slice::<Value>(&data) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(translator.format(
                "config.snippets.invalid",
                &[("path", &path.display()), ("error", &error)],
            ));
            return (found, warnings);
        }
    };
    let Some(entries) = value.as_object() else {
        warnings
            .push(translator.format("config.snippets.not_object", &[("path", &path.display())]));
        return (found, warnings);
    };
    for (name, item) in entries {
        let Some(prefix) = item.get("prefix") else {
            warnings.push(translator.format(
                "config.snippets.missing_prefix",
                &[("name", name), ("path", &path.display())],
            ));
            return (found, warnings);
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
            warnings.push(translator.format(
                "config.snippets.missing_body",
                &[("name", name), ("path", &path.display())],
            ));
            return (found, warnings);
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

    found.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    (found, warnings)
}

fn read_snippet_files(
    dir: &Path,
    code_snippets_only: bool,
    translator: &Translator,
) -> (Vec<Snippet>, Vec<String>) {
    let mut found = Vec::new();
    let mut warnings = Vec::new();
    let files = match fs::read_dir(dir) {
        Ok(files) => files,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (found, warnings),
        Err(error) => {
            warnings.push(translator.format(
                "config.snippets.cannot_read_dir",
                &[("path", &dir.display()), ("error", &error)],
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
        let (snippets, file_warnings) = parse_snippet_file(&path, translator);
        found.extend(snippets);
        warnings.extend(file_warnings);
    }
    found.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    (found, warnings)
}
