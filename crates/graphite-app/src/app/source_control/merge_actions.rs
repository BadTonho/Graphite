//! Block actions of the merge editor: deciding a conflict, comparing with the base and handing
//! the result over to a normal editor.

use super::merge_editor::MergeEditor;
use super::*;
use crate::merge::{MergeState, Side};

fn has_base(editor: &MergeEditor) -> bool {
    editor
        .state
        .parsed()
        .conflicts()
        .any(|block| block.base.is_some())
}

impl App {
    /// Applies `action` to the active conflict. With `advance`, a decision moves on to the next
    /// unresolved conflict so a file can be settled with one key per conflict.
    pub(crate) fn merge_edit(
        &mut self,
        advance: bool,
        action: impl FnOnce(&mut MergeState, usize) -> bool,
    ) {
        let Some(editor) = self.merge_editor.as_mut() else {
            return;
        };
        let Some(active) = editor.active else {
            self.status = self.t("merge.none_active").into();
            return;
        };
        if !action(&mut editor.state, active) {
            self.status = self.t("merge.none_active").into();
            return;
        }
        editor.refresh();
        editor.close_armed = false;
        if advance {
            editor.goto_next_unresolved();
        }
        let left = editor.state.unresolved_count();
        self.status = if left == 0 {
            self.t("merge.all_resolved").into()
        } else {
            self.tf("merge.unresolved_count", &[("count", &left)])
        };
    }

    /// Switches the input the compare commands start from between current and incoming.
    pub(crate) fn merge_toggle_focused_input(&mut self) {
        let Some(editor) = self.merge_editor.as_mut() else {
            return;
        };
        editor.focused_input = 1 - editor.focused_input;
        let key = if editor.focused_input == 0 {
            "merge.pane.current"
        } else {
            "merge.pane.incoming"
        };
        self.status = self.tf("merge.focused_input", &[("input", &self.t(key))]);
    }

    fn merge_show_diff(&mut self, input: &str, left: String, right: String, right_name: String) {
        self.diff_view = Some(super::scm_ui::DiffView {
            title: self.tf("merge.compare_title", &[("input", &input)]),
            left_name: self.t("merge.pane.base").to_owned(),
            right_name,
            left,
            right,
            inline: false,
        });
        self.status = self.t("diff.opened").into();
    }

    /// Compares the base with one input (0 current, 1 incoming) in the diff view.
    pub(crate) fn merge_compare_with_base(&mut self, input: usize) {
        let Some(editor) = self.merge_editor.as_ref() else {
            return;
        };
        if !has_base(editor) {
            self.status = self.t("merge.no_base").into();
            return;
        }
        let (side, key) = if input == 0 {
            (Side::Current, "merge.pane.current")
        } else {
            (Side::Incoming, "merge.pane.incoming")
        };
        let base = editor.state.side_text(Side::Base);
        let other = editor.state.side_text(side);
        let name = self.t(key).to_owned();
        self.merge_show_diff(&name, base, other, name.clone());
    }

    /// Compares the base with the result as it stands.
    pub(crate) fn merge_open_base(&mut self) {
        let Some(editor) = self.merge_editor.as_ref() else {
            return;
        };
        if !has_base(editor) {
            self.status = self.t("merge.no_base").into();
            return;
        }
        let base = editor.state.side_text(Side::Base);
        let result = editor.state.render();
        let name = self.t("merge.pane.result").to_owned();
        self.merge_show_diff(&name, base, result, name.clone());
    }

    /// Closes the merge editor and opens the file in a normal editor holding the result so far.
    /// Conflicts that were not decided keep their markers, so the rest can be finished by hand.
    pub(crate) fn merge_open_result(&mut self) {
        let Some(editor) = self.merge_editor.take() else {
            return;
        };
        let result = editor.state.render();
        self.open_file(editor.path);
        if self.editor().text() != result {
            let length = self.editor().len_chars();
            self.editor_mut().set_selection(0, length);
            self.editor_mut().insert(&result);
            self.editor_mut().set_selection(0, 0);
        }
        self.status = self.t("merge.result_opened").into();
    }
}
