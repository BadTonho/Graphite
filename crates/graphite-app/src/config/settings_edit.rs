//! Changing one top-level key of a JSONC settings file while keeping the rest of the text
//! (comments, order, formatting) as the user wrote it.

use serde_json::Value;
use std::path::Path;

/// A top-level member: byte spans of the whole member (key to value end) and of the value.
#[derive(Debug)]
struct Member {
    key: String,
    start: usize,
    value_start: usize,
    end: usize,
}

struct Scan {
    open: usize,
    close: usize,
    members: Vec<Member>,
}

/// Skips whitespace and comments from `at`; returns the next significant byte offset.
fn skip_trivia(bytes: &[u8], mut at: usize) -> usize {
    loop {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if bytes[at..].starts_with(b"//") {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
        } else if bytes[at..].starts_with(b"/*") {
            at += 2;
            while at < bytes.len() && !bytes[at..].starts_with(b"*/") {
                at += 1;
            }
            at = (at + 2).min(bytes.len());
        } else {
            return at;
        }
    }
}

/// Returns the offset just past the string starting at `at` (a `"`).
fn skip_string(bytes: &[u8], mut at: usize) -> Result<usize, String> {
    at += 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'"' => return Ok(at + 1),
            _ => at += 1,
        }
    }
    Err("unterminated string".into())
}

/// Returns the offset just past the value starting at `at`.
fn skip_value(bytes: &[u8], at: usize) -> Result<usize, String> {
    match bytes.get(at) {
        Some(b'"') => skip_string(bytes, at),
        Some(b'{' | b'[') => {
            let mut depth = 0usize;
            let mut index = at;
            while index < bytes.len() {
                index = skip_trivia(bytes, index);
                match bytes.get(index) {
                    Some(b'"') => {
                        index = skip_string(bytes, index)?;
                        continue;
                    }
                    Some(b'{' | b'[') => depth += 1,
                    Some(b'}' | b']') => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(index + 1);
                        }
                    }
                    None => break,
                    _ => {}
                }
                index += 1;
            }
            Err("unterminated value".into())
        }
        Some(_) => {
            let mut index = at;
            while index < bytes.len()
                && !matches!(bytes[index], b',' | b'}' | b']' | b'/')
                && !bytes[index].is_ascii_whitespace()
            {
                index += 1;
            }
            (index > at)
                .then_some(index)
                .ok_or_else(|| "missing value".into())
        }
        None => Err("missing value".into()),
    }
}

fn scan(text: &str) -> Result<Scan, String> {
    let bytes = text.as_bytes();
    let open = skip_trivia(bytes, 0);
    if bytes.get(open) != Some(&b'{') {
        return Err("the settings file is not a JSON object".into());
    }
    let mut members = Vec::new();
    let mut at = open + 1;
    loop {
        at = skip_trivia(bytes, at);
        match bytes.get(at) {
            Some(b'}') => {
                return Ok(Scan {
                    open,
                    close: at,
                    members,
                })
            }
            Some(b',') => at += 1,
            Some(b'"') => {
                let start = at;
                let key_end = skip_string(bytes, at)?;
                let key: String = serde_json::from_str(&text[start..key_end])
                    .map_err(|error| error.to_string())?;
                at = skip_trivia(bytes, key_end);
                if bytes.get(at) != Some(&b':') {
                    return Err(format!("missing ':' after {key}"));
                }
                let value_start = skip_trivia(bytes, at + 1);
                let end = skip_value(bytes, value_start)?;
                members.push(Member {
                    key,
                    start,
                    value_start,
                    end,
                });
                at = end;
            }
            _ => return Err("unexpected character in the settings file".into()),
        }
    }
}

/// The indentation of the line holding `at`.
fn indentation(text: &str, at: usize) -> &str {
    let line_start = text[..at].rfind('\n').map_or(0, |index| index + 1);
    let line = &text[line_start..at];
    &line[..line.len() - line.trim_start().len()]
}

/// Sets `key` to `value` (or removes it with `None`) in the JSONC object `text`.
pub(crate) fn set_top_level(
    text: &str,
    key: &str,
    value: Option<&Value>,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(match value {
            Some(value) => format!("{{\n    {}: {}\n}}\n", Value::from(key), value),
            None => "{}\n".into(),
        });
    }
    let scan = scan(text)?;
    let bytes = text.as_bytes();
    let existing = scan.members.iter().position(|member| member.key == key);
    let mut out = String::with_capacity(text.len() + 64);
    match (existing, value) {
        (Some(index), Some(value)) => {
            let member = &scan.members[index];
            out.push_str(&text[..member.value_start]);
            out.push_str(&value.to_string());
            out.push_str(&text[member.end..]);
        }
        (Some(index), None) => {
            let member = &scan.members[index];
            let after = skip_trivia(bytes, member.end);
            let (cut_start, cut_end) = if bytes.get(after) == Some(&b',') {
                // Remove the member, its comma and the line break that follows.
                let line_start = text[..member.start]
                    .rfind('\n')
                    .filter(|line| text[line + 1..member.start].trim().is_empty())
                    .map_or(member.start, |line| line + 1);
                let mut end = after + 1;
                if let Some(next) = text[end..].find('\n') {
                    if text[end..end + next].trim().is_empty() {
                        end += next + 1;
                    }
                }
                (line_start, end)
            } else if index > 0 {
                // The last member: drop the comma that precedes it.
                let previous = &scan.members[index - 1];
                let comma = text[previous.end..member.start]
                    .find(',')
                    .map_or(previous.end, |offset| previous.end + offset);
                (comma, member.end)
            } else if text[scan.open + 1..member.start].trim().is_empty()
                && text[member.end..scan.close].trim().is_empty()
            {
                // The only member, with nothing else inside the braces.
                (scan.open + 1, scan.close)
            } else {
                (member.start, member.end)
            };
            out.push_str(&text[..cut_start]);
            out.push_str(&text[cut_end..]);
        }
        (None, Some(value)) => {
            let entry = format!("{}: {}", Value::from(key), value);
            match scan.members.last() {
                Some(last) => {
                    let indent = indentation(text, scan.members[0].start).to_owned();
                    let separator =
                        if indent.is_empty() && !text[scan.open..scan.close].contains('\n') {
                            " ".to_owned()
                        } else {
                            format!("\n{indent}")
                        };
                    // A trailing comma after the last member is kept in place.
                    let after = skip_trivia(bytes, last.end);
                    let insert_at = if bytes.get(after) == Some(&b',') {
                        out.push_str(&text[..after + 1]);
                        after + 1
                    } else {
                        out.push_str(&text[..last.end]);
                        out.push(',');
                        last.end
                    };
                    out.push_str(&separator);
                    out.push_str(&entry);
                    out.push_str(&text[insert_at..]);
                }
                None => {
                    out.push_str(&text[..scan.open + 1]);
                    out.push_str("\n    ");
                    out.push_str(&entry);
                    out.push('\n');
                    out.push_str(text[scan.open + 1..].trim_start_matches([' ', '\t', '\r', '\n']));
                }
            }
        }
        (None, None) => out.push_str(text),
    }
    Ok(out)
}

/// Sets or removes `key` in the profile's `settings.json`.
pub(crate) fn set_user_setting(
    config_root: &Path,
    key: &str,
    value: Option<&Value>,
) -> Result<(), String> {
    let path = config_root.join("settings.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.to_string()),
    };
    let updated = set_top_level(&text, key, value)?;
    super::parse_jsonc(&updated).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(config_root).map_err(|error| error.to_string())?;
    std::fs::write(&path, updated).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set(text: &str, key: &str, value: Option<Value>) -> String {
        let out = set_top_level(text, key, value.as_ref()).unwrap();
        crate::config::parse_jsonc(&out).expect("the result is valid JSONC");
        out
    }

    const SETTINGS: &str = "{\n    // Font\n    \"editor.fontSize\": 14,\n    \"workbench.iconTheme\": \"old\", // chosen\n    \"nested\": {\"a\": [1, {\"b\": \"}\"}]}\n}\n";

    #[test]
    fn persisted_preferences_reload_and_recover_after_corruption_without_data_loss() {
        let dir = crate::test_support::TempDir::new();
        let profile = dir.path().join("profile");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let translator = graphite_i18n::Translator::english();
        set_user_setting(&profile, "editor.tabSize", Some(&json!(2))).unwrap();
        set_user_setting(&profile, "editor.wordWrap", Some(&json!("on"))).unwrap();
        let load =
            || super::super::editor_prefs_for_workspace_in(&profile, &workspace, &translator);
        let prefs = load().unwrap();
        assert_eq!(prefs.tab_size, 2);
        assert!(prefs.word_wrap);

        let path = profile.join("settings.json");
        let saved = std::fs::read(&path).unwrap();
        std::fs::write(&path, "{ corrupted").unwrap();
        assert!(load().is_err());
        assert!(set_user_setting(&profile, "editor.tabSize", Some(&json!(8))).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ corrupted");

        std::fs::write(&path, saved).unwrap();
        assert_eq!(load().unwrap().tab_size, 2);
        set_user_setting(&profile, "editor.tabSize", None).unwrap();
        let prefs = load().unwrap();
        assert_eq!(prefs.tab_size, 4);
        assert!(prefs.word_wrap);
    }

    #[test]
    fn settings_write_failure_preserves_existing_profile_data() {
        let dir = crate::test_support::TempDir::new();
        let profile = dir.path().join("profile");
        std::fs::write(&profile, "existing file, not a directory").unwrap();
        assert!(set_user_setting(&profile, "editor.tabSize", Some(&json!(2))).is_err());
        assert_eq!(
            std::fs::read_to_string(&profile).unwrap(),
            "existing file, not a directory"
        );
    }

    #[test]
    fn replacing_a_value_keeps_comments_and_layout() {
        assert_eq!(
            set(SETTINGS, "workbench.iconTheme", Some(json!("new"))),
            SETTINGS.replace("\"old\"", "\"new\"")
        );
        assert_eq!(
            set(SETTINGS, "nested", Some(Value::Null)),
            "{\n    // Font\n    \"editor.fontSize\": 14,\n    \"workbench.iconTheme\": \"old\", // chosen\n    \"nested\": null\n}\n"
        );
    }

    #[test]
    fn adding_a_key_follows_the_existing_indentation() {
        assert_eq!(
            set("{\n\t\"a\": 1\n}", "b", Some(json!(true))),
            "{\n\t\"a\": 1,\n\t\"b\": true\n}"
        );
        assert_eq!(
            set("{\n  \"a\": 1,\n}", "b", Some(json!(2))),
            "{\n  \"a\": 1,\n  \"b\": 2\n}"
        );
        assert_eq!(
            set("{\"a\": 1}", "b", Some(json!(2))),
            "{\"a\": 1, \"b\": 2}"
        );
        assert_eq!(
            set("{ /* empty */ }", "b", Some(json!("x"))),
            "{\n    \"b\": \"x\"\n/* empty */ }"
        );
        assert_eq!(set("", "b", Some(json!(1))), "{\n    \"b\": 1\n}\n");
    }

    #[test]
    fn removing_a_key_drops_its_line_and_comma() {
        assert_eq!(
            set(SETTINGS, "editor.fontSize", None),
            "{\n    // Font\n    \"workbench.iconTheme\": \"old\", // chosen\n    \"nested\": {\"a\": [1, {\"b\": \"}\"}]}\n}\n"
        );
        assert_eq!(
            set("{\n  \"a\": 1,\n  \"b\": 2\n}", "b", None),
            "{\n  \"a\": 1\n}"
        );
        assert_eq!(set("{\"a\": 1}", "a", None), "{}");
        assert_eq!(set("{\"a\": 1}", "missing", None), "{\"a\": 1}");
    }

    #[test]
    fn malformed_files_are_not_rewritten() {
        assert!(set_top_level("[1]", "a", Some(&json!(1))).is_err());
        assert!(set_top_level("{\"a\" 1}", "a", Some(&json!(1))).is_err());
        assert!(set_top_level("{\"a\": \"open", "a", None).is_err());
    }

    #[test]
    fn user_settings_are_written_in_place() {
        let dir = crate::test_support::TempDir::new();
        set_user_setting(dir.path(), "workbench.iconTheme", Some(&json!("pack"))).unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(
            crate::config::parse_jsonc(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            json!({"workbench.iconTheme": "pack"})
        );
        set_user_setting(dir.path(), "workbench.iconTheme", None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "{}");
        std::fs::write(&path, "{ broken").unwrap();
        assert!(set_user_setting(dir.path(), "x", Some(&json!(1))).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ broken");
    }
}
