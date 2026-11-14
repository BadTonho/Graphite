//! Edit Sessions (working change serialization, store, resume, output channel log).

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const EDIT_SESSIONS_FILE: &str = "edit-sessions.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WorkingChangeFile {
    pub relative_path: String,
    pub content: String,
    #[serde(default)]
    pub dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EditSessionRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub remote_host: Option<String>,
    pub created_at_epoch_s: u64,
    #[serde(default)]
    pub files: Vec<WorkingChangeFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct EditSessionStore {
    #[serde(default)]
    pub sessions: Vec<EditSessionRecord>,
    #[serde(default)]
    pub next_seq: u64,
    #[serde(skip)]
    pub isolated: bool,
    #[serde(skip)]
    pub output_log: Vec<String>,
}

fn now_epoch_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Resolves a stored relative path strictly inside `workspace_root`.
pub(crate) fn resolve_relative_workspace_path(
    workspace_root: &Path,
    rel: &str,
) -> Result<PathBuf, String> {
    let p = Path::new(rel);
    if p.is_absolute() {
        return Err(format!("Edit session path must be relative: {rel}"));
    }
    let mut clean = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("Invalid traversal in edit session path: {rel}"));
            }
        }
    }
    if clean.as_os_str().is_empty() {
        return Err("Empty path in edit session".into());
    }
    Ok(workspace_root.join(clean))
}

impl EditSessionStore {
    pub fn isolated() -> Self {
        Self {
            isolated: true,
            ..Self::default()
        }
    }

    pub fn load(profile_root: &Path) -> Self {
        let path = profile_root.join(EDIT_SESSIONS_FILE);
        let mut store = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Self>(&raw).ok())
            .unwrap_or_default();
        store.isolated = false;
        store
    }

    pub fn save(&self, profile_root: &Path) -> Result<(), String> {
        if self.isolated {
            return Ok(());
        }
        std::fs::create_dir_all(profile_root).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(profile_root.join(EDIT_SESSIONS_FILE), json).map_err(|e| e.to_string())
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        self.output_log.push(msg.into());
        if self.output_log.len() > 200 {
            let drain = self.output_log.len() - 200;
            self.output_log.drain(0..drain);
        }
    }

    pub fn store_record(
        &mut self,
        name: String,
        branch: Option<String>,
        remote_host: Option<String>,
        files: Vec<WorkingChangeFile>,
        profile_root: &Path,
    ) -> EditSessionRecord {
        self.next_seq += 1;
        let id = format!("es-{}", self.next_seq);
        let record = EditSessionRecord {
            id: id.clone(),
            name,
            branch,
            remote_host,
            created_at_epoch_s: now_epoch_s(),
            files,
        };
        self.sessions.insert(0, record.clone());
        self.log(format!(
            "Stored edit session {id} ({} file(s))",
            record.files.len()
        ));
        let _ = self.save(profile_root);
        record
    }

    pub fn parse_serialized_payload(payload: &str) -> Result<EditSessionRecord, String> {
        serde_json::from_str::<EditSessionRecord>(payload)
            .map_err(|e| format!("Invalid edit session payload: {e}"))
    }

    pub fn delete_by_id(&mut self, id: &str, profile_root: &Path) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.id != id);
        let removed = self.sessions.len() != before;
        if removed {
            self.log(format!("Deleted edit session {id}"));
            let _ = self.save(profile_root);
        }
        removed
    }

    pub fn delete_all(&mut self, profile_root: &Path) -> usize {
        let count = self.sessions.len();
        self.sessions.clear();
        self.log(format!("Deleted all ({count}) edit sessions"));
        let _ = self.save(profile_root);
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_and_round_trips_serialized_payload() {
        let root = Path::new("/tmp/workspace");
        assert!(resolve_relative_workspace_path(root, "../escape.txt").is_err());
        assert!(resolve_relative_workspace_path(root, "src/main.rs").is_ok());

        let dir = crate::test_support::TempDir::new();
        let mut store = EditSessionStore::default();
        let rec = store.store_record(
            "WIP refactor".into(),
            Some("feat/sync".into()),
            None,
            vec![WorkingChangeFile {
                relative_path: "src/lib.rs".into(),
                content: "pub fn ok() {}".into(),
                dirty: true,
            }],
            dir.path(),
        );
        let json = serde_json::to_string(&rec).unwrap();
        let parsed = EditSessionStore::parse_serialized_payload(&json).unwrap();
        assert_eq!(parsed.name, "WIP refactor");
        assert_eq!(parsed.files.len(), 1);
    }
}
