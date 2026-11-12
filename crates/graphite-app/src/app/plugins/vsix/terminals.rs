//! `window.createTerminal` and the terminal methods of the extension API. Extension terminals
//! are ordinary Graphite terminal tabs; this module only keeps the identity mapping between
//! the extension host's terminals and the tabs, and reports when a tab closes or exits.

use super::*;
use crate::config::TerminalProfile;

/// `TerminalExitReason` values of the extension API.
const EXIT_PROCESS: u64 = 2;
const EXIT_USER: u64 = 3;
const EXIT_EXTENSION: u64 = 4;

struct Item {
    /// Number the extension host knows the terminal by.
    id: u64,
    /// Identifier the extension host chose when it created the terminal.
    extension_id: String,
    /// Stable `TerminalSession` identity (tab indexes shift as terminals close).
    session: u64,
}

#[derive(Default)]
pub(super) struct Terminals {
    items: Vec<Item>,
    next: u64,
}

/// Arguments from `shellArgs`, which may be a list or a single command-line string.
fn shell_args(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect(),
        ),
        Value::String(line) => Some(line.split_whitespace().map(str::to_owned).collect()),
        _ => None,
    }
}

/// Working directory from a plain path, a `file:` URI string or a serialized URI.
fn directory(value: &Value) -> Option<PathBuf> {
    match value.as_str() {
        Some(text) if text.starts_with("file:") => crate::lsp::uri_to_path(text),
        Some(text) if !text.is_empty() => Some(PathBuf::from(text)),
        Some(_) => None,
        None => workbench::file_path(value).filter(|_| value.is_object()),
    }
}

impl Terminals {
    fn position(&self, identifier: &Value) -> Option<usize> {
        match identifier {
            Value::String(id) => self.items.iter().position(|item| &item.extension_id == id),
            other => {
                let id = other.as_u64()?;
                self.items.iter().position(|item| item.id == id)
            }
        }
    }
}

/// Answers `MainThreadTerminalService` requests, or returns `None` for other services.
pub(super) fn dispatch(
    app: &mut App,
    runtime: &mut Runtime,
    actor: &str,
    method: &str,
    args: &[Value],
) -> Option<Result<Value>> {
    if actor != "MainThreadTerminalService" {
        return None;
    }
    let first = args.first().cloned().unwrap_or(Value::Null);
    Some(match method {
        "$createTerminal" => create(app, runtime, args),
        "$sendText" => runtime
            .terminals
            .position(&first)
            .and_then(|index| {
                let session = runtime.terminals.items[index].session;
                let text = args.get(1)?.as_str()?;
                let mut bytes = text.as_bytes().to_vec();
                if args.get(2).and_then(Value::as_bool).unwrap_or(false) {
                    bytes.push(b'\r');
                }
                app.terminals.iter().find(|tab| tab.id() == session)?.input(bytes);
                Some(Value::Null)
            })
            .ok_or_else(|| "Unknown terminal".to_owned()),
        "$show" => {
            let preserve_focus = args.get(1).and_then(Value::as_bool).unwrap_or(false);
            terminal_index(app, runtime, &first).map(|index| {
                app.terminal_visible = true;
                app.terminal_problems = false;
                app.select_terminal(index);
                app.terminal_focused = !preserve_focus;
                app.resize_terminal();
                Value::Null
            })
        }
        "$hide" => {
            app.terminal_visible = false;
            app.terminal_focused = false;
            Ok(Value::Null)
        }
        "$dispose" => terminal_index(app, runtime, &first).and_then(|index| {
            let position = runtime.terminals.position(&first).ok_or("Unknown terminal")?;
            let item = runtime.terminals.items.remove(position);
            app.close_terminal_at(index);
            closed(runtime, item.id, None, EXIT_EXTENSION)?;
            Ok(Value::Null)
        }),
        "$sendProcessData" | "$sendProcessReady" | "$sendProcessProperty" | "$sendProcessExit" => {
            Err("Graphite does not support extension-owned pseudoterminals".into())
        }
        // Toggles for optional events and registrations that have nothing to wire yet.
        _ => Ok(Value::Null),
    })
}

fn terminal_index(app: &App, runtime: &Runtime, identifier: &Value) -> Result<usize> {
    let position = runtime
        .terminals
        .position(identifier)
        .ok_or("Unknown terminal")?;
    let session = runtime.terminals.items[position].session;
    app.terminals
        .iter()
        .position(|tab| tab.id() == session)
        .ok_or_else(|| "The terminal was closed".to_owned())
}

fn create(app: &mut App, runtime: &mut Runtime, args: &[Value]) -> Result<Value> {
    let extension_id = args
        .first()
        .and_then(Value::as_str)
        .ok_or("Invalid terminal identifier")?
        .to_owned();
    let config = args.get(1).unwrap_or(&Value::Null);
    if config["isExtensionCustomPtyTerminal"].as_bool() == Some(true) {
        return Err("Graphite does not support extension-owned pseudoterminals".into());
    }
    let default = app
        .terminal_profiles
        .get(app.selected_terminal_profile)
        .cloned();
    let name = config["name"].as_str().unwrap_or("Extension terminal");
    let profile = match config["shellPath"].as_str().filter(|path| !path.is_empty()) {
        Some(path) => TerminalProfile {
            name: name.to_owned(),
            path: path.to_owned(),
            args: shell_args(&config["shellArgs"]).unwrap_or_default(),
        },
        None => {
            let mut profile = default.ok_or("No terminal profile is available")?;
            profile.name = name.to_owned();
            if let Some(args) = shell_args(&config["shellArgs"]) {
                profile.args = args;
            }
            profile
        }
    };
    let directory = directory(&config["cwd"]);
    let index = app.spawn_terminal_profile(&profile, directory.as_deref(), Some(name))?;
    // Extension terminals appear as tabs; the panel opens only when the extension shows one.
    let session = app.terminals[index].id();
    runtime.terminals.next += 1;
    let id = runtime.terminals.next;
    runtime.terminals.items.push(Item {
        id,
        extension_id: extension_id.clone(),
        session,
    });
    runtime.session.send(json!({
        "type": "call", "actor": "ExtHostTerminalService", "method": "$acceptTerminalOpened",
        "args": [id, extension_id, name, {"name": name}],
    }))?;
    Ok(Value::Null)
}

fn closed(runtime: &Runtime, id: u64, exit_code: Option<u32>, reason: u64) -> Result<()> {
    runtime.session.send(json!({
        "type": "call", "actor": "ExtHostTerminalService", "method": "$acceptTerminalClosed",
        "args": [id, exit_code, reason],
    }))
}

impl App {
    /// Tells the extension host about terminals whose tab was closed or whose process exited.
    pub(super) fn sync_vsix_terminals(&self, runtime: &mut Runtime) -> Result<()> {
        let mut index = 0;
        while index < runtime.terminals.items.len() {
            let session = runtime.terminals.items[index].session;
            let outcome = match self.terminals.iter().find(|tab| tab.id() == session) {
                None => Some((None, EXIT_USER)),
                Some(tab) => tab
                    .snapshot()
                    .exit_code
                    .map(|code| (Some(code), EXIT_PROCESS)),
            };
            match outcome {
                Some((code, reason)) => {
                    let item = runtime.terminals.items.remove(index);
                    closed(runtime, item.id, code, reason)?;
                }
                None => index += 1,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_arguments_accept_a_list_or_a_command_line() {
        assert_eq!(
            shell_args(&json!(["-c", "echo hi"])),
            Some(vec!["-c".into(), "echo hi".into()])
        );
        assert_eq!(
            shell_args(&json!("-l  -i")),
            Some(vec!["-l".into(), "-i".into()])
        );
        assert_eq!(shell_args(&Value::Null), None);
    }

    #[test]
    fn directories_accept_paths_file_uris_and_serialized_uris() {
        assert_eq!(directory(&json!("/work")), Some(PathBuf::from("/work")));
        assert_eq!(directory(&json!("")), None);
        assert_eq!(directory(&Value::Null), None);
        let serialized = json!({"scheme": "file", "authority": "", "path": "/work/sub"});
        assert_eq!(directory(&serialized), Some(PathBuf::from("/work/sub")));
    }

    #[test]
    fn terminals_are_found_by_extension_identifier_or_number() {
        let terminals = Terminals {
            items: vec![
                Item { id: 1, extension_id: "a".into(), session: 10 },
                Item { id: 2, extension_id: "b".into(), session: 11 },
            ],
            next: 2,
        };
        assert_eq!(terminals.position(&json!("b")), Some(1));
        assert_eq!(terminals.position(&json!(1)), Some(0));
        assert_eq!(terminals.position(&json!("missing")), None);
        assert_eq!(terminals.position(&json!(9)), None);
    }
}
