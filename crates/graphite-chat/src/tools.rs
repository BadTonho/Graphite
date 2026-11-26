//! Workspace tools, path safety, approval policies, and execution outcomes.

use crate::message::ToolSpec;
use crate::session::AgentMode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Tool names exposed to the model.
pub const TOOL_READ_FILE: &str = "read_file";
pub const TOOL_LIST_FILES: &str = "list_files";
pub const TOOL_SEARCH_WORKSPACE: &str = "search_workspace";
pub const TOOL_PROPOSE_EDIT: &str = "propose_edit";
pub const TOOL_RUN_COMMAND: &str = "run_command";

/// Controls which tools can run automatically vs. requiring explicit user approval.
///
/// "Always allow" is remembered for one workspace only. For `run_command` it covers the exact
/// command that was approved, never every command. Files saved by older versions kept a single
/// list of trusted tools for every workspace; that list is not read, so those approvals are
/// asked for again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolApprovalPolicy {
    #[serde(default = "default_true")]
    pub auto_approve_read_only: bool,
    /// Remembered approvals, keyed by the workspace root they were given in.
    #[serde(default)]
    pub workspaces: BTreeMap<String, WorkspaceApprovals>,
}

/// The approvals remembered for one workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceApprovals {
    /// Tools (other than `run_command`) that run without asking.
    #[serde(default)]
    pub tools: BTreeSet<String>,
    /// Exact `run_command` commands that run without asking.
    #[serde(default)]
    pub commands: BTreeSet<String>,
}

fn default_true() -> bool {
    true
}

impl Default for ToolApprovalPolicy {
    fn default() -> Self {
        Self {
            auto_approve_read_only: true,
            workspaces: BTreeMap::new(),
        }
    }
}

/// The key a workspace's approvals are stored under.
fn workspace_key(workspace_root: &Path) -> String {
    let root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let key = root.to_string_lossy().into_owned();
    // Windows paths are case-insensitive.
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

fn command_of(input: &Value) -> Option<&str> {
    input
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
}

impl ToolApprovalPolicy {
    pub fn is_read_only(tool_name: &str) -> bool {
        matches!(
            tool_name,
            TOOL_READ_FILE | TOOL_LIST_FILES | TOOL_SEARCH_WORKSPACE
        )
    }

    /// Whether this call was allowed with "Always allow" in `workspace_root`.
    pub fn is_trusted(&self, workspace_root: &Path, tool_name: &str, input: &Value) -> bool {
        let Some(approvals) = self.workspaces.get(&workspace_key(workspace_root)) else {
            return false;
        };
        if tool_name == TOOL_RUN_COMMAND {
            command_of(input).is_some_and(|command| approvals.commands.contains(command))
        } else {
            approvals.tools.contains(tool_name)
        }
    }

    pub fn requires_approval(&self, workspace_root: &Path, tool_name: &str, input: &Value) -> bool {
        if self.auto_approve_read_only && Self::is_read_only(tool_name) {
            return false;
        }
        !self.is_trusted(workspace_root, tool_name, input)
    }

    /// Remembers "Always allow" for this call in `workspace_root`: the exact command for
    /// `run_command`, the tool for anything else.
    pub fn trust(&mut self, workspace_root: &Path, tool_name: &str, input: &Value) {
        if tool_name == TOOL_RUN_COMMAND {
            let Some(command) = command_of(input) else {
                return;
            };
            self.workspaces
                .entry(workspace_key(workspace_root))
                .or_default()
                .commands
                .insert(command.to_owned());
        } else {
            self.workspaces
                .entry(workspace_key(workspace_root))
                .or_default()
                .tools
                .insert(tool_name.to_owned());
        }
    }

    pub fn reset_trusted(&mut self) {
        self.workspaces.clear();
    }

    /// Whether any approval is remembered, in any workspace.
    pub fn has_trusted(&self) -> bool {
        self.workspaces
            .values()
            .any(|approvals| !approvals.tools.is_empty() || !approvals.commands.is_empty())
    }
}

/// User response to an interactive tool approval prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    AllowOnce,
    AllowAlways,
    Deny,
}

/// A tool invocation awaiting user approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingToolApproval {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub input: Value,
}

/// A proposed workspace edit produced by `propose_edit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedEdit {
    pub path: PathBuf,
    pub before: Option<String>,
    pub after: String,
}

/// Outcome of running a single tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionResult {
    pub content: String,
    pub is_error: bool,
    pub proposed_edit: Option<ProposedEdit>,
}

/// Pluggable shell/command runner so `run_command` can be tested deterministically.
pub trait CommandRunner: Send + Sync {
    fn run(&self, cwd: &Path, command: &str) -> Result<String, String>;
}

/// Returns the tool definitions allowed for `mode`.
pub fn tool_specs_for_mode(mode: AgentMode) -> Vec<ToolSpec> {
    let mut specs = vec![
        ToolSpec {
            name: TOOL_READ_FILE.into(),
            description: "Read UTF-8 text from a file inside the workspace.".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative file path" },
                    "start_line": { "type": "integer", "description": "1-based inclusive start line" },
                    "end_line": { "type": "integer", "description": "1-based inclusive end line" }
                },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: TOOL_LIST_FILES.into(),
            description: "List files and directories inside a workspace directory.".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative directory (empty for root)" }
                }
            }),
        },
        ToolSpec {
            name: TOOL_SEARCH_WORKSPACE.into(),
            description: "Search for a literal text query across workspace files.".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Text to find" }
                },
                "required": ["query"]
            }),
        },
    ];

    if matches!(mode, AgentMode::Edit | AgentMode::Agent) {
        specs.push(ToolSpec {
            name: TOOL_PROPOSE_EDIT.into(),
            description:
                "Propose creating or replacing the content of a workspace file for user review."
                    .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative file path" },
                    "content": { "type": "string", "description": "Complete updated file contents" }
                },
                "required": ["path", "content"]
            }),
        });
    }

    if mode == AgentMode::Agent {
        specs.push(ToolSpec {
            name: TOOL_RUN_COMMAND.into(),
            description: "Run a shell command in the workspace root (requires user approval)."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to run" }
                },
                "required": ["command"]
            }),
        });
    }

    specs
}

/// Resolves `raw` inside `workspace_root`, rejecting path traversal outside the workspace.
pub fn resolve_workspace_path(workspace_root: &Path, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(workspace_root.to_path_buf());
    }
    let root_canonical = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let candidate = Path::new(trimmed);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        let mut clean = PathBuf::new();
        for comp in candidate.components() {
            match comp {
                Component::CurDir => {}
                Component::ParentDir => {
                    if !clean.pop() {
                        return Err("Path escapes workspace root".into());
                    }
                }
                Component::Normal(part) => clean.push(part),
                Component::RootDir | Component::Prefix(_) => {
                    return Err("Invalid path component".into());
                }
            }
        }
        root_canonical.join(clean)
    };

    // Every existing part of the path is resolved, links included, so a link inside the
    // workspace that points outside is caught even when the final file does not exist yet.
    let check = resolve_existing_prefix(&joined).ok_or("Path escapes workspace root")?;

    if !check.starts_with(&root_canonical) {
        return Err("Path escapes workspace root".into());
    }
    Ok(check)
}

/// `path` with its longest existing prefix canonicalized and the missing rest appended. `None`
/// when the missing rest is not plain names (a `..` there could not be checked).
fn resolve_existing_prefix(path: &Path) -> Option<PathBuf> {
    let mut existing = path;
    let mut missing = Vec::new();
    loop {
        if let Ok(mut resolved) = existing.canonicalize() {
            resolved.extend(missing.iter().rev());
            return Some(resolved);
        }
        missing.push(existing.file_name()?);
        existing = existing.parent()?;
    }
}

/// Human-readable one-line summary for approval dialogs and debug logs.
pub fn summarize_tool_call(name: &str, input: &Value) -> String {
    match name {
        TOOL_READ_FILE => format!(
            "Read {}",
            input.get("path").and_then(Value::as_str).unwrap_or("?")
        ),
        TOOL_LIST_FILES => format!(
            "List {}",
            input
                .get("path")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(".")
        ),
        TOOL_SEARCH_WORKSPACE => format!(
            "Search {:?}",
            input.get("query").and_then(Value::as_str).unwrap_or("")
        ),
        TOOL_PROPOSE_EDIT => format!(
            "Edit {}",
            input.get("path").and_then(Value::as_str).unwrap_or("?")
        ),
        TOOL_RUN_COMMAND => format!(
            "Run `{}`",
            input.get("command").and_then(Value::as_str).unwrap_or("")
        ),
        other => format!("Tool {other}"),
    }
}

/// Executes a workspace tool against `workspace_root`.
pub fn execute_tool(
    workspace_root: &Path,
    mode: AgentMode,
    name: &str,
    input: &Value,
    runner: &dyn CommandRunner,
) -> ToolExecutionResult {
    match name {
        TOOL_READ_FILE => run_read_file(workspace_root, input),
        TOOL_LIST_FILES => run_list_files(workspace_root, input),
        TOOL_SEARCH_WORKSPACE => run_search_workspace(workspace_root, input),
        TOOL_PROPOSE_EDIT => {
            if !matches!(mode, AgentMode::Edit | AgentMode::Agent) {
                return ToolExecutionResult {
                    content: "Editing is disabled in Ask mode.".into(),
                    is_error: true,
                    proposed_edit: None,
                };
            }
            run_propose_edit(workspace_root, input)
        }
        TOOL_RUN_COMMAND => {
            if mode != AgentMode::Agent {
                return ToolExecutionResult {
                    content: "Running commands is only enabled in Agent mode.".into(),
                    is_error: true,
                    proposed_edit: None,
                };
            }
            let Some(cmd) = input.get("command").and_then(Value::as_str) else {
                return ToolExecutionResult {
                    content: "Missing 'command' string".into(),
                    is_error: true,
                    proposed_edit: None,
                };
            };
            match runner.run(workspace_root, cmd) {
                Ok(out) => ToolExecutionResult {
                    content: out,
                    is_error: false,
                    proposed_edit: None,
                },
                Err(err) => ToolExecutionResult {
                    content: err,
                    is_error: true,
                    proposed_edit: None,
                },
            }
        }
        other => ToolExecutionResult {
            content: format!("Unknown tool: {other}"),
            is_error: true,
            proposed_edit: None,
        },
    }
}

fn run_read_file(workspace_root: &Path, input: &Value) -> ToolExecutionResult {
    let Some(raw) = input.get("path").and_then(Value::as_str) else {
        return ToolExecutionResult {
            content: "Missing 'path' string".into(),
            is_error: true,
            proposed_edit: None,
        };
    };
    let path = match resolve_workspace_path(workspace_root, raw) {
        Ok(p) => p,
        Err(err) => {
            return ToolExecutionResult {
                content: err,
                is_error: true,
                proposed_edit: None,
            }
        }
    };
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            return ToolExecutionResult {
                content: format!("Failed to read {}: {err}", path.display()),
                is_error: true,
                proposed_edit: None,
            }
        }
    };
    let start = input
        .get("start_line")
        .and_then(Value::as_u64)
        .map(|n| n.max(1) as usize)
        .unwrap_or(1);
    let end = input
        .get("end_line")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(start.saturating_add(400));
    let mut out = String::new();
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        if line_no < start {
            continue;
        }
        if line_no > end {
            out.push_str("\n...[truncated]");
            break;
        }
        out.push_str(&format!("{line_no}: {line}\n"));
        if out.len() > 24_000 {
            out.push_str("\n...[truncated]");
            break;
        }
    }
    ToolExecutionResult {
        content: out,
        is_error: false,
        proposed_edit: None,
    }
}

fn run_list_files(workspace_root: &Path, input: &Value) -> ToolExecutionResult {
    let raw = input.get("path").and_then(Value::as_str).unwrap_or("");
    let dir = match resolve_workspace_path(workspace_root, raw) {
        Ok(p) => p,
        Err(err) => {
            return ToolExecutionResult {
                content: err,
                is_error: true,
                proposed_edit: None,
            }
        }
    };
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) => {
            return ToolExecutionResult {
                content: format!("Failed to list {}: {err}", dir.display()),
                is_error: true,
                proposed_edit: None,
            }
        }
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(name.as_str(), ".git" | "target" | "node_modules") {
            continue;
        }
        let suffix = if entry.file_type().is_ok_and(|t| t.is_dir()) {
            "/"
        } else {
            ""
        };
        names.push(format!("{name}{suffix}"));
    }
    names.sort();
    names.truncate(300);
    ToolExecutionResult {
        content: names.join("\n"),
        is_error: false,
        proposed_edit: None,
    }
}

fn run_search_workspace(workspace_root: &Path, input: &Value) -> ToolExecutionResult {
    let Some(query) = input.get("query").and_then(Value::as_str) else {
        return ToolExecutionResult {
            content: "Missing 'query' string".into(),
            is_error: true,
            proposed_edit: None,
        };
    };
    if query.is_empty() {
        return ToolExecutionResult {
            content: "Search query must not be empty".into(),
            is_error: true,
            proposed_edit: None,
        };
    }
    let root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let mut matches = Vec::new();
    collect_search_matches(&root, &root, query, &mut matches, 60);
    let content = if matches.is_empty() {
        "No matches found.".into()
    } else {
        matches.join("\n")
    };
    ToolExecutionResult {
        content,
        is_error: false,
        proposed_edit: None,
    }
}

fn collect_search_matches(
    root: &Path,
    dir: &Path,
    query: &str,
    out: &mut Vec<String>,
    limit: usize,
) {
    if out.len() >= limit {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if out.len() >= limit {
            break;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if matches!(name, ".git" | "target" | "node_modules") {
            continue;
        }
        // Links are not followed: they can lead outside the workspace or back into a parent.
        let Ok(kind) = fs::symlink_metadata(&path).map(|meta| meta.file_type()) else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect_search_matches(root, &path, query, out, limit);
            continue;
        }
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if meta.len() > 512 * 1024 {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let rel = path.strip_prefix(root).unwrap_or(&path);
        for (idx, line) in text.lines().enumerate() {
            if line.contains(query) {
                let snippet: String = line.chars().take(160).collect();
                out.push(format!("{}:{}: {}", rel.display(), idx + 1, snippet));
                if out.len() >= limit {
                    break;
                }
            }
        }
    }
}

fn run_propose_edit(workspace_root: &Path, input: &Value) -> ToolExecutionResult {
    let Some(raw) = input.get("path").and_then(Value::as_str) else {
        return ToolExecutionResult {
            content: "Missing 'path' string".into(),
            is_error: true,
            proposed_edit: None,
        };
    };
    let Some(after) = input.get("content").and_then(Value::as_str) else {
        return ToolExecutionResult {
            content: "Missing 'content' string".into(),
            is_error: true,
            proposed_edit: None,
        };
    };
    let path = match resolve_workspace_path(workspace_root, raw) {
        Ok(p) => p,
        Err(err) => {
            return ToolExecutionResult {
                content: err,
                is_error: true,
                proposed_edit: None,
            }
        }
    };
    let before = fs::read_to_string(&path).ok();
    let action = if before.is_some() {
        "Staged update"
    } else {
        "Staged new file"
    };
    ToolExecutionResult {
        content: format!("{action} for {} ({} bytes)", path.display(), after.len()),
        is_error: false,
        proposed_edit: Some(ProposedEdit {
            path,
            before,
            after: after.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeRunner(Mutex<Vec<String>>);

    impl CommandRunner for FakeRunner {
        fn run(&self, _cwd: &Path, command: &str) -> Result<String, String> {
            self.0.lock().unwrap().push(command.to_owned());
            Ok(format!("ran: {command}"))
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("graphite-chat-tools-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn workspace_path_resolution_blocks_escape() {
        let dir = temp_workspace("escape");
        assert!(resolve_workspace_path(&dir, "../outside.txt").is_err());
        assert!(resolve_workspace_path(&dir, "src/lib.rs").is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_search_edit_and_command_respect_modes_and_approval() {
        let dir = temp_workspace("exec");
        fs::write(
            dir.join("hello.rs"),
            "fn main() {\n    println!(\"alpha\");\n}\n",
        )
        .unwrap();
        let runner = FakeRunner(Mutex::new(Vec::new()));

        let read = execute_tool(
            &dir,
            AgentMode::Ask,
            TOOL_READ_FILE,
            &json!({"path": "hello.rs"}),
            &runner,
        );
        assert!(!read.is_error);
        assert!(read.content.contains("1: fn main()"));

        let search = execute_tool(
            &dir,
            AgentMode::Ask,
            TOOL_SEARCH_WORKSPACE,
            &json!({"query": "alpha"}),
            &runner,
        );
        assert!(!search.is_error);
        assert!(search.content.contains("hello.rs:2:"));

        let blocked_edit = execute_tool(
            &dir,
            AgentMode::Ask,
            TOOL_PROPOSE_EDIT,
            &json!({"path": "hello.rs", "content": "x"}),
            &runner,
        );
        assert!(blocked_edit.is_error);

        let staged = execute_tool(
            &dir,
            AgentMode::Edit,
            TOOL_PROPOSE_EDIT,
            &json!({"path": "hello.rs", "content": "fn main() {}\n"}),
            &runner,
        );
        assert!(!staged.is_error);
        assert_eq!(
            staged.proposed_edit.as_ref().unwrap().before.as_deref(),
            Some("fn main() {\n    println!(\"alpha\");\n}\n")
        );

        let cmd = execute_tool(
            &dir,
            AgentMode::Agent,
            TOOL_RUN_COMMAND,
            &json!({"command": "cargo test"}),
            &runner,
        );
        assert!(!cmd.is_error);
        assert_eq!(runner.0.lock().unwrap().as_slice(), ["cargo test"]);

        let mut policy = ToolApprovalPolicy::default();
        let none = json!({});
        assert!(!policy.requires_approval(&dir, TOOL_READ_FILE, &none));
        assert!(policy.requires_approval(&dir, TOOL_PROPOSE_EDIT, &none));
        assert!(policy.requires_approval(&dir, TOOL_RUN_COMMAND, &none));
        policy.trust(&dir, TOOL_PROPOSE_EDIT, &none);
        assert!(!policy.requires_approval(&dir, TOOL_PROPOSE_EDIT, &none));
        policy.reset_trusted();
        assert!(policy.requires_approval(&dir, TOOL_PROPOSE_EDIT, &none));

        let _ = fs::remove_dir_all(&dir);
    }

    /// Makes `link` a directory link to `target`: a junction on Windows, which needs no extra
    /// privilege, and a symbolic link elsewhere.
    fn link_dir(target: &Path, link: &Path) {
        #[cfg(windows)]
        {
            let status = std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(link)
                .arg(target)
                .stdout(std::process::Stdio::null())
                .status()
                .unwrap();
            assert!(status.success());
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[test]
    fn a_link_out_of_the_workspace_is_refused_even_for_files_that_do_not_exist() {
        let root = temp_workspace("link-root");
        let outside = temp_workspace("link-outside");
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        link_dir(&outside, &root.join("link"));

        for raw in [
            "link/secret.txt",
            "link/new.txt",
            "link/new-dir/deeper/new.txt",
        ] {
            assert!(resolve_workspace_path(&root, raw).is_err(), "{raw}");
        }
        let inside = resolve_workspace_path(&root, "a/b/new.txt").unwrap();
        assert!(inside.starts_with(root.canonicalize().unwrap()));
        assert!(inside.ends_with("a/b/new.txt"));

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn search_does_not_follow_links_out_of_the_workspace() {
        let root = temp_workspace("search-root");
        let outside = temp_workspace("search-outside");
        fs::write(outside.join("secret.txt"), "needle outside").unwrap();
        fs::write(root.join("own.txt"), "needle inside").unwrap();
        link_dir(&outside, &root.join("link"));
        // A link back to the workspace itself must not loop.
        link_dir(&root, &root.join("loop"));

        let result = execute_tool(
            &root,
            AgentMode::Ask,
            TOOL_SEARCH_WORKSPACE,
            &json!({"query": "needle"}),
            &FakeRunner(Mutex::new(Vec::new())),
        );

        assert!(!result.is_error);
        assert!(result.content.contains("own.txt"), "{}", result.content);
        assert!(!result.content.contains("outside"), "{}", result.content);
        assert_eq!(result.content.lines().count(), 1, "{}", result.content);
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn always_allow_is_limited_to_its_workspace() {
        let first = temp_workspace("trust-first");
        let second = temp_workspace("trust-second");
        let none = json!({});
        let mut policy = ToolApprovalPolicy::default();

        policy.trust(&first, TOOL_PROPOSE_EDIT, &none);

        assert!(!policy.requires_approval(&first, TOOL_PROPOSE_EDIT, &none));
        assert!(policy.requires_approval(&second, TOOL_PROPOSE_EDIT, &none));
        let _ = fs::remove_dir_all(&first);
        let _ = fs::remove_dir_all(&second);
    }

    #[test]
    fn always_allow_for_a_command_covers_only_that_command() {
        let dir = temp_workspace("trust-command");
        let mut policy = ToolApprovalPolicy::default();

        policy.trust(&dir, TOOL_RUN_COMMAND, &json!({"command": " cargo test "}));

        assert!(!policy.requires_approval(
            &dir,
            TOOL_RUN_COMMAND,
            &json!({"command": "cargo test"})
        ));
        assert!(policy.requires_approval(
            &dir,
            TOOL_RUN_COMMAND,
            &json!({"command": "cargo test && curl example.invalid"})
        ));
        assert!(policy.requires_approval(&dir, TOOL_RUN_COMMAND, &json!({"command": "rm -rf ."})));
        assert!(policy.requires_approval(&dir, TOOL_RUN_COMMAND, &json!({})));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn approvals_saved_for_every_workspace_by_older_versions_are_not_honored() {
        let dir = temp_workspace("trust-legacy");
        let policy: ToolApprovalPolicy = serde_json::from_str(
            r#"{"auto_approve_read_only":true,"trusted_tools":["run_command","propose_edit"]}"#,
        )
        .unwrap();

        assert!(policy.requires_approval(&dir, TOOL_RUN_COMMAND, &json!({"command": "ls"})));
        assert!(policy.requires_approval(&dir, TOOL_PROPOSE_EDIT, &json!({})));
        assert!(!policy.has_trusted());
        let _ = fs::remove_dir_all(&dir);
    }
}
