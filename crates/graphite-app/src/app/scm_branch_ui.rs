//! Amend, create a branch, merge, rebase, and finishing or abandoning a stopped merge or rebase.
//! The git work lives in `scm_branches.rs`; this file chooses the branch (reusing the branch
//! picker), asks for a new branch name, and starts the operation on a worker thread.

use super::scm_remote_ui::RemoteOp;
use super::*;

/// What choosing a branch in the branch picker does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BranchPickerAction {
    /// Switch to the branch (the original behavior).
    #[default]
    Checkout,
    /// Merge the branch into the current one.
    Merge,
    /// Rebase the current branch onto the branch.
    Rebase,
}

impl BranchPickerAction {
    /// Locale key of the picker heading.
    pub(crate) fn prompt_key(self) -> &'static str {
        match self {
            Self::Checkout => "scm.branch.prompt",
            Self::Merge => "scm.branch.prompt_merge",
            Self::Rebase => "scm.branch.prompt_rebase",
        }
    }
}

/// The branch name being typed for **Git: Create Branch**.
#[derive(Clone, Debug, Default)]
pub(crate) struct BranchNamePrompt {
    pub(crate) value: String,
}

impl App {
    /// Opens the branch picker for `action`.
    pub(crate) fn open_branch_picker_for(&mut self, action: BranchPickerAction) {
        self.open_branch_picker();
        self.branch_picker_action = action;
    }

    /// Enter in the branch picker: do what the picker was opened for.
    pub(crate) fn confirm_branch_picker(&mut self) {
        let action = self.branch_picker_action;
        match action {
            BranchPickerAction::Checkout => self.checkout_selected_branch(),
            BranchPickerAction::Merge | BranchPickerAction::Rebase => {
                let Some(branch) = self
                    .scm_snapshot
                    .branches
                    .get(self.branch_selected)
                    .cloned()
                else {
                    return;
                };
                self.branch_picker = false;
                self.branch_picker_action = BranchPickerAction::Checkout;
                if action == BranchPickerAction::Merge {
                    self.run_scm_remote(RemoteOp::Merge(branch));
                } else {
                    self.run_scm_remote(RemoteOp::Rebase(branch));
                }
            }
        }
    }

    /// Closes the branch picker and forgets what it was opened for.
    pub(crate) fn close_branch_picker(&mut self) {
        self.branch_picker = false;
        self.branch_picker_action = BranchPickerAction::Checkout;
    }

    pub(crate) fn open_create_branch_prompt(&mut self) {
        self.branch_name_prompt = Some(BranchNamePrompt::default());
    }

    /// Routes keys to the branch name prompt while it is open. Returns whether it took the key.
    pub(crate) fn handle_branch_name_prompt_key(&mut self, key: &Key, text: Option<&str>) -> bool {
        let Some(prompt) = self.branch_name_prompt.as_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => self.branch_name_prompt = None,
            Key::Named(NamedKey::Enter) => {
                let name = prompt.value.trim().to_owned();
                if name.is_empty() {
                    self.status = self.t("scm.prompt.branch_empty").into();
                } else {
                    self.branch_name_prompt = None;
                    self.run_scm_remote(RemoteOp::CreateBranch(name));
                }
            }
            Key::Named(NamedKey::Backspace) => {
                prompt.value.pop();
            }
            _ => {
                if let Some(text) = text.filter(|text| !text.chars().any(char::is_control)) {
                    prompt.value.push_str(text);
                }
            }
        }
        true
    }

    /// Amends the last commit. Text in the commit message field becomes the new message;
    /// otherwise the message is kept.
    pub(crate) fn amend_last_commit(&mut self) {
        let message = self.scm_commit_message.trim().to_owned();
        let message = (!message.is_empty()).then_some(message);
        let used_message = message.is_some();
        self.run_scm_remote(RemoteOp::Amend(message));
        if used_message {
            self.scm_commit_message.clear();
        }
    }

    pub(crate) fn abort_git_operation(&mut self) {
        self.run_scm_remote(RemoteOp::AbortOperation);
    }

    pub(crate) fn continue_git_operation(&mut self) {
        self.run_scm_remote(RemoteOp::ContinueOperation);
    }

    pub(crate) fn draw_branch_name_prompt(&self, renderer: &mut Renderer) {
        let Some(prompt) = &self.branch_name_prompt else {
            return;
        };
        let (width, height) = renderer.size();
        let cw = renderer.char_width();
        let panel_width = (width as f32 * 0.72)
            .clamp(360.0, 700.0)
            .min((width as f32 - 24.0).max(0.0));
        let panel_height = 120.0;
        let x = ((width as f32 - panel_width) * 0.5).max(12.0);
        let y = ((height as f32 - panel_height) * 0.5).max(12.0);
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        renderer.draw_rect(x, y, panel_width, panel_height, border);
        renderer.draw_rect(
            x + 1.0,
            y + 1.0,
            (panel_width - 2.0).max(0.0),
            panel_height - 2.0,
            bg,
        );
        renderer.draw_text(self.t("scm.prompt.new_branch"), x + 16.0, y + 18.0, text);
        let max_chars = ((panel_width - 32.0) / cw.max(1.0)).floor() as usize;
        let input = Self::help_menu_visible_label(
            &format!("{}▏", prompt.value),
            cw,
            (max_chars as f32 * cw).max(0.0),
        );
        renderer.draw_rect(x + 12.0, y + 48.0, panel_width - 24.0, 28.0, border);
        renderer.draw_text(&input, x + 18.0, y + 54.0, text);
        renderer.draw_text(
            self.t("scm.prompt.new_branch_hint"),
            x + 16.0,
            y + 90.0,
            dim,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn app(dir: &TempDir) -> App {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.scm_snapshot.branches = vec!["main".into(), "topic".into()];
        app
    }

    #[test]
    fn the_picker_remembers_what_it_was_opened_for_and_resets() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.open_branch_picker_for(BranchPickerAction::Merge);
        assert!(app.branch_picker);
        assert_eq!(app.branch_picker_action, BranchPickerAction::Merge);
        app.close_branch_picker();
        assert!(!app.branch_picker);
        assert_eq!(app.branch_picker_action, BranchPickerAction::Checkout);
        // The plain picker (the branch button) is still checkout.
        app.open_branch_picker();
        assert_eq!(app.branch_picker_action, BranchPickerAction::Checkout);
    }

    #[test]
    fn each_action_has_its_own_heading() {
        let keys: Vec<_> = [
            BranchPickerAction::Checkout,
            BranchPickerAction::Merge,
            BranchPickerAction::Rebase,
        ]
        .map(BranchPickerAction::prompt_key)
        .into();
        assert_eq!(keys.len(), 3);
        assert!(keys.windows(2).all(|pair| pair[0] != pair[1]));
        let translator = graphite_i18n::Translator::english();
        for key in keys {
            assert!(translator.has(key), "{key} is not translated");
        }
    }

    #[test]
    fn merge_and_rebase_refuse_to_run_over_unsaved_edits() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.editor_mut().insert("unsaved");
        app.status.clear();
        app.run_scm_remote(RemoteOp::Merge("topic".into()));
        assert_eq!(app.status, app.t("scm.error.unsaved_for_git_operation"));
        app.status.clear();
        app.run_scm_remote(RemoteOp::Rebase("topic".into()));
        assert_eq!(app.status, app.t("scm.error.unsaved_for_git_operation"));
        assert!(RemoteOp::ContinueOperation.needs_saved_documents());
        assert!(!RemoteOp::Amend(None).needs_saved_documents());
        assert!(!RemoteOp::CreateBranch("x".into()).needs_saved_documents());
        assert!(!RemoteOp::AbortOperation.needs_saved_documents());
    }

    #[test]
    fn the_branch_name_prompt_edits_cancels_and_rejects_blank_names() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        assert!(!app.handle_branch_name_prompt_key(&Key::Named(NamedKey::Enter), None));
        app.open_create_branch_prompt();
        for text in ["fe", "at", "x"] {
            assert!(app.handle_branch_name_prompt_key(&Key::Character("a".into()), Some(text)));
        }
        assert_eq!(app.branch_name_prompt.as_ref().unwrap().value, "featx");
        app.handle_branch_name_prompt_key(&Key::Named(NamedKey::Backspace), None);
        assert_eq!(app.branch_name_prompt.as_ref().unwrap().value, "feat");
        // Control characters are not typed into the name.
        app.handle_branch_name_prompt_key(&Key::Character("a".into()), Some("\u{7}"));
        assert_eq!(app.branch_name_prompt.as_ref().unwrap().value, "feat");

        app.handle_branch_name_prompt_key(&Key::Named(NamedKey::Escape), None);
        assert!(app.branch_name_prompt.is_none());

        app.open_create_branch_prompt();
        app.handle_branch_name_prompt_key(&Key::Character(" ".into()), Some("  "));
        app.handle_branch_name_prompt_key(&Key::Named(NamedKey::Enter), None);
        assert!(
            app.branch_name_prompt.is_some(),
            "a blank name keeps the prompt open"
        );
        assert_eq!(app.status, app.t("scm.prompt.branch_empty"));
    }

    #[test]
    fn amend_uses_and_clears_the_commit_message_only_when_there_is_one() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.scm_commit_message = "  new words ".into();
        app.amend_last_commit();
        assert!(
            app.scm_commit_message.is_empty(),
            "the message was consumed"
        );
        app.scm_commit_message = "   ".into();
        app.amend_last_commit();
        assert_eq!(app.scm_commit_message, "   ", "a blank field is left alone");
    }

    #[test]
    fn every_git_command_is_registered_and_listed_under_the_git_prefix() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.open_command_palette();
        let titles: Vec<String> = app
            .command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .filter(|item| item.title.starts_with("Git: "))
            .map(|item| item.title.clone())
            .collect();
        for expected in [
            "Git: Pull",
            "Git: Push",
            "Git: Fetch",
            "Git: Sync",
            "Git: Stash",
            "Git: View History",
            "Git: Create Pull Request",
            "Git: More Actions...",
            "Git: Amend Last Commit",
            "Git: Create Branch...",
            "Git: Merge Branch...",
            "Git: Rebase onto Branch...",
            "Git: Abort Merge or Rebase",
            "Git: Continue Merge or Rebase",
        ] {
            assert_eq!(
                titles.iter().filter(|title| *title == expected).count(),
                1,
                "{expected} should be listed once; found {titles:?}"
            );
        }
        // The SCM view's "…" button filters the palette by this prefix.
        app.open_scm_more_menu();
        assert_eq!(app.command_palette.as_ref().unwrap().query, "Git: ");
    }

    #[test]
    fn the_merge_and_rebase_commands_open_the_picker_in_their_mode() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.run_ui_command("graphite.scm.merge");
        assert!(app.branch_picker);
        assert_eq!(app.branch_picker_action, BranchPickerAction::Merge);
        app.run_ui_command("graphite.branch.close");
        assert!(!app.branch_picker);
        app.run_ui_command("graphite.scm.rebase");
        assert_eq!(app.branch_picker_action, BranchPickerAction::Rebase);
        app.run_ui_command("graphite.branch.close");
        assert_eq!(app.branch_picker_action, BranchPickerAction::Checkout);
    }

    #[test]
    fn choosing_a_branch_for_a_merge_starts_the_merge_not_a_checkout() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.run_ui_command("graphite.scm.merge");
        app.branch_selected = 1;
        app.status.clear();
        app.run_ui_command("graphite.scm.checkout");
        assert!(!app.branch_picker, "the picker closes");
        assert_eq!(
            app.branch_picker_action,
            BranchPickerAction::Checkout,
            "and resets"
        );
        // No event loop in tests: reaching this message shows the merge was handed to
        // `run_scm_remote`; a checkout would have reported something else.
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));
    }

    #[test]
    fn creating_a_branch_asks_for_a_name_then_hands_it_to_git() {
        let dir = TempDir::new();
        let mut app = app(&dir);
        app.run_ui_command("graphite.scm.createBranch");
        assert!(app.branch_name_prompt.is_some());
        app.handle_branch_name_prompt_key(&Key::Character("x".into()), Some("feature/x"));
        app.status.clear();
        app.handle_branch_name_prompt_key(&Key::Named(NamedKey::Enter), None);
        assert!(
            app.branch_name_prompt.is_none(),
            "the prompt closes on Enter"
        );
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));
    }
}
