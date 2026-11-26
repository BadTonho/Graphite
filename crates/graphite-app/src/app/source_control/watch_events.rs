//! Turns the file watcher's stream of notifications into a few events for the event loop.
//!
//! A build or a package install changes thousands of files in a few seconds. Each notification
//! used to start a `git status` and a commit-graph load and check every open document. Now
//! notifications are collected for [`SETTLE`] and sent as one batch: at most one source control
//! refresh, one external-change check, and one list of file changes for extensions. Changes to
//! files that the root `.gitignore` excludes (such as `node_modules`) do not refresh source
//! control, but still lead to the external-change check, since an ignored file may be open.

use crate::app::plugins::FileChange;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How long notifications are collected before one batch is sent.
pub(crate) const SETTLE: Duration = Duration::from_millis(250);

/// What one notification asks for.
pub(crate) struct Signal {
    /// Whether the change can alter what source control shows.
    pub(crate) scm: bool,
    /// Whether a file Quick Open lists was created or removed.
    pub(crate) files: bool,
    pub(crate) changes: Vec<(FileChange, PathBuf)>,
}

/// Everything the notifications of one settle period ask for.
#[derive(Default, Debug, PartialEq)]
pub(crate) struct Batch {
    pub(crate) scm: bool,
    pub(crate) files: bool,
    /// Distinct changes in the order they first happened.
    pub(crate) changes: Vec<(FileChange, PathBuf)>,
}

impl Batch {
    fn add(&mut self, signal: Signal) {
        self.scm |= signal.scm;
        self.files |= signal.files;
        for change in signal.changes {
            if !self.changes.contains(&change) {
                self.changes.push(change);
            }
        }
    }
}

/// Collects signals from `signals` and calls `emit` once per settle period that received any,
/// until the sending side is dropped. A burst that never pauses is still reported every
/// `settle`, so source control keeps up while it lasts.
pub(crate) fn coalesce(signals: Receiver<Signal>, settle: Duration, mut emit: impl FnMut(Batch)) {
    while let Ok(first) = signals.recv() {
        let mut batch = Batch::default();
        batch.add(first);
        let deadline = Instant::now() + settle;
        let disconnected = loop {
            match signals.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(signal) => batch.add(signal),
                Err(RecvTimeoutError::Timeout) => break false,
                Err(RecvTimeoutError::Disconnected) => break true,
            }
        };
        emit(batch);
        if disconnected {
            return;
        }
    }
}

/// Decides which changed paths matter to source control.
pub(crate) struct ScmPathFilter {
    ignores: Vec<Gitignore>,
}

impl ScmPathFilter {
    /// Reads each root's `.gitignore` and `.git/info/exclude`. Nested `.gitignore` files are not
    /// read; a change they would exclude only costs a refresh.
    pub(crate) fn new(roots: &[PathBuf]) -> Self {
        let ignores = roots
            .iter()
            .filter_map(|root| {
                let mut builder = GitignoreBuilder::new(root);
                for file in [root.join(".gitignore"), root.join(".git/info/exclude")] {
                    if file.is_file() {
                        // A malformed line is skipped by the builder; the rest still applies.
                        let _ = builder.add(file);
                    }
                }
                builder.build().ok()
            })
            .collect();
        Self { ignores }
    }

    /// Whether a change to `path` is a change to the workspace at all. Git's own bookkeeping
    /// inside `.git` (index and lock files, rewritten by every status refresh) is not, or a
    /// refresh would feed itself; neither is build output under `target`.
    pub(crate) fn is_watched(path: &Path) -> bool {
        let names: Vec<_> = path.components().map(|part| part.as_os_str()).collect();
        if let Some(git) = names.iter().position(|name| *name == ".git") {
            // Branch switches and commits move HEAD or refs; everything else is Git internals.
            return names
                .get(git + 1)
                .is_some_and(|name| *name == "HEAD" || *name == "refs" || *name == "packed-refs");
        }
        !names.iter().any(|name| *name == "target")
    }

    /// Whether creating or removing `path` changes the files Quick Open lists, which leave out
    /// `.git` and what the root `.gitignore` excludes.
    pub(crate) fn is_listed(&self, path: &Path) -> bool {
        !path.components().any(|part| part.as_os_str() == ".git") && self.is_relevant(path)
    }

    /// Whether a change to `path` can alter what source control shows: a watched path that the
    /// root `.gitignore` does not exclude.
    pub(crate) fn is_relevant(&self, path: &Path) -> bool {
        if !Self::is_watched(path) {
            return false;
        }
        if path.components().any(|part| part.as_os_str() == ".git") {
            return true;
        }
        !self.ignores.iter().any(|ignore| {
            path.starts_with(ignore.path())
                && ignore.matched_path_or_any_parents(path, false).is_ignore()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn signal(scm: bool, path: &str) -> Signal {
        Signal {
            scm,
            files: false,
            changes: vec![(FileChange::Changed, PathBuf::from(path))],
        }
    }

    #[test]
    fn a_burst_of_notifications_becomes_one_batch() {
        let (sender, receiver) = mpsc::channel();
        for index in 0..5_000 {
            sender
                .send(signal(index == 4_999, &format!("file-{}", index % 10)))
                .unwrap();
        }
        drop(sender);
        let mut batches = Vec::new();

        coalesce(receiver, Duration::from_millis(50), |batch| {
            batches.push(batch)
        });

        assert_eq!(batches.len(), 1);
        assert!(batches[0].scm);
        assert_eq!(batches[0].changes.len(), 10, "repeated changes are merged");
        assert_eq!(batches[0].changes[0].1, PathBuf::from("file-0"));
    }

    #[test]
    fn changes_after_a_pause_form_a_new_batch() {
        let (sender, receiver) = mpsc::channel();
        let producer = std::thread::spawn(move || {
            sender.send(signal(false, "a")).unwrap();
            std::thread::sleep(Duration::from_millis(200));
            sender.send(signal(true, "b")).unwrap();
        });
        let mut batches = Vec::new();

        coalesce(receiver, Duration::from_millis(40), |batch| {
            batches.push(batch)
        });

        producer.join().unwrap();
        assert_eq!(batches.len(), 2);
        assert!(!batches[0].scm && batches[1].scm);
    }

    #[test]
    fn ignored_folders_and_git_internals_do_not_refresh_source_control() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path().to_path_buf();
        std::fs::write(root.join(".gitignore"), "node_modules/\n*.log\n").unwrap();
        let filter = ScmPathFilter::new(std::slice::from_ref(&root));

        assert!(filter.is_relevant(&root.join("src/main.rs")));
        assert!(!filter.is_relevant(&root.join("node_modules/pkg/index.js")));
        assert!(!filter.is_relevant(&root.join("debug.log")));
        assert!(!filter.is_relevant(&root.join("target/debug/app")));
        assert!(!filter.is_relevant(&root.join(".git/objects/ab/cdef")));
        assert!(filter.is_relevant(&root.join(".git/HEAD")));
        assert!(filter.is_relevant(&root.join(".git/refs/heads/main")));
    }

    #[test]
    fn quick_open_lists_files_outside_git_and_ignored_folders() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path().to_path_buf();
        std::fs::write(
            root.join(".gitignore"),
            "node_modules/
",
        )
        .unwrap();
        let filter = ScmPathFilter::new(std::slice::from_ref(&root));

        assert!(filter.is_listed(&root.join("src/new.rs")));
        assert!(!filter.is_listed(&root.join("node_modules/pkg/index.js")));
        assert!(!filter.is_listed(&root.join(".git/refs/heads/main")));
    }

    #[test]
    fn ignored_files_are_still_watched_for_extensions() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path().to_path_buf();
        std::fs::write(root.join(".gitignore"), "out/\n").unwrap();
        let filter = ScmPathFilter::new(std::slice::from_ref(&root));
        let built = root.join("out/test.js");

        assert!(!filter.is_relevant(&built));
        assert!(ScmPathFilter::is_watched(&built));
        assert!(!ScmPathFilter::is_watched(&root.join(".git/index")));
    }

    #[test]
    fn a_folder_without_gitignore_refreshes_for_every_change() {
        let dir = crate::test_support::TempDir::new();
        let filter = ScmPathFilter::new(&[dir.path().to_path_buf()]);
        assert!(filter.is_relevant(&dir.path().join("node_modules/pkg/index.js")));
    }
}
