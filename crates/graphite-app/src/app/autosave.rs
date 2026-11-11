//! Automatic saving of dirty documents (`files.autoSave`).

use super::*;
use crate::config::AutoSaveMode;
use std::time::{Duration, Instant};

impl App {
    /// The mode in effect: the settings value, else the File menu toggle (`afterDelay` or `off`).
    pub(crate) fn effective_auto_save(&self) -> AutoSaveMode {
        self.auto_save_setting.unwrap_or(if self.auto_save {
            AutoSaveMode::AfterDelay
        } else {
            AutoSaveMode::Off
        })
    }

    /// Runs the `afterDelay` timers and the `onFocusChange` check. Called from the event loop.
    pub(crate) fn autosave_tick(&mut self) {
        let mode = self.effective_auto_save();
        let active = self.active_document();
        let previous = self.autosave_last_document.replace(active);
        if mode == AutoSaveMode::OnFocusChange {
            if let Some(previous) = previous.filter(|previous| *previous != active) {
                self.save_if_dirty(previous);
            }
        }
        if mode != AutoSaveMode::AfterDelay {
            self.autosave_deadlines.clear();
            return;
        }
        let delay = Duration::from_millis(self.auto_save_delay_ms);
        let now = Instant::now();
        let mut due = Vec::new();
        for document in self.documents.iter() {
            if !document.is_dirty()
                || document.buffer.path().is_none()
                || document.buffer.is_read_only()
            {
                self.autosave_deadlines.remove(&document.id);
                continue;
            }
            let revision = document.buffer.revision();
            let deadline = self
                .autosave_deadlines
                .entry(document.id)
                .or_insert((revision, now + delay));
            if deadline.0 != revision {
                *deadline = (revision, now + delay);
            } else if now >= deadline.1 && !self.saving.document_pending(document.id) {
                due.push(document.id);
                deadline.1 = now + Duration::from_secs(3600);
            }
        }
        for document in due {
            self.save_document(document);
        }
    }

    /// `onWindowChange`: saves every dirty file when the window loses focus.
    pub(crate) fn autosave_window_lost_focus(&mut self) {
        if self.effective_auto_save() != AutoSaveMode::OnWindowChange {
            return;
        }
        let documents: Vec<DocumentId> = self.documents.iter().map(|tab| tab.id).collect();
        for document in documents {
            self.save_if_dirty(document);
        }
    }

    fn save_if_dirty(&mut self, document: DocumentId) {
        let Some(tab) = self.documents.get(document) else {
            return;
        };
        if tab.is_dirty()
            && tab.buffer.path().is_some()
            && !tab.buffer.is_read_only()
            && !self.saving.document_pending(document)
        {
            self.save_document(document);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn dirty_app(name: &str) -> (crate::test_support::TempDir, App, DocumentId, PathBuf) {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join(name);
        fs::write(&path, "saved").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(path.clone());
        let id = app.active_document();
        app.editor_mut().insert("!");
        (dir, app, id, path)
    }

    #[test]
    fn settings_mode_overrides_the_menu_toggle() {
        let (_dir, mut app, _id, _path) = dirty_app("a.txt");
        assert_eq!(app.effective_auto_save(), AutoSaveMode::Off);
        app.auto_save = true;
        assert_eq!(app.effective_auto_save(), AutoSaveMode::AfterDelay);
        app.auto_save_setting = Some(AutoSaveMode::OnWindowChange);
        assert_eq!(app.effective_auto_save(), AutoSaveMode::OnWindowChange);
    }

    #[test]
    fn window_change_saves_dirty_files_only_in_that_mode() {
        let (_dir, mut app, id, path) = dirty_app("b.txt");
        app.autosave_window_lost_focus();
        app.wait_for_saves();
        assert!(app.documents[id].is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "saved");

        app.auto_save_setting = Some(AutoSaveMode::OnWindowChange);
        app.autosave_window_lost_focus();
        app.wait_for_saves();
        app.wait_for_saves();
        assert!(!app.documents[id].is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "!saved");
    }

    #[test]
    fn focus_change_saves_the_document_being_left() {
        let (dir, mut app, id, path) = dirty_app("c.txt");
        app.auto_save_setting = Some(AutoSaveMode::OnFocusChange);
        app.autosave_tick();
        let other = dir.path().join("other.txt");
        fs::write(&other, "other").unwrap();
        app.open_file(other);
        app.autosave_tick();
        app.wait_for_saves();
        app.wait_for_saves();
        assert!(!app.documents[id].is_dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "!saved");
    }
}
