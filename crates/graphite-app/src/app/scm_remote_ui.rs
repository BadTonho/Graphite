//! SCM view actions that talk to the remote or show history. The git work runs on a worker
//! thread; results come back to the event loop as `UserEvent`s.

use super::*;
use crate::scm_remote;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RemoteOp {
    Pull,
    Push,
    Fetch,
    Sync,
    Stash,
    /// Amend the last commit, with a new message when one is given.
    Amend(Option<String>),
    CreateBranch(String),
    Merge(String),
    Rebase(String),
    /// Delete a local branch; `true` deletes it even if it is not fully merged.
    DeleteBranch(String, bool),
    /// Abandon the merge or rebase that stopped on conflicts.
    AbortOperation,
    /// Continue a rebase (or finish a merge) after the conflicts were resolved.
    ContinueOperation,
}

impl RemoteOp {
    fn run(&self, root: &std::path::Path) -> Result<String, String> {
        use crate::scm_branches as branches;
        match self {
            RemoteOp::Pull => scm_remote::pull(root),
            RemoteOp::Push => scm_remote::push(root),
            RemoteOp::Fetch => scm_remote::fetch(root),
            RemoteOp::Sync => scm_remote::sync(root),
            RemoteOp::Stash => scm_remote::stash(root),
            RemoteOp::Amend(message) => branches::amend(root, message.as_deref()),
            RemoteOp::CreateBranch(name) => branches::create_branch(root, name),
            RemoteOp::Merge(branch) => branches::merge(root, branch),
            RemoteOp::Rebase(onto) => branches::rebase(root, onto),
            RemoteOp::DeleteBranch(name, force) => branches::delete_branch(root, name, *force),
            RemoteOp::AbortOperation => branches::abort(root),
            RemoteOp::ContinueOperation => branches::continue_operation(root),
        }
    }

    /// Operations that rewrite files under the open documents, which would clash with unsaved
    /// edits.
    pub(crate) fn needs_saved_documents(&self) -> bool {
        matches!(
            self,
            RemoteOp::Merge(_) | RemoteOp::Rebase(_) | RemoteOp::ContinueOperation
        )
    }
}

impl App {
    /// Runs a pull, push, fetch, sync or stash on a worker thread and refreshes the SCM view
    /// with the result.
    pub(crate) fn run_scm_remote(&mut self, op: RemoteOp) {
        if op.needs_saved_documents() && self.documents.iter().any(|tab| tab.is_dirty()) {
            self.status = self.t("scm.error.unsaved_for_git_operation").into();
            return;
        }
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = self.t("scm.error.actions_need_loop").into();
            return;
        };
        let root = self.scm_workspace_root();
        self.status = self.t("scm.status.running").into();
        std::thread::spawn(move || {
            let message = match op.run(&root) {
                Ok(output) if output.is_empty() => "Done".to_owned(),
                Ok(output) => output.lines().last().unwrap_or_default().to_owned(),
                Err(error) => error,
            };
            let snapshot = crate::scm::snapshot(&root);
            let _ = proxy.send_event(UserEvent::ScmOperationCompleted {
                message,
                snapshot: Some(snapshot),
            });
        });
    }

    /// Loads the recent history in the background; the result opens as a read-only view.
    pub(crate) fn open_scm_history(&mut self) {
        let Some(proxy) = self.event_proxy.clone() else {
            self.status = self.t("scm.error.actions_need_loop").into();
            return;
        };
        let root = self.scm_workspace_root();
        std::thread::spawn(move || {
            let _ = proxy.send_event(UserEvent::ScmHistory(scm_remote::history(&root)));
        });
    }

    /// Shows the history text in a read-only view, refreshing it if it is already open.
    pub(crate) fn show_scm_history(&mut self, result: Result<String, String>) {
        let text = match result {
            Ok(text) if !text.is_empty() => text,
            Ok(_) => self.t("scm.history.none").to_owned(),
            Err(error) => {
                self.status = self.tf("scm.remote.failed", &[("error", &error)]);
                return;
            }
        };
        let path = self.profile_root.join("git-history.txt");
        match self.documents.id_for_path(&path) {
            Some(id) => {
                let _ = self.documents[id].buffer.reload_text(&text);
                self.activate_editor_document(id);
            }
            None => self.open_read_only_view(path, &text),
        }
    }

    /// Opens the GitHub page that creates a pull request for the current branch.
    pub(crate) fn open_pull_request(&mut self) {
        let root = self.scm_workspace_root();
        match scm_remote::pull_request_url(&root) {
            Ok(url) => {
                crate::app::plugins::drawing::open_external_https(&url);
                self.status = self.t("scm.pr.opened").into();
            }
            Err(error) => self.status = self.tf("scm.remote.failed", &[("error", &error)]),
        }
    }

    /// The "…" button: lists the Git commands, filtered to the Git group.
    pub(crate) fn open_scm_more_menu(&mut self) {
        self.open_command_palette();
        if let Some(palette) = self.command_palette.as_mut() {
            palette.query = "Git: ".into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_history_load_is_reported_in_the_status_bar() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.show_scm_history(Err("not a repository".into()));
        assert!(app.status.contains("not a repository"));
        assert!(app
            .documents
            .id_for_path(&app.profile_root.join("git-history.txt"))
            .is_none());
    }

    #[test]
    fn history_text_opens_as_a_read_only_view() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.profile_root = dir.path().join("profile");
        app.show_scm_history(Ok("* abc first".into()));
        let id = app
            .documents
            .id_for_path(&app.profile_root.join("git-history.txt"))
            .unwrap();
        assert!(app.documents[id].buffer.is_read_only());
        assert!(app.editor().text().contains("abc first"));
        // A second load refreshes the same view instead of opening another tab.
        app.show_scm_history(Ok("* def second".into()));
        assert!(app.editor().text().contains("def second"));
        assert_eq!(app.documents.len(), 2);
    }
}

#[cfg(test)]
mod button_layout_tests {
    use super::super::scm_ui::{changes_header_buttons, scm_panel_layout};

    #[test]
    fn repository_buttons_fit_beside_the_name_and_changes_buttons_sit_left_of_stage_all() {
        let width = 300.0;
        let layout = scm_panel_layout(
            &crate::scm::Snapshot::default(),
            width,
            600.0,
            20.0,
            9.0,
            0,
            false,
            false,
            false,
        );
        let more = layout.repo_more;
        assert!(more.x + more.width <= width);
        assert!(layout.repository_label.x + layout.repository_label.width <= layout.repo_sync.x);
        assert!(layout.repo_sync.x + layout.repo_sync.width <= layout.repo_pull_request.x);
        assert!(layout.repo_pull_request.x + layout.repo_pull_request.width <= more.x);

        let section = layout.list_rows.iter().find(|row| {
            matches!(
                row.kind,
                crate::app::scm_ui::ScmListRowKind::Section { staged: false, .. }
            )
        });
        if let Some(row) = section {
            let (history, stash) = changes_header_buttons(row.action, 9.0);
            assert!(history.x + history.width <= stash.x);
            assert!(stash.x + stash.width <= row.action.x);
        }
    }
}
