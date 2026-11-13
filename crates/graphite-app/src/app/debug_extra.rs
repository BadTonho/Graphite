//! Debug commands that work on breakpoints and the debug console from the keyboard.

use super::change_navigation::neighbor_hunk;
use super::*;
use crate::app::clipboard::{ClipboardAccess, SystemClipboard};

impl App {
    /// Path of the active file and its breakpoint lines, 0-based and sorted.
    fn active_breakpoint_lines(&self) -> Option<(PathBuf, Vec<usize>)> {
        let path = self.documents[self.command_document()]
            .buffer
            .path()?
            .to_path_buf();
        let mut lines: Vec<usize> = self
            .debug
            .breakpoints
            .get(&path)
            .map(|points| {
                points
                    .iter()
                    .filter_map(|point| point["line"].as_u64())
                    .map(|line| (line as usize).saturating_sub(1))
                    .collect()
            })
            .unwrap_or_default();
        lines.sort_unstable();
        lines.dedup();
        Some((path, lines))
    }

    /// Moves the cursor to the next or previous breakpoint of the active file.
    pub(crate) fn go_to_breakpoint(&mut self, forward: bool) {
        let Some((_, lines)) = self.active_breakpoint_lines() else {
            return;
        };
        let cursor = self.editor().cursor_line_col().0;
        match neighbor_hunk(&lines, cursor, forward) {
            Some((line, _)) => {
                self.editor_mut().set_cursor_line_col(line, 0);
                self.reveal_cursor = true;
            }
            None => self.status = self.t("debug.no_breakpoints").into(),
        }
    }

    /// Removes the breakpoint on the cursor line of the active file, if there is one.
    pub(crate) fn remove_breakpoint_at_cursor(&mut self) -> Result<(), String> {
        let Some((path, lines)) = self.active_breakpoint_lines() else {
            return Ok(());
        };
        let cursor = self.editor().cursor_line_col().0;
        if !lines.contains(&cursor) {
            self.status = self.t("debug.no_breakpoint_here").into();
            return Ok(());
        }
        let line = cursor as u64 + 1;
        let points = self.debug.breakpoints.entry(path.clone()).or_default();
        points.retain(|point| point["line"].as_u64() != Some(line));
        let remaining = points.clone();
        if remaining.is_empty() {
            self.debug.breakpoints.remove(&path);
        }
        if self.debug.session.is_some() {
            self.debug
                .set_breakpoints(&path, remaining)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Turns all breakpoints off, or back on when they were off.
    pub(crate) fn toggle_breakpoints_activated(&mut self) {
        let enabled = self.debug.breakpoints_disabled;
        self.set_all_breakpoints_enabled(enabled);
    }

    /// Sends the breakpoints of every file to the debug session again.
    pub(crate) fn reapply_breakpoints(&mut self) -> Result<(), String> {
        if self.debug.session.is_none() {
            return Ok(());
        }
        let files: Vec<PathBuf> = self.debug.breakpoints.keys().cloned().collect();
        for file in files {
            let points = self
                .debug
                .breakpoints
                .get(&file)
                .cloned()
                .unwrap_or_default();
            self.debug
                .set_breakpoints(&file, points)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(crate) fn clear_debug_console(&mut self) {
        self.debug.output.clear();
    }

    /// Copies the debug console text to the clipboard.
    pub(crate) fn copy_debug_console(&mut self) -> Result<(), String> {
        let text = self
            .debug
            .output
            .iter()
            .map(|line| line.text.trim_end_matches(['\r', '\n']))
            .collect::<Vec<_>>()
            .join("\n");
        SystemClipboard
            .set_text(&text)
            .map_err(|()| self.t("misc.clipboard_failed").to_owned())?;
        self.status = self.t("debug.console_copied").into();
        Ok(())
    }

    pub(crate) fn show_debug_view(&mut self) {
        self.debug_panel_visible = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn app_with_breakpoints(lines: &[u64]) -> (App, TempDir, PathBuf) {
        let dir = TempDir::new();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(file);
        let path = app.documents[app.active_document()]
            .buffer
            .path()
            .unwrap()
            .to_path_buf();
        app.debug.breakpoints.insert(
            path.clone(),
            lines
                .iter()
                .map(|line| serde_json::json!({"line": line}))
                .collect(),
        );
        (app, dir, path)
    }

    #[test]
    fn the_breakpoint_commands_walk_the_breakpoints_of_the_file() {
        // DAP lines are 1-based: these are editor lines 2, 5 and 8.
        let (mut app, _dir, _path) = app_with_breakpoints(&[3, 6, 9]);
        app.editor_mut().set_cursor_line_col(0, 0);
        for (command, expected) in [
            ("editor.debug.action.goToNextBreakpoint", 2),
            ("editor.debug.action.goToNextBreakpoint", 5),
            ("editor.debug.action.goToPreviousBreakpoint", 2),
            ("editor.debug.action.goToPreviousBreakpoint", 8),
        ] {
            app.run_ui_command(command);
            assert_eq!(app.editor().cursor_line_col().0, expected, "{command}");
        }
    }

    #[test]
    fn a_file_without_breakpoints_says_so() {
        let (mut app, _dir, _path) = app_with_breakpoints(&[]);
        app.run_ui_command("editor.debug.action.goToNextBreakpoint");
        assert_eq!(app.status, app.t("debug.no_breakpoints"));
    }

    #[test]
    fn removing_a_breakpoint_only_touches_the_cursor_line() {
        let (mut app, _dir, path) = app_with_breakpoints(&[3, 6]);
        app.editor_mut().set_cursor_line_col(2, 0);
        app.run_ui_command("debug.removeBreakpoint");
        let left: Vec<u64> = app.debug.breakpoints[&path]
            .iter()
            .filter_map(|point| point["line"].as_u64())
            .collect();
        assert_eq!(left, vec![6]);
        app.run_ui_command("debug.removeBreakpoint");
        assert_eq!(app.status, app.t("debug.no_breakpoint_here"));
        app.editor_mut().set_cursor_line_col(5, 0);
        app.run_ui_command("workbench.debug.viewlet.action.removeBreakpoint");
        assert!(!app.debug.breakpoints.contains_key(&path));
    }

    #[test]
    fn breakpoints_can_be_switched_off_and_on_together() {
        let (mut app, _dir, _path) = app_with_breakpoints(&[3]);
        assert!(!app.debug.breakpoints_disabled);
        app.run_ui_command("workbench.debug.viewlet.action.toggleBreakpointsActivatedAction");
        assert!(app.debug.breakpoints_disabled);
        app.run_ui_command("workbench.debug.viewlet.action.toggleBreakpointsActivatedAction");
        assert!(!app.debug.breakpoints_disabled);
        app.run_ui_command("workbench.debug.viewlet.action.reapplyBreakpointsAction");
    }

    #[test]
    fn the_console_can_be_cleared_and_the_debug_view_shown() {
        let (mut app, _dir, _path) = app_with_breakpoints(&[]);
        app.debug.output.push(crate::debugger::DebugOutput {
            category: "stdout".into(),
            text: "hello\n".into(),
        });
        app.run_ui_command("workbench.debug.panel.action.clearReplAction");
        assert!(app.debug.output.is_empty());
        app.debug_panel_visible = false;
        app.run_ui_command("debug.openView");
        assert!(app.debug_panel_visible);
    }

    #[test]
    fn the_renamed_breakpoint_commands_keep_their_default_key() {
        let (mut app, _dir, _path) = app_with_breakpoints(&[]);
        app.editor_mut().set_cursor_line_col(0, 0);
        for id in [
            "editor.debug.action.toggleBreakpoint",
            "editor.debug.action.conditionalBreakpoint",
            "editor.debug.action.editBreakpoint",
            "editor.debug.action.addLogPoint",
            "workbench.debug.viewlet.action.enableAllBreakpoints",
            "workbench.debug.viewlet.action.disableAllBreakpoints",
            "workbench.debug.viewlet.action.removeAllBreakpoints",
        ] {
            assert!(
                crate::app::command_registry::registry().get(id).is_some(),
                "{id}"
            );
        }
        assert!(crate::app::command_registry::registry()
            .get("graphite.debug.toggleBreakpoint")
            .is_none());
        assert!(crate::keybindings::default_keybindings_json()
            .contains("editor.debug.action.toggleBreakpoint"));
    }
}
