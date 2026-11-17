//! Application update manager backed by GitHub Releases.

pub(crate) mod apply;
pub(crate) mod state;
pub(crate) mod transport;

use crate::{App, UserEvent};
use state::{verify_sha256, ReleaseInfo, UpdateChannel, UpdateConfig, UpdateStatus};
use std::path::PathBuf;
use transport::{GitHubReleasesTransport, UpdateTransport};

#[derive(Debug, Clone)]
pub(crate) enum UpdateUiEvent {
    CheckCompleted(Result<Option<ReleaseInfo>, String>),
    BackgroundCheckCompleted(Result<Option<ReleaseInfo>, String>),
    DownloadCompleted {
        release: ReleaseInfo,
        result: Result<Vec<u8>, String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct UpdateState {
    pub config: UpdateConfig,
    pub current_version: String,
    pub status: UpdateStatus,
    pub latest_release: Option<ReleaseInfo>,
    pub staged_update_path: Option<PathBuf>,
    pub cancelled: bool,
}

impl UpdateState {
    pub(crate) fn isolated() -> Self {
        Self {
            config: UpdateConfig::default(),
            current_version: env!("CARGO_PKG_VERSION").to_owned(),
            status: UpdateStatus::Idle,
            latest_release: None,
            staged_update_path: None,
            cancelled: false,
        }
    }

    pub(crate) fn load(profile_root: &std::path::Path) -> Self {
        Self {
            config: UpdateConfig::load_or_default(profile_root),
            current_version: env!("CARGO_PKG_VERSION").to_owned(),
            status: UpdateStatus::Idle,
            latest_release: None,
            staged_update_path: None,
            cancelled: false,
        }
    }
}

impl App {
    pub(crate) fn check_for_updates_with(&mut self, transport: &dyn UpdateTransport) {
        self.updates.cancelled = false;
        self.updates.status = UpdateStatus::Checking;
        self.status = format!(
            "Checking {} updates for {}...",
            self.updates.config.channel.label(),
            self.updates.config.repo
        );
        let outcome = transport
            .fetch_latest_release(&self.updates.config.repo, self.updates.config.channel)
            .map(|release| {
                if release.is_newer_than(&self.updates.current_version) {
                    Some(release)
                } else {
                    None
                }
            });
        self.handle_update_event(UpdateUiEvent::CheckCompleted(outcome));
    }

    pub(crate) fn check_for_updates(&mut self) {
        self.updates.cancelled = false;
        self.updates.status = UpdateStatus::Checking;
        self.status = "Checking for updates...".into();
        let Some(proxy) = self.event_proxy.clone() else {
            self.updates.status = UpdateStatus::UpToDate {
                checked_version: self.updates.current_version.clone(),
            };
            self.status = format!("Graphite v{} is up to date", self.updates.current_version);
            return;
        };
        let repo = self.updates.config.repo.clone();
        let channel = self.updates.config.channel;
        let current = self.updates.current_version.clone();
        std::thread::spawn(move || {
            let outcome = GitHubReleasesTransport::new().and_then(|transport| {
                transport.fetch_latest_release(&repo, channel).map(|rel| {
                    if rel.is_newer_than(&current) {
                        Some(rel)
                    } else {
                        None
                    }
                })
            });
            let _ = proxy.send_event(UserEvent::Update(UpdateUiEvent::CheckCompleted(outcome)));
        });
    }

    /// Starts the background GitHub Releases poller once per process: checks shortly after
    /// startup and then every 10 minutes without disturbing the status bar when up to date.
    pub(crate) fn start_background_update_poller(&self) {
        use std::sync::atomic::{AtomicBool, Ordering};
        static POLLER_STARTED: AtomicBool = AtomicBool::new(false);

        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        if POLLER_STARTED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let repo = self.updates.config.repo.clone();
        let channel = self.updates.config.channel;
        let current = self.updates.current_version.clone();
        let _ = std::thread::Builder::new()
            .name("graphite-update-poller".into())
            .spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                loop {
                    let outcome = GitHubReleasesTransport::new().and_then(|transport| {
                        transport.fetch_latest_release(&repo, channel).map(|rel| {
                            if rel.is_newer_than(&current) {
                                Some(rel)
                            } else {
                                None
                            }
                        })
                    });
                    if proxy
                        .send_event(UserEvent::Update(UpdateUiEvent::BackgroundCheckCompleted(
                            outcome,
                        )))
                        .is_err()
                    {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(600));
                }
            });
    }

    pub(crate) fn download_update_with(&mut self, transport: &dyn UpdateTransport) {
        let Some(release) = self.updates.latest_release.clone() else {
            self.status = "No available update to download".into();
            return;
        };
        let Some(asset) = release.asset.clone() else {
            self.updates.status = UpdateStatus::Error("Release has no downloadable asset".into());
            self.status = "Release has no downloadable asset".into();
            return;
        };
        self.updates.cancelled = false;
        self.updates.status = UpdateStatus::Downloading {
            tag: release.tag_name.clone(),
            progress_pct: 10,
        };
        let result = transport.download_asset(&asset.download_url);
        self.handle_update_event(UpdateUiEvent::DownloadCompleted { release, result });
    }

    pub(crate) fn download_update(&mut self) {
        let Some(release) = self.updates.latest_release.clone() else {
            self.status = "No available update to download".into();
            return;
        };
        let Some(asset) = release.asset.clone() else {
            self.updates.status = UpdateStatus::Error("Release has no downloadable asset".into());
            self.status = "Release has no downloadable asset".into();
            return;
        };
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        self.updates.cancelled = false;
        self.updates.status = UpdateStatus::Downloading {
            tag: release.tag_name.clone(),
            progress_pct: 5,
        };
        self.status = format!("Downloading update {}...", release.tag_name);
        std::thread::spawn(move || {
            let result = GitHubReleasesTransport::new()
                .and_then(|transport| transport.download_asset(&asset.download_url));
            let _ = proxy.send_event(UserEvent::Update(UpdateUiEvent::DownloadCompleted {
                release,
                result,
            }));
        });
    }

    pub(crate) fn cancel_update(&mut self) {
        self.updates.cancelled = true;
        self.updates.status = UpdateStatus::Cancelling;
        self.status = "Cancelling update operation".into();
        self.updates.status = UpdateStatus::Idle;
    }

    pub(crate) fn install_update(&mut self) {
        match &self.updates.status {
            UpdateStatus::ReadyToRestart {
                release,
                staged_path,
            } => {
                self.status = format!(
                    "Update {} verified and staged at {}",
                    release.tag_name,
                    staged_path.display()
                );
            }
            UpdateStatus::Available(_) => {
                self.download_update();
            }
            _ => {
                self.status = format!(
                    "No downloaded update staged (current v{})",
                    self.updates.current_version
                );
            }
        }
    }

    pub(crate) fn restart_to_update(&mut self) {
        let Some(staged_path) = self.updates.staged_update_path.clone() else {
            self.status = "No staged update ready for restart".into();
            return;
        };
        let (tag, version, notes, html_url) = match self.updates.latest_release.as_ref() {
            Some(r) => (
                r.tag_name.clone(),
                r.version.clone(),
                r.notes.clone(),
                r.html_url.clone(),
            ),
            None => (
                "staged".into(),
                self.updates.current_version.clone(),
                String::new(),
                String::new(),
            ),
        };
        let marker_dir = self.profile_root.join("updates");
        let _ = std::fs::create_dir_all(&marker_dir);
        let marker = serde_json::json!({
            "tag": tag,
            "version": version,
            "notes": notes,
            "html_url": html_url,
            "staged_path": staged_path,
        });
        let _ = std::fs::write(
            marker_dir.join("pending-install.json"),
            serde_json::to_string_pretty(&marker).unwrap_or_default(),
        );
        if self.event_proxy.is_some() {
            if let Ok(exe) = std::env::current_exe() {
                match apply::apply_pending_update_in(&self.profile_root, &exe) {
                    Ok(Some(_)) => {
                        let workspace = self.tree.root().to_path_buf();
                        if std::process::Command::new(&exe)
                            .arg(workspace)
                            .spawn()
                            .is_ok()
                        {
                            std::process::exit(0);
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        crate::app::error_log::record(
                            crate::app::error_log::ErrorSeverity::High,
                            "updates.apply",
                            &err,
                        );
                        self.updates.status = UpdateStatus::Error(err.clone());
                        self.status = err;
                        return;
                    }
                }
            }
        }
        self.status = format!("Update {tag} scheduled to apply on next restart");
    }

    pub(crate) fn open_pending_update_changelog(&mut self) {
        let marker_path = self.profile_root.join("updates").join("just-updated.json");
        let Ok(raw) = std::fs::read_to_string(&marker_path) else {
            return;
        };
        let _ = std::fs::remove_file(&marker_path);
        let Ok(marker) = serde_json::from_str::<apply::JustUpdatedMarker>(&raw) else {
            return;
        };
        let version = if marker.version.trim().is_empty() {
            marker.tag.trim_start_matches('v').to_owned()
        } else {
            marker.version.clone()
        };
        let body = self.resolve_post_update_changelog_body(&version, &marker);
        let heading = self.tf("updates.changelog.heading", &[("tag", &marker.tag)]);
        let mut markdown = format!("# {heading}\n\n{body}\n");
        if !marker.html_url.trim().is_empty() {
            let link_label = self.t("updates.changelog.release_url");
            markdown.push_str(&format!("\n---\n\n{link_label}: {}\n", marker.html_url));
        }
        let tab_path = PathBuf::from(format!("CHANGELOG-{version}.md"));
        self.open_read_only_view(tab_path, &markdown);
        self.status = self.tf("updates.changelog.status", &[("tag", &marker.tag)]);
    }

    fn resolve_post_update_changelog_body(
        &self,
        version: &str,
        marker: &apply::JustUpdatedMarker,
    ) -> String {
        let lang = self.translator.language().code();
        let mut roots = vec![self.tree.root().to_path_buf()];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                roots.push(parent.to_path_buf());
            }
        }
        for root in roots {
            for candidate_lang in [lang, "en"] {
                let candidate = root
                    .join("changelog")
                    .join(version)
                    .join(format!("{candidate_lang}.md"));
                if let Ok(text) = std::fs::read_to_string(&candidate) {
                    if !text.trim().is_empty() {
                        return text.trim().to_owned();
                    }
                }
            }
        }
        if !marker.notes.trim().is_empty() {
            marker.notes.trim().to_owned()
        } else {
            self.tf("updates.changelog.empty_notes", &[("tag", &marker.tag)])
        }
    }

    pub(crate) fn show_update_info(&mut self) {
        self.new_untitled_document();
        let status_line = match &self.updates.status {
            UpdateStatus::Idle => "Idle".to_owned(),
            UpdateStatus::Checking => "Checking...".to_owned(),
            UpdateStatus::Available(r) => format!("Available: {}", r.tag_name),
            UpdateStatus::Downloading { tag, progress_pct } => {
                format!("Downloading {tag} ({progress_pct}%)")
            }
            UpdateStatus::ReadyToRestart {
                release,
                staged_path,
            } => format!(
                "Ready to restart into {} (staged at `{}`)",
                release.tag_name,
                staged_path.display()
            ),
            UpdateStatus::Cancelling => "Cancelling".to_owned(),
            UpdateStatus::UpToDate { checked_version } => {
                format!("Up to date (v{checked_version})")
            }
            UpdateStatus::Error(err) => format!("Error: {err}"),
        };
        let text = format!(
            "# Graphite Update Info\n\n- Current version: `v{}`\n- Channel: `{}`\n- GitHub repository: `{}`\n- Status: {}\n",
            self.updates.current_version,
            self.updates.config.channel.label(),
            self.updates.config.repo,
            status_line
        );
        self.editor_mut().insert(&text);
        self.status = status_line;
    }

    pub(crate) fn show_update_release_notes(&mut self) {
        self.new_untitled_document();
        let text = if let Some(rel) = &self.updates.latest_release {
            format!(
                "# Release Notes — {}\n\n{}\n\nURL: {}\n",
                rel.tag_name,
                if rel.notes.trim().is_empty() {
                    "No release notes provided."
                } else {
                    rel.notes.as_str()
                },
                rel.html_url
            )
        } else {
            format!(
                "# Release Notes — Graphite v{}\n\nRunning on channel `{}`.\n",
                self.updates.current_version,
                self.updates.config.channel.label()
            )
        };
        self.editor_mut().insert(&text);
        self.status = "Opened release notes".into();
    }

    pub(crate) fn set_update_channel(&mut self, channel: UpdateChannel) {
        self.updates.config.channel = channel;
        self.updates.config.save(&self.profile_root);
        self.status = format!("Switched update channel to {}", channel.label());
    }

    pub(crate) fn handle_update_event(&mut self, event: UpdateUiEvent) {
        if self.updates.cancelled {
            self.updates.status = UpdateStatus::Idle;
            self.status = "Update cancelled".into();
            return;
        }
        match event {
            UpdateUiEvent::CheckCompleted(Ok(Some(release))) => {
                self.status = format!("Update available: {}", release.tag_name);
                self.updates.latest_release = Some(release.clone());
                self.updates.status = UpdateStatus::Available(release);
            }
            UpdateUiEvent::CheckCompleted(Ok(None)) => {
                self.updates.latest_release = None;
                self.updates.status = UpdateStatus::UpToDate {
                    checked_version: self.updates.current_version.clone(),
                };
                self.status = format!("Graphite v{} is up to date", self.updates.current_version);
            }
            UpdateUiEvent::CheckCompleted(Err(err)) => {
                crate::app::error_log::record(
                    crate::app::error_log::ErrorSeverity::Medium,
                    "updates.check",
                    &err,
                );
                self.updates.status = UpdateStatus::Error(err.clone());
                self.status = format!("Update check failed: {err}");
            }
            UpdateUiEvent::BackgroundCheckCompleted(Ok(Some(release))) => {
                if !matches!(
                    self.updates.status,
                    UpdateStatus::Downloading { .. } | UpdateStatus::ReadyToRestart { .. }
                ) {
                    self.updates.latest_release = Some(release.clone());
                    self.updates.status = UpdateStatus::Available(release);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                }
            }
            UpdateUiEvent::BackgroundCheckCompleted(Ok(None)) => {
                if matches!(
                    self.updates.status,
                    UpdateStatus::Idle | UpdateStatus::Checking | UpdateStatus::UpToDate { .. }
                ) {
                    self.updates.latest_release = None;
                    self.updates.status = UpdateStatus::UpToDate {
                        checked_version: self.updates.current_version.clone(),
                    };
                }
            }
            UpdateUiEvent::BackgroundCheckCompleted(Err(err)) => {
                crate::app::error_log::record(
                    crate::app::error_log::ErrorSeverity::Low,
                    "updates.background_check",
                    err,
                );
            }
            UpdateUiEvent::DownloadCompleted { release, result } => match result {
                Ok(bytes) => {
                    if bytes.is_empty() {
                        let msg = "Downloaded update package is empty".to_owned();
                        crate::app::error_log::record(
                            crate::app::error_log::ErrorSeverity::High,
                            "updates.download",
                            &msg,
                        );
                        self.updates.status = UpdateStatus::Error(msg.clone());
                        self.status = msg;
                        return;
                    }
                    if let Some(expected_sha) =
                        release.asset.as_ref().and_then(|a| a.sha256.as_ref())
                    {
                        if let Err(err) = verify_sha256(&bytes, expected_sha) {
                            crate::app::error_log::record(
                                crate::app::error_log::ErrorSeverity::High,
                                "updates.verify",
                                &err,
                            );
                            self.updates.status = UpdateStatus::Error(err.clone());
                            self.status = err;
                            return;
                        }
                    }
                    let asset_name = release
                        .asset
                        .as_ref()
                        .map(|a| a.name.as_str())
                        .unwrap_or("update-package.bin")
                        .replace(['/', '\\', ':'], "_");
                    let stage_dir = self
                        .profile_root
                        .join("updates")
                        .join(release.tag_name.replace(['/', '\\', ':'], "_"));
                    if let Err(err) = std::fs::create_dir_all(&stage_dir) {
                        let msg = format!("Failed creating staging directory: {err}");
                        crate::app::error_log::record(
                            crate::app::error_log::ErrorSeverity::High,
                            "updates.stage",
                            &msg,
                        );
                        self.updates.status = UpdateStatus::Error(msg.clone());
                        self.status = msg;
                        return;
                    }
                    let staged_path = stage_dir.join(asset_name);
                    if let Err(err) = std::fs::write(&staged_path, &bytes) {
                        let msg = format!("Failed staging update artifact: {err}");
                        crate::app::error_log::record(
                            crate::app::error_log::ErrorSeverity::High,
                            "updates.stage",
                            &msg,
                        );
                        self.updates.status = UpdateStatus::Error(msg.clone());
                        self.status = msg;
                        return;
                    }
                    self.updates.staged_update_path = Some(staged_path.clone());
                    self.updates.latest_release = Some(release.clone());
                    self.status = format!(
                        "Update {} downloaded and verified; restart to apply",
                        release.tag_name
                    );
                    self.updates.status = UpdateStatus::ReadyToRestart {
                        release,
                        staged_path,
                    };
                }
                Err(err) => {
                    crate::app::error_log::record(
                        crate::app::error_log::ErrorSeverity::High,
                        "updates.download",
                        &err,
                    );
                    self.updates.status = UpdateStatus::Error(err.clone());
                    self.status = format!("Update download failed: {err}");
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::state::{sha256_hex, ReleaseAsset};
    use super::*;

    struct MockUpdateTransport {
        release: ReleaseInfo,
        bytes: Vec<u8>,
    }

    impl UpdateTransport for MockUpdateTransport {
        fn fetch_latest_release(
            &self,
            _repo: &str,
            _channel: UpdateChannel,
        ) -> Result<ReleaseInfo, String> {
            Ok(self.release.clone())
        }

        fn download_asset(&self, _url: &str) -> Result<Vec<u8>, String> {
            Ok(self.bytes.clone())
        }
    }

    #[test]
    fn mock_update_check_download_sha256_and_tamper_rejection_work() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        let good_bytes = b"verified-release-archive".to_vec();
        let good_sha = sha256_hex(&good_bytes);

        let transport = MockUpdateTransport {
            release: ReleaseInfo {
                tag_name: "v9.9.9".into(),
                version: "9.9.9".into(),
                prerelease: false,
                notes: "Bug fixes and improvements".into(),
                html_url: "https://github.com/BadTonho/Graphite/releases/tag/v9.9.9".into(),
                asset: Some(ReleaseAsset {
                    name: "graphite-9.9.9.zip".into(),
                    download_url: "https://example.invalid/graphite-9.9.9.zip".into(),
                    sha256: Some(good_sha.clone()),
                }),
            },
            bytes: good_bytes,
        };

        app.check_for_updates_with(&transport);
        assert!(matches!(app.updates.status, UpdateStatus::Available(_)));

        app.download_update_with(&transport);
        assert!(matches!(
            app.updates.status,
            UpdateStatus::ReadyToRestart { .. }
        ));
        assert!(app
            .updates
            .staged_update_path
            .as_ref()
            .is_some_and(|p| p.exists()));

        app.restart_to_update();
        assert!(app
            .profile_root
            .join("updates")
            .join("pending-install.json")
            .exists());

        // Tampered payload must be rejected cleanly
        let tampered = MockUpdateTransport {
            release: transport.release,
            bytes: b"corrupted-archive".to_vec(),
        };
        app.check_for_updates_with(&tampered);
        app.download_update_with(&tampered);
        assert!(matches!(app.updates.status, UpdateStatus::Error(_)));
        assert!(app.status.contains("SHA-256 verification failed"));
    }

    #[test]
    fn background_check_is_silent_on_errors_and_surfaces_available_release() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.status = "Ready".into();

        app.handle_update_event(UpdateUiEvent::BackgroundCheckCompleted(Err(
            "offline".into()
        )));
        assert_eq!(app.status, "Ready");
        assert!(matches!(app.updates.status, UpdateStatus::Idle));

        let release = ReleaseInfo {
            tag_name: "v0.2.0".into(),
            version: "0.2.0".into(),
            prerelease: false,
            notes: String::new(),
            html_url: "https://github.com/BadTonho/Graphite/releases/tag/v0.2.0".into(),
            asset: None,
        };
        app.handle_update_event(UpdateUiEvent::BackgroundCheckCompleted(Ok(Some(
            release.clone(),
        ))));
        assert!(matches!(app.updates.status, UpdateStatus::Available(_)));
        assert_eq!(
            app.updates
                .latest_release
                .as_ref()
                .map(|r| r.tag_name.as_str()),
            Some("v0.2.0")
        );
    }

    #[test]
    fn post_update_changelog_opens_once_on_first_launch_and_prefers_local_file() {
        let dir = crate::test_support::TempDir::new();
        let updates_dir = dir.path().join("updates");
        std::fs::create_dir_all(&updates_dir).unwrap();
        let marker_path = updates_dir.join("just-updated.json");
        std::fs::write(
            &marker_path,
            serde_json::json!({
                "tag": "v0.2.0",
                "version": "0.2.0",
                "notes": "Remote release notes",
                "html_url": "https://github.com/BadTonho/Graphite/releases/tag/v0.2.0",
            })
            .to_string(),
        )
        .unwrap();

        let local_changelog = dir.path().join("changelog").join("0.2.0");
        std::fs::create_dir_all(&local_changelog).unwrap();
        std::fs::write(
            local_changelog.join("en.md"),
            "## New features\n- Local changelog entry",
        )
        .unwrap();

        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().to_path_buf();
        let initial_tabs = app.documents.len();

        app.open_pending_update_changelog();
        assert!(!marker_path.exists());
        assert_eq!(app.documents.len(), initial_tabs + 1);
        let text = app.editor().text();
        assert!(text.contains("Local changelog entry"));
        assert!(text.contains("v0.2.0"));

        // Second launch must not re-open the changelog tab.
        app.open_pending_update_changelog();
        assert_eq!(app.documents.len(), initial_tabs + 1);
    }
}
