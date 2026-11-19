use super::*;
use crate::app::error_log::{self, ErrorSeverity};
use graphite_plugins::{catalog::Catalog, open_vsx::Registry, package};
use std::{collections::VecDeque, path::Path, time::Duration};

const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

pub(super) enum SearchResults {
    OpenVsx(Vec<Extension>),
    Graphite(Vec<Repository>),
}

#[derive(Clone)]
pub(super) struct Candidate {
    id: String,
    pub(super) directory: String,
    pub(super) version: String,
}

pub(super) struct DownloadedUpdate {
    pub(super) download: tempfile::TempDir,
    pub(super) path: PathBuf,
    pub(super) metadata: Metadata,
    pub(super) candidate: Candidate,
}

pub(super) struct CheckResult {
    candidates: Vec<Candidate>,
    failed: usize,
    purpose: CheckPurpose,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckPurpose {
    Manual,
    UpdateAll,
    Automatic,
}

#[derive(Default)]
pub(super) struct UpdateState {
    pub(super) automatic: bool,
    next_check: Option<Instant>,
    queue: VecDeque<Candidate>,
    review_now: bool,
    pub(super) ready: Option<DownloadedUpdate>,
    worker: Option<mpsc::Receiver<Result<AutomaticOutcome>>>,
}

enum AutomaticOutcome {
    Checked(CheckResult),
    Downloaded(DownloadedUpdate),
}

fn is_newer(installed: &str, available: &str) -> bool {
    use semver::Version;
    matches!((Version::parse(installed), Version::parse(available)),
        (Ok(installed), Ok(available)) if available.cmp_precedence(&installed).is_gt())
}

fn check_updates(registry: &Registry, records: &[Installed], purpose: CheckPurpose) -> CheckResult {
    let mut candidates = Vec::new();
    let mut failed = 0;
    for record in records {
        let Metadata::Vsix(vsix) = &record.metadata else {
            continue;
        };
        let Some((publisher, name)) = vsix.id.split_once('.') else {
            continue;
        };
        match registry.find_latest(publisher, name) {
            Ok(Some(latest)) if is_newer(&vsix.version, &latest.version) => {
                candidates.push(Candidate {
                    id: vsix.id.clone(),
                    directory: record.directory.clone(),
                    version: latest.version,
                });
            }
            Ok(_) => {}
            Err(_) => {
                failed += 1;
                let severity = if purpose == CheckPurpose::Automatic {
                    ErrorSeverity::Low
                } else {
                    ErrorSeverity::Medium
                };
                error_log::record(
                    severity,
                    "plugins.update.check",
                    format!("Could not check Open VSX for {}", vsix.id),
                );
            }
        }
    }
    CheckResult {
        candidates,
        failed,
        purpose,
    }
}

impl App {
    pub(super) fn reload_plugin_sidebar(&mut self) {
        if self.plugins.worker.is_some() || self.plugins.bisect.is_some() {
            return;
        }
        let tab = self.plugins.tab;
        let query = if self.plugins.path_input {
            String::new()
        } else {
            self.plugins.query.clone()
        };
        self.plugin_job(move |store| {
            let records = store.list()?;
            let search = match tab {
                PluginsTab::Installed => None,
                PluginsTab::OpenVsx => Some(
                    Registry::new()
                        .and_then(|registry| registry.search(&query, 30))
                        .map(SearchResults::OpenVsx),
                ),
                PluginsTab::Graphite => Some(
                    Catalog::new()
                        .and_then(|mut catalog| catalog.search(&query, 1))
                        .map(SearchResults::Graphite),
                ),
            };
            Ok(Outcome::SidebarReload(records, search))
        });
    }

    pub(crate) fn plugins_check_updates(&mut self) {
        self.request_plugin_updates(CheckPurpose::Manual);
    }

    pub(crate) fn plugins_update_all(&mut self) {
        if self.plugins.worker.is_some()
            || self.plugin_details_open()
            || self.plugins.bisect.is_some()
        {
            return;
        }
        self.plugins.update_state.review_now = true;
        if let Some(update) = self.plugins.update_state.ready.take() {
            self.show_update_preview(update);
        } else if self.plugins.update_state.queue.is_empty()
            && self.plugins.update_state.worker.is_none()
        {
            self.request_plugin_updates(CheckPurpose::UpdateAll);
        }
    }

    fn request_plugin_updates(&mut self, purpose: CheckPurpose) {
        if self.plugins.worker.is_some()
            || self.plugin_details_open()
            || self.plugins.bisect.is_some()
        {
            return;
        }
        let installed = self.plugins.installed.clone();
        self.plugin_job(move |_| {
            let registry = Registry::new()?;
            Ok(Outcome::UpdatesChecked(check_updates(
                &registry, &installed, purpose,
            )))
        });
    }

    pub(super) fn finish_plugin_update_check(&mut self, result: CheckResult) {
        let count = result.candidates.len();
        if result.purpose != CheckPurpose::Manual {
            let ready = self
                .plugins
                .update_state
                .ready
                .as_ref()
                .map(|update| &update.candidate.id);
            self.plugins.update_state.queue = result
                .candidates
                .into_iter()
                .filter(|candidate| ready != Some(&candidate.id))
                .collect();
        }
        if result.purpose != CheckPurpose::Automatic || count > 0 {
            self.plugins.message = self.tf(
                "plugins.toolbar.update_result",
                &[("count", &count), ("failed", &result.failed)],
            );
        }
    }

    pub(super) fn finish_plugin_update_download(&mut self, update: DownloadedUpdate) {
        if self.plugins.update_state.review_now
            && self.plugins.visible
            && self.plugins.focused
            && self.plugin_updates_idle()
        {
            self.show_update_preview(update);
        } else {
            self.plugins.update_state.ready = Some(update);
            self.plugins.message = self.t("plugins.toolbar.update_ready").into();
            self.status = self.plugins.message.clone();
        }
    }

    fn show_update_preview(&mut self, update: DownloadedUpdate) {
        self.open_plugins();
        self.leave_plugin_details();
        self.plugins.preview = Some((update.path.clone(), update.metadata.clone()));
        self.plugins.downloaded_update = Some(update);
    }

    pub(super) fn load_plugin_auto_updates(&mut self) {
        match std::fs::read(self.profile_root.join("extension-auto-updates.json")) {
            Ok(bytes) => match serde_json::from_slice::<bool>(&bytes) {
                Ok(enabled) => self.set_plugin_auto_update_schedule(enabled),
                Err(_) => error_log::record(
                    ErrorSeverity::High,
                    "plugins.update.settings",
                    "Invalid automatic extension update preference.",
                ),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => error_log::record(
                ErrorSeverity::High,
                "plugins.update.settings",
                error.to_string(),
            ),
        }
    }

    fn set_plugin_auto_update_schedule(&mut self, enabled: bool) {
        self.plugins.update_state.automatic = enabled;
        self.plugins.update_state.next_check = enabled.then(Instant::now);
    }

    pub(super) fn toggle_plugin_auto_updates(&mut self) {
        let enabled = !self.plugins.update_state.automatic;
        let result = persist_auto_updates(&self.profile_root, enabled);
        if let Err(error) = result {
            error_log::record(
                ErrorSeverity::High,
                "plugins.update.settings",
                error.to_string(),
            );
            self.plugins.message = self.t("plugins.toolbar.update_settings_failed").into();
            return;
        }
        self.set_plugin_auto_update_schedule(enabled);
        if !enabled {
            self.plugins.update_state.worker = None;
            if !self.plugins.update_state.review_now {
                self.plugins.update_state.queue.clear();
                self.plugins.update_state.ready = None;
            }
        }
    }

    pub(crate) fn plugin_auto_update_deadline(&self) -> Option<Instant> {
        self.plugins
            .update_state
            .next_check
            .filter(|_| self.plugin_updates_idle() && self.plugins.update_state.worker.is_none())
    }

    fn plugin_updates_idle(&self) -> bool {
        self.plugins.store.is_some()
            && self.plugins.worker.is_none()
            && !self.plugins.editing
            && !self.plugin_details_open()
            && self.plugins.toolbar_menu.is_none()
            && self.plugins.bisect.is_none()
            && self.plugins.running_scroll.is_none()
    }

    pub(super) fn poll_plugin_updates(&mut self) {
        self.poll_automatic_plugin_worker();
        if !self.plugin_updates_idle() || self.plugins.update_state.worker.is_some() {
            return;
        }
        if self.plugins.update_state.ready.is_none() {
            if let Some(candidate) = self.plugins.update_state.queue.pop_front() {
                if self.plugins.update_state.review_now {
                    self.download_plugin_update(candidate);
                } else {
                    self.start_automatic_plugin_worker(move |store| {
                        let registry = Registry::new()?;
                        download_update(&registry, &store.root, candidate)
                            .map(AutomaticOutcome::Downloaded)
                    });
                }
                return;
            }
            self.plugins.update_state.review_now = false;
        }
        let now = Instant::now();
        if self
            .plugins
            .update_state
            .next_check
            .is_some_and(|deadline| deadline <= now)
        {
            self.plugins.update_state.next_check = Some(now + CHECK_INTERVAL);
            if self
                .plugins
                .installed
                .iter()
                .any(|record| matches!(record.metadata, Metadata::Vsix(_)))
            {
                let installed = self.plugins.installed.clone();
                self.start_automatic_plugin_worker(move |_| {
                    let registry = Registry::new()?;
                    Ok(AutomaticOutcome::Checked(check_updates(
                        &registry,
                        &installed,
                        CheckPurpose::Automatic,
                    )))
                });
            }
        }
    }

    fn start_automatic_plugin_worker(
        &mut self,
        job: impl FnOnce(Store) -> Result<AutomaticOutcome> + Send + 'static,
    ) {
        let Some(store) = self.plugins.store.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.plugins.update_state.worker = Some(rx);
        let proxy = self.event_proxy.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job(store));
            if let Some(proxy) = proxy {
                let _ = proxy.send_event(UserEvent::ExtensionHostWake);
            }
        });
    }

    fn poll_automatic_plugin_worker(&mut self) {
        let Some(worker) = self.plugins.update_state.worker.as_ref() else {
            return;
        };
        let result = match worker.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(self.t("plugins.worker_stopped").into()),
        };
        self.plugins.update_state.worker = None;
        match result {
            Ok(AutomaticOutcome::Checked(result)) => self.finish_plugin_update_check(result),
            Ok(AutomaticOutcome::Downloaded(update)) => self.finish_plugin_update_download(update),
            Err(_) => error_log::record(
                ErrorSeverity::Low,
                "plugins.update.automatic",
                "Could not prepare automatic extension updates.",
            ),
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn download_plugin_update(&mut self, candidate: Candidate) {
        self.plugin_job(move |store| {
            let registry = Registry::new()?;
            download_update(&registry, &store.root, candidate).map(Outcome::UpdatePreview)
        });
    }

    pub(super) fn update_one_plugin(
        &mut self,
        publisher: String,
        name: String,
        installed_version: String,
        directory: String,
    ) {
        self.plugins.update_state.review_now = true;
        self.plugin_job(move |store| {
            let registry = Registry::new()?;
            let latest = registry.latest(&publisher, &name)?;
            if !is_newer(&installed_version, &latest.version) {
                return Ok(Outcome::Notice(format!(
                    "{publisher}.{name} is already up to date ({installed_version})"
                )));
            }
            let candidate = Candidate {
                id: format!("{publisher}.{name}"),
                directory,
                version: latest.version,
            };
            download_update(&registry, &store.root, candidate).map(Outcome::UpdatePreview)
        });
    }
}

fn download_update(
    registry: &Registry,
    root: &Path,
    candidate: Candidate,
) -> Result<DownloadedUpdate> {
    let (publisher, name) = candidate
        .id
        .split_once('.')
        .ok_or("Invalid VSIX identity")?;
    let download = new_plugin_download(root)?;
    let path = download.path().join("package.vsix");
    let latest = registry.download_latest(publisher, name, &path)?;
    if latest.version != candidate.version {
        return Err(
            "Open VSX version changed since the update check. Check again to review it.".into(),
        );
    }
    let metadata = package::inspect(&path)?;
    Ok(DownloadedUpdate {
        download,
        path,
        metadata,
        candidate,
    })
}

fn persist_auto_updates(root: &Path, enabled: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    use std::io::Write;
    file.write_all(if enabled { b"true" } else { b"false" })?;
    file.as_file().sync_all()?;
    file.persist(root.join("extension-auto-updates.json"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_release_precedence_without_downgrades() {
        assert!(is_newer("1.9.0", "1.10.0"));
        assert!(is_newer("2.0.0-rc.1", "2.0.0"));
        assert!(!is_newer("2.0.0", "2.0.0-rc.1"));
        assert!(!is_newer("2.0.0+first", "2.0.0+second"));
        assert!(!is_newer("2.0.0", "invalid"));
    }

    #[test]
    fn preference_is_isolated_by_profile_and_can_be_replaced() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let other = root.path().join("other");
        persist_auto_updates(&first, true).unwrap();
        persist_auto_updates(&other, false).unwrap();
        assert_eq!(
            std::fs::read(first.join("extension-auto-updates.json")).unwrap(),
            b"true"
        );
        assert_eq!(
            std::fs::read(other.join("extension-auto-updates.json")).unwrap(),
            b"false"
        );
        persist_auto_updates(&first, false).unwrap();
        assert_eq!(
            std::fs::read(first.join("extension-auto-updates.json")).unwrap(),
            b"false"
        );
    }

    #[test]
    fn registry_update_download_is_prepared_without_replacing_the_installed_version() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        fn archive(version: &str) -> Vec<u8> {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            zip.start_file(
                "extension/package.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            write!(
                zip,
                r#"{{"publisher":"fixture","name":"extension","version":"{version}"}}"#
            )
            .unwrap();
            zip.finish().unwrap().into_inner()
        }

        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("old.vsix");
        std::fs::write(&package, archive("1.9.0")).unwrap();
        let store = Store::new(root.path().join("plugins"));
        let installed = store.install(&package).unwrap();
        store.enable("fixture.extension", true).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = String::new();
                BufReader::new(&mut stream).read_line(&mut request).unwrap();
                let body = if request.starts_with("GET /package.vsix ") {
                    archive("1.10.0")
                } else {
                    format!(r#"{{"namespace":"Fixture","name":"extension","version":"1.10.0","files":{{"download":"http://{address}/package.vsix"}}}}"#).into_bytes()
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        let registry = Registry::with_base(&format!("http://{address}")).unwrap();
        let mut result = check_updates(&registry, &store.list().unwrap(), CheckPurpose::UpdateAll);
        assert_eq!(result.failed, 0);
        let candidate = result.candidates.pop().unwrap();
        let update = download_update(&registry, &store.root, candidate).unwrap();
        assert_eq!(update.metadata.version(), "1.10.0");
        let before = store.get("fixture.extension").unwrap();
        assert_eq!(before.directory, installed.directory);
        assert!(before.enabled);
        let downloaded = update.path.clone();
        drop(update);
        assert!(!downloaded.exists());
        assert_eq!(
            store.get("fixture.extension").unwrap().directory,
            installed.directory
        );
        server.join().unwrap();
    }
}
