//! Moving between the changed hunks of the active file, and the source control commands that
//! use the commit message box and the terminal.

use super::*;

/// First line of each run of consecutive changed lines.
pub(crate) fn hunk_starts(changed: &[usize]) -> Vec<usize> {
    let mut lines = changed.to_vec();
    lines.sort_unstable();
    lines.dedup();
    let mut starts = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if index == 0 || lines[index - 1] + 1 != *line {
            starts.push(*line);
        }
    }
    starts
}

/// The hunk start after (or before) `cursor`, wrapping around the file. `None` when there are no
/// hunks. The boolean says whether it wrapped.
pub(crate) fn neighbor_hunk(
    starts: &[usize],
    cursor: usize,
    forward: bool,
) -> Option<(usize, bool)> {
    if forward {
        starts
            .iter()
            .find(|&&start| start > cursor)
            .map(|&start| (start, false))
            .or_else(|| starts.first().map(|&start| (start, true)))
    } else {
        starts
            .iter()
            .rev()
            .find(|&&start| start < cursor)
            .map(|&start| (start, false))
            .or_else(|| starts.last().map(|&start| (start, true)))
    }
}

impl App {
    /// The changed lines of the active file according to the last source control snapshot.
    fn active_changed_lines(&self) -> Vec<usize> {
        let Some(path) = self.documents[self.command_document()].buffer.path() else {
            return Vec::new();
        };
        let root = self.tree.root_for(path);
        // Either side can be in canonical form (`\?\` on Windows), so try each pairing.
        let canonical_path = path.canonicalize().ok();
        let canonical_root = root.canonicalize().ok();
        let relative = [
            Some((path, root)),
            canonical_path.as_deref().map(|path| (path, root)),
            canonical_root.as_deref().map(|root| (path, root)),
            canonical_path.as_deref().zip(canonical_root.as_deref()),
        ]
        .into_iter()
        .flatten()
        .find_map(|(path, root)| path.strip_prefix(root).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
        self.scm_snapshot
            .line_changes
            .get(Path::new(&relative))
            .map(|lines| lines.iter().map(|(line, _)| *line).collect())
            .unwrap_or_default()
    }

    /// Moves the cursor to the next or previous changed hunk of the active file.
    pub(crate) fn go_to_change(&mut self, forward: bool) {
        let starts = hunk_starts(&self.active_changed_lines());
        let cursor = self.editor().cursor_line_col().0;
        match neighbor_hunk(&starts, cursor, forward) {
            Some((line, wrapped)) => {
                self.editor_mut().set_cursor_line_col(line, 0);
                self.reveal_cursor = true;
                if wrapped {
                    self.status = self.t("scm.change.wrapped").into();
                }
            }
            None => self.status = self.t("scm.change.none").into(),
        }
    }

    /// Commits the staged changes with the message in the commit box.
    pub(crate) fn scm_accept_input(&mut self) {
        if self.scm_commit_message.trim().is_empty() {
            self.status = self.t("scm.error.empty_message").into();
            return;
        }
        self.perform_scm_action(super::scm_ui::ScmAction::Commit);
    }

    pub(crate) fn scm_clear_input(&mut self) {
        self.scm_commit_message.clear();
    }

    /// Opens a terminal in the repository folder of the active file (or the workspace folder).
    pub(crate) fn open_repository_in_terminal(&mut self) {
        let directory = self.scm_workspace_root();
        let Some(profile) = self
            .terminal_profiles
            .get(self.selected_terminal_profile)
            .cloned()
        else {
            self.status = self.t("terminal.no_profiles").into();
            return;
        };
        match self.spawn_terminal_profile(&profile, Some(&directory), None) {
            Ok(index) => {
                self.terminal_visible = true;
                self.terminal_problems = false;
                self.terminal_focused = true;
                self.active_terminal = index;
                self.focused_terminal = index;
                self.resize_terminal();
            }
            Err(error) => self.status = error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn consecutive_changed_lines_form_one_hunk() {
        assert_eq!(hunk_starts(&[5, 3, 4, 10, 4, 12, 13]), vec![3, 10, 12]);
        assert!(hunk_starts(&[]).is_empty());
    }

    #[test]
    fn the_neighbor_hunk_wraps_at_both_ends() {
        let starts = [3, 10, 20];
        assert_eq!(neighbor_hunk(&starts, 0, true), Some((3, false)));
        assert_eq!(neighbor_hunk(&starts, 10, true), Some((20, false)));
        assert_eq!(neighbor_hunk(&starts, 20, true), Some((3, true)));
        assert_eq!(neighbor_hunk(&starts, 25, false), Some((20, false)));
        assert_eq!(neighbor_hunk(&starts, 10, false), Some((3, false)));
        assert_eq!(neighbor_hunk(&starts, 3, false), Some((20, true)));
        assert_eq!(neighbor_hunk(&[], 3, true), None);
    }

    fn app_with_changes(lines: &[usize]) -> (App, TempDir) {
        let dir = TempDir::new();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(file);
        app.scm_snapshot.line_changes.insert(
            PathBuf::from("a.txt"),
            lines
                .iter()
                .map(|line| (*line, crate::scm::LineChange::Modified))
                .collect(),
        );
        (app, dir)
    }

    #[test]
    fn the_change_commands_walk_through_the_hunks_and_report_a_wrap() {
        let (mut app, _dir) = app_with_changes(&[2, 3, 7]);
        app.editor_mut().set_cursor_line_col(0, 0);
        for (command, expected) in [
            ("workbench.action.editor.nextChange", 2),
            ("editor.action.dirtydiff.next", 7),
            ("workbench.action.editor.nextChange", 2),
            ("workbench.action.editor.previousChange", 7),
            ("editor.action.dirtydiff.previous", 2),
        ] {
            app.run_ui_command(command);
            assert_eq!(app.editor().cursor_line_col().0, expected, "{command}");
        }
    }

    #[test]
    fn a_file_without_changes_reports_it_and_stays_put() {
        let (mut app, _dir) = app_with_changes(&[]);
        app.editor_mut().set_cursor_line_col(4, 0);
        app.run_ui_command("workbench.action.editor.nextChange");
        assert_eq!(app.editor().cursor_line_col().0, 4);
        assert_eq!(app.status, app.t("scm.change.none"));
    }

    #[test]
    fn the_commit_box_commands_use_the_message() {
        let (mut app, _dir) = app_with_changes(&[]);
        app.scm_commit_message = "wip".into();
        app.run_ui_command("scm.clearInput");
        assert!(app.scm_commit_message.is_empty());
        app.run_ui_command("scm.acceptInput");
        assert_eq!(app.status, app.t("scm.error.empty_message"));
    }

    #[test]
    fn the_terminal_commands_are_registered() {
        for id in ["scm.openInTerminal", "scm.openInIntegratedTerminal"] {
            assert!(
                crate::app::command_registry::registry().get(id).is_some(),
                "{id}"
            );
        }
    }
}
