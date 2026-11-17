//! User Data Sync engine, background worker, local backups, and conflict resolution.

pub(crate) mod merge;
pub(crate) mod state;
pub(crate) mod transport;

use super::*;
use merge::{apply_conflict_resolutions, merge_snapshots};
pub(crate) use state::SyncState;
use state::{
    apply_snapshot_to_profile, capture_snapshot, now_epoch_s, save_persisted_sync,
    write_local_backup, ConflictChoice, PersistedSyncData, SyncBackend, SyncConflict, SyncSnapshot,
};
use transport::{GitHubGistTransport, GitHubRepoTransport, SyncTransport};

/// The transport for a backend.
fn make_transport(backend: SyncBackend) -> Result<Box<dyn SyncTransport>, String> {
    match backend {
        SyncBackend::Gist => Ok(Box::new(GitHubGistTransport::new()?)),
        SyncBackend::Repository => Ok(Box::new(GitHubRepoTransport::new()?)),
    }
}

#[derive(Debug)]
pub(crate) struct SyncOutcome {
    pub remote_id: String,
    pub merged_snapshot: SyncSnapshot,
    pub conflicts: Vec<SyncConflict>,
    pub pulled_remote: bool,
    pub pushed_remote: bool,
}

#[derive(Debug)]
pub(crate) enum SyncUiEvent {
    SyncFinished(Result<SyncOutcome, String>),
    ResetFinished(Result<(), String>),
}

/// Executes a complete fetch -> 3-way merge -> backup -> write -> push sync round.
pub(crate) fn execute_sync_round(
    profile_root: &Path,
    token: &str,
    persisted: &PersistedSyncData,
    transport: &dyn SyncTransport,
) -> Result<SyncOutcome, String> {
    let local = capture_snapshot(profile_root, &persisted.resources);
    let remote_bundle = transport.fetch_snapshot(token, persisted.remote_id.as_deref())?;

    let (remote_id, merged_snapshot, conflicts, pulled_remote, pushed_remote) = match remote_bundle
    {
        Some(remote) => {
            let (merged, conflicts) =
                merge_snapshots(persisted.last_base.as_ref(), &local, &remote.snapshot);
            let _ = write_local_backup(profile_root, &local, "pre-sync");
            if conflicts.is_empty() {
                apply_snapshot_to_profile(profile_root, &merged, &persisted.resources)?;
            }
            let final_remote_id = if merged != remote.snapshot && conflicts.is_empty() {
                transport.push_snapshot(token, Some(&remote.remote_id), &merged)?
            } else {
                remote.remote_id
            };
            (
                final_remote_id,
                merged.clone(),
                conflicts,
                true,
                merged != remote.snapshot,
            )
        }
        None => {
            let id = transport.push_snapshot(token, persisted.remote_id.as_deref(), &local)?;
            (id, local, Vec::new(), false, true)
        }
    };

    Ok(SyncOutcome {
        remote_id,
        merged_snapshot,
        conflicts,
        pulled_remote,
        pushed_remote,
    })
}

/// Whether a space-separated list of OAuth scopes includes `scope`.
fn has_scope(scopes: &str, scope: &str) -> bool {
    scopes.split_whitespace().any(|granted| granted == scope)
}

/// The scopes in `first` followed by those of `second` that are not there yet.
fn merge_scopes(first: &str, second: &str) -> String {
    let mut merged: Vec<&str> = first.split_whitespace().collect();
    for scope in second.split_whitespace() {
        if !merged.contains(&scope) {
            merged.push(scope);
        }
    }
    merged.join(" ")
}

/// Text key and number for "synced N units ago".
fn relative_age(seconds: u64) -> (&'static str, u64) {
    match seconds {
        0..=59 => ("sync_menu.just_now", 0),
        60..=3599 => ("sync_menu.minutes_ago", seconds / 60),
        3600..=86_399 => ("sync_menu.hours_ago", seconds / 3600),
        _ => ("sync_menu.days_ago", seconds / 86_400),
    }
}

impl App {
    pub(crate) fn start_sync_sign_in(&mut self) {
        self.start_sync_sign_in_for(self.sync.persisted.backend);
    }

    /// Signs in with what `backend` needs, then turns the sync on with it.
    pub(crate) fn start_sync_sign_in_for(&mut self, backend: SyncBackend) {
        self.sync.turning_on = true;
        self.sync.turning_on_backend = backend;
        let existing = self
            .github_session
            .as_ref()
            .map(|session| session.scope.clone());
        match existing {
            Some(scope) if has_scope(&scope, backend.required_scope()) => {
                self.finish_turning_on_sync()
            }
            // Signed in without the permission: ask again, keeping what was already granted.
            Some(scope) => {
                let wanted = merge_scopes(&scope, backend.sign_in_scope());
                self.start_github_sign_in_with_scope(&wanted);
            }
            None => self.start_github_sign_in_with_scope(backend.sign_in_scope()),
        }
    }

    /// One line for the profile menu: what the sync is doing, or when it last finished.
    pub(crate) fn sync_status_text(&self) -> String {
        let backend = match self.sync.persisted.backend {
            SyncBackend::Gist => self.t("sync_menu.backend_gist"),
            SyncBackend::Repository => self.t("sync_menu.backend_repository"),
        };
        format!("{backend} · {}", self.sync_progress_text())
    }

    fn sync_progress_text(&self) -> String {
        if self.sync.syncing {
            return self.t("sync_menu.syncing").to_owned();
        }
        if self.sync.last_error.is_some() {
            return self.t("sync_menu.failed").to_owned();
        }
        let conflicts = self.sync.persisted.conflicts.len();
        if conflicts > 0 {
            return self.tf("sync_menu.conflicts", &[("count", &conflicts)]);
        }
        match self.sync.persisted.last_sync_epoch_s {
            Some(at) => {
                let (key, count) = relative_age(now_epoch_s().saturating_sub(at));
                self.tf(key, &[("count", &count)])
            }
            None => self.t("sync_menu.never").to_owned(),
        }
    }

    pub(crate) fn turn_on_sync(&mut self) {
        self.turn_on_sync_with(self.sync.persisted.backend);
    }

    /// Turns the sync on with `backend`, signing in first when the account lacks its permission.
    pub(crate) fn turn_on_sync_with(&mut self, backend: SyncBackend) {
        let can_sync = self
            .github_session
            .as_ref()
            .is_some_and(|session| has_scope(&session.scope, backend.required_scope()));
        if !can_sync {
            self.start_sync_sign_in_for(backend);
            return;
        }
        self.enable_sync(backend);
    }

    fn enable_sync(&mut self, backend: SyncBackend) {
        self.sync.persisted.use_backend(backend);
        self.sync.persisted.enabled = true;
        self.sync.turning_on = false;
        let _ = self.sync.save(&self.profile_root);
        self.status = "User Data Sync enabled".into();
        self.sync_now();
    }

    pub(crate) fn finish_turning_on_sync(&mut self) {
        if !self.sync.turning_on {
            return;
        }
        let backend = self.sync.turning_on_backend;
        self.enable_sync(backend);
    }

    pub(crate) fn turn_off_sync(&mut self) {
        self.sync.persisted.enabled = false;
        self.sync.turning_on = false;
        let _ = self.sync.save(&self.profile_root);
        self.status = "User Data Sync turned off".into();
    }

    pub(crate) fn cancel_turn_on_sync(&mut self) {
        self.sync.turning_on = false;
        self.status = "Cancelled turning on User Data Sync".into();
    }

    pub(crate) fn sync_now(&mut self) {
        if self.sync.syncing {
            self.status = "User Data Sync is already running".into();
            return;
        }
        let Some(session) = self.github_session.clone() else {
            self.status = "Sign in with GitHub before syncing user data".into();
            return;
        };
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = "Event loop unavailable for background sync".into();
            return;
        };
        self.sync.syncing = true;
        self.sync.last_error = None;
        self.status = "Syncing user data...".into();
        let profile = self.profile_root.clone();
        let persisted = self.sync.persisted.clone();
        let backend = persisted.backend;

        std::thread::Builder::new()
            .name("graphite-user-sync".into())
            .spawn(move || {
                let result = make_transport(backend).and_then(|transport| {
                    execute_sync_round(&profile, &session.token, &persisted, transport.as_ref())
                });
                let _ = proxy.send_event(UserEvent::Sync(SyncUiEvent::SyncFinished(result)));
            })
            .ok();
    }

    pub(crate) fn handle_sync_event(&mut self, event: SyncUiEvent) {
        match event {
            SyncUiEvent::SyncFinished(Ok(outcome)) => {
                self.sync.syncing = false;
                self.sync.last_error = None;
                self.apply_sync_outcome(outcome);
            }
            SyncUiEvent::SyncFinished(Err(err)) => {
                crate::app::error_log::record(
                    crate::app::error_log::ErrorSeverity::Medium,
                    "user-data-sync",
                    &err,
                );
                self.sync.syncing = false;
                self.sync.last_error = Some(err.clone());
                self.status = format!("User Data Sync failed: {err}");
            }
            SyncUiEvent::ResetFinished(Ok(())) => {
                self.sync.syncing = false;
                self.status = "Reset synced user data".into();
            }
            SyncUiEvent::ResetFinished(Err(err)) => {
                crate::app::error_log::record(
                    crate::app::error_log::ErrorSeverity::Medium,
                    "user-data-sync",
                    &err,
                );
                self.sync.syncing = false;
                self.sync.last_error = Some(err.clone());
                self.status = format!("Reset remote sync failed: {err}");
            }
        }
    }

    pub(crate) fn apply_sync_outcome(&mut self, outcome: SyncOutcome) {
        self.sync.persisted.remote_id = Some(outcome.remote_id);
        self.sync.persisted.last_sync_epoch_s = Some(now_epoch_s());
        self.sync.persisted.conflicts = outcome.conflicts;
        if self.sync.persisted.conflicts.is_empty() {
            self.sync.persisted.last_base = Some(outcome.merged_snapshot);
            self.sync.last_summary = Some(format!(
                "Synced (pulled={}, pushed={})",
                outcome.pulled_remote, outcome.pushed_remote
            ));
            self.status = "User Data Sync completed".into();
        } else {
            let count = self.sync.persisted.conflicts.len();
            self.sync.persisted.last_base = Some(outcome.merged_snapshot);
            self.sync.last_summary = Some(format!("{count} merge conflict(s) pending"));
            self.status =
                format!("User Data Sync paused with {count} conflict(s); run Accept Merges");
        }
        let _ = self.sync.save(&self.profile_root);
    }

    /// Synchronously resolves all pending sync conflicts using `choice` and writes merged files.
    pub(crate) fn accept_sync_merges(&mut self, choice: ConflictChoice) -> usize {
        if self.sync.persisted.conflicts.is_empty() {
            self.status = "No pending sync conflicts to accept".into();
            return 0;
        }
        let mut base = self.sync.persisted.last_base.clone().unwrap_or_else(|| {
            capture_snapshot(&self.profile_root, &self.sync.persisted.resources)
        });
        let resolved_count = self.sync.persisted.conflicts.len();
        apply_conflict_resolutions(&mut base, &mut self.sync.persisted.conflicts, choice);
        let _ = write_local_backup(&self.profile_root, &base, "merge-resolved");
        if let Err(err) =
            apply_snapshot_to_profile(&self.profile_root, &base, &self.sync.persisted.resources)
        {
            self.status = format!("Failed to apply merged data: {err}");
            return 0;
        }
        self.sync.persisted.conflicts.clear();
        self.sync.persisted.last_base = Some(base);
        self.sync.persisted.last_sync_epoch_s = Some(now_epoch_s());
        let _ = self.sync.save(&self.profile_root);
        self.status = format!("Accepted {resolved_count} sync merge resolution(s)");
        resolved_count
    }

    pub(crate) fn reset_sync_data(&mut self) {
        let remote_id = self.sync.persisted.remote_id.clone();
        let backend = self.sync.persisted.backend;
        let token = self.github_session.as_ref().map(|s| s.token.clone());
        self.sync.persisted = PersistedSyncData::default();
        self.sync.last_error = None;
        self.sync.last_summary = Some("Reset local sync metadata".into());
        let _ = self.sync.save(&self.profile_root);

        if let (Some(remote_id), Some(token), Some(proxy)) =
            (remote_id, token, self.event_proxy.clone())
        {
            self.sync.syncing = true;
            std::thread::spawn(move || {
                let result = make_transport(backend)
                    .and_then(|transport| transport.delete_snapshot(&token, &remote_id));
                let _ = proxy.send_event(UserEvent::Sync(SyncUiEvent::ResetFinished(result)));
            });
            self.status = "Resetting remote User Data Sync gist...".into();
        } else {
            self.status = "Reset local User Data Sync state".into();
        }
    }

    pub(crate) fn open_sync_backups_folder(&mut self) {
        let dir = self.profile_root.join(state::SYNC_BACKUPS_DIR);
        let _ = std::fs::create_dir_all(&dir);
        #[cfg(target_os = "windows")]
        if self.event_proxy.is_some() {
            let _ = std::process::Command::new("explorer").arg(&dir).spawn();
        }
        #[cfg(target_os = "linux")]
        if self.event_proxy.is_some() {
            let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
        }
        self.status = format!("Sync backups folder: {}", dir.display());
    }

    pub(crate) fn show_synced_data(&mut self) {
        let path = self.profile_root.join(state::SYNC_STATE_FILE);
        if !path.exists() {
            let _ = save_persisted_sync(&self.profile_root, &self.sync.persisted);
        }
        self.open_file(path.clone());
        self.status = format!("Opened synced data view: {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeTransport {
        remote: Mutex<Option<transport::RemoteSyncBundle>>,
    }

    impl FakeTransport {
        fn with_remote(remote: Option<transport::RemoteSyncBundle>) -> Self {
            Self {
                remote: Mutex::new(remote),
            }
        }
    }

    impl SyncTransport for FakeTransport {
        fn fetch_snapshot(
            &self,
            _token: &str,
            _remote_id: Option<&str>,
        ) -> Result<Option<transport::RemoteSyncBundle>, String> {
            Ok(self.remote.lock().unwrap().clone())
        }

        fn push_snapshot(
            &self,
            _token: &str,
            remote_id: Option<&str>,
            snapshot: &SyncSnapshot,
        ) -> Result<String, String> {
            let id = remote_id.unwrap_or("gist-created").to_owned();
            *self.remote.lock().unwrap() = Some(transport::RemoteSyncBundle {
                remote_id: id.clone(),
                snapshot: snapshot.clone(),
            });
            Ok(id)
        }

        fn delete_snapshot(&self, _token: &str, _remote_id: &str) -> Result<(), String> {
            *self.remote.lock().unwrap() = None;
            Ok(())
        }
    }

    #[test]
    fn sync_round_pushes_merges_backs_up_and_resolves_conflicts() {
        let dir = crate::test_support::TempDir::new();
        let profile = dir.path().join("profile");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(
            profile.join("settings.json"),
            r#"{"editor.fontSize":16,"files.autoSave":"afterDelay"}"#,
        )
        .unwrap();

        let fake = FakeTransport::with_remote(None);
        let persisted = PersistedSyncData {
            enabled: true,
            ..Default::default()
        };

        // First push when no remote gist exists.
        let outcome1 = execute_sync_round(&profile, "tok", &persisted, &fake).unwrap();
        assert_eq!(outcome1.remote_id, "gist-created");
        assert!(outcome1.conflicts.is_empty());
        assert!(outcome1.pushed_remote);

        // Simulate concurrent remote edit to fontSize -> produces conflict while keeping disjoint key.
        {
            let mut guard = fake.remote.lock().unwrap();
            guard.as_mut().unwrap().snapshot.settings_json =
                Some(r#"{"editor.fontSize":12,"window.zoomLevel":1}"#.into());
        }
        let persisted2 = PersistedSyncData {
            enabled: true,
            remote_id: Some(outcome1.remote_id.clone()),
            last_base: Some(SyncSnapshot {
                settings_json: Some(r#"{"editor.fontSize":14}"#.into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let outcome2 = execute_sync_round(&profile, "tok", &persisted2, &fake).unwrap();
        assert_eq!(outcome2.conflicts.len(), 1);
        assert!(profile.join(state::SYNC_BACKUPS_DIR).exists());

        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = profile.clone();
        app.apply_sync_outcome(outcome2);
        assert_eq!(app.sync.persisted.conflicts.len(), 1);

        let accepted = app.accept_sync_merges(ConflictChoice::AcceptRemote);
        assert_eq!(accepted, 1);
        assert!(app.sync.persisted.conflicts.is_empty());
        let saved_settings = std::fs::read_to_string(profile.join("settings.json")).unwrap();
        assert!(saved_settings.contains("12"));
        assert!(saved_settings.contains("afterDelay"));

        app.reset_sync_data();
        assert!(!app.sync.persisted.enabled);
        assert!(app.sync.persisted.remote_id.is_none());
    }

    #[test]
    fn scopes_are_checked_and_merged_without_losing_the_old_ones() {
        assert!(has_scope("read:user gist", "gist"));
        assert!(!has_scope("read:user", "gist"));
        assert!(!has_scope("read:user gistx", "gist"));
        assert_eq!(
            merge_scopes("read:user repo", "read:user gist"),
            "read:user repo gist"
        );
        assert_eq!(merge_scopes("", "read:user gist"), "read:user gist");
    }

    #[test]
    fn the_age_of_the_last_sync_is_given_in_the_largest_whole_unit() {
        assert_eq!(relative_age(5), ("sync_menu.just_now", 0));
        assert_eq!(relative_age(60), ("sync_menu.minutes_ago", 1));
        assert_eq!(relative_age(3599), ("sync_menu.minutes_ago", 59));
        assert_eq!(relative_age(7200), ("sync_menu.hours_ago", 2));
        assert_eq!(relative_age(3 * 86_400), ("sync_menu.days_ago", 3));
    }

    #[test]
    fn the_menu_status_line_reports_syncing_failure_conflicts_or_age() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert_eq!(app.sync_progress_text(), app.t("sync_menu.never"));
        app.sync.persisted.last_sync_epoch_s = Some(now_epoch_s());
        assert_eq!(app.sync_progress_text(), app.t("sync_menu.just_now"));
        app.sync.persisted.conflicts = vec![SyncConflict {
            resource: "settings".into(),
            key: "a".into(),
            local_value: None,
            remote_value: None,
            resolved: None,
        }];
        assert!(app.sync_progress_text().contains('1'));
        app.sync.last_error = Some("boom".into());
        assert_eq!(app.sync_progress_text(), app.t("sync_menu.failed"));
        app.sync.syncing = true;
        assert_eq!(app.sync_progress_text(), app.t("sync_menu.syncing"));
    }

    #[test]
    fn the_status_line_names_the_backend() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(app
            .sync_status_text()
            .starts_with(app.t("sync_menu.backend_gist")));
        app.sync.persisted.backend = SyncBackend::Repository;
        assert!(app
            .sync_status_text()
            .starts_with(app.t("sync_menu.backend_repository")));
    }

    #[test]
    fn switching_backend_forgets_the_remote_and_the_merge_base_of_the_old_one() {
        let mut data = PersistedSyncData {
            remote_id: Some("abc123".into()),
            last_base: Some(SyncSnapshot::default()),
            conflicts: vec![SyncConflict {
                resource: "settings".into(),
                key: "k".into(),
                local_value: None,
                remote_value: None,
                resolved: None,
            }],
            ..Default::default()
        };
        data.use_backend(SyncBackend::Gist);
        assert_eq!(
            data.remote_id.as_deref(),
            Some("abc123"),
            "same backend keeps its state"
        );
        data.use_backend(SyncBackend::Repository);
        assert_eq!(data.backend, SyncBackend::Repository);
        assert!(data.remote_id.is_none() && data.last_base.is_none() && data.conflicts.is_empty());
    }

    #[test]
    fn a_state_file_from_before_backends_loads_as_a_gist() {
        let old = r#"{"enabled": true, "gist_id": "abc123"}"#;
        let data: PersistedSyncData = serde_json::from_str(old).unwrap();
        assert!(data.enabled);
        assert_eq!(data.backend, SyncBackend::Gist);
        assert_eq!(data.remote_id.as_deref(), Some("abc123"));
    }

    #[test]
    fn each_backend_asks_for_its_own_permission() {
        assert_eq!(SyncBackend::Gist.required_scope(), "gist");
        assert!(has_scope(SyncBackend::Gist.sign_in_scope(), "gist"));
        assert_eq!(SyncBackend::Repository.required_scope(), "repo");
        assert!(has_scope(SyncBackend::Repository.sign_in_scope(), "repo"));
        assert!(!has_scope(SyncBackend::Repository.sign_in_scope(), "gist"));
    }
}
