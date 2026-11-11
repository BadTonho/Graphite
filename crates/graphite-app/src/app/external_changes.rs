//! Keeps open documents in step with files changed outside the editor.

use super::*;
use std::time::{Duration, SystemTime};

/// A file written this recently may still be in progress; it is checked again later.
pub(crate) const SETTLE_DELAY: Duration = Duration::from_secs(1);

impl App {
    /// Called when the file watcher reports a change. Clean documents adopt the new disk
    /// contents; documents with unsaved edits are kept and reported.
    pub(crate) fn sync_external_changes(&mut self) {
        self.sync_external_changes_after(SETTLE_DELAY);
    }

    pub(crate) fn sync_external_changes_after(&mut self, settle: Duration) {
        let now = SystemTime::now();
        let mut reloads = Vec::new();
        let mut kept_edits = 0usize;
        let mut unsettled = false;
        for tab in self.documents.iter() {
            let Some(path) = tab.buffer.path().map(PathBuf::from) else {
                continue;
            };
            let Ok(modified) = std::fs::metadata(&path).and_then(|m| m.modified()) else {
                continue;
            };
            if now.duration_since(modified).unwrap_or_default() < settle {
                unsettled = true;
                continue;
            }
            let Ok(file) = crate::file_format::read(&path, &self.translator) else {
                continue;
            };
            if file.text == tab.buffer.text() {
                continue;
            }
            if tab.is_dirty() {
                kept_edits += 1;
            } else {
                reloads.push((path, Ok(file)));
            }
        }
        let reloaded = reloads.len();
        if reloaded > 0 {
            self.reload_after_branch_checkout(reloads);
            self.status = self.tf("external.reloaded", &[("count", &reloaded)]);
        }
        if kept_edits > 0 {
            self.status = self.tf("external.kept_edits", &[("count", &kept_edits)]);
        }
        if unsettled {
            self.recheck_external_changes_later();
        }
    }

    /// Runs the external-change check again once recently written files have settled.
    fn recheck_external_changes_later(&self) {
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        std::thread::spawn(move || {
            std::thread::sleep(SETTLE_DELAY);
            let _ = proxy.send_event(UserEvent::ExternalChangeCheck);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn clean_documents_reload_and_dirty_documents_keep_their_edits() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("notes.txt");
        fs::write(&path, "one").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(path.clone());
        let id = app.active_document();

        fs::write(&path, "two").unwrap();
        app.sync_external_changes_after(Duration::ZERO);
        assert_eq!(app.editor().text(), "two");
        assert!(!app.documents[id].is_dirty());
        assert_eq!(app.status, app.tf("external.reloaded", &[("count", &1usize)]));

        app.editor_mut().insert("x");
        fs::write(&path, "three").unwrap();
        app.sync_external_changes_after(Duration::ZERO);
        assert!(app.editor().text().contains('x'));
        assert_ne!(app.editor().text(), "three");
        assert!(app.documents[id].is_dirty());
        assert_eq!(app.status, app.tf("external.kept_edits", &[("count", &1usize)]));
    }

    #[test]
    fn unchanged_files_are_left_alone() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("same.txt");
        fs::write(&path, "stable").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(path);
        app.editor_mut().set_cursor_line_col(0, 3);
        app.sync_external_changes_after(Duration::ZERO);
        assert_eq!(app.editor().text(), "stable");
        assert_eq!(app.editor().cursor_line_col(), (0, 3));
    }
}
