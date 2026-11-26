//! Native plugin system APIs: terminal, scoped workspace FS, SCM status, chat tools, and permission enforcement.

use super::*;
use crate::config::TerminalProfile;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;

const MAX_WORKSPACE_READ_BYTES: u64 = 2 * 1024 * 1024;
const BUILTIN_CHAT_TOOLS: &[&str] = &[
    "read_file",
    "list_files",
    "search_workspace",
    "propose_edit",
    "run_command",
];

#[derive(Clone, Debug)]
pub(crate) struct NativeChatTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub schema: Value,
}

/// Maps an incoming host RPC request to the manifest permission required to invoke it.
pub(crate) fn required_permission(request: &RpcRequest) -> Option<&'static str> {
    match request {
        RpcRequest::RegisterCommand(_) => Some("commands"),
        RpcRequest::RegisterSaveParticipant(_) | RpcRequest::SubscribeDocuments => {
            Some("documents.read")
        }
        RpcRequest::Service { request, .. } => match request {
            ServiceRequest::Commands | ServiceRequest::Execute(_) => Some("commands"),
            ServiceRequest::Documents | ServiceRequest::Snapshot { .. } => Some("documents.read"),
            ServiceRequest::ApplyEdits { .. }
            | ServiceRequest::Open { .. }
            | ServiceRequest::Save(_) => Some("documents.write"),
            ServiceRequest::StorageGet { .. } | ServiceRequest::StorageSet { .. } => {
                Some("storage")
            }
            ServiceRequest::Focus { .. }
            | ServiceRequest::Message { .. }
            | ServiceRequest::PromptInput { .. }
            | ServiceRequest::QuickPick { .. }
            | ServiceRequest::SetStatusBarItem { .. }
            | ServiceRequest::DisposeStatusBarItem { .. }
            | ServiceRequest::CreateOutputChannel { .. }
            | ServiceRequest::AppendOutput { .. }
            | ServiceRequest::ShowOutput { .. }
            | ServiceRequest::RegisterTreeDataProvider { .. }
            | ServiceRequest::RefreshTreeView { .. }
            | ServiceRequest::CreateWebviewPanel { .. }
            | ServiceRequest::SetWebviewHtml { .. }
            | ServiceRequest::SetWebviewTitle { .. }
            | ServiceRequest::PostWebviewMessage { .. }
            | ServiceRequest::DisposeWebviewPanel { .. } => Some("ui"),
            ServiceRequest::ActiveEditor
            | ServiceRequest::SetEditorSelection { .. }
            | ServiceRequest::RegisterDecorationType { .. }
            | ServiceRequest::SetDecorations { .. }
            | ServiceRequest::DisposeDecorationType { .. }
            | ServiceRequest::SetDiagnostics { .. }
            | ServiceRequest::RegisterLanguageProvider { .. }
            | ServiceRequest::DisposeLanguageProvider { .. } => Some("languages"),
            ServiceRequest::WorkspaceFindFiles { .. }
            | ServiceRequest::WorkspaceReadFile { .. }
            | ServiceRequest::ScmStatus => Some("workspace.read"),
            ServiceRequest::WorkspaceWriteFile { .. } => Some("workspace.write"),
            ServiceRequest::CreateTerminal { .. }
            | ServiceRequest::SendTerminalText { .. }
            | ServiceRequest::ShowTerminal { .. } => Some("terminal"),
            ServiceRequest::RegisterChatTool { .. } | ServiceRequest::DisposeChatTool { .. } => {
                Some("chat")
            }
            ServiceRequest::Workspace | ServiceRequest::Configuration => None,
        },
        _ => None,
    }
}

fn default_terminal_profile(name: &str) -> TerminalProfile {
    let path = if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    };
    TerminalProfile {
        name: name.to_owned(),
        path,
        args: Vec::new(),
    }
}

impl App {
    pub(crate) fn check_native_permission(&self, request: &RpcRequest) -> ServiceResult<()> {
        let Some(manifest) = &self.extension_host.native_manifest else {
            return Ok(());
        };
        if let Some(cap) = required_permission(request) {
            if !manifest.allows_permission(cap) {
                return Err(ServiceError::Failed(format!(
                    "Plugin `{}` is missing required `{cap}` permission",
                    manifest.id
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn resolve_plugin_workspace_path(
        &self,
        raw: &str,
        must_exist: bool,
    ) -> ServiceResult<PathBuf> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ServiceError::InvalidArguments(
                "Path cannot be empty".into(),
            ));
        }
        let rel = Path::new(trimmed);
        if rel.is_absolute() {
            return Err(ServiceError::InvalidArguments(
                "Workspace paths must be relative to the workspace root".into(),
            ));
        }
        let mut clean_parts = Vec::new();
        for comp in rel.components() {
            match comp {
                Component::CurDir => {}
                Component::Normal(part) => {
                    if part == ".git" {
                        return Err(ServiceError::InvalidArguments(
                            "Access to .git is forbidden for plugins".into(),
                        ));
                    }
                    clean_parts.push(part.to_owned());
                }
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(ServiceError::InvalidArguments(
                        "Path escapes workspace root".into(),
                    ));
                }
            }
        }
        if clean_parts.is_empty() {
            return Err(ServiceError::InvalidArguments(
                "Path must point to a file inside workspace".into(),
            ));
        }
        let ws_root = self
            .tree
            .root()
            .canonicalize()
            .map_err(|e| ServiceError::Io(e.to_string()))?;
        let mut candidate = ws_root.clone();
        for part in &clean_parts {
            candidate.push(part);
        }
        if must_exist {
            let resolved = candidate
                .canonicalize()
                .map_err(|e| ServiceError::Io(e.to_string()))?;
            if !resolved.starts_with(&ws_root) {
                return Err(ServiceError::InvalidArguments(
                    "Path symlink escapes workspace root".into(),
                ));
            }
            return Ok(resolved);
        }
        // For new/written files, ensure the deepest existing ancestor resolves inside ws_root
        // so creating nested files cannot follow a symlink out of the workspace.
        let mut probe = candidate.clone();
        while !probe.exists() {
            let Some(parent) = probe.parent() else {
                break;
            };
            probe = parent.to_path_buf();
        }
        let ancestor = probe
            .canonicalize()
            .map_err(|e| ServiceError::Io(e.to_string()))?;
        if !ancestor.starts_with(&ws_root) {
            return Err(ServiceError::InvalidArguments(
                "Target parent escapes workspace root".into(),
            ));
        }
        Ok(candidate)
    }

    pub(crate) fn handle_native_system_service(
        &mut self,
        request: ServiceRequest,
    ) -> Option<ServiceResult<Value>> {
        match request {
            ServiceRequest::CreateTerminal {
                name,
                shell_path,
                shell_args,
                cwd,
            } => Some(self.native_create_terminal(name, shell_path, shell_args, cwd)),
            ServiceRequest::SendTerminalText {
                id,
                text,
                add_newline,
            } => Some(self.native_send_terminal_text(id, text, add_newline)),
            ServiceRequest::ShowTerminal { id, preserve_focus } => {
                Some(self.native_show_terminal(id, preserve_focus))
            }
            ServiceRequest::WorkspaceFindFiles {
                include,
                exclude,
                max_results,
            } => Some(self.native_workspace_find_files(include, exclude, max_results)),
            ServiceRequest::WorkspaceReadFile { path } => {
                Some(self.native_workspace_read_file(&path))
            }
            ServiceRequest::WorkspaceWriteFile { path, content } => {
                Some(self.native_workspace_write_file(&path, &content))
            }
            ServiceRequest::ScmStatus => Some(self.native_scm_status()),
            ServiceRequest::RegisterChatTool {
                tool_id,
                name,
                description,
                schema,
            } => Some(self.native_register_chat_tool(tool_id, name, description, schema)),
            ServiceRequest::DisposeChatTool { tool_id } => {
                self.extension_host.chat_tools.remove(&tool_id);
                Some(Ok(Value::Null))
            }
            _ => None,
        }
    }

    fn native_create_terminal(
        &mut self,
        name: Option<String>,
        shell_path: Option<String>,
        shell_args: Vec<String>,
        cwd: Option<String>,
    ) -> ServiceResult<Value> {
        let title = name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Plugin terminal")
            .to_owned();
        let profile = match shell_path.filter(|p| !p.trim().is_empty()) {
            Some(path) => TerminalProfile {
                name: title.clone(),
                path,
                args: shell_args,
            },
            None => {
                let mut profile = self
                    .terminal_profiles
                    .get(self.selected_terminal_profile)
                    .cloned()
                    .unwrap_or_else(|| default_terminal_profile(&title));
                profile.name = title.clone();
                if !shell_args.is_empty() {
                    profile.args = shell_args;
                }
                profile
            }
        };
        let directory = match cwd {
            Some(ref raw) if !raw.trim().is_empty() => {
                let rel = Path::new(raw.trim());
                if raw.trim() == "." {
                    Some(self.tree.root().to_path_buf())
                } else if rel.is_absolute() {
                    let ws_root = self
                        .tree
                        .root()
                        .canonicalize()
                        .map_err(|e| ServiceError::Io(e.to_string()))?;
                    let target = rel
                        .canonicalize()
                        .map_err(|e| ServiceError::Io(e.to_string()))?;
                    if !target.starts_with(&ws_root) {
                        return Err(ServiceError::InvalidArguments(
                            "Terminal cwd escapes workspace root".into(),
                        ));
                    }
                    Some(target)
                } else {
                    Some(self.resolve_plugin_workspace_path(raw, true)?)
                }
            }
            _ => None,
        };
        let index = self
            .spawn_terminal_profile(&profile, directory.as_deref(), Some(&title))
            .map_err(ServiceError::Failed)?;
        let session = self.terminals[index].id();
        self.extension_host.next_terminal_id += 1;
        let term_id = self.extension_host.next_terminal_id;
        self.extension_host.terminals.insert(term_id, session);
        Ok(json!(term_id))
    }

    fn native_send_terminal_text(
        &mut self,
        id: u64,
        text: String,
        add_newline: bool,
    ) -> ServiceResult<Value> {
        let Some(&session) = self.extension_host.terminals.get(&id) else {
            return Err(ServiceError::NotFound);
        };
        let Some(tab) = self.terminals.iter().find(|t| t.id() == session) else {
            self.extension_host.terminals.remove(&id);
            return Err(ServiceError::Failed("Terminal has closed".into()));
        };
        let mut bytes = text.into_bytes();
        if add_newline {
            bytes.push(b'\r');
        }
        tab.input(bytes);
        Ok(Value::Null)
    }

    fn native_show_terminal(&mut self, id: u64, preserve_focus: bool) -> ServiceResult<Value> {
        let Some(&session) = self.extension_host.terminals.get(&id) else {
            return Err(ServiceError::NotFound);
        };
        let Some(index) = self.terminals.iter().position(|t| t.id() == session) else {
            self.extension_host.terminals.remove(&id);
            return Err(ServiceError::Failed("Terminal has closed".into()));
        };
        self.terminal_visible = true;
        self.terminal_problems = false;
        self.select_terminal(index);
        self.terminal_focused = !preserve_focus;
        self.resize_terminal();
        Ok(Value::Null)
    }

    fn native_workspace_find_files(
        &self,
        include: Option<String>,
        exclude: Option<String>,
        max_results: Option<usize>,
    ) -> ServiceResult<Value> {
        let limit = max_results.unwrap_or(500).clamp(1, 5_000);
        let ws_root = self.tree.root();
        let include: Vec<String> = include
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect();
        let exclude: Vec<String> = exclude
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect();
        let found = self
            .tree
            .workspace_snapshot()
            .find_files(None, &include, &exclude, limit);
        let rels: Vec<String> = found
            .into_iter()
            .filter_map(|p| {
                p.strip_prefix(ws_root)
                    .ok()
                    .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            })
            .collect();
        serde_json::to_value(rels).map_err(|e| ServiceError::Failed(e.to_string()))
    }

    fn native_workspace_read_file(&self, path: &str) -> ServiceResult<Value> {
        let target = self.resolve_plugin_workspace_path(path, true)?;
        let meta = std::fs::metadata(&target).map_err(|e| ServiceError::Io(e.to_string()))?;
        if meta.len() > MAX_WORKSPACE_READ_BYTES {
            return Err(ServiceError::InvalidArguments(
                "Workspace file exceeds 2 MiB limit".into(),
            ));
        }
        let text = std::fs::read_to_string(&target).map_err(|e| ServiceError::Io(e.to_string()))?;
        Ok(Value::String(text))
    }

    fn native_workspace_write_file(&mut self, path: &str, content: &str) -> ServiceResult<Value> {
        let target = self.resolve_plugin_workspace_path(path, false)?;
        if self.open_document_has_unsaved_changes(&target) {
            return Err(ServiceError::Failed(format!(
                "{} has unsaved changes in the editor",
                target.display()
            )));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ServiceError::Io(e.to_string()))?;
        }
        std::fs::write(&target, content.as_bytes()).map_err(|e| ServiceError::Io(e.to_string()))?;
        self.sync_open_document_text(&target, Some(content));
        Ok(Value::Null)
    }

    fn native_scm_status(&self) -> ServiceResult<Value> {
        let snap = crate::vcs::scm::snapshot(self.tree.root());
        let out = ScmStatusSnapshot {
            branch: snap.branch,
            files: snap
                .files
                .into_iter()
                .map(|f| {
                    let status = if f.conflicted {
                        "conflicted"
                    } else if f.untracked {
                        "untracked"
                    } else if f.staged && f.unstaged {
                        "staged+modified"
                    } else if f.staged {
                        "staged"
                    } else {
                        "modified"
                    };
                    ScmFileStatus {
                        path: f.path.to_string_lossy().replace('\\', "/"),
                        status: status.to_owned(),
                    }
                })
                .collect(),
            error: snap.error,
        };
        serde_json::to_value(out).map_err(|e| ServiceError::Failed(e.to_string()))
    }

    fn native_register_chat_tool(
        &mut self,
        tool_id: String,
        name: String,
        description: String,
        schema: Value,
    ) -> ServiceResult<Value> {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.len() > 64 {
            return Err(ServiceError::InvalidArguments(
                "Chat tool name must be 1..=64 characters".into(),
            ));
        }
        if BUILTIN_CHAT_TOOLS.contains(&trimmed)
            || self
                .extension_host
                .chat_tools
                .values()
                .any(|t| t.name == trimmed && t.id != tool_id)
            || self
                .plugins
                .hosts
                .values()
                .flat_map(|h| h.chat_tools.values())
                .any(|t| t.name == trimmed)
        {
            return Err(ServiceError::InvalidArguments(format!(
                "Chat tool name `{trimmed}` is already registered"
            )));
        }
        if self.extension_host.chat_tools.len() >= 64 {
            return Err(ServiceError::Busy);
        }
        self.extension_host.chat_tools.insert(
            tool_id.clone(),
            NativeChatTool {
                id: tool_id,
                name: trimmed.to_owned(),
                description,
                schema,
            },
        );
        Ok(Value::Null)
    }

    /// Collects all chat tools currently registered by active native plugins.
    pub(crate) fn plugin_chat_tool_specs(&self) -> Vec<graphite_chat::ToolSpec> {
        let mut specs = Vec::new();
        for tool in self.extension_host.chat_tools.values().chain(
            self.plugins
                .hosts
                .values()
                .flat_map(|h| h.chat_tools.values()),
        ) {
            specs.push(graphite_chat::ToolSpec {
                name: tool.name.clone(),
                description: tool.description.clone(),
                schema: tool.schema.clone(),
            });
        }
        specs.extend(self.vsix_lm_tool_specs());
        specs
    }

    /// Invokes a named plugin chat tool across `extension_host`, `plugins.hosts`, and active VSIX extensions.
    pub(crate) fn invoke_plugin_chat_tool(
        &mut self,
        name: &str,
        input: Value,
        reply: Option<mpsc::Sender<Result<String, String>>>,
    ) -> Result<u64, String> {
        let request_for =
            |tool_id: String, input: Value| RpcRequest::InvokeChatTool { tool_id, input };
        if let Some(tool_id) = self
            .extension_host
            .chat_tools
            .values()
            .find(|t| t.name == name)
            .map(|t| t.id.clone())
        {
            if let Some(host) = self.extension_host.supervisor.as_mut() {
                if host.state == HostState::Ready {
                    let rpc_id = host
                        .request(
                            request_for(tool_id, input),
                            CancellationToken::default(),
                            Instant::now(),
                        )
                        .map_err(|e| e.to_string())?;
                    self.extension_host.chat_replies.insert(rpc_id, reply);
                    return Ok(rpc_id);
                }
            }
        }
        let host_id = self
            .plugins
            .hosts
            .iter()
            .find(|(_, h)| h.chat_tools.values().any(|t| t.name == name))
            .map(|(id, _)| id.clone());
        if let Some(plugin_id) = host_id {
            if let Some(host) = self.plugins.hosts.get_mut(&plugin_id) {
                if let Some(tool_id) = host
                    .chat_tools
                    .values()
                    .find(|t| t.name == name)
                    .map(|t| t.id.clone())
                {
                    if let Some(supervisor) = host.supervisor.as_mut() {
                        if supervisor.state == HostState::Ready {
                            let rpc_id = supervisor
                                .request(
                                    request_for(tool_id, input),
                                    CancellationToken::default(),
                                    Instant::now(),
                                )
                                .map_err(|e| e.to_string())?;
                            host.chat_replies.insert(rpc_id, reply);
                            return Ok(rpc_id);
                        }
                    }
                }
            }
        }
        self.invoke_vsix_lm_tool(name, input, reply)
    }

    pub(crate) fn apply_native_chat_tool_reply(
        &mut self,
        reply: Option<mpsc::Sender<Result<String, String>>>,
        result: ServiceResult<Value>,
    ) {
        let outcome = match result {
            Ok(value) => match serde_json::from_value::<ChatToolOutcome>(value.clone()) {
                Ok(out) => {
                    if out.is_error {
                        Err(out.content)
                    } else {
                        Ok(out.content)
                    }
                }
                Err(_) => Ok(match value {
                    Value::String(s) => s,
                    Value::Null => String::new(),
                    other => other.to_string(),
                }),
            },
            Err(err) => Err(err.to_string()),
        };
        if let Some(sender) = reply {
            let _ = sender.send(outcome);
        }
    }
}
