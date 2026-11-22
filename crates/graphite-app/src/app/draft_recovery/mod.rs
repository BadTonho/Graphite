//! Local snapshots of unsaved text, owned by the window that can still edit them.
mod review;
mod store;
#[cfg(test)]
mod tests;

use crate::{
    app::error_log,
    file_format::{Encoding, LineEnding},
    App, DocumentId,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use store::{Draft, NativePath, Session};

pub(crate) const SETTING: &str = "files.draftRecovery";
const INTERVAL: Duration = Duration::from_secs(1);
const RETRY: Duration = Duration::from_secs(5);
type Job = Box<dyn FnOnce() + Send>;

#[derive(Default)]
pub(crate) struct Recovery {
    profile_settings: HashMap<PathBuf, bool>,
    settings_checked: HashMap<PathBuf, Instant>,
    sessions: HashMap<PathBuf, Arc<Session>>,
    tracked: HashMap<DocumentId, Tracked>,
    inaccessible: HashSet<PathBuf>,
    offered_profiles: HashSet<PathBuf>,
    open_review: Option<review::Review>,
    retry_session: Option<Instant>,
    worker: Option<mpsc::Sender<Job>>,
}

#[derive(Clone, PartialEq, Eq)]
struct SnapshotKey {
    revision: u64,
    original: Option<PathBuf>,
    base: Option<[u8; 32]>,
    encoding: Encoding,
    ending: LineEnding,
}

struct Tracked {
    session: Arc<Session>,
    snapshot: PathBuf,
    written: Option<SnapshotKey>,
    running: Option<(Option<SnapshotKey>, mpsc::Receiver<io::Result<()>>)>,
    next_write: Instant,
    remove_retry: Option<Instant>,
}

impl Tracked {
    fn start(
        &mut self,
        worker: &mut Option<mpsc::Sender<Job>>,
        key: Option<SnapshotKey>,
        operation: impl FnOnce(&Path) -> io::Result<()> + Send + 'static,
    ) -> io::Result<()> {
        if worker.is_none() {
            let (sender, receiver) = mpsc::channel::<Job>();
            std::thread::Builder::new()
                .name("graphite-drafts".into())
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        job();
                    }
                })?;
            *worker = Some(sender);
        }
        let path = self.snapshot.clone();
        let session = self.session.clone();
        let (sender, receiver) = mpsc::channel();
        if worker
            .as_ref()
            .unwrap()
            .send(Box::new(move || {
                let result = operation(&path);
                drop(session);
                let _ = sender.send(result);
            }))
            .is_err()
        {
            *worker = None;
            return Err(ErrorKind::BrokenPipe.into());
        }
        self.running = Some((key, receiver));
        Ok(())
    }
}

fn report(source: &str, error: &io::Error) {
    // Paths, file text and parser input are deliberately excluded from the error log.
    error_log::record(
        error_log::ErrorSeverity::High,
        source,
        format!("Draft recovery: {:?}", error.kind()),
    );
}

impl Recovery {
    pub(crate) fn for_profile(profile: &Path) -> Self {
        let mut recovery = Self::default();
        recovery.reload_settings(profile);
        recovery
    }

    fn profile_enabled(profile: &Path) -> bool {
        let raw = match fs::read_to_string(profile.join("settings.json")) {
            Ok(raw) => raw,
            Err(error) if error.kind() == ErrorKind::NotFound => return true,
            Err(error) => {
                report("drafts.settings", &error);
                return false;
            }
        };
        match crate::config::parse_jsonc(&raw) {
            Ok(settings) => match settings.get(SETTING) {
                None => true,
                Some(value) => value.as_bool().unwrap_or_else(|| {
                    report("drafts.settings", &ErrorKind::InvalidData.into());
                    false
                }),
            },
            Err(_) => {
                report("drafts.settings", &ErrorKind::InvalidData.into());
                false
            }
        }
    }

    pub(crate) fn reload_settings(&mut self, profile: &Path) {
        self.set_enabled(profile, Self::profile_enabled(profile));
        self.retry_session = None;
    }

    pub(crate) fn set_enabled(&mut self, profile: &Path, enabled: bool) {
        self.profile_settings.insert(profile.into(), enabled);
        self.settings_checked.insert(profile.into(), Instant::now());
    }

    fn capture_enabled(&mut self, profile: &Path, now: Instant) -> bool {
        if self
            .settings_checked
            .get(profile)
            .is_none_or(|checked| now.saturating_duration_since(*checked) >= INTERVAL)
        {
            self.profile_settings
                .insert(profile.into(), Self::profile_enabled(profile));
            self.settings_checked.insert(profile.into(), now);
        }
        self.profile_settings[profile]
    }

    fn capture_session(&mut self, profile: &Path) -> io::Result<Arc<Session>> {
        if let Some(session) = self.sessions.get(profile) {
            return Ok(session.clone());
        }
        let session = Session::create(profile)?;
        self.sessions.insert(profile.into(), session.clone());
        Ok(session)
    }

    fn take_session(&self, directory: &Path) -> io::Result<Option<Arc<Session>>> {
        if let Some(tracked) = self
            .tracked
            .values()
            .find(|draft| draft.session.directory == directory)
        {
            return Ok(Some(tracked.session.clone()));
        }
        if let Some(session) = self
            .sessions
            .values()
            .find(|session| session.directory == directory)
        {
            return Ok(Some(session.clone()));
        }
        Session::acquire(directory)
    }
}

impl App {
    pub(crate) fn draft_tick(&mut self, now: Instant) -> Option<Instant> {
        let mut wake: Option<Instant> = None;
        let mut schedule = |at: Instant| wake = Some(wake.map_or(at, |next| next.min(at)));
        let ids: HashSet<_> = self
            .documents
            .ids()
            .chain(self.drafts.tracked.keys().copied())
            .collect();
        for id in ids {
            let current = self.documents.get(id).filter(|document| {
                document.is_dirty()
                    && document.notebook.is_none()
                    && !document.custom_dirty
                    && !document
                        .buffer
                        .path()
                        .is_some_and(|path| self.custom_editor_is_bound(path))
                    && self.editor_groups.view_count(id) > 0
                    && !self.exit_requested
            });
            if let Some(tracked) = self.drafts.tracked.get_mut(&id) {
                if let Some((_, receiver)) = &tracked.running {
                    let result = match receiver.try_recv() {
                        Ok(result) => result,
                        Err(mpsc::TryRecvError::Empty) => {
                            schedule(now + Duration::from_millis(20));
                            continue;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => Err(ErrorKind::BrokenPipe.into()),
                    };
                    let (key, _) = tracked.running.take().unwrap();
                    match result {
                        Ok(()) if key.is_none() => {
                            self.drafts.tracked.remove(&id);
                        }
                        Ok(()) => tracked.written = key,
                        Err(error) => {
                            if key.is_none() {
                                report("drafts.remove", &error);
                                tracked.remove_retry = Some(now + RETRY);
                            } else {
                                report("drafts.write", &error);
                                tracked.next_write = now + RETRY;
                            }
                        }
                    }
                }
            }
            let Some(document) = current else {
                if let Some(tracked) = self.drafts.tracked.get_mut(&id) {
                    // Cleanup bypasses the capture throttle. A failed removal can retry while open.
                    if let Some(retry) = tracked.remove_retry.filter(|retry| *retry > now) {
                        if !self.exit_requested {
                            schedule(retry);
                        }
                        continue;
                    }
                    match tracked.start(&mut self.drafts.worker, None, store::remove) {
                        Ok(()) => schedule(now + Duration::from_millis(20)),
                        Err(error) => {
                            report("drafts.remove", &error);
                            tracked.remove_retry = Some(now + RETRY);
                            if !self.exit_requested {
                                schedule(now + RETRY);
                            }
                        }
                    }
                }
                continue;
            };
            let owner = self
                .drafts
                .tracked
                .get(&id)
                .and_then(|tracked| tracked.session.directory.parent().and_then(Path::parent))
                .unwrap_or(&self.profile_root)
                .to_path_buf();
            if !self.drafts.capture_enabled(&owner, now) {
                continue;
            }
            if !self.drafts.tracked.contains_key(&id) {
                if let Some(retry) = self.drafts.retry_session.filter(|retry| *retry > now) {
                    schedule(retry);
                    continue;
                }
                let session = match self.drafts.capture_session(&self.profile_root) {
                    Ok(session) => session,
                    Err(error) => {
                        report("drafts.session", &error);
                        self.drafts.retry_session = Some(now + RETRY);
                        schedule(now + RETRY);
                        continue;
                    }
                };
                self.drafts.tracked.insert(
                    id,
                    Tracked {
                        snapshot: session.directory.join(format!("{}.json", id.0)),
                        session,
                        written: None,
                        running: None,
                        next_write: now,
                        remove_retry: None,
                    },
                );
            }
            let tracked = self.drafts.tracked.get_mut(&id).unwrap();
            let key = SnapshotKey {
                revision: document.buffer.revision(),
                original: document
                    .buffer
                    .path()
                    .and_then(|path| std::path::absolute(path).ok()),
                base: document.saved_fingerprint,
                encoding: document.encoding,
                ending: document.line_ending,
            };
            if tracked.written.as_ref() == Some(&key) {
                continue;
            }
            if tracked.next_write > now {
                schedule(tracked.next_write);
                continue;
            }
            let mut draft = Draft {
                version: store::VERSION,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                original: key.original.as_deref().map(NativePath::new),
                label: document.recovery_label.clone(),
                base: key.base,
                revision: key.revision,
                encoding: key.encoding,
                ending: key.ending,
                text: String::new(),
            };
            let text = document.buffer.rope().clone();
            match tracked.start(&mut self.drafts.worker, Some(key), move |path| {
                draft.text = text.to_string();
                store::write(path, &draft)
            }) {
                Ok(()) => {
                    tracked.next_write = now + INTERVAL;
                    schedule(now + Duration::from_millis(20));
                }
                Err(error) => {
                    report("drafts.write", &error);
                    tracked.next_write = now + RETRY;
                    schedule(tracked.next_write);
                }
            }
        }
        wake
    }

    pub(crate) fn drafts_busy(&self) -> bool {
        self.drafts
            .tracked
            .values()
            .any(|draft| draft.running.is_some())
    }
}
