//! Layout and navigation for proposal review in AI mode.

use super::*;
use crate::app::{
    error_log::{self, ErrorSeverity},
    scm_ui::DiffView,
};

pub(crate) struct ReviewReturn {
    diff: DiffView,
    scroll: usize,
    row: Option<usize>,
}

impl App {
    fn ai_review_proposal(&self) -> Option<&graphite_chat::FileProposal> {
        let (session, id) = self.diff_view.as_ref()?.review.as_ref()?;
        if session != &self.chat.active_session_id {
            return None;
        }
        self.chat
            .active_session()
            .proposals
            .iter()
            .find(|proposal| proposal.id == *id)
    }

    pub(crate) fn can_open_review_hunk(&self) -> bool {
        self.ai_mode.is_some()
            && self
                .ai_review_proposal()
                .is_some_and(|proposal| proposal.before.is_some())
    }

    pub(crate) fn can_return_to_review(&self) -> bool {
        self.ai_mode.is_none() && self.ai_review_return.is_some()
    }

    pub(crate) fn start_ai_review(&mut self) {
        self.select_review_row(None);
        if self.ai_mode.is_some() {
            self.focus_region(FocusRegion::Review);
        }
    }

    pub(crate) fn finish_ai_review(&mut self) {
        if self.ai_mode.is_some() && self.chat_review_open() {
            self.diff_view = None;
            self.select_review_row(None);
            self.focus_ai_chat();
        }
    }

    pub(crate) fn reconcile_ai_review(&mut self) {
        if self.ai_mode.is_some() && self.chat_review_open() && self.ai_review_proposal().is_none()
        {
            self.finish_ai_review();
        }
    }

    pub(crate) fn ai_review_tab_selected(&self) -> bool {
        self.ai_mode.as_ref().is_some_and(|mode| mode.review_tab)
    }

    pub(crate) fn ai_main_focus_region(&self) -> FocusRegion {
        if self.chat_review_open() && self.ai_review_tab_selected() {
            FocusRegion::Review
        } else {
            FocusRegion::Chat
        }
    }

    pub(crate) fn set_ai_review_tab(&mut self, review: bool) {
        if let Some(mode) = &mut self.ai_mode {
            mode.review_tab = review;
        }
    }

    pub(crate) fn selected_review_row(&self) -> Option<usize> {
        self.ai_mode.as_ref().and_then(|mode| mode.selected_row)
    }

    pub(crate) fn select_review_row(&mut self, row: Option<usize>) {
        if let Some(mode) = &mut self.ai_mode {
            mode.selected_row = row;
        }
    }

    pub(super) fn suspend_ai_review(&mut self) {
        if self.chat_review_open() {
            self.ai_review_return = self.diff_view.take().map(|diff| ReviewReturn {
                diff,
                scroll: self.diff_scroll,
                row: self.selected_review_row(),
            });
            self.diff_scroll = 0;
        }
    }

    pub(super) fn restore_ai_review(&mut self) {
        let Some(saved) = self.ai_review_return.take() else {
            if self.chat_review_open() {
                self.start_ai_review();
            }
            return;
        };
        self.diff_view = None;
        let Some((session_id, proposal_id)) = &saved.diff.review else {
            return;
        };
        let Some(session) = self
            .chat
            .sessions
            .iter()
            .find(|session| &session.id == session_id)
        else {
            self.status = self.t("workbench.review.ended").into();
            return;
        };
        let proposal = session
            .proposals
            .iter()
            .find(|proposal| proposal.id == *proposal_id);
        if let Some(proposal) = proposal {
            if self.tree.root().canonicalize().ok().as_deref() != Some(proposal.workspace.as_path())
            {
                let message = self.t("chat.review.wrong_workspace");
                error_log::record(ErrorSeverity::Low, "workbench.review.restore", message);
                self.status = message.into();
                return;
            }
        }
        let exists = proposal.is_some();
        if self.chat.active_session_id != *session_id {
            self.chat.clear_outgoing_draft();
            self.chat.active_session_id.clone_from(session_id);
            self.chat.show_agent_sessions = false;
        }
        if !exists {
            self.status = self.t("workbench.review.ended").into();
            return;
        }
        self.diff_view = Some(saved.diff);
        self.diff_scroll = saved.scroll;
        self.select_review_row(saved.row);
        self.focus_region(FocusRegion::Review);
    }

    pub(crate) fn open_review_hunk_in_editor(&mut self) {
        if !self.can_open_review_hunk() {
            return;
        }
        let Some(proposal) = self.ai_review_proposal() else {
            return;
        };
        let path = match self.resolve_chat_review_source(proposal) {
            Ok(path) => path,
            Err(message) => {
                error_log::record(ErrorSeverity::Low, "workbench.review.open", &message);
                self.status = message;
                return;
            }
        };
        let Some(diff) = &self.diff_view else { return };
        let model = self.diff_model();
        let requested = self.selected_review_row().unwrap_or_else(|| {
            if model
                .rows
                .get(diff.selected_change)
                .is_some_and(|row| row.kind != crate::vcs::side_by_side::RowKind::Equal)
            {
                diff.selected_change
            } else {
                model.change_starts().first().copied().unwrap_or(0)
            }
        });
        let original_on_left = proposal.before.as_deref() == Some(diff.left.as_str());
        let line = model
            .original_line(requested, original_on_left)
            .unwrap_or(1);
        self.toggle_workbench_mode();
        if self.ai_mode.is_some() {
            return;
        }
        self.open_text_document(path.clone());
        if self.documents.id_for_path(&path) != Some(self.active_document()) {
            let status = self.status.clone();
            self.toggle_workbench_mode();
            self.status = status;
            return;
        }
        self.focus_region(FocusRegion::Editor);
        self.editor_mut().set_cursor_line_col(line - 1, 0);
        self.push_navigation_location();
        self.reveal_cursor = true;
    }

    pub(crate) fn click_ai_review_tab(&mut self, layout: &AiLayout, x: f32, y: f32) -> bool {
        let Some(tabs) = layout.review_tabs else {
            return false;
        };
        let Some(index) = tabs.iter().position(|tab| {
            x >= tab.x && x < tab.x + tab.width && y >= tab.y && y < tab.y + tab.height
        }) else {
            return false;
        };
        self.focus_region(if index == 0 {
            FocusRegion::Chat
        } else {
            FocusRegion::Review
        });
        true
    }

    pub(crate) fn draw_ai_review_tabs(&self, r: &mut Renderer, layout: &AiLayout, lh: f32) {
        let Some(tabs) = layout.review_tabs else {
            return;
        };
        for (index, (tab, key)) in tabs
            .into_iter()
            .zip(["workbench.review.chat", "workbench.review.tab"])
            .enumerate()
        {
            let selected = self.ai_review_tab_selected() == (index == 1);
            let color = if selected {
                Theme::ACTIVE_ROW
            } else {
                Theme::SIDEBAR
            };
            r.set_clip(Some([tab.x, tab.y, tab.x + tab.width, tab.y + tab.height]));
            r.draw_rect(
                tab.x,
                tab.y,
                tab.width,
                tab.height,
                rgb(Theme::color(color, 0x252526)),
            );
            r.draw_text(
                self.t(key),
                tab.x + PAD,
                tab.y + (tab.height - lh) * 0.5,
                rgb(Theme::color(Theme::TEXT, 0xd4d4d4)),
            );
            r.set_clip(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use graphite_chat::{AgentMode, ProposedEdit};

    fn review(before: Option<&str>, after: &str) -> (TempDir, App, PathBuf, u64) {
        let dir = TempDir::new();
        let path = dir.path().join("example.txt");
        if let Some(text) = before {
            std::fs::write(&path, text).unwrap();
        }
        let mut app = App::new_with_options(dir.path().into(), None, true);
        let session = app.chat.active_session_id.clone();
        let turn = app.chat.active_session_mut().start_turn("Review changes");
        app.stage_chat_proposal(
            &session,
            turn,
            ProposedEdit {
                path: path.clone(),
                before: before.map(|text| text.as_bytes().to_vec()),
                after: after.into(),
            },
        )
        .unwrap();
        let id = app.chat.active_session().proposals[0].id;
        app.toggle_workbench_mode();
        app.open_chat_proposal(id);
        (dir, app, path, id)
    }

    #[test]
    fn ai_review_tiles_wide_windows_and_routes_narrow_tabs_and_terminal() {
        let (_dir, mut app, _, _) = review(Some("before\n"), "after\n");
        let wide = app.ai_layout(1200.0, 800.0, 20.0);
        let diff = wide.review.unwrap();
        assert_eq!(wide.chat.width, 400.0);
        assert_eq!(diff.x, 400.0);
        assert_eq!(diff.width, 800.0);
        assert_eq!(diff.y + diff.height, wide.terminal.y);
        assert!(!app.diff_contains_point(1200.0, 800.0, 20.0, 399.0, diff.y));
        assert!(app.diff_contains_point(1200.0, 800.0, 20.0, 400.0, diff.y));
        assert_eq!(app.current_focus_region(), FocusRegion::Review);

        app.cycle_focus_region(false);
        assert_eq!(app.current_focus_region(), FocusRegion::Chat);
        assert!(app.ai_layout(1200.0, 800.0, 20.0).review.is_some());
        let narrow = app.ai_layout(500.0, 800.0, 20.0);
        assert!(narrow.review.is_none());
        let [chat_tab, review_tab] = narrow.review_tabs.unwrap();
        assert_eq!(narrow.chat.y, chat_tab.y + chat_tab.height);
        assert!(app.click_ai_review_tab(&narrow, review_tab.x + 1.0, review_tab.y + 1.0));
        let narrow = app.ai_layout(500.0, 800.0, 20.0);
        assert_eq!(narrow.chat.height, 0.0);
        assert_eq!(narrow.review.unwrap().width, 500.0);
        assert_eq!(app.current_focus_region(), FocusRegion::Review);
        let accessible = app.grouped_accessibility_tree(500.0, 800.0, 20.0);
        assert_eq!(accessible.focus, crate::app::accessibility::AI_REVIEW_NODE);
        assert!(!accessible
            .nodes
            .iter()
            .any(|(id, _)| *id == crate::app::accessibility::AI_CHAT_NODE));
        assert!(accessible.nodes.iter().any(|(id, node)| {
            *id == crate::app::accessibility::AI_OPEN_HUNK_NODE
                && node.supports_action(Action::Click)
        }));

        let (_, _output) = app.spawn_virtual_terminal("Review layout test").unwrap();
        app.toggle_terminal();
        let narrow = app.ai_layout(500.0, 800.0, 20.0);
        let diff = narrow.review.unwrap();
        assert_eq!(diff.y + diff.height, narrow.terminal.y);
        assert!(!app.diff_contains_point(500.0, 800.0, 20.0, 250.0, narrow.terminal.y));
        assert_eq!(app.current_focus_region(), FocusRegion::Terminal);
        app.toggle_terminal();
        assert_eq!(app.current_focus_region(), FocusRegion::Review);
        assert!(app.ai_layout(500.0, 800.0, 20.0).review.is_some());

        app.close_diff_view();
        let resolved = app.ai_layout(500.0, 800.0, 20.0);
        assert!(resolved.review.is_none() && resolved.review_tabs.is_none());
        assert_eq!(resolved.chat.width, 500.0);
        assert_eq!(app.current_focus_region(), FocusRegion::Chat);
    }

    #[test]
    fn editor_jump_and_return_preserve_the_diff_and_protect_manual_edits() {
        let before: String = (0..150).map(|line| format!("line {line}\n")).collect();
        let after = before
            .replace("line 8\n", "change eight\n")
            .replace("line 100\n", "change hundred\n");
        let (_dir, mut app, path, id) = review(Some(&before), &after);
        let session = app.chat.active_session_id.clone();
        app.chat.input = "follow-up draft".into();
        app.chat.scroll = 3;
        app.run_ui_command("graphite.diff.toggleLayout");
        let diff = app.diff_view.as_mut().unwrap();
        diff.collapse_unchanged = true;
        diff.selected_change = 100;
        app.diff_visible_rows.set(3);
        app.scroll_diff_to_row(100);
        let scroll = app.diff_scroll;
        assert!(scroll > 0);
        app.run_ui_command("graphite.diff.openHunkInEditor");
        assert!(app.can_return_to_review());
        assert!(!app.chat_review_open());
        assert_eq!(app.editor().cursor_line_col(), (100, 0));
        app.editor_mut().insert("manual ");
        app.run_ui_command("graphite.workbench.returnToReview");

        let restored = app.diff_view.as_ref().unwrap();
        assert_eq!(restored.review, Some((session.clone(), id)));
        assert!(restored.inline && restored.collapse_unchanged);
        assert_eq!(restored.selected_change, 100);
        assert_eq!(app.diff_scroll, scroll);
        assert_eq!(app.chat.active_session_id, session);
        assert_eq!(app.chat.input, "follow-up draft");
        assert_eq!(app.chat.scroll, 3);
        app.decide_chat_review(true);
        assert!(app.chat_review_open());
        assert!(app.chat_review_notice().is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        let document = app.documents.id_for_path(&path).unwrap();
        assert!(app.documents[document]
            .buffer
            .text()
            .contains("manual line 100"));
        assert!(app.documents[document].buffer.is_dirty());
    }

    #[test]
    fn click_in_inline_diff_jumps_to_the_original_line_and_restores_selection() {
        let (_dir, mut app, _, _) = review(Some("one\ntwo\nthree\n"), "ONE\ntwo\nTHREE\n");
        app.run_ui_command("graphite.diff.toggleLayout");
        let area = app.ai_layout(1200.0, 800.0, 20.0).review.unwrap();
        let first_row = area.y + 20.0 * (1.5 * 3.0 + 1.2);
        assert!(app.click_diff_view(
            area.x + 20.0,
            first_row + 20.0 * 4.0 + 1.0,
            1200.0,
            800.0,
            20.0,
            8.0
        ));
        assert_eq!(app.selected_review_row(), Some(2));
        app.open_review_hunk_in_editor();
        assert_eq!(app.editor().cursor_line_col(), (2, 0));
        app.toggle_workbench_mode();
        assert_eq!(app.selected_review_row(), Some(2));
    }

    #[test]
    fn returning_to_an_ended_proposal_recovers_its_conversation() {
        let (_dir, mut app, _, id) = review(Some("before\n"), "after\n");
        let session = app.chat.active_session_id.clone();
        app.open_review_hunk_in_editor();
        assert!(app.discard_chat_proposal(id));
        app.chat.new_session(AgentMode::Ask);
        assert_ne!(app.chat.active_session_id, session);

        app.run_ui_command("graphite.workbench.returnToReview");
        assert_eq!(app.chat.active_session_id, session);
        assert!(!app.chat_review_open());
        assert_eq!(app.current_focus_region(), FocusRegion::Chat);
        assert_eq!(app.status, app.t("workbench.review.ended"));
    }

    #[test]
    fn new_files_and_unavailable_originals_keep_review_open() {
        let (_dir, mut app, path, _) = review(None, "new file\n");
        assert!(!app.can_open_review_hunk());
        app.open_review_hunk_in_editor();
        assert!(app.chat_review_open() && app.ai_mode.is_some());
        assert!(!path.exists());

        let (_dir, mut app, path, _) = review(Some("before\n"), "after\n");
        std::fs::remove_file(path).unwrap();
        app.open_review_hunk_in_editor();
        assert!(app.chat_review_open() && app.ai_mode.is_some());
        assert!(!app.can_return_to_review());
    }
}
