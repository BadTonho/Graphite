//! The saved model service settings, sessions, and tool approvals of a profile.

use crate::provider::ProviderConfig;
use crate::session::ChatSession;
use crate::tools::ToolApprovalPolicy;
use std::fs;
use std::path::{Path, PathBuf};

/// File name of the saved settings, inside the profile folder.
pub const SETTINGS_FILE: &str = "chat-provider.json";
pub const APPROVAL_POLICY_FILE: &str = "chat-tool-approvals.json";
pub const SESSIONS_DIR: &str = "chat-sessions";

/// Writes the settings into `dir`. The file holds the API key, so on Unix it is readable only
/// by its owner.
pub fn save_provider(dir: &Path, config: &ProviderConfig) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let path = dir.join(SETTINGS_FILE);
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    restrict_to_owner(&path)
}

/// The saved settings in `dir`, if there is a readable file.
pub fn load_provider(dir: &Path) -> Option<ProviderConfig> {
    let bytes = fs::read(dir.join(SETTINGS_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Forgets the saved settings, key included. A missing file is not an error.
pub fn delete_provider(dir: &Path) -> Result<(), String> {
    match fs::remove_file(dir.join(SETTINGS_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

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
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    restrict_to_owner(&path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderKind;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("graphite-chat-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn config() -> ProviderConfig {
        ProviderConfig {
            kind: ProviderKind::Anthropic,
            base_url: String::new(),
            model: "m".into(),
            api_key: Some("sk-secret".into()),
        }
    }

    #[test]
    fn settings_round_trip_and_delete_is_idempotent() {
        let dir = temp_dir("roundtrip");
        assert_eq!(load_provider(&dir), None);
        save_provider(&dir, &config()).unwrap();
        assert_eq!(load_provider(&dir), Some(config()));
        delete_provider(&dir).unwrap();
        assert_eq!(load_provider(&dir), None);
        delete_provider(&dir).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_file_reads_as_no_settings() {
        let dir = temp_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(SETTINGS_FILE), "{not json").unwrap();
        assert_eq!(load_provider(&dir), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn the_saved_file_is_readable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("mode");
        save_provider(&dir, &config()).unwrap();
        let mode = fs::metadata(dir.join(SETTINGS_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = fs::remove_dir_all(&dir);
    }
}
