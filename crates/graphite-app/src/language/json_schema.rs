use crate::lsp::LspUiEvent;
use graphite_i18n::Translator;
use jsonschema::{Retrieve, Uri};
use lsp_types::{
    CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity, Position, Range,
};
use serde_json::{json, Value};
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::error::Error;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

enum Request {
    Validate {
        path: PathBuf,
        text: String,
        revision: u64,
    },
    Complete {
        path: PathBuf,
        text: String,
        position: Position,
        request_id: u64,
    },
}

pub struct JsonSchemaManager {
    sender: Sender<Request>,
}

impl JsonSchemaManager {
    pub fn new(
        event_sender: Sender<LspUiEvent>,
        cache_dir: PathBuf,
        translator: Translator,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("graphite-json-schema".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    match request {
                        Request::Validate {
                            path,
                            text,
                            revision,
                        } => {
                            let diagnostics =
                                validate_document(&path, &text, &cache_dir, &translator);
                            if let Some(uri) = crate::lsp::path_to_uri(&path) {
                                let _ = event_sender.send(LspUiEvent::JsonDiagnostics {
                                    uri,
                                    revision,
                                    diagnostics,
                                });
                            }
                        }
                        Request::Complete {
                            path,
                            text,
                            position,
                            request_id,
                        } => {
                            let items =
                                schema_completions(&path, &text, position, &cache_dir, &translator);
                            let _ =
                                event_sender.send(LspUiEvent::JsonCompletion { request_id, items });
                        }
                    }
                }
            })
            .expect("failed to create JSON Schema worker");
        Self { sender }
    }

    pub fn validate(&self, path: PathBuf, text: String, revision: u64) {
        let _ = self.sender.send(Request::Validate {
            path,
            text,
            revision,
        });
    }

    pub fn complete(&self, path: PathBuf, text: String, position: Position, request_id: u64) {
        let _ = self.sender.send(Request::Complete {
            path,
            text,
            position,
            request_id,
        });
    }
}

#[derive(Clone)]
struct CachedRetriever {
    cache_dir: PathBuf,
    client: reqwest::blocking::Client,
    translator: Translator,
}

impl CachedRetriever {
    fn new(cache_dir: PathBuf, translator: Translator) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            cache_dir,
            client,
            translator,
        })
    }

    fn cache_path(&self, uri: &str) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        uri.hash(&mut hasher);
        self.cache_dir
            .join(format!("{:016x}.json", hasher.finish()))
    }

    fn retrieve_text(&self, uri: &str) -> Result<String, String> {
        let parsed = reqwest::Url::parse(uri).map_err(|error| error.to_string())?;
        match parsed.scheme() {
            "https" => {
                let cache_path = self.cache_path(uri);
                if let Ok(cached) = fs::read_to_string(&cache_path) {
                    if serde_json::from_str::<Value>(&cached).is_ok() {
                        return Ok(cached);
                    }
                    let _ = fs::remove_file(&cache_path);
                }
                let response = self.client.get(parsed).send().map_err(|error| {
                    self.translator
                        .format("json.error.download_failed", &[("error", &error)])
                })?;
                if response.url().scheme() != "https" {
                    return Err(self.translator.text("json.error.redirect_https").into());
                }
                if !response.status().is_success() {
                    return Err(self.translator.format(
                        "json.error.server_status",
                        &[("status", &response.status())],
                    ));
                }
                let bytes = response.bytes().map_err(|error| {
                    self.translator
                        .format("json.error.read_response", &[("error", &error)])
                })?;
                if bytes.len() > 8 * 1024 * 1024 {
                    return Err(self.translator.text("json.error.size_limit").into());
                }
                let text = String::from_utf8(bytes.to_vec()).map_err(|error| {
                    self.translator
                        .format("json.error.not_utf8", &[("error", &error)])
                })?;
                serde_json::from_str::<Value>(&text).map_err(|error| {
                    self.translator
                        .format("json.error.invalid_json", &[("error", &error)])
                })?;
                fs::create_dir_all(&self.cache_dir).map_err(|error| error.to_string())?;
                fs::write(cache_path, &text).map_err(|error| error.to_string())?;
                Ok(text)
            }
            "file" => {
                let path = parsed.to_file_path().map_err(|_| {
                    self.translator
                        .text("json.error.invalid_local_path")
                        .to_owned()
                })?;
                fs::read_to_string(path).map_err(|error| error.to_string())
            }
            scheme => Err(self
                .translator
                .format("json.error.scheme", &[("scheme", &scheme)])),
        }
    }
}

impl Retrieve for CachedRetriever {
    fn retrieve(&self, uri: &Uri<String>) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let text = self
            .retrieve_text(uri.as_str())
            .map_err(std::io::Error::other)?;
        serde_json::from_str(&text).map_err(|error| Box::new(error) as _)
    }
}

fn schema_for_document(
    path: &Path,
    document: &Value,
    cache_dir: &Path,
    translator: &Translator,
) -> Result<Option<(Value, String, CachedRetriever)>, String> {
    let Some(schema_uri) = document.get("$schema").and_then(Value::as_str) else {
        return Ok(None);
    };
    let retriever = CachedRetriever::new(cache_dir.to_path_buf(), *translator)?;
    let (schema_text, base_uri) =
        if schema_uri.starts_with("https://") || schema_uri.starts_with("file://") {
            let text = retriever.retrieve_text(schema_uri)?;
            (text, schema_uri.to_owned())
        } else if schema_uri.contains("://") {
            return Err(translator.text("json.error.only_https_file").into());
        } else {
            let schema_path = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(schema_uri);
            let schema_path = schema_path.canonicalize().unwrap_or(schema_path);
            let base_uri = reqwest::Url::from_file_path(&schema_path)
                .map_err(|_| translator.text("json.error.local_path_uri").to_owned())?
                .to_string();
            let text = fs::read_to_string(&schema_path).map_err(|error| {
                translator.format(
                    "json.error.cannot_read_schema",
                    &[("path", &schema_path.display()), ("error", &error)],
                )
            })?;
            (text, base_uri)
        };
    let schema = serde_json::from_str(&schema_text)
        .map_err(|error| translator.format("json.error.invalid_json", &[("error", &error)]))?;
    Ok(Some((schema, base_uri, retriever)))
}

fn validate_document(
    path: &Path,
    text: &str,
    cache_dir: &Path,
    translator: &Translator,
) -> Vec<Diagnostic> {
    let document = match crate::config::parse_jsonc(text) {
        Ok(value) => value,
        Err(error) => {
            return vec![parse_error_diagnostic(
                text,
                &error.to_string(),
                error.line(),
                error.column(),
                translator,
            )]
        }
    };
    let (schema, base_uri, retriever) =
        match schema_for_document(path, &document, cache_dir, translator) {
            Ok(Some(schema)) => schema,
            Ok(None) => return Vec::new(),
            Err(error) => return vec![schema_error_diagnostic(&error, translator)],
        };
    let validator = match jsonschema::options()
        .with_base_uri(base_uri)
        .with_retriever(retriever)
        .build(&schema)
    {
        Ok(validator) => validator,
        Err(error) => {
            return vec![schema_error_diagnostic(
                &translator.format("json.error.invalid_schema", &[("error", &error)]),
                translator,
            )]
        }
    };
    let mut instance = document.clone();
    if let Value::Object(properties) = &mut instance {
        properties.remove("$schema");
    }
    validator
        .iter_errors(&instance)
        .take(100)
        .map(|error| {
            let instance_path = error.instance_path.to_string();
            let (line, column) = locate_instance_path(text, &instance_path);
            let position = Position::new(line as u32, column as u32);
            make_diagnostic(
                Range::new(position, Position::new(line as u32, column as u32 + 1)),
                error.to_string(),
            )
        })
        .collect()
}

fn schema_completions(
    path: &Path,
    text: &str,
    position: Position,
    cache_dir: &Path,
    translator: &Translator,
) -> Vec<CompletionItem> {
    let Ok(document) = crate::config::parse_jsonc(text) else {
        return Vec::new();
    };
    let Ok(Some((schema, base_uri, retriever))) =
        schema_for_document(path, &document, cache_dir, translator)
    else {
        return Vec::new();
    };
    let mut properties = serde_json::Map::new();
    collect_properties(
        &schema,
        &schema,
        &base_uri,
        &retriever,
        &mut HashSet::new(),
        &mut properties,
    );
    if let Some((key, inside_string)) = value_completion_context(text, position) {
        let Some(definition) = properties.get(&key) else {
            return Vec::new();
        };
        let values = definition
            .get("enum")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .chain(definition.get("const"))
            .filter(|value| !inside_string || value.is_string())
            .map(|value| {
                let insertion = if inside_string {
                    value.as_str().unwrap_or_default().to_owned()
                } else {
                    value.to_string()
                };
                CompletionItem {
                    label: if inside_string {
                        value.as_str().unwrap_or_default().to_owned()
                    } else {
                        value.to_string()
                    },
                    kind: Some(CompletionItemKind::VALUE),
                    detail: Some(
                        translator.format("json.completion.allowed_value", &[("key", &key)]),
                    ),
                    insert_text: Some(insertion),
                    ..CompletionItem::default()
                }
            })
            .collect();
        return values;
    }
    let existing = document.as_object();
    properties
        .into_iter()
        .filter(|(name, _)| !existing.is_some_and(|object| object.contains_key(name)))
        .filter_map(|(name, definition)| {
            let description = definition.get("description").and_then(Value::as_str);
            serde_json::from_value(json!({
                "label": name,
                "kind": CompletionItemKind::PROPERTY,
                "detail": description,
                "insertText": name
            }))
            .ok()
        })
        .collect()
}

fn value_completion_context(text: &str, position: Position) -> Option<(String, bool)> {
    let line_start = text
        .split_inclusive('\n')
        .take(position.line as usize)
        .map(str::len)
        .sum::<usize>();
    let line = text.get(line_start..)?;
    let mut utf16_units = 0;
    let mut byte_column = line.len();
    for (byte, ch) in line.char_indices() {
        if utf16_units + ch.len_utf16() as u32 > position.character {
            byte_column = byte;
            break;
        }
        utf16_units += ch.len_utf16() as u32;
        if utf16_units == position.character {
            byte_column = byte + ch.len_utf8();
            break;
        }
    }
    let absolute = (line_start + byte_column).min(text.len());
    let prefix = &text[..absolute];
    let string_start = active_string_start(prefix);
    let inside_string = string_start.is_some();
    let boundary = string_start.unwrap_or(absolute);
    let colon = last_colon_outside_strings(text, boundary)?;
    if inside_string && !text[colon + 1..boundary].trim().is_empty() {
        return None;
    }
    let before_colon = text[..colon].trim_end();
    let end = before_colon.len().checked_sub(1)?;
    if before_colon.as_bytes().get(end) != Some(&b'"') {
        return None;
    }
    let start = previous_unescaped_quote(before_colon, end)?;
    serde_json::from_str(&before_colon[start..=end])
        .ok()
        .map(|key| (key, inside_string))
}

fn active_string_start(prefix: &str) -> Option<usize> {
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0;
    for (index, byte) in prefix.bytes().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
            start = index;
        }
    }
    in_string.then_some(start)
}

fn last_colon_outside_strings(text: &str, limit: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;
    let mut last_colon = None;
    let mut index = 0;
    while index < limit {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();
        if line_comment {
            line_comment = byte != b'\n';
        } else if block_comment {
            if byte == b'*' && next == Some(b'/') {
                block_comment = false;
                index += 1;
            }
        } else if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'/' && next == Some(b'/') {
            line_comment = true;
            index += 1;
        } else if byte == b'/' && next == Some(b'*') {
            block_comment = true;
            index += 1;
        } else if byte == b'"' {
            in_string = true;
        } else if byte == b':' {
            last_colon = Some(index);
        }
        index += 1;
    }
    last_colon
}

fn previous_unescaped_quote(text: &str, before: usize) -> Option<usize> {
    (0..before).rev().find(|index| {
        if text.as_bytes()[*index] != b'"' {
            return false;
        }
        let backslashes = text.as_bytes()[..*index]
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\\')
            .count();
        backslashes % 2 == 0
    })
}

fn collect_properties(
    schema: &Value,
    root_schema: &Value,
    base_uri: &str,
    retriever: &CachedRetriever,
    visited: &mut HashSet<String>,
    properties: &mut serde_json::Map<String, Value>,
) {
    if let Some(entries) = schema.get("properties").and_then(Value::as_object) {
        properties.extend(entries.clone());
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = schema.get(keyword).and_then(Value::as_array) {
            for branch in branches {
                collect_properties(
                    branch,
                    root_schema,
                    base_uri,
                    retriever,
                    visited,
                    properties,
                );
            }
        }
    }
    let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
        return;
    };
    let Some(resolved_uri) = reqwest::Url::parse(base_uri)
        .ok()
        .and_then(|base| base.join(reference).ok())
    else {
        return;
    };
    let fragment = resolved_uri.fragment().unwrap_or_default().to_owned();
    if reference.starts_with('#') {
        let key = format!("{base_uri}#{fragment}");
        if visited.insert(key) {
            if let Some(target) = root_schema.pointer(&fragment) {
                collect_properties(
                    target,
                    root_schema,
                    base_uri,
                    retriever,
                    visited,
                    properties,
                );
            }
        }
        return;
    }
    let key = resolved_uri.to_string();
    if !visited.insert(key) {
        return;
    }
    let mut document_uri = resolved_uri.clone();
    document_uri.set_fragment(None);
    let Ok(text) = retriever.retrieve_text(document_uri.as_str()) else {
        return;
    };
    let Ok(referenced_schema) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    let target = if fragment.is_empty() {
        &referenced_schema
    } else if let Some(target) = referenced_schema.pointer(&fragment) {
        target
    } else {
        return;
    };
    collect_properties(
        target,
        &referenced_schema,
        document_uri.as_str(),
        retriever,
        visited,
        properties,
    );
}

fn make_diagnostic(range: Range, message: String) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("json-schema".into()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

fn schema_error_diagnostic(message: &str, translator: &Translator) -> Diagnostic {
    make_diagnostic(
        Range::new(Position::new(0, 0), Position::new(0, 1)),
        translator.format("json.schema_diagnostic", &[("message", &message)]),
    )
}

fn parse_error_diagnostic(
    text: &str,
    message: &str,
    line: usize,
    column: usize,
    translator: &Translator,
) -> Diagnostic {
    let line = line.saturating_sub(1);
    let line_text = text.lines().nth(line).unwrap_or_default();
    let column = line_text
        .chars()
        .take(column.saturating_sub(1))
        .map(|ch| ch.len_utf16() as u32)
        .sum::<u32>();
    make_diagnostic(
        Range::new(
            Position::new(line as u32, column),
            Position::new(line as u32, column.saturating_add(1)),
        ),
        translator.format("json.error.invalid_document", &[("error", &message)]),
    )
}

fn locate_instance_path(text: &str, pointer: &str) -> (usize, usize) {
    let Some(key) = pointer
        .rsplit('/')
        .next()
        .filter(|key| !key.is_empty())
        .map(|key| key.replace("~1", "/").replace("~0", "~"))
    else {
        return (0, 0);
    };
    let needle = format!("\"{key}\"");
    let Some(byte) = text.find(&needle) else {
        return (0, 0);
    };
    let before = &text[..byte];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let column_text = before.rsplit('\n').next().unwrap_or_default();
    let column = column_text.encode_utf16().count();
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translator() -> Translator {
        Translator::english()
    }

    #[test]
    fn local_schema_reports_property_and_type_errors() {
        let dir = crate::test_support::TempDir::new();
        let schema_path = dir.path().join("schema.json");
        fs::write(
            &schema_path,
            r#"{"type":"object","required":["name"],"properties":{"name":{"type":"string"}}}"#,
        )
        .unwrap();
        let document_path = dir.path().join("settings.json");
        let uri = schema_path.file_name().unwrap().to_string_lossy();
        let text = format!(r#"{{"$schema":"{uri}","name":4}}"#);
        let diagnostics = validate_document(
            &document_path,
            &text,
            &dir.path().join("cache"),
            &translator(),
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("string"));
    }

    #[test]
    fn schema_association_is_not_validated_as_document_data() {
        let dir = crate::test_support::TempDir::new();
        fs::write(
            dir.path().join("schema.json"),
            r#"{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string"}}}"#,
        )
        .unwrap();
        let text = r#"{"$schema":"schema.json","name":"Graphite"}"#;
        assert!(validate_document(
            &dir.path().join("settings.json"),
            text,
            &dir.path().join("cache"),
            &translator(),
        )
        .is_empty());
    }

    #[test]
    fn invalid_json_is_reported_without_panicking() {
        let dir = crate::test_support::TempDir::new();
        let diagnostics = validate_document(
            &dir.path().join("settings.json"),
            "{\n  \"name\": }",
            &dir.path().join("cache"),
            &translator(),
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert!(diagnostics[0].message.starts_with("Invalid JSON:"));
    }

    #[test]
    fn cached_https_schema_is_available_without_network() {
        let dir = crate::test_support::TempDir::new();
        let retriever = CachedRetriever::new(dir.path().join("cache"), translator()).unwrap();
        fs::create_dir_all(&retriever.cache_dir).unwrap();
        let uri = "https://schemas.example/config.json";
        fs::write(retriever.cache_path(uri), r#"{"type":"object"}"#).unwrap();
        let parsed = uri.parse::<Uri<String>>().unwrap();
        assert_eq!(retriever.retrieve(&parsed).unwrap()["type"], "object");
    }

    #[test]
    fn schema_completion_offers_unused_property_names() {
        let dir = crate::test_support::TempDir::new();
        let schema_path = dir.path().join("schema.json");
        fs::write(
            &schema_path,
            r#"{"type":"object","properties":{"one":{"type":"string"},"two":{"type":"boolean"}}}"#,
        )
        .unwrap();
        let document_path = dir.path().join("settings.json");
        let text = r#"{"$schema":"schema.json","one":"yes"}"#;
        let completions = schema_completions(
            &document_path,
            text,
            Position::new(0, 0),
            &dir.path().join("cache"),
            &translator(),
        );
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].label, "two");
    }

    #[test]
    fn schema_completion_offers_enum_values_inside_a_property_string() {
        let dir = crate::test_support::TempDir::new();
        fs::write(
            dir.path().join("schema.json"),
            r#"{"type":"object","properties":{"mode":{"enum":["safe","fast"]}}}"#,
        )
        .unwrap();
        let document_path = dir.path().join("settings.json");
        let text = r#"{"$schema":"schema.json","title":"🙂","mode":"s"}"#;
        let byte_position = text.find("s\"}").unwrap();
        let position = Position::new(0, text[..byte_position].encode_utf16().count() as u32);
        let completions = schema_completions(
            &document_path,
            text,
            position,
            &dir.path().join("cache"),
            &translator(),
        );
        assert_eq!(
            completions
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["safe", "fast"]
        );
    }

    #[test]
    fn schema_completion_offers_non_string_enum_values_outside_a_string() {
        let dir = crate::test_support::TempDir::new();
        fs::write(
            dir.path().join("schema.json"),
            r#"{"type":"object","properties":{"enabled":{"enum":[true,false]}}}"#,
        )
        .unwrap();
        let document_path = dir.path().join("settings.json");
        let text = r#"{"$schema":"schema.json","enabled":true}"#;
        let position = Position::new(0, text.find("true").unwrap() as u32);
        let completions = schema_completions(
            &document_path,
            text,
            position,
            &dir.path().join("cache"),
            &translator(),
        );
        assert_eq!(
            completions
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["true", "false"]
        );
    }

    #[test]
    fn schema_completion_follows_local_refs() {
        let dir = crate::test_support::TempDir::new();
        fs::write(
            dir.path().join("schema.json"),
            r#"{"$ref":"properties.json"}"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("properties.json"),
            r#"{"type":"object","properties":{"fromRef":{"type":"string"}}}"#,
        )
        .unwrap();
        let completions = schema_completions(
            &dir.path().join("settings.json"),
            r#"{"$schema":"schema.json"}"#,
            Position::new(0, 0),
            &dir.path().join("cache"),
            &translator(),
        );
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].label, "fromRef");
    }

    #[test]
    fn invalid_schema_is_reported_as_a_nonfatal_diagnostic() {
        let dir = crate::test_support::TempDir::new();
        fs::write(dir.path().join("schema.json"), "{").unwrap();
        let diagnostics = validate_document(
            &dir.path().join("settings.json"),
            r#"{"$schema":"schema.json"}"#,
            &dir.path().join("cache"),
            &translator(),
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Invalid schema JSON"));
    }
}
