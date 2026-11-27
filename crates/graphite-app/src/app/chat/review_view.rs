//! In-memory Chat comparison and review controls.

use super::*;

impl App {
    pub(super) fn review_rows(body: super::layout::Rect, line_height: f32) -> usize {
        (body.h / (line_height * 1.5)).floor().max(0.0) as usize
    }

    pub(crate) fn chat_review_open(&self) -> bool {
        self.diff_view
            .as_ref()
            .is_some_and(|diff| diff.review.is_some())
    }

    pub(crate) fn open_chat_proposal(&mut self, id: u64) {
        let session = self.chat.active_session();
        if let Some(proposal) = session.proposals.iter().find(|proposal| proposal.id == id) {
            self.diff_view = Some(crate::app::scm_ui::DiffView {
                review: Some((session.id.clone(), id)),
                title: proposal
                    .path
                    .strip_prefix(&proposal.workspace)
                    .unwrap_or(&proposal.path)
                    .display()
                    .to_string(),
                left_name: self.t("chat.review.original").into(),
                right_name: self.t("chat.review.proposal").into(),
                left: proposal.before.clone().unwrap_or_default(),
                right: proposal.after.clone(),
                inline: false,
                collapse_unchanged: false,
                selected_change: 0,
            });
            self.diff_scroll = 0;
        }
    }

    pub(crate) fn open_next_chat_review(&mut self) {
        let session = self.chat.active_session();
        if let Some(id) = session.proposals.first().map(|entry| entry.id) {
            self.open_chat_proposal(id);
        } else if let Some(entry) = session.pending_edits.last() {
            self.diff_view = Some(crate::app::scm_ui::DiffView {
                review: None,
                title: entry.path.display().to_string(),
                left_name: self.t("chat.review.original").into(),
                right_name: self.t("chat.review.legacy").into(),
                left: entry.before.clone().unwrap_or_default(),
                right: entry.after.clone(),
                inline: false,
                collapse_unchanged: false,
                selected_change: 0,
            });
            self.diff_scroll = 0;
        }
    }

    pub(crate) fn decide_chat_review(&mut self, apply: bool) {
        let Some((session, id)) = self.diff_view.as_ref().and_then(|diff| diff.review.clone())
        else {
            return;
        };
        if self.chat.active_session_id != session {
            self.status = self.t("chat.review.outdated").into();
            return;
        }
        let done = if apply {
            self.apply_chat_proposal(id)
        } else {
            self.discard_chat_proposal(id)
        };
        if done {
            self.diff_view = None;
        }
    }

    pub(crate) fn decide_active_chat_file(&mut self, apply: bool) {
        if self.chat_review_open() {
            self.decide_chat_review(apply);
            return;
        }
        let session = self.chat.active_session();
        let active = self.documents[self.active_document()].buffer.path();
        let exact = active.and_then(|path| {
            session
                .proposals
                .iter()
                .find(|proposal| same_file_path(&proposal.path, path))
        });
        if let Some(id) = exact
            .or_else(|| session.proposals.first())
            .map(|proposal| proposal.id)
        {
            self.open_chat_proposal(id);
            return;
        }
        let legacy = session
            .pending_edits
            .iter()
            .find(|entry| {
                entry.status == graphite_chat::EditReviewStatus::Pending
                    && active.is_none_or(|path| same_file_path(&entry.path, path))
            })
            .map(|entry| entry.path.clone());
        if let Some(path) = legacy {
            if apply {
                self.accept_chat_edit(&path);
            } else {
                self.discard_chat_edit(&path);
            }
        }
    }

    pub(crate) fn chat_review_notice(&self) -> Option<&str> {
        let (session, id) = self.diff_view.as_ref()?.review.as_ref()?;
        if session != &self.chat.active_session_id {
            return Some(self.t("chat.review.outdated"));
        }
        let proposal = self
            .chat
            .active_session()
            .proposals
            .iter()
            .find(|entry| entry.id == *id);
        match proposal {
            Some(proposal) => proposal.issue.as_deref(),
            None => Some(self.t("chat.review.outdated")),
        }
    }
}
