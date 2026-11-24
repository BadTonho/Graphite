//! The saved model service settings, sessions, and tool approvals of a profile.

use crate::session::ChatSession;
use crate::tools::ToolApprovalPolicy;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const APPROVAL_POLICY_FILE: &str = "chat-tool-approvals.json";
pub const SESSIONS_DIR: &str = "chat-sessions";

pub fn save_approval_policy(dir: &Path, policy: &ToolApprovalPolicy) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(policy).map_err(|error| error.to_string())?;
    fs::write(dir.join(APPROVAL_POLICY_FILE), bytes).map_err(|error| error.to_string())
}

pub fn load_approval_policy(dir: &Path) -> ToolApprovalPolicy {
    fs::read(dir.join(APPROVAL_POLICY_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn sanitize_session_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn session_file_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(SESSIONS_DIR)
        .join(format!("{}.json", sanitize_session_id(id)))
}

pub fn save_session(dir: &Path, session: &ChatSession) -> Result<(), String> {
    let sessions_dir = dir.join(SESSIONS_DIR);
    fs::create_dir_all(&sessions_dir).map_err(|error| error.to_string())?;
    let path = session_file_path(dir, &session.id);
    let bytes = serde_json::to_vec_pretty(session).map_err(|error| error.to_string())?;
    let mut pending =
        tempfile::NamedTempFile::new_in(sessions_dir).map_err(|error| error.to_string())?;
    restrict_to_owner(pending.path())?;
    pending
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    pending
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    pending.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn load_sessions(dir: &Path) -> Vec<ChatSession> {
    let sessions_dir = dir.join(SESSIONS_DIR);
    let Ok(entries) = fs::read_dir(&sessions_dir) else {
        return Vec::new();
    };
    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(session) = serde_json::from_slice::<ChatSession>(&bytes) {
                sessions.push(session);
            }
        }
    }
    sessions.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.updated_at_epoch_s.cmp(&a.updated_at_epoch_s))
            .then_with(|| a.id.cmp(&b.id))
    });
    sessions
}

pub fn delete_session(dir: &Path, id: &str) -> Result<(), String> {
    let path = session_file_path(dir, id);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) -> Result<(), String> {
    // Windows files inherit the ACL of the user's profile folder, which only that user can read.
    Ok(())
}
