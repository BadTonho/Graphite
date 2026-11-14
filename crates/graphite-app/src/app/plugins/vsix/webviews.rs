//! VSIX Webview panels bridge (`MainThreadWebviewPanels` / `MainThreadWebviews` / `ExtHostWebviews`).
//!
//! Tracks active panels in headless and GUI modes, enforces `localResourceRoots` path confinement
//! when rewriting extension asset URIs (`vscode-resource:`, `vscode-webview-resource:`,
//! `vscode-cdn.net`), and delivers bidirectional `postMessage` and panel disposal events.

use super::*;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

pub(super) const DEFAULT_HTML: &str =
    "<!doctype html><html><head><title>Graphite Webview</title></head><body></body></html>";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VsixWebviewPanel {
    pub handle: String,
    pub view_type: String,
    pub title: String,
    pub html: String,
    pub enable_scripts: bool,
    pub local_resource_roots: Vec<PathBuf>,
    pub extension_root: Option<PathBuf>,
    pub outbound_messages: Vec<Value>,
    pub visible: bool,
}

#[derive(Default)]
pub(super) struct Webviews {
    pub(crate) panels: BTreeMap<String, VsixWebviewPanel>,
    pub(crate) active: Option<String>,
}

impl Webviews {
    pub(crate) fn active_panel(&self) -> Option<&VsixWebviewPanel> {
        self.active
            .as_deref()
            .and_then(|handle| self.panels.get(handle))
            .or_else(|| self.panels.values().next())
    }

    pub(crate) fn contains(&self, handle: &str) -> bool {
        self.panels.contains_key(handle)
    }
}

fn canonicalize_or_normalize(path: &Path) -> Option<PathBuf> {
    if let Ok(canon) = std::fs::canonicalize(path) {
        return Some(canon);
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Some(normalized)
}

pub(super) fn uri_value_to_path(value: &Value) -> Option<PathBuf> {
    if let Some(raw) = value
        .as_str()
        .or_else(|| value["$graphiteUri"].as_str())
        .or_else(|| value["external"].as_str())
    {
        return uri_str_to_path(raw);
    }
    if value["scheme"].as_str() == Some("file") {
        if let Some(fs_path) = value["fsPath"].as_str().or_else(|| value["path"].as_str()) {
            let trimmed = normalize_file_path_str(fs_path);
            return canonicalize_or_normalize(Path::new(&trimmed));
        }
    }
    None
}

pub(super) fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(val) = u8::from_str_radix(hex, 16) {
                    out.push(val);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn normalize_file_path_str(raw: &str) -> String {
    let decoded = percent_decode(raw);
    let stripped = decoded.strip_prefix("file://").unwrap_or(&decoded);
    // Strip optional authority (`file:///C:/foo` -> `/C:/foo` -> `C:/foo` on Windows).
    let path_part = if let Some(rest) = stripped.strip_prefix('/') {
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            rest.to_owned()
        } else {
            format!("/{rest}")
        }
    } else {
        stripped.to_owned()
    };
    path_part
}

fn uri_str_to_path(raw: &str) -> Option<PathBuf> {
    if raw.contains('\0') {
        return None;
    }
    let candidate = if raw.starts_with("file://") {
        normalize_file_path_str(raw)
    } else if Path::new(raw).is_absolute() {
        raw.to_owned()
    } else {
        return None;
    };
    canonicalize_or_normalize(Path::new(&candidate))
}

fn parse_resource_roots(value: &Value) -> Vec<PathBuf> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(uri_value_to_path)
        .collect()
}

fn path_to_file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    // Strip Windows verbatim `\\?\` prefix after canonicalize so webviews can load it cleanly.
    let clean = text
        .strip_prefix("//?/UNC/")
        .map(|s| format!("//{s}"))
        .or_else(|| text.strip_prefix("//?/").map(str::to_owned))
        .unwrap_or(text);
    if clean.starts_with('/') {
        format!("file://{clean}")
    } else {
        format!("file:///{clean}")
    }
}

/// Decodes a VS Code webview asset URI (`vscode-resource:`, `vscode-webview-resource:`,
/// `https://file+.vscode-resource.vscode-cdn.net/...`) into a local path if it stays strictly inside
/// `allowed_roots`. Returns `Some("about:blank#blocked")` when blocked, or `None` if `raw` is not
/// a webview resource URI.
pub(super) fn rewrite_webview_resource_uri(raw: &str, allowed_roots: &[PathBuf]) -> Option<String> {
    let encoded_path =
        if let Some(rest) = raw.strip_prefix("https://file+.vscode-resource.vscode-cdn.net") {
            rest
        } else if let Some(rest) = raw.strip_prefix("vscode-webview-resource://") {
            rest.find('/').map(|idx| &rest[idx..])?
        } else if let Some(rest) = raw.strip_prefix("vscode-resource://") {
            rest.strip_prefix("file").unwrap_or(rest)
        } else if let Some(rest) = raw.strip_prefix("vscode-resource:") {
            rest
        } else {
            return None;
        };

    let without_query = encoded_path
        .split(['?', '#'])
        .next()
        .unwrap_or(encoded_path);
    if without_query.contains('\0') {
        return Some("about:blank#blocked".into());
    }
    let decoded = percent_decode(without_query);
    if decoded.split(['/', '\\']).any(|seg| seg == "..") {
        return Some("about:blank#blocked".into());
    }
    let local_str = normalize_file_path_str(&decoded);
    let Some(resolved) = canonicalize_or_normalize(Path::new(&local_str)) else {
        return Some("about:blank#blocked".into());
    };

    let allowed = allowed_roots.iter().any(|root| {
        canonicalize_or_normalize(root).is_some_and(|canon_root| resolved.starts_with(&canon_root))
    });
    if allowed {
        Some(path_to_file_uri(&resolved))
    } else {
        Some("about:blank#blocked".into())
    }
}

/// Rewrites webview resource URIs inside `src="..."`, `href="..."`, and CSS `url(...)` attributes.
pub(super) fn sanitize_webview_html(html: &str, allowed_roots: &[PathBuf]) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;
    let prefixes = [
        "https://file+.vscode-resource.vscode-cdn.net",
        "vscode-webview-resource://",
        "vscode-resource://",
        "vscode-resource:",
    ];

    while !rest.is_empty() {
        let next_match = prefixes
            .iter()
            .filter_map(|prefix| rest.find(prefix).map(|pos| (pos, *prefix)))
            .min_by_key(|(pos, _)| *pos);

        let Some((pos, _)) = next_match else {
            output.push_str(rest);
            break;
        };

        output.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let end = after
            .find(|c: char| matches!(c, '"' | '\'' | ')' | ' ' | '\t' | '\r' | '\n' | '<' | '>'))
            .unwrap_or(after.len());
        let raw_uri = &after[..end];
        let replacement = rewrite_webview_resource_uri(raw_uri, allowed_roots)
            .unwrap_or_else(|| "about:blank#blocked".into());
        output.push_str(&replacement);
        rest = &after[end..];
    }

    output
}

pub(super) fn parse_webview_options(
    options: &Value,
    extension_root: Option<&Path>,
    workspace_root: &Path,
) -> (bool, Option<Vec<PathBuf>>) {
    let enable_scripts = options["enableScripts"].as_bool().unwrap_or(false);
    let explicit_roots = options
        .get("localResourceRoots")
        .filter(|v| v.is_array())
        .map(parse_resource_roots);
    let default_roots = || {
        let mut roots = Vec::new();
        if let Some(ext) = extension_root.and_then(canonicalize_or_normalize) {
            roots.push(ext);
        }
        if let Some(ws) = canonicalize_or_normalize(workspace_root) {
            roots.push(ws);
        }
        roots
    };
    (
        enable_scripts,
        explicit_roots.or_else(|| Some(default_roots())),
    )
}

impl App {
    pub(super) fn handle_vsix_webview_panel_request(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        match method {
            "$createWebviewPanel" => {
                let ext_root = args
                    .first()
                    .and_then(|ext| uri_value_to_path(&ext["location"]));
                let handle = args
                    .get(1)
                    .and_then(Value::as_str)
                    .ok_or("Missing webview panel handle")?
                    .to_owned();
                let view_type = args
                    .get(2)
                    .and_then(Value::as_str)
                    .unwrap_or("webview")
                    .to_owned();
                let init_data = args.get(3).unwrap_or(&Value::Null);
                let title = init_data["title"].as_str().unwrap_or("Webview").to_owned();
                let options = init_data
                    .get("webviewOptions")
                    .or_else(|| init_data.get("contentOptions"))
                    .unwrap_or(&Value::Null);
                let (enable_scripts, roots) =
                    parse_webview_options(options, ext_root.as_deref(), self.tree.root());

                let panel = VsixWebviewPanel {
                    handle: handle.clone(),
                    view_type,
                    title: title.clone(),
                    html: DEFAULT_HTML.to_owned(),
                    enable_scripts,
                    local_resource_roots: roots.unwrap_or_default(),
                    extension_root: ext_root,
                    outbound_messages: Vec::new(),
                    visible: true,
                };
                runtime.webviews.panels.insert(handle.clone(), panel);
                runtime.webviews.active = Some(handle.clone());
                self.webview_handle = Some(handle);
                self.webview_html = DEFAULT_HTML.to_owned();
                self.status = title;
                if self.gpu.is_some() {
                    self.show_webview_html(DEFAULT_HTML)?;
                }
                Ok(Value::Null)
            }
            "$disposeWebview" => {
                let handle = args
                    .first()
                    .and_then(Value::as_str)
                    .ok_or("Missing webview handle")?;
                runtime.webviews.panels.remove(handle);
                runtime.webview_views.forget_handle(handle);
                if runtime.webviews.active.as_deref() == Some(handle) {
                    runtime.webviews.active = runtime.webviews.panels.keys().next().cloned();
                }
                if self.webview_handle.as_deref() == Some(handle) {
                    self.close_webview();
                }
                Ok(Value::Null)
            }
            "$setTitle" => {
                let handle = args
                    .first()
                    .and_then(Value::as_str)
                    .ok_or("Missing webview handle")?;
                let title = args.get(1).and_then(Value::as_str).unwrap_or("Webview");
                if let Some(panel) = runtime.webviews.panels.get_mut(handle) {
                    panel.title = title.to_owned();
                }
                if self.webview_handle.as_deref() == Some(handle) {
                    self.status = title.to_owned();
                }
                Ok(Value::Null)
            }
            "$reveal" => {
                let handle = args
                    .first()
                    .and_then(Value::as_str)
                    .ok_or("Missing webview handle")?;
                if let Some(panel) = runtime.webviews.panels.get_mut(handle) {
                    panel.visible = true;
                    let html = panel.html.clone();
                    let title = panel.title.clone();
                    runtime.webviews.active = Some(handle.to_owned());
                    self.webview_handle = Some(handle.to_owned());
                    self.webview_html = html.clone();
                    self.status = title;
                    if self.gpu.is_some() {
                        self.show_webview_html(&html)?;
                    }
                    let _ = runtime.session.send(json!({
                        "type": "call",
                        "actor": "ExtHostWebviewPanels",
                        "method": "$onDidChangeWebviewPanelViewState",
                        "args": [handle, { "active": true, "visible": true, "viewColumn": 1 }],
                    }));
                }
                Ok(Value::Null)
            }
            "$setIconPath" => Ok(Value::Null),
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadWebviewPanels", method);
                Err(format!(
                    "Graphite does not support MainThreadWebviewPanels.{method}"
                ))
            }
        }
    }

    pub(super) fn handle_vsix_webview_request(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        let handle = args
            .first()
            .and_then(Value::as_str)
            .ok_or("Missing webview handle")?;

        match method {
            "$setHtml" => {
                let raw_html = args.get(1).and_then(Value::as_str).unwrap_or("");
                let rewritten = if let Some(panel) = runtime.webviews.panels.get_mut(handle) {
                    let sanitized = sanitize_webview_html(raw_html, &panel.local_resource_roots);
                    panel.html = sanitized.clone();
                    sanitized
                } else {
                    raw_html.to_owned()
                };
                if self.webview_handle.as_deref() == Some(handle) {
                    self.webview_html = rewritten.clone();
                    if self.gpu.is_some() {
                        self.show_webview_html(&rewritten)?;
                    }
                }
                Ok(Value::Null)
            }
            "$setOptions" => {
                let options = args.get(1).unwrap_or(&Value::Null);
                if let Some(panel) = runtime.webviews.panels.get_mut(handle) {
                    if let Some(scripts) = options["enableScripts"].as_bool() {
                        panel.enable_scripts = scripts;
                    }
                    if options
                        .get("localResourceRoots")
                        .is_some_and(Value::is_array)
                    {
                        panel.local_resource_roots =
                            parse_resource_roots(&options["localResourceRoots"]);
                    }
                }
                Ok(Value::Null)
            }
            "$postMessage" => {
                let raw_message = args.get(1).ok_or("Missing webview postMessage payload")?;
                let parsed = match raw_message {
                    Value::String(text) => serde_json::from_str::<Value>(text)
                        .unwrap_or_else(|_| Value::String(text.clone())),
                    other => other.clone(),
                };
                if let Some(panel) = runtime.webviews.panels.get_mut(handle) {
                    panel.outbound_messages.push(parsed.clone());
                }
                let encoded = serde_json::to_string(&parsed).map_err(|e| e.to_string())?;
                let script = format!(
                    "window.postMessage(JSON.parse({}), '*')",
                    serde_json::to_string(&encoded).unwrap_or_default()
                );
                if self.webview_handle.as_deref() == Some(handle) {
                    if let Some(webview) = &self.webview {
                        webview.evaluate_script(&script)?;
                    }
                } else if let Some(sidebar) = self.sidebar_webviews.get(handle) {
                    sidebar.host.evaluate_script(&script)?;
                }
                Ok(json!(true))
            }
            "$setWebviewTitle" | "$registerSerializer" | "$unregisterSerializer" => Ok(Value::Null),
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadWebviews", method);
                Err(format!(
                    "Graphite does not support MainThreadWebviews.{method}"
                ))
            }
        }
    }

    pub(super) fn sync_vsix_webview_host(&mut self, runtime: &mut Runtime) -> Result<()> {
        for (handle, sidebar) in &self.sidebar_webviews {
            for message in sidebar.host.take_messages() {
                let value =
                    serde_json::from_str::<Value>(&message).unwrap_or(Value::String(message));
                runtime.session.send(json!({
                    "type": "call",
                    "actor": "ExtHostWebviews",
                    "method": "$onMessage",
                    "args": [handle, value],
                }))?;
            }
        }
        let Some(handle) = self.webview_handle.as_deref() else {
            return Ok(());
        };
        if !runtime.webviews.contains(handle) {
            return Ok(());
        }
        let Some(webview) = self.webview.as_ref() else {
            return Ok(());
        };
        for message in webview.take_messages() {
            let value = serde_json::from_str::<Value>(&message).unwrap_or(Value::String(message));
            runtime.session.send(json!({
                "type": "call",
                "actor": "ExtHostWebviews",
                "method": "$onMessage",
                "args": [handle, value],
            }))?;
        }
        Ok(())
    }

    pub(crate) fn send_vsix_webview_message(&mut self, handle: &str, message: Value) -> Result<()> {
        let runtime = self.plugins.vsix.as_mut().ok_or("No active VSIX runtime")?;
        if !runtime.webviews.contains(handle) {
            return Err(format!("Unknown VSIX webview panel: {handle}"));
        }
        runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostWebviews",
            "method": "$onMessage",
            "args": [handle, message],
        }))
    }

    pub(crate) fn close_vsix_webview(&mut self, handle: &str) -> Result<()> {
        if let Some(runtime) = self.plugins.vsix.as_mut() {
            if runtime.webviews.panels.remove(handle).is_some() {
                if runtime.webviews.active.as_deref() == Some(handle) {
                    runtime.webviews.active = runtime.webviews.panels.keys().next().cloned();
                }
                runtime.session.send(json!({
                    "type": "call",
                    "actor": "ExtHostWebviewPanels",
                    "method": "$onDidDisposeWebviewPanel",
                    "args": [handle],
                }))?;
            }
        }
        if self.webview_handle.as_deref() == Some(handle) {
            self.close_webview();
        }
        Ok(())
    }

    pub(crate) fn vsix_webview(&self) -> Option<&VsixWebviewPanel> {
        self.plugins
            .vsix
            .as_ref()
            .and_then(|runtime| runtime.webviews.active_panel())
    }
}
