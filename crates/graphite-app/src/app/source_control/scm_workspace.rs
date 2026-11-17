//! Keeping the source control view in step with the folders that are open: the file watcher that
//! asks for refreshes, and the state that belongs to one repository and must not outlive it.

use super::*;

/// Watches `roots` and sends a refresh event when something relevant to git (or to extensions)
/// changes.
pub(crate) fn start_watcher(
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    roots: &[PathBuf],
) -> Option<notify::RecommendedWatcher> {
    use notify::Watcher;
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if result
            .as_ref()
            .map_or(true, |event| crate::scm::is_relevant_watch_event(event))
        {
            let _ = proxy.send_event(UserEvent::ScmRefresh);
            if let Ok(event) = &result {
                let changes = crate::app::plugins::file_changes(event);
                if !changes.is_empty() {
                    let _ = proxy.send_event(UserEvent::FileEvents(changes));
                }
            }
        }
    })
    .ok()?;
    let mut watching = false;
    for root in roots {
        watching |= watcher
            .watch(root, notify::RecursiveMode::Recursive)
            .is_ok();
    }
    watching.then_some(watcher)
}

impl App {
    /// Forgets everything of the previous repository and loads the one of the folder that is open
    /// now: the lists, the Graph and its position, the commit message, open menus and pickers,
    /// and the watcher. Called whenever the open folders change.
    pub(crate) fn reset_scm_for_workspace(&mut self) {
        self.scm_snapshot = crate::scm::Snapshot::default();
        self.scm_graph_rows.clear();
        self.scm_graph_scroll = 0;
        self.scm_graph_limit = super::scm_graph_ui::GRAPH_PAGE;
        self.scm_graph_selected_hash = None;
        self.scm_selected = 0;
        self.scm_scroll = 0;
        self.scm_commit_message.clear();
        self.scm_commit_prompt = false;
        self.scm_commit_pending = false;
        self.scm_collapsed_folders.clear();
        self.scm_batch_paths.clear();
        self.scm_graph_menu = None;
        self.scm_more_menu = None;
        self.scm_confirm = None;
        self.branch_name_prompt = None;
        self.close_branch_picker();
        self.interactive_rebase = None;
        let roots = self.tree.roots().to_vec();
        self.scm_watcher = self
            .event_proxy
            .clone()
            .and_then(|proxy| start_watcher(proxy, &roots));
        self.refresh_scm_async();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scm::FileStatus;
    use crate::test_support::TempDir;

    #[test]
    fn switching_folders_drops_the_state_of_the_previous_repository() {
        let first = TempDir::new();
        let mut app = App::new_with_options(first.path().to_path_buf(), None, true);
        app.scm_snapshot.branch = Some("old-branch".into());
        app.scm_snapshot.files.push(FileStatus {
            path: "old.txt".into(),
            unstaged: true,
            ..Default::default()
        });
        app.scm_graph_scroll = 40;
        app.scm_graph_limit = 500;
        app.scm_graph_selected_hash = Some("abc".into());
        app.scm_selected = 3;
        app.scm_scroll = 2;
        app.scm_commit_message = "half written".into();
        app.scm_collapsed_folders.insert((false, "src".into()));
        app.scm_batch_paths.push("old.txt".into());
        app.open_scm_commit_menu((10.0, 10.0));
        app.confirm_drop_all_stashes();
        app.open_create_branch_prompt();
        app.branch_picker = true;
        app.branch_picker_items = vec!["v1".into()];

        let second = TempDir::new();
        let workspace =
            graphite_workspace::Workspace::from_roots(vec![second.path().to_path_buf()]).unwrap();
        app.tree.replace_workspace(workspace);
        app.reset_scm_for_workspace();

        assert_ne!(app.scm_snapshot.branch.as_deref(), Some("old-branch"));
        assert!(app.scm_snapshot.files.is_empty());
        assert_eq!(
            (app.scm_graph_scroll, app.scm_graph_limit),
            (0, super::super::scm_graph_ui::GRAPH_PAGE)
        );
        assert!(app.scm_graph_selected_hash.is_none());
        assert_eq!((app.scm_selected, app.scm_scroll), (0, 0));
        assert!(app.scm_commit_message.is_empty());
        assert!(app.scm_collapsed_folders.is_empty() && app.scm_batch_paths.is_empty());
        assert!(app.scm_more_menu.is_none() && app.scm_confirm.is_none());
        assert!(app.scm_graph_menu.is_none() && app.branch_name_prompt.is_none());
        assert!(!app.branch_picker && app.branch_picker_items.is_empty());
    }

    #[test]
    fn results_loaded_for_another_folder_are_ignored() {
        let shown = TempDir::new();
        let other = TempDir::new();
        let mut app = App::new_with_options(shown.path().to_path_buf(), None, true);
        let stale = crate::scm::Snapshot {
            branch: Some("stale".into()),
            ..Default::default()
        };
        app.handle_user_event(UserEvent::ScmSnapshot(other.path().to_path_buf(), stale));
        assert_ne!(app.scm_snapshot.branch.as_deref(), Some("stale"));
        let current = crate::scm::Snapshot {
            branch: Some("current".into()),
            ..Default::default()
        };
        let root = app.scm_workspace_root();
        app.handle_user_event(UserEvent::ScmSnapshot(root, current));
        assert_eq!(app.scm_snapshot.branch.as_deref(), Some("current"));

        app.handle_user_event(UserEvent::ScmGraph(
            other.path().to_path_buf(),
            Err("late".into()),
        ));
        assert!(app.scm_graph_rows.is_empty());
    }

    #[test]
    fn the_new_folder_is_the_one_that_gets_read() {
        let first = TempDir::new();
        let mut app = App::new_with_options(first.path().to_path_buf(), None, true);
        let second = TempDir::new();
        let ran = std::process::Command::new("git")
            .current_dir(second.path())
            .args(["init", "-q", "-b", "fresh"])
            .status()
            .unwrap();
        assert!(ran.success());
        let workspace =
            graphite_workspace::Workspace::from_roots(vec![second.path().to_path_buf()]).unwrap();
        app.tree.replace_workspace(workspace);
        app.reset_scm_for_workspace();
        // Without an event loop the refresh runs right away, so the snapshot is the new repository's.
        assert_eq!(app.scm_snapshot.branch.as_deref(), Some("fresh"));
        assert!(
            app.scm_graph_rows.is_empty(),
            "no commits yet in the new repository"
        );
    }
}
