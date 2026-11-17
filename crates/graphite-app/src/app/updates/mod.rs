//! Application update manager backed by GitHub Releases.
//!
//! Security model: a package is installed only when the release ships `<asset>.sig`, an Ed25519
//! signature (by a key in [`signature::TRUSTED_UPDATE_KEYS`]) over the release tag, asset name
//! and SHA-256, **and** the downloaded bytes hash to that SHA-256. Anything else is rejected
//! before it is staged.

pub(crate) mod apply;
pub(crate) mod signature;
pub(crate) mod state;
pub(crate) mod transport;

use crate::{App, UserEvent};
use state::{
    extract_asset_sha256_from_notes, ReleaseInfo, UpdateChannel, UpdateConfig, UpdateStatus,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use transport::{GitHubReleasesTransport, UpdateTransport, DOWNLOAD_CANCELLED};

/// A package that was downloaded, hash-checked and signature-checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedPackage {
    pub staged_path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) enum UpdateUiEvent {
    CheckCompleted {
        generation: u64,
        result: Result<Option<ReleaseInfo>, String>,
    },
    BackgroundCheckCompleted(Result<Option<ReleaseInfo>, String>),
    DownloadProgress {
        generation: u64,
        tag: String,
        progress_pct: u8,
    },
    DownloadCompleted {
        generation: u64,
        release: ReleaseInfo,
        result: Result<VerifiedPackage, String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct UpdateState {
    pub config: UpdateConfig,
    pub current_version: String,
    pub status: UpdateStatus,
    pub latest_release: Option<ReleaseInfo>,
    pub staged_update_path: Option<PathBuf>,
    /// SHA-256 of the staged package, re-checked by the installer.
    pub staged_sha256: Option<String>,
    /// Bumped by every cancellation; events from an older generation are stale and ignored.
    pub generation: u64,
    /// Shared with in-flight worker threads of the current generation.
    pub cancel_flag: Arc<AtomicBool>,
    pub trusted_keys: Vec<[u8; 32]>,
}

impl UpdateState {
    fn with_config(config: UpdateConfig) -> Self {
        Self {
            config,
            current_version: env!("CARGO_PKG_VERSION").to_owned(),
            status: UpdateStatus::Idle,
            latest_release: None,
            staged_update_path: None,
            staged_sha256: None,
            generation: 0,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            trusted_keys: signature::TRUSTED_UPDATE_KEYS.to_vec(),
        }
    }

    pub(crate) fn isolated() -> Self {
        Self::with_config(UpdateConfig::default())
    }

    pub(crate) fn load(profile_root: &std::path::Path) -> Self {
        Self::with_config(UpdateConfig::load_or_default(profile_root))
    }
}

/// Settings read by the background poller on every cycle, so changing the channel or turning
/// auto-check off takes effect without restarting.
#[derive(Clone)]
struct PollerConfig {
    repo: String,
    channel: UpdateChannel,
    current_version: String,
    auto_check: bool,
}

static POLLER_CONFIG: Mutex<Option<PollerConfig>> = Mutex::new(None);

const POLL_INTERVAL_SECS: u64 = 600;
const MAX_POLL_BACKOFF_SECS: u64 = 3600;

/// Delay before the next background check: every 10 minutes while healthy, doubling per
/// consecutive failure (rate limiting, offline) up to one hour.
pub(crate) fn next_poll_delay(consecutive_failures: u32) -> Duration {
    let factor = 1u64 << consecutive_failures.min(10);
    Duration::from_secs((POLL_INTERVAL_SECS * factor).min(MAX_POLL_BACKOFF_SECS))
}

fn check_newer(
    transport: &dyn UpdateTransport,
    repo: &str,
    channel: UpdateChannel,
    current: &str,
) -> Result<Option<ReleaseInfo>, String> {
    transport
        .fetch_latest_release(repo, channel)
        .map(|release| release.is_newer_than(current).then_some(release))
}

/// Downloads, hash-checks and signature-checks the release package for this platform.
///
/// Order matters: the signature and the expected digest are validated *before* the (large)
/// package is downloaded, and the package only becomes visible under its final name after the
/// downloaded bytes match the signed digest.
pub(crate) fn fetch_verified_package(
    transport: &dyn UpdateTransport,
    release: &ReleaseInfo,
    trusted_keys: &[[u8; 32]],
    updates_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u8),
) -> Result<VerifiedPackage, String> {
    let asset = release
        .asset
        .as_ref()
        .ok_or_else(|| "Release has no downloadable asset".to_owned())?;
    let signature_url = asset.signature_url.as_deref().ok_or_else(|| {
        format!(
            "Release is not signed (missing {}.sig); refusing to install",
            asset.name
        )
    })?;

    let expected_sha = match (&asset.sha256, &asset.sha256_url) {
        (Some(sha), _) => sha.clone(),
        (None, Some(url)) => {
            let text = transport.download_text(url, 4096)?;
            extract_asset_sha256_from_notes(&text, &asset.name).ok_or_else(|| {
                format!(
                    "Checksum file for {} does not contain a valid SHA-256",
                    asset.name
                )
            })?
        }
        (None, None) => {
            return Err(format!(
                "Release does not declare a SHA-256 for {}; refusing to install",
                asset.name
            ))
        }
    }
    .to_ascii_lowercase();

    let signature_text =
        transport.download_text(signature_url, signature::MAX_SIGNATURE_FILE_BYTES)?;
    signature::verify_release_signature(
        trusted_keys,
        &signature::signed_message(&release.tag_name, &asset.name, &expected_sha),
        &signature_text,
    )?;

    let file_name = asset.name.replace(['/', '\\', ':'], "_");
    if file_name.is_empty() || file_name == "." || file_name == ".." {
        return Err("Release asset has an invalid file name".into());
    }
    let stage_dir = updates_dir.join(release.tag_name.replace(['/', '\\', ':'], "_"));
    std::fs::create_dir_all(&stage_dir)
        .map_err(|err| format!("Failed creating staging directory: {err}"))?;
    let part_path = stage_dir.join(format!("{file_name}.part"));
    let staged_path = stage_dir.join(&file_name);

    let mut last_pct = 0u8;
    let actual_sha =
        transport.download_to_file(&asset.download_url, &part_path, &mut |done, total| {
            if cancel.load(Ordering::SeqCst) {
                return false;
            }
            if let Some(total) = total.filter(|t| *t > 0) {
                let pct = ((done.min(total) * 100) / total).min(99) as u8;
                if pct != last_pct {
                    last_pct = pct;
                    on_progress(pct);
                }
            }
            true
        })?;

    if actual_sha != expected_sha {
        let _ = std::fs::remove_file(&part_path);
        return Err(format!(
            "SHA-256 verification failed: expected {expected_sha}, got {actual_sha}"
        ));
    }
    let _ = std::fs::remove_file(&staged_path);
    if let Err(err) = std::fs::rename(&part_path, &staged_path) {
        let _ = std::fs::remove_file(&part_path);
        return Err(format!("Failed staging update artifact: {err}"));
    }
    Ok(VerifiedPackage {
        staged_path,
        sha256: actual_sha,
    })
}

/// Picks the error-log source for a failed download/verification.
fn download_error_source(err: &str) -> &'static str {
    let lower = err.to_ascii_lowercase();
    if lower.contains("sha-256")
        || lower.contains("signature")
        || lower.contains("not signed")
        || lower.contains("checksum")
    {
        "updates.verify"
    } else if lower.contains("staging") {
        "updates.stage"
    } else {
        "updates.download"
    }
}

impl App {
    fn sync_poller_config(&self) {
        if let Ok(mut guard) = POLLER_CONFIG.lock() {
            *guard = Some(PollerConfig {
                repo: self.updates.config.repo.clone(),
                channel: self.updates.config.channel,
                current_version: self.updates.current_version.clone(),
                auto_check: self.updates.config.auto_check,
            });
        }
    }

    /// Marks a check as running unless a download/staged update must stay visible.
    fn begin_check(&mut self) {
        if !matches!(
            self.updates.status,
            UpdateStatus::Downloading { .. } | UpdateStatus::ReadyToRestart { .. }
        ) {
            self.updates.status = UpdateStatus::Checking;
        }
    }

    #[cfg(test)]
    pub(crate) fn check_for_updates_with(&mut self, transport: &dyn UpdateTransport) {
        self.begin_check();
        self.status = format!(
            "Checking {} updates for {}...",
            self.updates.config.channel.label(),
            self.updates.config.repo
        );
        let result = check_newer(
            transport,
            &self.updates.config.repo,
            self.updates.config.channel,
            &self.updates.current_version,
        );
        let generation = self.updates.generation;
        self.handle_update_event(UpdateUiEvent::CheckCompleted { generation, result });
    }

    pub(crate) fn check_for_updates(&mut self) {
        self.begin_check();
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
        let generation = self.updates.generation;
        std::thread::spawn(move || {
            let result = GitHubReleasesTransport::new()
                .and_then(|transport| check_newer(&transport, &repo, channel, &current));
            let _ = proxy.send_event(UserEvent::Update(UpdateUiEvent::CheckCompleted {
                generation,
                result,
            }));
        });
    }

    /// Starts the background GitHub Releases poller once per process: checks shortly after
    /// startup and then every 10 minutes (backing off on failures) without disturbing the
    /// status bar when up to date.
    pub(crate) fn start_background_update_poller(&self) {
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
        self.sync_poller_config();
        let _ = std::thread::Builder::new()
            .name("graphite-update-poller".into())
            .spawn(move || {
                std::thread::sleep(Duration::from_secs(2));
                let mut failures = 0u32;
                loop {
                    let config = POLLER_CONFIG.lock().ok().and_then(|g| g.clone());
                    if let Some(cfg) = config.filter(|c| c.auto_check) {
                        let outcome = GitHubReleasesTransport::new().and_then(|transport| {
                            check_newer(&transport, &cfg.repo, cfg.channel, &cfg.current_version)
                        });
                        failures = if outcome.is_ok() {
                            0
                        } else {
                            failures.saturating_add(1)
                        };
                        if proxy
                            .send_event(UserEvent::Update(UpdateUiEvent::BackgroundCheckCompleted(
                                outcome,
                            )))
                            .is_err()
                        {
                            break;
                        }
                    }
                    std::thread::sleep(next_poll_delay(failures));
                }
            });
    }

    #[cfg(test)]
    pub(crate) fn download_update_with(&mut self, transport: &dyn UpdateTransport) {
        let Some(release) = self.updates.latest_release.clone() else {
            self.status = "No available update to download".into();
            return;
        };
        if release.asset.is_none() {
            self.updates.status = UpdateStatus::Error("Release has no downloadable asset".into());
            self.status = "Release has no downloadable asset".into();
            return;
        }
        self.updates.status = UpdateStatus::Downloading {
            tag: release.tag_name.clone(),
            progress_pct: 0,
        };
        let result = fetch_verified_package(
            transport,
            &release,
            &self.updates.trusted_keys,
            &self.profile_root.join("updates"),
            &self.updates.cancel_flag,
            &mut |_| {},
        );
        let generation = self.updates.generation;
        self.handle_update_event(UpdateUiEvent::DownloadCompleted {
            generation,
            release,
            result,
        });
    }

    pub(crate) fn download_update(&mut self) {
        let Some(release) = self.updates.latest_release.clone() else {
            self.status = "No available update to download".into();
            return;
        };
        if release.asset.is_none() {
            self.updates.status = UpdateStatus::Error("Release has no downloadable asset".into());
            self.status = "Release has no downloadable asset".into();
            return;
        }
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        self.updates.status = UpdateStatus::Downloading {
            tag: release.tag_name.clone(),
            progress_pct: 0,
        };
        self.status = format!("Downloading update {}...", release.tag_name);
        let generation = self.updates.generation;
        let cancel = Arc::clone(&self.updates.cancel_flag);
        let trusted_keys = self.updates.trusted_keys.clone();
        let updates_dir = self.profile_root.join("updates");
        std::thread::spawn(move || {
            let tag = release.tag_name.clone();
            let progress_proxy = proxy.clone();
            let result = GitHubReleasesTransport::new().and_then(|transport| {
                fetch_verified_package(
                    &transport,
                    &release,
                    &trusted_keys,
                    &updates_dir,
                    &cancel,
                    &mut |progress_pct| {
                        let _ = progress_proxy.send_event(UserEvent::Update(
                            UpdateUiEvent::DownloadProgress {
                                generation,
                                tag: tag.clone(),
                                progress_pct,
                            },
                        ));
                    },
                )
            });
            let _ = proxy.send_event(UserEvent::Update(UpdateUiEvent::DownloadCompleted {
                generation,
                release,
                result,
            }));
        });
    }

    /// Aborts any in-flight check or download. Late results from the aborted work are ignored
    /// (stale generation); later checks, including the background poller, keep working.
    pub(crate) fn cancel_update(&mut self) {
        self.updates.cancel_flag.store(true, Ordering::SeqCst);
        self.updates.cancel_flag = Arc::new(AtomicBool::new(false));
        self.updates.generation += 1;
        self.updates.status = UpdateStatus::Idle;
        self.status = "Update operation cancelled".into();
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

    fn fail_update(
        &mut self,
        severity: crate::app::error_log::ErrorSeverity,
        source: &str,
        msg: String,
    ) {
        crate::app::error_log::record(severity, source, &msg);
        self.updates.status = UpdateStatus::Error(msg.clone());
        self.status = msg;
    }

    pub(crate) fn restart_to_update(&mut self) {
        use crate::app::error_log::ErrorSeverity;
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
        let marker = serde_json::json!({
            "tag": tag,
            "version": version,
            "notes": notes,
            "html_url": html_url,
            "staged_path": staged_path,
            "sha256": self.updates.staged_sha256,
        });
        let written = std::fs::create_dir_all(&marker_dir).and_then(|()| {
            std::fs::write(
                marker_dir.join("pending-install.json"),
                serde_json::to_string_pretty(&marker).map_err(std::io::Error::other)?,
            )
        });
        if let Err(err) = written {
            self.fail_update(
                ErrorSeverity::High,
                "updates.apply",
                format!("Could not schedule update {tag}: {err}"),
            );
            return;
        }
        if self.event_proxy.is_some() {
            let exe = match std::env::current_exe() {
                Ok(exe) => exe,
                Err(err) => {
                    self.fail_update(
                        ErrorSeverity::High,
                        "updates.apply",
                        format!("Could not locate the running executable: {err}"),
                    );
                    return;
                }
            };
            match apply::apply_pending_update_in(&self.profile_root, &exe) {
                Ok(Some(_)) => {
                    let workspace = self.tree.root().to_path_buf();
                    match std::process::Command::new(&exe).arg(workspace).spawn() {
                        Ok(_) => std::process::exit(0),
                        Err(err) => {
                            let outcome = match apply::rollback_update(&exe) {
                                Ok(()) => "the previous version was restored".to_owned(),
                                Err(rollback) => format!("rollback also failed: {rollback}"),
                            };
                            self.fail_update(
                                ErrorSeverity::High,
                                "updates.apply",
                                format!(
                                    "The updated Graphite could not be started ({err}); {outcome}"
                                ),
                            );
                        }
                    }
                    return;
                }
                Ok(None) => {
                    self.fail_update(
                        ErrorSeverity::High,
                        "updates.apply",
                        "The staged update could not be applied (pending marker unreadable)".into(),
                    );
                    return;
                }
                Err(err) => {
                    self.fail_update(ErrorSeverity::High, "updates.apply", err);
                    return;
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
        use crate::app::error_log::ErrorSeverity;
        let changed = self.updates.config.channel != channel;
        self.updates.config.channel = channel;
        if changed {
            // Results gathered on the previous channel say nothing about this one.
            if matches!(
                self.updates.status,
                UpdateStatus::Available(_) | UpdateStatus::UpToDate { .. } | UpdateStatus::Error(_)
            ) {
                self.updates.latest_release = None;
                self.updates.status = UpdateStatus::Idle;
            }
        }
        self.sync_poller_config();
        match self.updates.config.save(&self.profile_root) {
            Ok(()) => self.status = format!("Switched update channel to {}", channel.label()),
            Err(err) => {
                let msg = format!(
                    "Switched update channel to {} for this session, but saving the setting failed: {err}",
                    channel.label()
                );
                crate::app::error_log::record(ErrorSeverity::Medium, "updates.config", &msg);
                self.status = msg;
            }
        }
    }

    pub(crate) fn handle_update_event(&mut self, event: UpdateUiEvent) {
        use crate::app::error_log::ErrorSeverity;
        match event {
            UpdateUiEvent::CheckCompleted { generation, .. }
            | UpdateUiEvent::DownloadProgress { generation, .. }
            | UpdateUiEvent::DownloadCompleted { generation, .. }
                if generation != self.updates.generation =>
            {
                // Result of an operation that was cancelled meanwhile.
            }
            UpdateUiEvent::CheckCompleted { result, .. } => {
                let busy = matches!(
                    self.updates.status,
                    UpdateStatus::Downloading { .. } | UpdateStatus::ReadyToRestart { .. }
                );
                match result {
                    Ok(Some(release)) => {
                        self.status = format!("Update available: {}", release.tag_name);
                        if !busy {
                            self.updates.latest_release = Some(release.clone());
                            self.updates.status = UpdateStatus::Available(release);
                        }
                    }
                    Ok(None) => {
                        self.status =
                            format!("Graphite v{} is up to date", self.updates.current_version);
                        if !busy {
                            self.updates.latest_release = None;
                            self.updates.status = UpdateStatus::UpToDate {
                                checked_version: self.updates.current_version.clone(),
                            };
                        }
                    }
                    Err(err) => {
                        crate::app::error_log::record(ErrorSeverity::Medium, "updates.check", &err);
                        self.status = format!("Update check failed: {err}");
                        if !busy {
                            self.updates.status = UpdateStatus::Error(err);
                        }
                    }
                }
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
                crate::app::error_log::record(ErrorSeverity::Low, "updates.background_check", err);
            }
            UpdateUiEvent::DownloadProgress {
                tag, progress_pct, ..
            } => {
                if matches!(self.updates.status, UpdateStatus::Downloading { .. }) {
                    self.status = format!("Downloading update {tag}... {progress_pct}%");
                    self.updates.status = UpdateStatus::Downloading { tag, progress_pct };
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                }
            }
            UpdateUiEvent::DownloadCompleted {
                release, result, ..
            } => match result {
                Ok(package) => {
                    self.updates.staged_update_path = Some(package.staged_path.clone());
                    self.updates.staged_sha256 = Some(package.sha256);
                    self.updates.latest_release = Some(release.clone());
                    self.status = format!(
                        "Update {} downloaded and verified; restart to apply",
                        release.tag_name
                    );
                    self.updates.status = UpdateStatus::ReadyToRestart {
                        release,
                        staged_path: package.staged_path,
                    };
                }
                Err(err) if err == DOWNLOAD_CANCELLED => {
                    self.updates.status = match self.updates.latest_release.clone() {
                        Some(release) => UpdateStatus::Available(release),
                        None => UpdateStatus::Idle,
                    };
                    self.status = err;
                }
                Err(err) => {
                    let source = download_error_source(&err);
                    crate::app::error_log::record(ErrorSeverity::High, source, &err);
                    self.updates.status = UpdateStatus::Error(err.clone());
                    self.status = format!("Update download failed: {err}");
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::signature::test_keys::TestSigner;
    use super::state::{sha256_hex, ReleaseAsset};
    use super::*;
    use std::collections::HashMap;

    struct MockUpdateTransport {
        release: ReleaseInfo,
        bytes: Vec<u8>,
        texts: HashMap<String, String>,
    }

    impl UpdateTransport for MockUpdateTransport {
        fn fetch_latest_release(
            &self,
            _repo: &str,
            _channel: UpdateChannel,
        ) -> Result<ReleaseInfo, String> {
            Ok(self.release.clone())
        }

        fn download_text(&self, url: &str, _max_bytes: usize) -> Result<String, String> {
            self.texts
                .get(url)
                .cloned()
                .ok_or_else(|| format!("404 for {url}"))
        }

        fn download_to_file(
            &self,
            _url: &str,
            dest: &Path,
            progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
        ) -> Result<String, String> {
            transport::write_stream(
                &self.bytes[..],
                Some(self.bytes.len() as u64),
                dest,
                progress,
            )
        }
    }

    const SIG_URL: &str = "https://example.invalid/graphite-9.9.9.zip.sig";
    const SHA_URL: &str = "https://example.invalid/graphite-9.9.9.zip.sha256";

    fn release_with(asset: Option<ReleaseAsset>) -> ReleaseInfo {
        ReleaseInfo {
            tag_name: "v9.9.9".into(),
            version: "9.9.9".into(),
            prerelease: false,
            notes: "Bug fixes and improvements".into(),
            html_url: "https://github.com/BadTonho/Graphite/releases/tag/v9.9.9".into(),
            asset,
        }
    }

    /// A signed release for `bytes`, with the digest declared in the notes.
    fn signed_fixture(signer: &TestSigner, bytes: &[u8]) -> MockUpdateTransport {
        let sha = sha256_hex(bytes);
        let mut texts = HashMap::new();
        texts.insert(
            SIG_URL.to_owned(),
            signer.sign("v9.9.9", "graphite-9.9.9.zip", &sha),
        );
        MockUpdateTransport {
            release: release_with(Some(ReleaseAsset {
                name: "graphite-9.9.9.zip".into(),
                download_url: "https://example.invalid/graphite-9.9.9.zip".into(),
                sha256: Some(sha),
                sha256_url: None,
                signature_url: Some(SIG_URL.into()),
            })),
            bytes: bytes.to_vec(),
            texts,
        }
    }

    fn app_trusting(dir: &crate::test_support::TempDir, signer: &TestSigner) -> App {
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().to_path_buf();
        app.updates.trusted_keys = vec![signer.public_key()];
        app
    }

    fn staged_leftovers(app: &App) -> Vec<String> {
        let mut found = Vec::new();
        let root = app.profile_root.join("updates");
        if let Ok(tags) = std::fs::read_dir(&root) {
            for tag in tags.flatten().filter(|t| t.path().is_dir()) {
                for f in std::fs::read_dir(tag.path()).unwrap().flatten() {
                    found.push(f.file_name().to_string_lossy().into_owned());
                }
            }
        }
        found
    }

    #[test]
    fn signed_update_downloads_verifies_stages_and_schedules_install() {
        let dir = crate::test_support::TempDir::new();
        let signer = TestSigner::new();
        let mut app = app_trusting(&dir, &signer);
        let transport = signed_fixture(&signer, b"verified-release-archive");

        app.check_for_updates_with(&transport);
        assert!(matches!(app.updates.status, UpdateStatus::Available(_)));

        app.download_update_with(&transport);
        assert!(matches!(
            app.updates.status,
            UpdateStatus::ReadyToRestart { .. }
        ));
        let staged = app.updates.staged_update_path.clone().unwrap();
        assert!(staged.is_file());
        assert!(!staged.with_extension("zip.part").exists());

        app.restart_to_update();
        let marker: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                app.profile_root
                    .join("updates")
                    .join("pending-install.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(marker["sha256"], sha256_hex(b"verified-release-archive"));
    }

    #[test]
    fn tampered_package_is_rejected_and_nothing_is_left_staged() {
        let dir = crate::test_support::TempDir::new();
        let signer = TestSigner::new();
        let mut app = app_trusting(&dir, &signer);
        let mut transport = signed_fixture(&signer, b"verified-release-archive");
        transport.bytes = b"corrupted-archive".to_vec();

        app.check_for_updates_with(&transport);
        app.download_update_with(&transport);
        assert!(matches!(app.updates.status, UpdateStatus::Error(_)));
        assert!(app.status.contains("SHA-256 verification failed"));
        assert!(app.updates.staged_update_path.is_none());
        assert!(
            staged_leftovers(&app).is_empty(),
            "{:?}",
            staged_leftovers(&app)
        );
    }

    #[test]
    fn unsigned_untrusted_or_misbound_releases_are_refused() {
        let signer = TestSigner::new();
        let bytes = b"verified-release-archive";

        let try_download = |transport: &MockUpdateTransport, signer_for_app: &TestSigner| {
            let dir = crate::test_support::TempDir::new();
            let mut app = app_trusting(&dir, signer_for_app);
            app.check_for_updates_with(transport);
            app.download_update_with(transport);
            (
                app.status.clone(),
                app.updates.staged_update_path.clone(),
                staged_leftovers(&app),
            )
        };

        // No signature published.
        let mut unsigned = signed_fixture(&signer, bytes);
        unsigned.release.asset.as_mut().unwrap().signature_url = None;
        let (status, staged, left) = try_download(&unsigned, &signer);
        assert!(status.contains("not signed"), "{status}");
        assert!(staged.is_none() && left.is_empty());

        // Signed by a key the app does not trust.
        let attacker = TestSigner::new();
        let forged = signed_fixture(&attacker, bytes);
        let (status, staged, left) = try_download(&forged, &signer);
        assert!(status.contains("signature verification failed"), "{status}");
        assert!(staged.is_none() && left.is_empty());

        // Valid signature, but issued for a different release tag (replay).
        let mut replay = signed_fixture(&signer, bytes);
        replay.texts.insert(
            SIG_URL.to_owned(),
            signer.sign("v1.0.0", "graphite-9.9.9.zip", &sha256_hex(bytes)),
        );
        let (status, staged, _) = try_download(&replay, &signer);
        assert!(status.contains("signature verification failed"), "{status}");
        assert!(staged.is_none());

        // No digest anywhere.
        let mut no_digest = signed_fixture(&signer, bytes);
        no_digest.release.asset.as_mut().unwrap().sha256 = None;
        let (status, staged, _) = try_download(&no_digest, &signer);
        assert!(status.contains("does not declare a SHA-256"), "{status}");
        assert!(staged.is_none());
    }

    #[test]
    fn digest_can_come_from_the_companion_sha256_file() {
        let dir = crate::test_support::TempDir::new();
        let signer = TestSigner::new();
        let mut app = app_trusting(&dir, &signer);
        let bytes = b"verified-release-archive";
        let mut transport = signed_fixture(&signer, bytes);
        {
            let asset = transport.release.asset.as_mut().unwrap();
            asset.sha256 = None;
            asset.sha256_url = Some(SHA_URL.into());
        }
        transport.texts.insert(
            SHA_URL.to_owned(),
            format!("{}  graphite-9.9.9.zip\n", sha256_hex(bytes)),
        );
        app.check_for_updates_with(&transport);
        app.download_update_with(&transport);
        assert!(matches!(
            app.updates.status,
            UpdateStatus::ReadyToRestart { .. }
        ));
    }

    #[test]
    fn cancelled_download_is_aborted_and_cleans_partial_file() {
        let dir = crate::test_support::TempDir::new();
        let signer = TestSigner::new();
        let transport = signed_fixture(&signer, &vec![1u8; 300_000]);
        let release = transport.release.clone();
        let cancel = AtomicBool::new(true);

        let err = fetch_verified_package(
            &transport,
            &release,
            &[signer.public_key()],
            dir.path(),
            &cancel,
            &mut |_| {},
        )
        .unwrap_err();
        assert_eq!(err, DOWNLOAD_CANCELLED);
        let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("v9.9.9"))
            .unwrap()
            .flatten()
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn download_reports_monotonic_progress() {
        let dir = crate::test_support::TempDir::new();
        let signer = TestSigner::new();
        let transport = signed_fixture(&signer, &vec![1u8; 500_000]);
        let release = transport.release.clone();
        let mut seen = Vec::new();
        fetch_verified_package(
            &transport,
            &release,
            &[signer.public_key()],
            dir.path(),
            &AtomicBool::new(false),
            &mut |pct| seen.push(pct),
        )
        .unwrap();
        assert!(!seen.is_empty());
        assert!(seen.windows(2).all(|w| w[0] < w[1]));
        assert!(seen.iter().all(|p| *p < 100));
    }

    #[test]
    fn cancel_ignores_stale_results_but_keeps_later_checks_working() {
        let dir = crate::test_support::TempDir::new();
        let signer = TestSigner::new();
        let mut app = app_trusting(&dir, &signer);
        let transport = signed_fixture(&signer, b"verified-release-archive");
        let old_flag = Arc::clone(&app.updates.cancel_flag);
        let old_generation = app.updates.generation;

        app.cancel_update();
        assert!(
            old_flag.load(Ordering::SeqCst),
            "in-flight work must be told to stop"
        );
        assert!(!app.updates.cancel_flag.load(Ordering::SeqCst));
        assert!(matches!(app.updates.status, UpdateStatus::Idle));

        // A result from the cancelled operation is dropped.
        app.handle_update_event(UpdateUiEvent::CheckCompleted {
            generation: old_generation,
            result: Ok(Some(transport.release.clone())),
        });
        assert!(matches!(app.updates.status, UpdateStatus::Idle));

        // The background poller and new manual checks still work afterwards.
        app.handle_update_event(UpdateUiEvent::BackgroundCheckCompleted(Ok(Some(
            transport.release.clone(),
        ))));
        assert!(matches!(app.updates.status, UpdateStatus::Available(_)));
        app.updates.status = UpdateStatus::Idle;
        app.check_for_updates_with(&transport);
        assert!(matches!(app.updates.status, UpdateStatus::Available(_)));
    }

    #[test]
    fn manual_check_does_not_clobber_a_staged_update() {
        let dir = crate::test_support::TempDir::new();
        let signer = TestSigner::new();
        let mut app = app_trusting(&dir, &signer);
        let transport = signed_fixture(&signer, b"verified-release-archive");
        app.check_for_updates_with(&transport);
        app.download_update_with(&transport);
        app.check_for_updates_with(&transport);
        assert!(matches!(
            app.updates.status,
            UpdateStatus::ReadyToRestart { .. }
        ));
    }

    #[test]
    fn poll_delay_backs_off_and_is_capped() {
        assert_eq!(next_poll_delay(0), Duration::from_secs(600));
        assert_eq!(next_poll_delay(1), Duration::from_secs(1200));
        assert_eq!(next_poll_delay(2), Duration::from_secs(2400));
        assert_eq!(next_poll_delay(3), Duration::from_secs(3600));
        assert_eq!(next_poll_delay(u32::MAX), Duration::from_secs(3600));
    }

    #[test]
    fn channel_switch_persists_resets_stale_result_and_reports_save_failure() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.profile_root = dir.path().to_path_buf();
        app.updates.status = UpdateStatus::Available(release_with(None));

        app.set_update_channel(UpdateChannel::Prerelease);
        assert!(matches!(app.updates.status, UpdateStatus::Idle));
        assert_eq!(
            UpdateConfig::load_or_default(dir.path()).channel,
            UpdateChannel::Prerelease
        );

        let blocked = dir.path().join("blocked");
        std::fs::write(&blocked, b"x").unwrap();
        app.profile_root = blocked;
        app.set_update_channel(UpdateChannel::Stable);
        assert!(
            app.status.contains("saving the setting failed"),
            "{}",
            app.status
        );
    }

    #[test]
    fn restart_reports_failure_when_the_install_marker_cannot_be_written() {
        let dir = crate::test_support::TempDir::new();
        let signer = TestSigner::new();
        let mut app = app_trusting(&dir, &signer);
        let transport = signed_fixture(&signer, b"verified-release-archive");
        app.check_for_updates_with(&transport);
        app.download_update_with(&transport);

        // Replace the updates directory with a file so the marker cannot be created.
        let updates = app.profile_root.join("updates");
        std::fs::remove_dir_all(&updates).unwrap();
        std::fs::write(&updates, b"not a directory").unwrap();
        app.restart_to_update();
        assert!(matches!(app.updates.status, UpdateStatus::Error(_)));
        assert!(
            app.status.contains("Could not schedule update"),
            "{}",
            app.status
        );
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
