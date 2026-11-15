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
    /// Choose the commits and actions of a rebase onto the branch, in an editable plan.
    RebaseInteractive,
    /// Delete the local branch (git refuses it when it is not fully merged).
    Delete,
    /// Delete the local branch even if it is not fully merged.
    DeleteForce,
}

impl BranchPickerAction {
    /// Locale key of the picker heading.
    pub(crate) fn prompt_key(self) -> &'static str {
        match self {
            Self::Checkout => "scm.branch.prompt",
            Self::Merge => "scm.branch.prompt_merge",
            Self::Rebase => "scm.branch.prompt_rebase",
            Self::RebaseInteractive => "scm.branch.prompt_rebase_interactive",
            Self::Delete => "scm.branch.prompt_delete",
            Self::DeleteForce => "scm.branch.prompt_delete_force",
        }
    }
}

/// An interactive rebase whose plan is open for editing.
#[derive(Clone, Debug)]
pub(crate) struct InteractiveRebase {
    pub(crate) onto: String,
    pub(crate) path: PathBuf,
    pub(crate) commits: Vec<crate::scm_branches::Commit>,
}

/// What the name being typed will be given to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum NameTarget {
    /// A new branch at HEAD (**Git: Create Branch**).
    #[default]
    Branch,
    /// A new branch at the commit with this id.
    BranchAt(String),
    /// A new tag on the commit with this id.
    TagAt(String),
    /// A new tag on HEAD.
    TagHead,
    /// The new name of the checked out branch.
    RenameBranch,
}

/// The branch or tag name being typed.
#[derive(Clone, Debug, Default)]
pub(crate) struct BranchNamePrompt {
    pub(crate) value: String,
    pub(crate) target: NameTarget,
}

impl App {
    /// The branches the picker lists for its current action. Checkout offers the local branches and
    /// then the remote-tracking ones (which become tracking branches);
    /// deleting offers the local ones except the checked out branch; merging and rebasing offer
    /// those too plus the remote-tracking branches (`origin/main`).
    pub(crate) fn branch_picker_entries(&self) -> Vec<String> {
        let snapshot = &self.scm_snapshot;
        let others = |list: &[String]| -> Vec<String> {
            list.iter()
                .filter(|name| Some(*name) != snapshot.branch.as_ref())
                .cloned()
                .collect()
        };
        match self.branch_picker_action {
            BranchPickerAction::Checkout => {
                let mut entries = snapshot.branches.clone();
                entries.extend(snapshot.remote_branches.iter().cloned());
                entries
            }
            BranchPickerAction::Delete | BranchPickerAction::DeleteForce => {
                others(&snapshot.branches)
            }
            BranchPickerAction::Merge
            | BranchPickerAction::Rebase
            | BranchPickerAction::RebaseInteractive => {
                let mut entries = others(&snapshot.branches);
                entries.extend(snapshot.remote_branches.iter().cloned());
                entries
            }
        }
    }

    /// Opens the branch picker for `action`.
    pub(crate) fn open_branch_picker_for(&mut self, action: BranchPickerAction) {
        self.open_branch_picker();
        self.branch_picker_action = action;
        // The list is not the plain branch list any more, so start from its top.
        if action != BranchPickerAction::Checkout {
            self.branch_selected = 0;
        }
    }

    /// Enter in the branch picker: do what the picker was opened for.
    pub(crate) fn confirm_branch_picker(&mut self) {
        let action = self.branch_picker_action;
        match action {
            BranchPickerAction::Checkout => self.checkout_selected_branch(),
            BranchPickerAction::RebaseInteractive => {
                let Some(branch) = self
                    .branch_picker_entries()
                    .get(self.branch_selected)
                    .cloned()
                else {
                    return;
                };
                self.close_branch_picker();
                self.begin_interactive_rebase(branch);
            }
            _ => {
                let Some(branch) = self
                    .branch_picker_entries()
                    .get(self.branch_selected)
                    .cloned()
                else {
                    return;
                };
                self.branch_picker = false;
                self.branch_picker_action = BranchPickerAction::Checkout;
                self.run_scm_remote(match action {
                    BranchPickerAction::Merge => RemoteOp::Merge(branch),
                    BranchPickerAction::Rebase => RemoteOp::Rebase(branch),
                    BranchPickerAction::Delete => RemoteOp::DeleteBranch(branch, false),
                    _ => RemoteOp::DeleteBranch(branch, true),
                });
            }
        }
    }

    /// Closes the branch picker and forgets what it was opened for.
    pub(crate) fn close_branch_picker(&mut self) {
        self.branch_picker = false;
        self.branch_picker_action = BranchPickerAction::Checkout;
    }

    /// Writes the plan of an interactive rebase onto `onto` to a file in the profile and opens it
    /// for editing; **Git: Run Interactive Rebase** then runs it.
    pub(crate) fn begin_interactive_rebase(&mut self, onto: String) {
        let root = self.scm_workspace_root();
        let commits = match crate::scm_branches::commits_to_rebase(&root, &onto) {
            Ok(commits) => commits,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        if commits.is_empty() {
            self.status = self.tf("scm.rebase.nothing", &[("branch", &onto)]);
            return;
        }
        let path = self.profile_root.join("git-rebase-plan.txt");
        if let Err(error) = std::fs::create_dir_all(&self.profile_root)
            .and_then(|_| std::fs::write(&path, crate::scm_branches::format_plan(&commits)))
        {
            self.status = error.to_string();
            return;
        }
        self.interactive_rebase = Some(InteractiveRebase {
            onto,
            path: path.clone(),
            commits,
        });
        // A plan left open from an earlier attempt is reloaded from disk.
        if let Some(id) = self.documents.id_for_path(&path) {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let _ = self.documents[id].buffer.reload_text(&text);
        }
        self.open_file(path);
        self.status = self.t("scm.rebase.plan_opened").into();
    }

    /// Validates the plan in the editor and starts the rebase it describes.
    pub(crate) fn run_interactive_rebase(&mut self) {
        let Some(plan) = self.interactive_rebase.clone() else {
            self.status = self.t("scm.rebase.no_plan").into();
            return;
        };
        let text = match self.documents.id_for_path(&plan.path) {
            Some(id) => self.documents[id].buffer.text(),
            None => std::fs::read_to_string(&plan.path).unwrap_or_default(),
        };
        let todo = match crate::scm_branches::plan_to_todo(&text, &plan.commits) {
            Ok(todo) => todo,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        // The plan itself is edited, so only the other documents have to be saved.
        let plan_id = self.documents.id_for_path(&plan.path);
        let others_unsaved = self
            .documents
            .iter()
            .any(|tab| Some(tab.id) != plan_id && tab.is_dirty());
        if others_unsaved {
            self.status = self.t("scm.error.unsaved_for_git_operation").into();
            return;
        }
        self.interactive_rebase = None;
        self.run_scm_remote(RemoteOp::RebaseInteractive(plan.onto, todo));
    }

    /// Forgets the plan without running it.
    pub(crate) fn cancel_interactive_rebase(&mut self) {
        if self.interactive_rebase.take().is_some() {
            self.status = self.t("scm.rebase.cancelled").into();
        } else {
            self.status = self.t("scm.rebase.no_plan").into();
        }
    }

    pub(crate) fn open_create_branch_prompt(&mut self) {
        self.open_name_prompt(NameTarget::Branch);
    }

    pub(crate) fn open_name_prompt(&mut self, target: NameTarget) {
        self.branch_name_prompt = Some(BranchNamePrompt {
            value: String::new(),
            target,
        });
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
                let target = prompt.target.clone();
                if name.is_empty() {
                    self.status = self.t("scm.prompt.branch_empty").into();
                } else {
                    self.branch_name_prompt = None;
                    self.run_scm_remote(match target {
                        NameTarget::Branch => RemoteOp::CreateBranch(name),
                        NameTarget::BranchAt(hash) => RemoteOp::CreateBranchAt(name, hash),
                        NameTarget::TagAt(hash) => RemoteOp::CreateTag(name, hash),
                        NameTarget::TagHead => RemoteOp::CreateTag(name, "HEAD".into()),
                        NameTarget::RenameBranch => RemoteOp::RenameBranch(name),
                    });
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
        if !message.is_empty() {
            self.record_scm_commit_history(&message);
        }
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
        let title = match prompt.target {
            NameTarget::TagAt(_) | NameTarget::TagHead => "scm.prompt.new_tag",
            NameTarget::RenameBranch => "scm.prompt.rename_branch",
            _ => "scm.prompt.new_branch",
        };
        renderer.draw_text(self.t(title), x + 16.0, y + 18.0, text);
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
            "Git: Delete Branch...",
            "Git: Delete Branch (Force)...",
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
        app.open_scm_git_commands();
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

    fn app_with_remote_branches(dir: &TempDir) -> App {
        let mut app = app(dir);
        app.scm_snapshot.branch = Some("main".into());
        app.scm_snapshot.branches = vec!["main".into(), "topic".into()];
        app.scm_snapshot.remote_branches = vec!["origin/main".into(), "origin/topic".into()];
        app
    }

    #[test]
    fn each_picker_action_lists_the_branches_that_make_sense_for_it() {
        let dir = TempDir::new();
        let mut app = app_with_remote_branches(&dir);
        app.branch_picker_action = BranchPickerAction::Checkout;
        assert_eq!(
            app.branch_picker_entries(),
            ["main", "topic", "origin/main", "origin/topic"],
            "local branches (current included), then the remote ones"
        );
        for action in [BranchPickerAction::Delete, BranchPickerAction::DeleteForce] {
            app.branch_picker_action = action;
            assert_eq!(
                app.branch_picker_entries(),
                ["topic"],
                "the checked out branch is not offered"
            );
        }
        for action in [BranchPickerAction::Merge, BranchPickerAction::Rebase] {
            app.branch_picker_action = action;
            assert_eq!(
                app.branch_picker_entries(),
                ["topic", "origin/main", "origin/topic"],
                "local branches except the current one, then the remote ones"
            );
        }
    }

    #[test]
    fn choosing_an_entry_uses_the_filtered_list_not_the_raw_branch_list() {
        let dir = TempDir::new();
        let mut app = app_with_remote_branches(&dir);
        app.run_ui_command("graphite.scm.merge");
        assert_eq!(app.branch_selected, 0, "the list starts at its first entry");
        app.branch_selected = 1; // origin/main, because the current branch is hidden
        app.status.clear();
        app.run_ui_command("graphite.scm.checkout");
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));
        assert!(!app.branch_picker);

        // The selection can go down to the last remote entry and no further.
        app.run_ui_command("graphite.scm.rebase");
        for _ in 0..10 {
            app.run_ui_command("graphite.branch.next");
        }
        assert_eq!(app.branch_selected, 2, "topic, origin/main, origin/topic");
    }

    #[test]
    fn the_delete_commands_open_the_picker_in_their_mode() {
        let dir = TempDir::new();
        let mut app = app_with_remote_branches(&dir);
        app.run_ui_command("graphite.scm.deleteBranch");
        assert_eq!(app.branch_picker_action, BranchPickerAction::Delete);
        app.run_ui_command("graphite.branch.close");
        app.run_ui_command("graphite.scm.deleteBranchForce");
        assert_eq!(app.branch_picker_action, BranchPickerAction::DeleteForce);
        let translator = graphite_i18n::Translator::english();
        assert!(translator.has(BranchPickerAction::Delete.prompt_key()));
        assert!(translator.has(BranchPickerAction::DeleteForce.prompt_key()));
        assert_ne!(
            BranchPickerAction::Delete.prompt_key(),
            BranchPickerAction::DeleteForce.prompt_key()
        );
    }

    #[test]
    fn deleting_never_needs_saved_documents() {
        assert!(!RemoteOp::DeleteBranch("topic".into(), false).needs_saved_documents());
        assert!(!RemoteOp::DeleteBranch("topic".into(), true).needs_saved_documents());
    }

    /// A workspace that is a git repository with `main` and a `work` branch of three commits.
    fn repository_app(dir: &TempDir) -> App {
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(dir.path())
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "core.autocrlf", "false"]);
        std::fs::write(dir.path().join("base.txt"), "base\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "base"]);
        git(&["switch", "-q", "-c", "work"]);
        for name in ["a", "b", "c"] {
            std::fs::write(dir.path().join(format!("{name}.txt")), name).unwrap();
            git(&["add", "-A"]);
            git(&["commit", "-q", "-m", &format!("add {name}")]);
        }
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.profile_root = dir.path().join("profile");
        app
    }

    #[test]
    fn the_interactive_picker_lists_like_rebase_and_has_its_own_heading() {
        let dir = TempDir::new();
        let mut app = app_with_remote_branches(&dir);
        app.branch_picker_action = BranchPickerAction::RebaseInteractive;
        assert_eq!(
            app.branch_picker_entries(),
            ["topic", "origin/main", "origin/topic"]
        );
        let translator = graphite_i18n::Translator::english();
        assert!(translator.has(BranchPickerAction::RebaseInteractive.prompt_key()));
        app.run_ui_command("graphite.scm.interactiveRebase");
        assert_eq!(
            app.branch_picker_action,
            BranchPickerAction::RebaseInteractive
        );
    }

    #[test]
    fn starting_an_interactive_rebase_opens_an_editable_plan_of_the_commits() {
        let dir = TempDir::new();
        let mut app = repository_app(&dir);
        app.begin_interactive_rebase("main".into());
        let plan = app.interactive_rebase.clone().expect("a plan is pending");
        assert_eq!(plan.onto, "main");
        assert_eq!(plan.commits.len(), 3);
        let text = app.editor().text();
        assert_eq!(
            text.lines()
                .take(3)
                .filter(|line| line.starts_with("pick "))
                .count(),
            3
        );
        assert!(text.contains("add a") && text.contains("add c"));
        assert_eq!(app.status, app.t("scm.rebase.plan_opened"));
        assert!(plan.path.ends_with("git-rebase-plan.txt"));
    }

    #[test]
    fn there_is_nothing_to_plan_without_commits_or_with_a_bad_base() {
        let dir = TempDir::new();
        let mut app = repository_app(&dir);
        app.begin_interactive_rebase("work".into());
        assert!(app.interactive_rebase.is_none());
        assert!(
            app.status.contains("no commits to rebase onto work"),
            "{}",
            app.status
        );
        app.begin_interactive_rebase("--all".into());
        assert!(app.interactive_rebase.is_none());
        assert!(
            app.status.contains("not a valid branch name"),
            "{}",
            app.status
        );
    }

    #[test]
    fn running_validates_the_edited_plan_and_hands_the_todo_to_git() {
        let dir = TempDir::new();
        let mut app = repository_app(&dir);
        app.run_ui_command("graphite.scm.runInteractiveRebase");
        assert_eq!(app.status, app.t("scm.rebase.no_plan"));

        app.begin_interactive_rebase("main".into());
        // An invalid edit is reported and the plan stays open.
        let id = app.active_document();
        let _ = app.documents[id].buffer.reload_text("reword abc\n");
        app.run_ui_command("graphite.scm.runInteractiveRebase");
        assert!(
            app.status.contains("unknown action 'reword'"),
            "{}",
            app.status
        );
        assert!(app.interactive_rebase.is_some());

        // A valid plan is accepted; with no event loop in tests it stops at that point.
        let first = app.interactive_rebase.as_ref().unwrap().commits[0]
            .hash
            .clone();
        let _ = app.documents[id]
            .buffer
            .reload_text(&format!("pick {first} add a\n"));
        app.run_ui_command("graphite.scm.runInteractiveRebase");
        assert_eq!(app.status, app.t("scm.error.actions_need_loop"));
        assert!(app.interactive_rebase.is_none(), "the plan is consumed");
    }

    #[test]
    fn other_unsaved_documents_block_the_run_but_the_plan_itself_does_not() {
        let dir = TempDir::new();
        let mut app = repository_app(&dir);
        app.begin_interactive_rebase("main".into());
        let plan_id = app.active_document();
        app.editor_mut().insert("# edited\n");
        assert!(
            app.documents[plan_id].is_dirty(),
            "the plan has unsaved edits"
        );
        let other = dir.path().join("other.txt");
        std::fs::write(&other, "x").unwrap();
        app.open_file(other);
        app.editor_mut().insert("unsaved ");
        app.run_ui_command("graphite.scm.runInteractiveRebase");
        assert_eq!(app.status, app.t("scm.error.unsaved_for_git_operation"));
        assert!(app.interactive_rebase.is_some());
    }

    #[test]
    fn cancelling_forgets_the_plan() {
        let dir = TempDir::new();
        let mut app = repository_app(&dir);
        app.begin_interactive_rebase("main".into());
        app.run_ui_command("graphite.scm.cancelInteractiveRebase");
        assert!(app.interactive_rebase.is_none());
        assert_eq!(app.status, app.t("scm.rebase.cancelled"));
        app.run_ui_command("graphite.scm.cancelInteractiveRebase");
        assert_eq!(app.status, app.t("scm.rebase.no_plan"));
    }

    #[test]
    fn restarting_reloads_a_plan_left_open() {
        let dir = TempDir::new();
        let mut app = repository_app(&dir);
        app.begin_interactive_rebase("main".into());
        let id = app.active_document();
        let _ = app.documents[id].buffer.reload_text("garbage\n");
        app.begin_interactive_rebase("main".into());
        assert!(
            app.editor().text().starts_with("pick "),
            "the plan is regenerated"
        );
    }
}
