//! Applies, reviews, and rolls back the file edits the assistant makes.
//!
//! Every write goes through the main thread so it can be checked against the open editors: a
//! file whose editor has unsaved changes is never overwritten, and a failed write is reported to
//! the user, to the model, and to the error log instead of being recorded as applied.

use super::*;
use graphite_chat::{write_proposed_edit, ProposedEdit};

/// Error-log source for chat edit writes.
const EDIT_LOG_SOURCE: &str = "chat.edits";

impl App {
    /// Whether `path` is open in an editor whose text has not been saved.
    pub(crate) fn open_document_has_unsaved_changes(&self, path: &Path) -> bool {
        self.documents.iter().any(|tab| {
            tab.buffer.is_dirty()
                && tab
                    .buffer
                    .path()
                    .is_some_and(|open| same_file_path(open, path))
        })
    }

    /// The first of `paths` that is open with unsaved changes.
    fn first_unsaved_path<'a>(
        &self,
        mut paths: impl Iterator<Item = &'a Path>,
    ) -> Option<&'a Path> {
        paths.find(|path| self.open_document_has_unsaved_changes(path))
    }

    fn unsaved_changes_message(path: &Path) -> String {
        format!(
            "{} has unsaved changes in the editor. Save or revert them first.",
            path.display()
        )
    }

    /// Applies an edit proposed by the model: refuses files with unsaved editor changes, writes
    /// the file, updates its editor, and stages the change for review. The returned text is what
    /// the model is told.
    pub(crate) fn apply_chat_proposed_edit(
        &mut self,
        session_id: &str,
        turn_id: u64,
        edit: ProposedEdit,
    ) -> Result<String, String> {
        if self.open_document_has_unsaved_changes(&edit.path) {
            let message = Self::unsaved_changes_message(&edit.path);
            self.status = message.clone();
            if let Some(session) = self.chat.session_by_id_mut(session_id) {
                session.log("edit.blocked", message.clone());
            }
            return Err(format!(
                "{message} The edit was not applied. Ask the user to save or revert the file, \
                 then propose the edit again."
            ));
        }
        // The file is read again here: it may have changed since the model proposed the edit.
        let before = std::fs::read_to_string(&edit.path).ok();
        if let Err(error) = write_proposed_edit(&edit.path, &edit.after) {
            let message = format!("Could not write {}: {error}", edit.path.display());
            crate::app::error_log::record(
                crate::app::error_log::ErrorSeverity::High,
                EDIT_LOG_SOURCE,
                &message,
            );
            self.status = message.clone();
            if let Some(session) = self.chat.session_by_id_mut(session_id) {
                session.log("edit.failed", message.clone());
            }
            return Err(format!("{message}. The edit was not applied."));
        }
        self.sync_open_document_text(&edit.path, Some(&edit.after));
        let action = if before.is_some() {
            "Updated"
        } else {
            "Created"
        };
        let content = format!(
            "{action} {} ({} bytes). The user can keep, discard, or undo the change.",
            edit.path.display(),
            edit.after.len()
        );
        if let Some(session) = self.chat.session_by_id_mut(session_id) {
            session.stage_edit(turn_id, edit.path, before, edit.after);
        }
        Ok(content)
    }

    /// Updates an already-open document buffer when a file on disk is updated or reverted by chat.
    /// Callers check [`Self::open_document_has_unsaved_changes`] first: this replaces the text.
    pub(crate) fn sync_open_document_text(&mut self, path: &Path, new_text: Option<&str>) {
        let Some(doc_id) = self
            .documents
            .iter()
            .find(|tab| {
                tab.buffer
                    .path()
                    .is_some_and(|open| same_file_path(open, path))
            })
            .map(|tab| tab.id)
        else {
            return;
        };
        if let Some(text) = new_text {
            let tab = &mut self.documents[doc_id];
            let outcome = tab.buffer.reload_text(text);
            let line_count = tab.buffer.len_lines();
            tab.highlights = None;
            tab.sticky_headers_cache = None;
            tab.highlighter = IncrementalHighlighter::new();
            self.editor_groups.for_each_view_mut(|_, view| {
                if view.document == doc_id {
                    view.selection_state
                        .remap(&outcome.positions, outcome.len_chars);
                    view.scroll_line = view.scroll_line.min(line_count.saturating_sub(1));
                }
            });
        }
    }

    pub(crate) fn accept_chat_edit(&mut self, path: &Path) -> bool {
        let accepted = self.chat.active_session_mut().accept_edit(path);
        if accepted {
            self.chat
                .persist_active_session(&self.profile_root, self.profiles.is_none());
            self.status = format!("Accepted changes to {}", path.display());
        }
        accepted
    }

    pub(crate) fn discard_chat_edit(&mut self, path: &Path) -> bool {
        if self.open_document_has_unsaved_changes(path) {
            self.status = Self::unsaved_changes_message(path);
            return false;
        }
        let Some(before) = self.chat.active_session_mut().discard_edit(path) else {
            return false;
        };
        let failed = self.restore_files([(path.to_path_buf(), before)]);
        self.finish_rollback(failed, format!("Discarded changes to {}", path.display()));
        true
    }

    pub(crate) fn accept_all_chat_edits(&mut self) -> usize {
        let count = self.chat.active_session_mut().accept_all_edits();
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.status = format!("Accepted {count} file edit(s)");
        count
    }

    pub(crate) fn discard_all_chat_edits(&mut self) -> usize {
        let session = self.chat.active_session();
        let pending = session
            .pending_edits
            .iter()
            .filter(|edit| edit.status == graphite_chat::EditReviewStatus::Pending)
            .map(|edit| edit.path.as_path());
        if let Some(path) = self.first_unsaved_path(pending) {
            self.status = Self::unsaved_changes_message(path);
            return 0;
        }
        let reverted = self.chat.active_session_mut().discard_all_edits();
        let count = reverted.len();
        let failed = self.restore_files(reverted);
        self.finish_rollback(failed, format!("Discarded {count} file edit(s)"));
        count
    }

    pub(crate) fn undo_chat_edit(&mut self) -> bool {
        let Some(last) = self.chat.active_session().checkpoints.last() else {
            return false;
        };
        if let Some(path) = self.first_unsaved_path(last.entries.iter().map(|e| e.path.as_path())) {
            self.status = Self::unsaved_changes_message(path);
            return false;
        }
        let Some(cp) = self.chat.active_session_mut().undo_checkpoint() else {
            return false;
        };
        let failed = self.restore_files(
            cp.entries
                .iter()
                .map(|e| (e.path.clone(), e.before.clone())),
        );
        self.finish_rollback(failed, format!("Undid {}", cp.label));
        true
    }

    pub(crate) fn redo_chat_edit(&mut self) -> bool {
        let Some(next) = self.chat.active_session().undone_checkpoints.last() else {
            return false;
        };
        if let Some(path) = self.first_unsaved_path(next.entries.iter().map(|e| e.path.as_path())) {
            self.status = Self::unsaved_changes_message(path);
            return false;
        }
        let Some(cp) = self.chat.active_session_mut().redo_checkpoint() else {
            return false;
        };
        let failed =
            self.restore_files(cp.entries.iter().map(|e| (e.path.clone(), e.after.clone())));
        self.finish_rollback(failed, format!("Redid {}", cp.label));
        true
    }

    /// Writes (or removes, for `None`) each file and updates its editor. Returns the files that
    /// could not be restored; each failure is recorded in the error log.
    fn restore_files(
        &mut self,
        files: impl IntoIterator<Item = (PathBuf, Option<String>)>,
    ) -> Vec<PathBuf> {
        let mut failed = Vec::new();
        for (path, content) in files {
            let result = match content.as_deref() {
                Some(text) => write_proposed_edit(&path, text),
                None => match std::fs::remove_file(&path) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(error.to_string()),
                },
            };
            match result {
                Ok(()) => self.sync_open_document_text(&path, content.as_deref()),
                Err(error) => {
                    crate::app::error_log::record(
                        crate::app::error_log::ErrorSeverity::High,
                        EDIT_LOG_SOURCE,
                        format!("Could not restore {}: {error}", path.display()),
                    );
                    failed.push(path);
                }
            }
        }
        failed
    }

    fn finish_rollback(&mut self, failed: Vec<PathBuf>, done: String) {
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.refresh_scm_async();
        self.status = match failed.as_slice() {
            [] => done,
            [path] => format!("Could not restore {}", path.display()),
            paths => format!("Could not restore {} files", paths.len()),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat::ChatUiEvent;
    use std::sync::{atomic::AtomicBool, mpsc, Arc};

    struct Turn {
        app: App,
        session_id: String,
        turn_id: u64,
        run: Arc<AtomicBool>,
    }

    fn running_turn(dir: &Path) -> Turn {
        let mut app = App::new_with_options(dir.to_path_buf(), None, true);
        let turn_id = app.chat.active_session_mut().start_turn("Edit the file");
        let session_id = app.chat.active_session_id.clone();
        let run = Arc::new(AtomicBool::new(false));
        app.chat.running_turn = Some((session_id.clone(), turn_id, run.clone()));
        Turn {
            app,
            session_id,
            turn_id,
            run,
        }
    }

    fn apply(turn: &mut Turn, path: &Path, after: &str) -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        turn.app.handle_chat_ui_event(ChatUiEvent::ApplyEdit {
            run: turn.run.clone(),
            session_id: turn.session_id.clone(),
            turn_id: turn.turn_id,
            edit: ProposedEdit {
                path: path.to_path_buf(),
                before: None,
                after: after.into(),
            },
            reply: tx,
        });
        rx.recv().unwrap()
    }

    #[test]
    fn edit_is_written_staged_and_reported_with_the_content_on_disk() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "old\n").unwrap();
        let mut turn = running_turn(dir.path());
        turn.app.open_file(file.clone());

        let reply = apply(&mut turn, &file, "new\n").unwrap();

        assert!(reply.starts_with("Updated"), "{reply}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new\n");
        assert_eq!(turn.app.editor().text(), "new\n");
        let staged = &turn.app.chat.active_session().pending_edits[0];
        assert_eq!(staged.before.as_deref(), Some("old\n"));
    }

    #[test]
    fn edit_never_overwrites_unsaved_editor_changes() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "saved\n").unwrap();
        let mut turn = running_turn(dir.path());
        turn.app.open_file(file.clone());
        turn.app.editor_mut().insert("unsaved ");
        let typed = turn.app.editor().text();

        let error = apply(&mut turn, &file, "from the model\n").unwrap_err();

        assert!(error.contains("unsaved changes"), "{error}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "saved\n");
        assert_eq!(turn.app.editor().text(), typed);
        assert!(turn.app.documents[turn.app.active_document()]
            .buffer
            .is_dirty());
        assert!(turn.app.chat.active_session().pending_edits.is_empty());
    }

    #[test]
    fn failed_write_is_reported_and_not_staged() {
        let dir = crate::test_support::TempDir::new();
        // A directory cannot be written as a file.
        let target = dir.path().join("folder");
        std::fs::create_dir(&target).unwrap();
        let mut turn = running_turn(dir.path());

        let error = apply(&mut turn, &target, "text").unwrap_err();

        assert!(error.contains("Could not write"), "{error}");
        assert!(turn.app.chat.active_session().pending_edits.is_empty());
        assert!(turn.app.chat.active_session().checkpoints.is_empty());
    }

    #[test]
    fn edit_after_the_turn_ended_is_refused() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        let mut turn = running_turn(dir.path());
        turn.app.chat.running_turn = None;

        assert!(apply(&mut turn, &file, "text").is_err());
        assert!(!file.exists());
    }

    #[test]
    fn undo_redo_and_discard_keep_unsaved_editor_changes() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "one\n").unwrap();
        let mut turn = running_turn(dir.path());
        turn.app.open_file(file.clone());
        apply(&mut turn, &file, "two\n").unwrap();
        turn.app.editor_mut().insert("typed ");
        let typed = turn.app.editor().text();

        assert!(!turn.app.undo_chat_edit());
        assert!(!turn.app.discard_chat_edit(&file));
        assert_eq!(turn.app.discard_all_chat_edits(), 0);

        assert_eq!(turn.app.editor().text(), typed);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two\n");
        assert!(turn.app.status.contains("unsaved changes"));
        let session = turn.app.chat.active_session();
        assert_eq!(session.checkpoints.len(), 1);
        assert_eq!(
            session.pending_edits[0].status,
            graphite_chat::EditReviewStatus::Pending
        );
    }

    #[test]
    fn rollback_reports_a_file_that_could_not_be_restored() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        let mut turn = running_turn(dir.path());
        apply(&mut turn, &file, "created\n").unwrap();
        // Replace the created file with a directory, so restoring "no file" fails.
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();

        assert!(turn.app.undo_chat_edit());

        assert!(
            turn.app.status.starts_with("Could not restore"),
            "{}",
            turn.app.status
        );
    }
}
