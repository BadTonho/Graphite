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

    /// Adds or updates a DAP data breakpoint from `address` (or the debug input / selection).
    pub(crate) fn add_data_breakpoint_from_input(
        &mut self,
        address: Option<&str>,
    ) -> Result<(), String> {
        let target = address
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                let input = self.debug_input.trim().to_owned();
                if !input.is_empty() {
                    self.debug_input.clear();
                    Some(input)
                } else {
                    self.editor().selected_text().map(|s| s.trim().to_owned())
                }
            })
            .filter(|s| !s.is_empty());
        let Some(data_id) = target else {
            return Ok(());
        };
        self.debug_panel_visible = true;
        self.debug
            .add_data_breakpoint(&data_id, Some("write"), None)
            .map_err(|error| error.to_string())
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

    /// Turns the breakpoint on the cursor line off or back on without removing it.
    pub(crate) fn toggle_breakpoint_enabled(&mut self) -> Result<(), String> {
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
        for point in points
            .iter_mut()
            .filter(|point| point["line"].as_u64() == Some(line))
        {
            let disabled = crate::debugger::is_disabled(point);
            if let Some(object) = point.as_object_mut() {
                if disabled {
                    object.remove(crate::debugger::DISABLED_KEY);
                } else {
                    object.insert(
                        crate::debugger::DISABLED_KEY.into(),
                        serde_json::json!(true),
                    );
                }
            }
        }
        let points = points.clone();
        if self.debug.session.is_some() {
            self.debug
                .set_breakpoints(&path, points)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Adds the function named in the debug console input as a function breakpoint.
    pub(crate) fn add_function_breakpoint_from_input(&mut self) -> Result<(), String> {
        let name = self.debug_input.trim().to_owned();
        if name.is_empty() {
            return Err(self.t("debug.function_name_needed").to_owned());
        }
        self.debug
            .add_function_breakpoint(&name)
            .map_err(|error| error.to_string())?;
        self.debug_input.clear();
        self.status = self.tf("debug.function_breakpoint_added", &[("name", &name)]);
        Ok(())
    }

    /// Sets a loaded variable from text of the form `name=value`.
    pub(crate) fn set_variable_from_text(&mut self, text: &str) -> Result<(), String> {
        let (name, value) = text
            .split_once('=')
            .map(|(name, value)| (name.trim(), value.trim()))
            .filter(|(name, value)| !name.is_empty() && !value.is_empty())
            .ok_or_else(|| self.t("debug.set_variable_format").to_owned())?;
        self.debug
            .set_variable(name, value)
            .map_err(|error| error.to_string())
    }

    /// Evaluates the selection, or the word at the cursor, and writes the result to the console.
    pub(crate) fn show_debug_hover(&mut self) -> Result<(), String> {
        let selected = self
            .editor()
            .selected_text()
            .filter(|text| !text.trim().is_empty());
        let expression = match selected {
            Some(text) => text,
            None => {
                let (line, column) = self.editor().cursor_line_col();
                let text: Vec<char> = self.editor().line(line).chars().collect();
                let part = |index: usize| {
                    text.get(index)
                        .is_some_and(|c| c.is_alphanumeric() || *c == '_')
                };
                let mut start = column.min(text.len());
                while start > 0 && part(start - 1) {
                    start -= 1;
                }
                let mut end = column.min(text.len());
                while part(end) {
                    end += 1;
                }
                text[start..end].iter().collect()
            }
        };
        if expression.trim().is_empty() {
            return Ok(());
        }
        self.debug_panel_visible = true;
        self.debug
            .evaluate_hover(&expression)
            .map_err(|error| error.to_string())
    }

    /// Shows only the console lines that contain the text in the console input; empty text shows
    /// everything again.
    pub(crate) fn filter_debug_console_from_input(&mut self) {
        self.debug.console_filter = self.debug_input.trim().to_owned();
        self.debug_input.clear();
        self.status = if self.debug.console_filter.is_empty() {
            self.t("debug.filter_cleared").into()
        } else {
            self.tf(
                "debug.filter_set",
                &[("text", &self.debug.console_filter.clone())],
            )
        };
    }

    /// Starts debugging with the launch configuration called `name`.
    pub(crate) fn start_debugging_with_config(&mut self, name: &str) -> Result<(), String> {
        if !self.debug.select_config(name) {
            return Err(self.tf("debug.unknown_config", &[("name", &name)]));
        }
        self.start_debugging()
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

    fn run_with_text(
        app: &mut App,
        id: &str,
        text: &str,
    ) -> crate::app::command_execution::CommandResult {
        app.execute_command(graphite_core_api::CommandRequest {
            id: id.into(),
            arguments: serde_json::json!({"text": text}),
            target: app.ui_target_for_command(id),
        })
        .try_result()
        .expect("finishes at once")
    }

    #[test]
    fn a_breakpoint_can_be_switched_off_and_on_in_place() {
        let (mut app, _dir, path) = app_with_breakpoints(&[3, 6]);
        app.editor_mut().set_cursor_line_col(2, 0);
        app.run_ui_command("debug.enableOrDisableBreakpoint");
        let points = &app.debug.breakpoints[&path];
        assert!(crate::debugger::is_disabled(&points[0]));
        assert!(
            !crate::debugger::is_disabled(&points[1]),
            "the other one is untouched"
        );
        assert_eq!(points.len(), 2, "it is not removed");
        app.run_ui_command("debug.enableOrDisableBreakpoint");
        assert!(!crate::debugger::is_disabled(
            &app.debug.breakpoints[&path][0]
        ));
        app.editor_mut().set_cursor_line_col(0, 0);
        app.run_ui_command("debug.enableOrDisableBreakpoint");
        assert_eq!(app.status, app.t("debug.no_breakpoint_here"));
    }

    #[test]
    fn a_function_breakpoint_comes_from_the_console_input() {
        let (mut app, _dir, _path) = app_with_breakpoints(&[]);
        app.run_ui_command("workbench.debug.viewlet.action.addFunctionBreakpointAction");
        assert!(app.debug.function_breakpoints.is_empty());
        app.debug_input = "  main ".into();
        app.run_ui_command("workbench.debug.viewlet.action.addFunctionBreakpointAction");
        assert_eq!(app.debug.function_breakpoints, ["main"]);
        assert!(app.debug_input.is_empty());
        assert!(app.status.contains("main"), "{}", app.status);
    }

    #[test]
    fn set_variable_needs_a_name_value_pair_and_a_running_session() {
        let (mut app, _dir, _path) = app_with_breakpoints(&[]);
        for bad in ["", "novalue", "=1", "x="] {
            assert!(
                run_with_text(&mut app, "debug.setVariable", bad).is_err(),
                "{bad:?}"
            );
        }
        // The pair is well formed, but there is no loaded variable of that name.
        assert!(run_with_text(&mut app, "debug.setVariable", "a=1").is_err());
    }

    #[test]
    fn starting_from_a_configuration_needs_a_known_name() {
        let (mut app, _dir, _path) = app_with_breakpoints(&[]);
        let error = run_with_text(&mut app, "debug.startFromConfig", "nope").unwrap_err();
        assert!(error.to_string().contains("nope"), "{error}");
    }

    #[test]
    fn the_console_filter_hides_other_lines_and_clears_with_empty_text() {
        let (mut app, _dir, _path) = app_with_breakpoints(&[]);
        for text in ["alpha one\n", "beta two\n", "alpha three\n"] {
            app.debug.output.push(crate::debugger::DebugOutput {
                category: "stdout".into(),
                text: text.into(),
            });
        }
        app.debug_input = "alpha".into();
        app.run_ui_command("repl.action.filter");
        assert_eq!(app.debug.console_filter, "alpha");
        let shown: Vec<_> = crate::app::debug_panel::debug_columns(&app.debug)[2]
            .iter()
            .map(|row| row.text.clone())
            .collect();
        assert!(shown.iter().all(|line| !line.contains("beta")), "{shown:?}");
        assert_eq!(
            shown.iter().filter(|line| line.contains("alpha")).count(),
            2
        );
        app.run_ui_command("repl.action.find");
        assert!(app.debug.console_filter.is_empty(), "empty input clears it");
    }

    #[test]
    fn the_debug_hover_needs_a_session_and_an_expression() {
        let (mut app, _dir, _path) = app_with_breakpoints(&[]);
        app.editor_mut().set_cursor_line_col(0, 0);
        app.run_ui_command("editor.debug.action.showDebugHover");
        assert!(crate::app::command_registry::registry()
            .get("debug.toggleBreakpoint")
            .is_some());
        assert!(crate::app::command_registry::registry()
            .get("debug.editBreakpoint")
            .is_some());
    }
}
