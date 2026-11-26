//! The workspace file index Quick Open ranks, kept up to date in the background.
//!
//! The index is built on a background thread the first time Quick Open is used. Afterwards it is
//! built again only when the file watcher reports a file created or removed outside the paths
//! the root `.gitignore` excludes; edits to existing files do not change it. Until the first
//! index is ready, Quick Open walks the disk as before.

use super::*;
use graphite_workspace::FileIndex;
use std::sync::atomic::AtomicBool;

#[derive(Default)]
pub(crate) struct FileIndexState {
    /// The latest index. Shared with Quick Open threads, which only read it.
    pub(crate) index: Option<Arc<FileIndex>>,
    /// Cancels the build in progress, if any.
    building: Option<Arc<AtomicBool>>,
    /// Files were created or removed since `index` was built.
    stale: bool,
    /// Identifies the build whose result is still wanted.
    generation: u64,
}

impl App {
    /// Starts building the index when there is none yet or files were added or removed, unless
    /// a build is already running.
    pub(crate) fn ensure_file_index(&mut self) {
        let state = &mut self.file_index;
        if state.building.is_some() || (state.index.is_some() && !state.stale) {
            return;
        }
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        state.stale = false;
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        state.building = Some(cancel.clone());
        let workspace = self.tree.workspace_snapshot();
        let started = std::thread::Builder::new()
            .name("graphite-file-index".into())
            .spawn(move || {
                if let Some(index) = workspace.file_index(|| cancel.load(Ordering::Relaxed)) {
                    let _ = proxy.send_event(UserEvent::FileIndexReady {
                        generation,
                        index: Arc::new(index),
                    });
                }
            });
        if started.is_err() {
            self.file_index.building = None;
        }
    }

    /// Takes a finished index, unless the workspace changed while it was built.
    pub(crate) fn accept_file_index(&mut self, generation: u64, index: Arc<FileIndex>) {
        let state = &mut self.file_index;
        if generation != state.generation {
            return;
        }
        state.building = None;
        state.index = Some(index);
        // Files added or removed during the build make it out of date already.
        if state.stale {
            self.ensure_file_index();
        }
    }

    /// Files were created or removed: the index is built again when Quick Open is open, or the
    /// next time it opens.
    pub(crate) fn mark_file_index_stale(&mut self) {
        self.file_index.stale = true;
        if self.quick_open.is_some() {
            self.ensure_file_index();
        }
    }

    /// Forgets the index of the previous folders, after the open folders change.
    pub(crate) fn reset_file_index(&mut self) {
        if let Some(cancel) = self.file_index.building.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        let generation = self.file_index.generation.wrapping_add(1);
        self.file_index = FileIndexState {
            generation,
            ..FileIndexState::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_of(dir: &std::path::Path) -> Arc<FileIndex> {
        Arc::new(
            graphite_workspace::Workspace::open(dir)
                .unwrap()
                .file_index(|| false)
                .unwrap(),
        )
    }

    #[test]
    fn a_result_for_the_current_folders_is_kept_and_a_late_one_is_dropped() {
        let dir = crate::test_support::TempDir::new();
        std::fs::write(dir.path().join("a.rs"), "").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let generation = app.file_index.generation;

        app.accept_file_index(generation, index_of(dir.path()));
        assert_eq!(
            app.file_index.index.as_ref().map(|index| index.len()),
            Some(1)
        );

        // The folders change while another build runs: its result is out of date.
        app.reset_file_index();
        app.accept_file_index(generation, index_of(dir.path()));
        assert!(app.file_index.index.is_none());
    }

    #[test]
    fn added_or_removed_files_make_the_index_stale_until_it_is_built_again() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let generation = app.file_index.generation;
        app.accept_file_index(generation, index_of(dir.path()));
        assert!(!app.file_index.stale);

        app.mark_file_index_stale();

        assert!(app.file_index.stale);
        // The old index still answers until a new one is ready.
        assert!(app.file_index.index.is_some());
    }
}
