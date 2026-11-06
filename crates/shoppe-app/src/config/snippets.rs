use super::*;

#[derive(Debug, Clone)]
pub struct Snippet {
    pub prefix: String,
    pub body: String,
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
pub fn load_snippets() -> Vec<Snippet> {
    let mut found = Vec::new();
    let dir = app_config_dir().join("imported/snippets");
    let Ok(files) = fs::read_dir(dir) else {
        return found;
    };
    for file in files.flatten() {
        let Ok(data) = fs::read(file.path()) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&data) else {
            continue;
        };
        let Some(entries) = value.as_object() else {
            continue;
        };
        for item in entries.values() {
            let Some(prefix) = item.get("prefix") else {
                continue;
            };
            let prefixes: Vec<String> = if let Some(s) = prefix.as_str() {
                vec![s.to_owned()]
            } else {
                prefix
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let body = item
                .get("body")
                .map(|v| {
                    if let Some(s) = v.as_str() {
                        s.to_owned()
                    } else {
                        v.as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .unwrap_or_default()
                    }
                })
                .unwrap_or_default();
            let body = expand_snippet(&body);
            for prefix in prefixes {
                found.push(Snippet {
                    prefix,
                    body: body.clone(),
                });
            }
        }
    }
    found.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    found
}
