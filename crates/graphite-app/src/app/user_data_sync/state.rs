//! Persistent User Data Sync state, snapshots, backups, and conflict records.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const SYNC_STATE_FILE: &str = "user-data-sync.json";
pub(crate) const SYNC_BACKUPS_DIR: &str = "sync-backups";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SyncResourcePrefs {
    #[serde(default = "default_true")]
    pub settings: bool,
    #[serde(default = "default_true")]
    pub keybindings: bool,
    #[serde(default = "default_true")]
    pub snippets: bool,
    #[serde(default = "default_true")]
    pub plugins: bool,
    #[serde(default = "default_true")]
    pub profiles: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SyncResourcePrefs {
    fn default() -> Self {
        Self {
            settings: true,
            keybindings: true,
            snippets: true,
            plugins: true,
            profiles: true,
        }
    }
}

/// Captured user configuration data for sync and backups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SyncSnapshot {
    #[serde(default)]
    pub settings_json: Option<String>,
    #[serde(default)]
    pub keybindings_json: Option<String>,
    #[serde(default)]
    pub snippets: BTreeMap<String, String>,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default)]
    pub profiles_json: Option<String>,
    #[serde(default)]
    pub updated_at_epoch_s: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConflictChoice {
    AcceptLocal,
    AcceptRemote,
    KeepMerged,
}

/// Key-level or resource-level merge conflict between local and remote synced state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SyncConflict {
    pub resource: String,
    pub key: String,
    #[serde(default)]
    pub local_value: Option<String>,
    #[serde(default)]
    pub remote_value: Option<String>,
    #[serde(default)]
    pub resolved: Option<ConflictChoice>,
}

/// Where the synced data is kept on GitHub.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SyncBackend {
    /// A secret Gist.
    #[default]
    Gist,
    /// A private repository named `graphite-sync`.
    Repository,
}

impl SyncBackend {
    /// The OAuth scope the backend needs.
    pub(crate) fn required_scope(self) -> &'static str {
        match self {
            Self::Gist => "gist",
            Self::Repository => "repo",
        }
    }

    /// The scopes to request when signing in for the backend.
    pub(crate) fn sign_in_scope(self) -> &'static str {
        match self {
            Self::Gist => graphite_auth::SCOPE_GIST,
            Self::Repository => graphite_auth::SCOPE_REPO,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedSyncData {
    /// Backend in use; a file written before this existed means a Gist.
    #[serde(default)]
    pub backend: SyncBackend,
    #[serde(default)]
    pub enabled: bool,
    /// Gist id, or `owner/repository` for the repository backend.
    #[serde(default, alias = "gist_id")]
    pub remote_id: Option<String>,
    #[serde(default)]
    pub last_sync_epoch_s: Option<u64>,
    #[serde(default)]
    pub resources: SyncResourcePrefs,
    #[serde(default)]
    pub last_base: Option<SyncSnapshot>,
    #[serde(default)]
    pub conflicts: Vec<SyncConflict>,
}

#[derive(Debug, Clone)]
pub(crate) struct SyncState {
    pub persisted: PersistedSyncData,
    pub turning_on: bool,
    /// Backend the pending sign-in is for.
    pub turning_on_backend: SyncBackend,
    pub syncing: bool,
    pub isolated: bool,
    pub last_error: Option<String>,
    pub last_summary: Option<String>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            persisted: PersistedSyncData::default(),
            turning_on: false,
            turning_on_backend: SyncBackend::default(),
            syncing: false,
            isolated: false,
            last_error: None,
            last_summary: None,
        }
    }
}

impl PersistedSyncData {
    /// Chooses the backend. The remote and the merge base belong to the backend they came from,
    /// so changing it starts afresh.
    pub(crate) fn use_backend(&mut self, backend: SyncBackend) {
        if self.backend != backend {
            self.backend = backend;
            self.remote_id = None;
            self.last_base = None;
            self.conflicts.clear();
        }
    }
}

impl SyncState {
    pub fn isolated() -> Self {
        Self {
            isolated: true,
            ..Self::default()
        }
    }

    pub fn load(profile_root: &Path) -> Self {
        let path = profile_root.join(SYNC_STATE_FILE);
        let persisted = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PersistedSyncData>(&raw).ok())
            .unwrap_or_default();
        Self {
            persisted,
            ..Self::default()
        }
    }

    pub fn save(&self, profile_root: &Path) -> Result<(), String> {
        if self.isolated {
            return Ok(());
        }
        save_persisted_sync(profile_root, &self.persisted)
    }
}

pub(crate) fn save_persisted_sync(
    profile_root: &Path,
    persisted: &PersistedSyncData,
) -> Result<(), String> {
    std::fs::create_dir_all(profile_root).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(persisted).map_err(|e| e.to_string())?;
    std::fs::write(profile_root.join(SYNC_STATE_FILE), json).map_err(|e| e.to_string())
}

pub(crate) fn now_epoch_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Reads local syncable configuration files from `profile_root`.
pub(crate) fn capture_snapshot(profile_root: &Path, prefs: &SyncResourcePrefs) -> SyncSnapshot {
    let settings_json = prefs
        .settings
        .then(|| std::fs::read_to_string(profile_root.join("settings.json")).ok())
        .flatten();
    let keybindings_json = prefs
        .keybindings
        .then(|| std::fs::read_to_string(profile_root.join("keybindings.json")).ok())
        .flatten();

    let mut snippets = BTreeMap::new();
    if prefs.snippets {
        let snippets_dir = profile_root.join("snippets");
        if let Ok(entries) = std::fs::read_dir(snippets_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let (Some(name), Ok(content)) = (
                        path.file_name().and_then(|n| n.to_str()),
                        std::fs::read_to_string(&path),
                    ) {
                        snippets.insert(name.to_owned(), content);
                    }
                }
            }
        }
    }

    let mut plugins = Vec::new();
    if prefs.plugins {
        for folder in ["plugins", "extensions"] {
            let dir = profile_root.join(folder);
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if !plugins.iter().any(|p| p == name) {
                            plugins.push(name.to_owned());
                        }
                    }
                }
            }
        }
        plugins.sort();
    }

    let profiles_json = prefs
        .profiles
        .then(|| std::fs::read_to_string(profile_root.join("profiles.json")).ok())
        .flatten();

    SyncSnapshot {
        settings_json,
        keybindings_json,
        snippets,
        plugins,
        profiles_json,
        updated_at_epoch_s: now_epoch_s(),
    }
}

/// Writes a timestamped backup of `snapshot` inside `<profile_root>/sync-backups/`.
pub(crate) fn write_local_backup(
    profile_root: &Path,
    snapshot: &SyncSnapshot,
    label: &str,
) -> Result<PathBuf, String> {
    let root = profile_root.join(SYNC_BACKUPS_DIR);
    let folder = root.join(format!("{}-{label}", now_epoch_s()));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    if let Some(settings) = &snapshot.settings_json {
        std::fs::write(folder.join("settings.json"), settings).map_err(|e| e.to_string())?;
    }
    if let Some(kb) = &snapshot.keybindings_json {
        std::fs::write(folder.join("keybindings.json"), kb).map_err(|e| e.to_string())?;
    }
    if let Some(profiles) = &snapshot.profiles_json {
        std::fs::write(folder.join("profiles.json"), profiles).map_err(|e| e.to_string())?;
    }
    if !snapshot.snippets.is_empty() {
        let sdir = folder.join("snippets");
        std::fs::create_dir_all(&sdir).map_err(|e| e.to_string())?;
        for (name, body) in &snapshot.snippets {
            std::fs::write(sdir.join(name), body).map_err(|e| e.to_string())?;
        }
    }
    let meta = serde_json::to_string_pretty(snapshot).map_err(|e| e.to_string())?;
    std::fs::write(folder.join("snapshot.json"), meta).map_err(|e| e.to_string())?;
    Ok(folder)
}

/// Writes merged snapshot data back into `profile_root`.
pub(crate) fn apply_snapshot_to_profile(
    profile_root: &Path,
    snapshot: &SyncSnapshot,
    prefs: &SyncResourcePrefs,
) -> Result<(), String> {
    std::fs::create_dir_all(profile_root).map_err(|e| e.to_string())?;
    if prefs.settings {
        if let Some(settings) = &snapshot.settings_json {
            std::fs::write(profile_root.join("settings.json"), settings)
                .map_err(|e| e.to_string())?;
        }
    }
    if prefs.keybindings {
        if let Some(kb) = &snapshot.keybindings_json {
            std::fs::write(profile_root.join("keybindings.json"), kb).map_err(|e| e.to_string())?;
        }
    }
    if prefs.snippets && !snapshot.snippets.is_empty() {
        let sdir = profile_root.join("snippets");
        std::fs::create_dir_all(&sdir).map_err(|e| e.to_string())?;
        for (name, body) in &snapshot.snippets {
            std::fs::write(sdir.join(name), body).map_err(|e| e.to_string())?;
        }
    }
    if prefs.profiles {
        if let Some(profiles) = &snapshot.profiles_json {
            std::fs::write(profile_root.join("profiles.json"), profiles)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
