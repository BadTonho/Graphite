//! Workbench services for extensions: opening links, output channels, status bar items and
//! file search. Each service is answered by Graphite, never by the extension host.

use super::*;
use std::path::{Path, PathBuf};

/// Output channels show at most this many trailing bytes of their log file.
const OUTPUT_LIMIT: usize = 1 << 20;
/// Largest number of files `workspace.findFiles` returns when the extension sets no limit.
const DEFAULT_FILE_LIMIT: usize = 1000;

pub(super) struct OutputChannel {
    label: String,
    path: Option<PathBuf>,
    /// Log file length and modification time when the view was last refreshed.
    stamp: Option<(u64, Option<std::time::SystemTime>)>,
}

#[derive(Default)]
pub(super) struct Workbench {
    outputs: BTreeMap<String, OutputChannel>,
    next_output: u64,
    status: BTreeMap<String, String>,
}

impl Workbench {
    /// Texts of the status bar items that extensions created, in id order.
    pub(super) fn status_texts(&self) -> Vec<String> {
        self.status.values().cloned().collect()
    }
}

/// Local path of a URI given either as a string or as a serialized `file` URI object.
fn file_path(value: &Value) -> Option<PathBuf> {
    if let Some(uri) = value.as_str() {
        return crate::lsp::uri_to_path(uri);
    }
    let path = value["path"].as_str()?;
    let path = match path.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => &path[1..],
        _ => path,
    };
    Some(PathBuf::from(path))
}

/// Serialized `file` URI for a local path, in the shape the extension host revives.
fn file_uri(path: &Path) -> Value {
    let text = path.to_string_lossy().replace('\\', "/");
    // Windows canonical paths carry the verbatim prefix `\\?\`, which is not part of the URI.
    let text = text.strip_prefix("//?/").unwrap_or(&text).to_owned();
    let path = if text.starts_with('/') {
        text
    } else {
        format!("/{text}")
    };
    json!({"scheme":"file","authority":"","path":path,"query":"","fragment":""})
}

/// Glob patterns from a string, an object with `pattern`, or an array of either.
fn patterns(value: &Value) -> Vec<String> {
    match value {
        Value::String(pattern) => vec![pattern.clone()],
        Value::Object(_) => value["pattern"]
            .as_str()
            .map(|pattern| vec![pattern.to_owned()])
            .unwrap_or_default(),
        Value::Array(items) => items.iter().flat_map(patterns).collect(),
        _ => Vec::new(),
    }
}

/// Answers the workbench requests, or returns `None` when they belong to another service.
pub(super) fn dispatch(
    app: &mut App,
    runtime: &mut Runtime,
    actor: &str,
    method: &str,
    args: &[Value],
) -> Option<Result<Value>> {
    let first = |index: usize| args.get(index).cloned().unwrap_or(Value::Null);
    let text = |index: usize| args.get(index).and_then(Value::as_str).unwrap_or_default();
    let result = match (actor, method) {
        ("MainThreadWindow", "$openUri") => {
            let uri = args
                .get(1)
                .and_then(Value::as_str)
                .unwrap_or_else(|| text(0));
            if uri.starts_with("https://") {
                crate::app::plugins::drawing::open_external_https(uri);
                Ok(json!(true))
            } else {
                Err("Graphite opens only https links for extensions".into())
            }
        }
        ("MainThreadWindow", "$asExternalUri") => {
            let uri = args.first().and_then(Value::as_str).unwrap_or_default();
            uri_object(uri).ok_or_else(|| "Invalid link for asExternalUri".to_owned())
        }
        ("MainThreadOutputService", "$register") => {
            runtime.workbench.next_output += 1;
            let id = format!("graphite-output-{}", runtime.workbench.next_output);
            runtime.workbench.outputs.insert(
                id.clone(),
                OutputChannel {
                    label: text(0).to_owned(),
                    path: file_path(&first(1)),
                    stamp: None,
                },
            );
            Ok(json!(id))
        }
        ("MainThreadOutputService", "$update") => {
            let id = text(0);
            match runtime.workbench.outputs.get(id).and_then(|c| c.path.clone()) {
                Some(path) => {
                    let content = read_output(&path);
                    if app.output_view_open(&path) {
                        app.show_output_text(&path, &content);
                    }
                    Ok(Value::Null)
                }
                None => Ok(Value::Null),
            }
        }
        ("MainThreadOutputService", "$reveal") => {
            let id = text(0);
            match runtime.workbench.outputs.get(id) {
                Some(channel) => match channel.path.clone() {
                    Some(path) => {
                        let content = read_output(&path);
                        app.show_output_text(&path, &content);
                        Ok(Value::Null)
                    }
                    None => Err(format!("Output channel {} has no log file", channel.label)),
                },
                None => Err(format!("Unknown output channel {id}")),
            }
        }
        ("MainThreadOutputService", "$close") => Ok(Value::Null),
        ("MainThreadStatusBar", "$setEntry") => {
            let id = match args.first() {
                Some(Value::String(id)) => id.clone(),
                Some(other) => other.to_string(),
                None => String::new(),
            };
            // Arguments: entry id, item id, extension id, name, text, tooltip, ...
            let label = text(4).to_owned();
            runtime.workbench.status.insert(id, label);
            Ok(Value::Null)
        }
        ("MainThreadStatusBar", "$disposeEntry") => {
            runtime.workbench.status.remove(text(0));
            Ok(Value::Null)
        }
        ("MainThreadWorkspace", "$startFileSearch") => {
            let folder = file_path(&first(0));
            let options = first(1);
            let include = patterns(&options["includePattern"]);
            let exclude = patterns(&options["excludePattern"]);
            let limit = options["maxResults"]
                .as_u64()
                .map_or(DEFAULT_FILE_LIMIT, |limit| limit as usize);
            let found = app.tree.workspace_snapshot().find_files(
                folder.as_deref(),
                &include,
                &exclude,
                limit,
            );
            Ok(json!(found.iter().map(|path| file_uri(path)).collect::<Vec<_>>()))
        }
        _ => return None,
    };
    Some(result)
}

/// The end of a channel's log file, decoded as text.
fn read_output(path: &Path) -> String {
    let Ok(bytes) = std::fs::read(path) else {
        return String::new();
    };
    let start = bytes.len().saturating_sub(OUTPUT_LIMIT);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

impl App {
    /// Whether a read-only view of this output log is open.
    pub(super) fn output_view_open(&self, path: &Path) -> bool {
        self.documents.id_for_path(path).is_some()
    }

    /// Shows the output text in a read-only view, refreshing it if already open.
    pub(super) fn show_output_text(&mut self, path: &Path, content: &str) {
        match self.documents.id_for_path(path) {
            Some(id) => {
                let _ = self.documents[id].buffer.reload_text(content);
                self.activate_editor_document(id);
            }
            None => self.open_read_only_view(path.to_path_buf(), content),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_round_trip_through_the_extension_host_shape() {
        let uri = file_uri(Path::new("C:\\code\\a b.rs"));
        assert_eq!(uri["path"], json!("/C:/code/a b.rs"));
        let verbatim = file_uri(Path::new("\\\\?\\C:\\code\\a.rs"));
        assert_eq!(verbatim["path"], json!("/C:/code/a.rs"));
        assert_eq!(file_path(&uri), Some(PathBuf::from("C:/code/a b.rs")));
        assert_eq!(
            file_path(&json!("/tmp/x")),
            None,
            "plain paths are not URIs"
        );
    }

    #[test]
    fn glob_arguments_accept_strings_objects_and_arrays() {
        assert_eq!(patterns(&json!("**/*.rs")), vec!["**/*.rs"]);
        assert_eq!(patterns(&json!({"pattern":"target/**"})), vec!["target/**"]);
        assert_eq!(
            patterns(&json!([{"pattern":"a/**"}, "b/**"])),
            vec!["a/**", "b/**"]
        );
        assert!(patterns(&Value::Null).is_empty());
    }

    #[test]
    fn output_logs_are_read_as_their_trailing_text() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("log.txt");
        std::fs::write(&path, "first\nsecond\n").unwrap();
        assert_eq!(read_output(&path), "first\nsecond\n");
        assert_eq!(read_output(&dir.path().join("missing")), "");
    }
}

impl Runtime {
    /// Status bar item texts that the running extensions created.
    pub(crate) fn workbench_status_texts(&self) -> Vec<String> {
        self.workbench.status_texts()
    }
}

/// Serialized URI object for a link, with the components the extension host expects.
fn uri_object(text: &str) -> Option<Value> {
    let url = reqwest::Url::parse(text).ok()?;
    let authority = match (url.host_str(), url.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        (None, _) => String::new(),
    };
    Some(json!({
        "scheme": url.scheme(),
        "authority": authority,
        "path": percent_decode(url.path()),
        "query": url.query().map(percent_decode).unwrap_or_default(),
        "fragment": url.fragment().map(percent_decode).unwrap_or_default(),
    }))
}

/// Decodes `%XX` escapes; invalid escapes are kept as written.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Some(value) = std::str::from_utf8(&bytes[index + 1..index + 3])
                .ok()
                .and_then(|digits| u8::from_str_radix(digits, 16).ok())
            {
                decoded.push(value);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

impl App {
    /// Refreshes open output views whose log file changed. The extension writes its log in the
    /// background, so changes can arrive after the view was opened.
    pub(super) fn refresh_vsix_outputs(&mut self, runtime: &mut Runtime) {
        let mut changed = Vec::new();
        for channel in runtime.workbench.outputs.values_mut() {
            let Some(path) = channel.path.clone() else {
                continue;
            };
            let Ok(metadata) = std::fs::metadata(&path) else {
                continue;
            };
            let stamp = (metadata.len(), metadata.modified().ok());
            if channel.stamp != Some(stamp) {
                channel.stamp = Some(stamp);
                changed.push(path);
            }
        }
        for path in changed {
            if self.output_view_open(&path) {
                let content = read_output(&path);
                self.show_output_text(&path, &content);
            }
        }
    }
}
