//! Applies, reviews, and rolls back the file edits the assistant makes.
//!
//! Every write goes through the main thread so it can be checked against the open editors: a
//! file whose editor has unsaved changes is never overwritten, and a failed write is reported to
//! the user, to the model, and to the error log instead of being recorded as applied.

use super::*;
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
        if let Some(id) = self
            .chat
            .active_session()
            .proposals
            .iter()
            .find(|proposal| same_file_path(&proposal.path, path))
            .map(|proposal| proposal.id)
        {
            return self.apply_chat_proposal(id);
        }
        let accepted = self.chat.active_session_mut().accept_edit(path);
        if accepted {
            self.chat
                .persist_active_session(&self.profile_root, self.profiles.is_none());
            self.status = format!("Accepted changes to {}", path.display());
        }
        accepted
    }

    pub(crate) fn discard_chat_edit(&mut self, path: &Path) -> bool {
        if let Some(id) = self
            .chat
            .active_session()
            .proposals
            .iter()
            .find(|proposal| same_file_path(&proposal.path, path))
            .map(|proposal| proposal.id)
        {
            return self.discard_chat_proposal(id);
        }
        let Some(entry) = self
            .chat
            .active_session()
            .pending_edits
            .iter()
            .find(|entry| {
                entry.path == path && entry.status == graphite_chat::EditReviewStatus::Pending
            })
            .cloned()
        else {
            return false;
        };
        if !self.restore_legacy_chat_file(path, Some(&entry.after), entry.before.as_deref()) {
            return false;
        }
        self.chat.active_session_mut().discard_edit(path);
        self.persist_chat_review();
        self.refresh_scm_async();
        self.status = self.t("chat.review.discarded").into();
        true
    }

    pub(crate) fn accept_all_chat_edits(&mut self) -> usize {
        if !self.chat.active_session().proposals.is_empty() {
            return self.apply_all_chat_proposals();
        }
        let count = self.chat.active_session_mut().accept_all_edits();
        self.chat
            .persist_active_session(&self.profile_root, self.profiles.is_none());
        self.status = format!("Accepted {count} file edit(s)");
        count
    }

    pub(crate) fn discard_all_chat_edits(&mut self) -> usize {
        if !self.chat.active_session().proposals.is_empty() {
            let count = self.chat.active_session().proposals.len();
            self.chat.active_session_mut().proposals.clear();
            self.persist_chat_review();
            self.status = self.tf("chat.review.discarded_many", &[("count", &count)]);
            return count;
        }
        let paths: Vec<_> = self
            .chat
            .active_session()
            .pending_edits
            .iter()
            .filter(|edit| edit.status == graphite_chat::EditReviewStatus::Pending)
            .map(|edit| edit.path.clone())
            .collect();
        paths
            .iter()
            .filter(|path| self.discard_chat_edit(path))
            .count()
    }

    pub(crate) fn undo_chat_edit(&mut self) -> bool {
        self.restore_chat_checkpoint(true)
    }

    pub(crate) fn redo_chat_edit(&mut self) -> bool {
        self.restore_chat_checkpoint(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat::ChatUiEvent;
    use graphite_chat::ProposedEdit;
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
        turn.app.handle_chat_ui_event(ChatUiEvent::StageEdit {
            run: turn.run.clone(),
            session_id: turn.session_id.clone(),
            turn_id: turn.turn_id,
            edit: ProposedEdit {
                path: path.to_path_buf(),
                before: std::fs::read(path).ok(),
                after: after.into(),
            },
            reply: tx,
        });
        rx.recv().unwrap()
    }

    #[test]
    fn review_uses_an_in_memory_diff_and_preserves_file_format() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        let original = b"\xef\xbb\xbfold\r\n";
        std::fs::write(&file, original).unwrap();
        let mut turn = running_turn(dir.path());
        turn.app.open_file(file.clone());

        let reply = apply(&mut turn, &file, "new\n").unwrap();
        assert!(reply.contains("queued"), "{reply}");
        assert_eq!(std::fs::read(&file).unwrap(), original);
        assert_eq!(turn.app.editor().text(), "old\n");
        assert!(turn.app.chat.active_session().checkpoints.is_empty());

        turn.app.open_next_chat_review();
        let diff = turn.app.diff_view.as_ref().unwrap();
        assert_eq!((&diff.left[..], &diff.right[..]), ("old\n", "new\n"));
        turn.app.decide_chat_review(true);
        assert!(turn.app.diff_view.is_none());
        assert_eq!(std::fs::read(&file).unwrap(), b"\xef\xbb\xbfnew\r\n");
        assert_eq!(turn.app.editor().text(), "new\n");
        assert!(!turn.app.documents[turn.app.active_document()].is_dirty());
        assert!(turn.app.undo_chat_edit());
        assert_eq!(std::fs::read(&file).unwrap(), original);
        assert!(turn.app.redo_chat_edit());
    }

    #[test]
    fn a_conflict_in_a_batch_keeps_manual_changes_and_applies_other_files() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        let other = dir.path().join("b.rs");
        let created_elsewhere = dir.path().join("c.rs");
        std::fs::write(&file, "original").unwrap();
        let mut turn = running_turn(dir.path());
        turn.app.open_file(file.clone());
        apply(&mut turn, &file, "from the model").unwrap();
        apply(&mut turn, &other, "created").unwrap();
        apply(&mut turn, &created_elsewhere, "from the model").unwrap();
        assert!(!other.exists());
        turn.app.editor_mut().insert("typed ");
        std::fs::write(&created_elsewhere, "external").unwrap();

        assert_eq!(turn.app.apply_all_chat_proposals(), 1);
        assert_eq!(turn.app.editor().text(), "typed original");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "original");
        assert_eq!(std::fs::read_to_string(&other).unwrap(), "created");
        assert_eq!(
            std::fs::read_to_string(&created_elsewhere).unwrap(),
            "external"
        );
        let session = turn.app.chat.active_session();
        assert_eq!(session.proposals.len(), 2);
        assert!(session.proposals.iter().all(|entry| entry.issue.is_some()));
        assert_eq!(session.checkpoints.len(), 1);
        assert_eq!(turn.app.discard_all_chat_edits(), 2);
        assert_eq!(turn.app.editor().text(), "typed original");
    }

    #[test]
    fn a_replaced_diff_cannot_apply_or_discard_a_newer_proposal() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        let mut turn = running_turn(dir.path());
        apply(&mut turn, &file, "old proposal").unwrap();
        turn.app.open_next_chat_review();
        apply(&mut turn, &file, "new proposal").unwrap();

        turn.app.decide_chat_review(true);
        turn.app.decide_chat_review(false);
        assert!(!file.exists());
        assert!(turn.app.chat_review_notice().is_some());
        turn.app.open_next_chat_review();
        turn.app.decide_chat_review(true);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new proposal");
        assert!(turn.app.undo_chat_edit());
        assert!(!file.exists());
        assert!(turn.app.redo_chat_edit());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new proposal");
    }

    #[test]
    fn undo_and_redo_preserve_intervening_disk_and_editor_changes() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "old").unwrap();
        let mut turn = running_turn(dir.path());
        turn.app.open_file(file.clone());
        apply(&mut turn, &file, "new").unwrap();
        assert!(turn.app.accept_chat_edit(&file));

        std::fs::write(&file, "external").unwrap();
        assert!(!turn.app.undo_chat_edit());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "external");
        std::fs::write(&file, "new").unwrap();
        assert!(turn.app.undo_chat_edit());
        turn.app.editor_mut().insert("typed ");
        assert!(!turn.app.redo_chat_edit());
        assert_eq!(turn.app.editor().text(), "typed old");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "old");
    }

    #[test]
    fn cancelled_turns_keep_existing_proposals_and_reject_late_edits() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        let mut turn = running_turn(dir.path());
        apply(&mut turn, &file, "before cancellation").unwrap();
        turn.app.cancel_chat_turn();
        assert!(apply(&mut turn, &file, "late").is_err());
        assert!(!file.exists());
        assert_eq!(
            turn.app.chat.active_session().proposals[0].after,
            "before cancellation"
        );
    }

    #[test]
    fn a_directory_or_outside_path_is_not_written() {
        let dir = crate::test_support::TempDir::new();
        let external = crate::test_support::TempDir::new();
        let target = dir.path().join("folder");
        std::fs::create_dir(&target).unwrap();
        let mut turn = running_turn(dir.path());
        let outside = external.path().join("outside.txt");
        assert!(apply(&mut turn, &outside, "text").is_err());
        assert!(!outside.exists());

        apply(&mut turn, &target, "text").unwrap();
        assert!(!turn.app.accept_chat_edit(&target));
        assert!(target.is_dir());
        assert!(turn.app.chat.active_session().proposals[0].issue.is_some());
        assert!(turn.app.chat.active_session().checkpoints.is_empty());
    }
    #[test]
    fn queued_proposals_survive_reloading_and_require_the_original_workspace() {
        let dir = crate::test_support::TempDir::new();
        let other = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "before").unwrap();
        let mut turn = running_turn(dir.path());
        apply(&mut turn, &file, "after").unwrap();
        let serialized = turn.app.chat.active_session().export_json().unwrap();

        let mut app = App::new_with_options(other.path().to_path_buf(), None, true);
        *app.chat.active_session_mut() =
            graphite_chat::ChatSession::import_json(&serialized).unwrap();
        let id = app.chat.active_session().proposals[0].id;
        assert!(!app.apply_chat_proposal(id));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "before");

        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        *app.chat.active_session_mut() =
            graphite_chat::ChatSession::import_json(&serialized).unwrap();
        assert!(app.apply_chat_proposal(id));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "after");
    }

    #[test]
    fn prior_session_history_still_loads_and_checks_the_file_before_rollback() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "legacy already applied").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let session = app.chat.active_session_mut();
        let turn = session.start_turn("old saved conversation");
        session.stage_edit(
            turn,
            file.clone(),
            Some("before".into()),
            "legacy already applied".into(),
        );
        let serialized = session.export_json().unwrap();
        assert!(!serialized.contains("\"proposals\""));
        *session = graphite_chat::ChatSession::import_json(&serialized).unwrap();
        assert!(session.proposals.is_empty());

        std::fs::write(&file, "edited later").unwrap();
        assert!(!app.undo_chat_edit());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "edited later");
        std::fs::write(&file, "legacy already applied").unwrap();
        assert!(app.undo_chat_edit());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "before");
    }
}
