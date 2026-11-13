use super::*;

const ROW_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
pub(crate) const TERMINAL_MENU_WIDTH: f32 = 420.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalMenuCommand {
    NewTerminal,
    SplitTerminal,
    NewTerminalWindow,
    RunTask,
    RunBuildTask,
    RunActiveFile,
    RunSelectedText,
    ShowRunningTasks,
    RestartRunningTask,
    TerminateTask,
    ConfigureTasks,
    ConfigureDefaultBuildTask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalMenuRow {
    Separator,
    Item {
        command: TerminalMenuCommand,
        label: &'static str,
        shortcut: &'static str,
        enabled: bool,
    },
}

impl App {
    pub(crate) fn terminal_menu_rows(&self) -> Vec<TerminalMenuRow> {
        use TerminalMenuCommand as C;
        let row = |command, label, shortcut, enabled| TerminalMenuRow::Item {
            command,
            label,
            shortcut,
            enabled,
        };
        let has_terminal = !self.terminals.is_empty();
        let editor = self.editor();
        let has_selection = editor.selections().iter().any(|s| s.anchor != s.head);
        drop(editor);
        let has_active_file = self.documents[self.active_document()]
            .buffer
            .path()
            .is_some();
        let task_running = self.task_cancel.is_some();
        vec![
            row(
                C::NewTerminal,
                "terminal.menu.new_terminal",
                "Ctrl+Shift+'",
                true,
            ),
            row(
                C::SplitTerminal,
                "terminal.menu.split_terminal",
                "Ctrl+Shift+5",
                has_terminal,
            ),
            row(
                C::NewTerminalWindow,
                "terminal.menu.new_terminal_window",
                "Ctrl+Shift+Alt+'",
                false,
            ),
            TerminalMenuRow::Separator,
            row(C::RunTask, "terminal.menu.run_task", "", true),
            row(
                C::RunBuildTask,
                "terminal.menu.run_build_task",
                "Ctrl+Shift+B",
                true,
            ),
            row(
                C::RunActiveFile,
                "terminal.menu.run_active_file",
                "",
                has_active_file,
            ),
            row(
                C::RunSelectedText,
                "terminal.menu.run_selected_text",
                "",
                has_selection,
            ),
            TerminalMenuRow::Separator,
            row(
                C::ShowRunningTasks,
                "terminal.menu.show_running_tasks",
                "",
                task_running,
            ),
            row(
                C::RestartRunningTask,
                "terminal.menu.restart_running_task",
                "",
                task_running,
            ),
            row(
                C::TerminateTask,
                "terminal.menu.terminate_task",
                "",
                task_running,
            ),
            TerminalMenuRow::Separator,
            row(C::ConfigureTasks, "terminal.menu.configure_tasks", "", true),
            row(
                C::ConfigureDefaultBuildTask,
                "terminal.menu.configure_default_build_task",
                "",
                true,
            ),
        ]
    }

    /// Enabled-state list of the selectable rows, in display order.
    fn terminal_menu_items(&self) -> Vec<(TerminalMenuCommand, bool)> {
        self.terminal_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                TerminalMenuRow::Item {
                    command, enabled, ..
                } => Some((command, enabled)),
                TerminalMenuRow::Separator => None,
            })
            .collect()
    }

    pub(crate) fn terminal_menu_command_at(
        &self,
        x: f32,
        y: f32,
        popup_x: f32,
        popup_y: f32,
    ) -> Option<(TerminalMenuCommand, bool)> {
        if x < popup_x || x >= popup_x + TERMINAL_MENU_WIDTH || y < popup_y + 4.0 {
            return None;
        }
        let mut row_y = popup_y + 4.0;
        for row in self.terminal_menu_rows() {
            let height = Self::terminal_menu_row_height(row);
            if y >= row_y && y < row_y + height {
                return match row {
                    TerminalMenuRow::Separator => None,
                    TerminalMenuRow::Item {
                        command, enabled, ..
                    } => Some((command, enabled)),
                };
            }
            row_y += height;
        }
        None
    }

    pub(crate) fn terminal_menu_move_selection(&mut self, delta: isize) {
        let count = self.terminal_menu_items().len();
        if count == 0 {
            self.terminal_menu_selection = 0;
            return;
        }
        self.terminal_menu_selection = self
            .terminal_menu_selection
            .saturating_add_signed(delta)
            .min(count - 1);
    }

    pub(crate) fn terminal_menu_selected(&self) -> Option<(TerminalMenuCommand, bool)> {
        self.terminal_menu_items()
            .get(self.terminal_menu_selection)
            .copied()
    }

    pub(crate) fn terminal_menu_pointer_selection(&mut self, y: f32, popup_y: f32) {
        let mut row_y = popup_y + 4.0;
        let mut index = 0;
        for row in self.terminal_menu_rows() {
            let height = Self::terminal_menu_row_height(row);
            if y >= row_y && y < row_y + height {
                if matches!(row, TerminalMenuRow::Item { .. }) {
                    self.terminal_menu_selection = index;
                }
                return;
            }
            row_y += height;
            if matches!(row, TerminalMenuRow::Item { .. }) {
                index += 1;
            }
        }
    }

    pub(crate) fn activate_terminal_menu_accessibility_item(&mut self, index: usize) {
        if let Some((command, true)) = self.terminal_menu_items().get(index).copied() {
            self.execute_terminal_menu_command(command);
            self.close_workbench_menu();
        }
    }

    pub(crate) fn execute_terminal_menu_command(&mut self, command: TerminalMenuCommand) {
        use TerminalMenuCommand as C;
        match command {
            C::NewTerminal => self.run_ui_command("graphite.terminal.new"),
            C::SplitTerminal => self.run_ui_command("graphite.terminal.split"),
            // Opening a separate window with a terminal is not implemented yet;
            // the row is always disabled, so this arm is never reached from input.
            C::NewTerminalWindow => {}
            C::RunTask => self.run_ui_command("graphite.tasks.pick"),
            C::RunBuildTask => self.run_ui_command("workbench.action.tasks.build"),
            C::RunActiveFile => self.run_ui_command("graphite.terminal.runActiveFile"),
            C::RunSelectedText => self.run_ui_command("graphite.terminal.runSelectedText"),
            C::ShowRunningTasks => self.run_ui_command("graphite.tasks.showRunning"),
            C::RestartRunningTask => self.run_ui_command("graphite.tasks.restartRunning"),
            C::TerminateTask => self.run_ui_command("workbench.action.tasks.cancel"),
            C::ConfigureTasks => self.run_ui_command("graphite.tasks.configure"),
            C::ConfigureDefaultBuildTask => {
                self.run_ui_command("graphite.tasks.configureDefaultBuild")
            }
        }
    }

    pub(crate) fn terminal_menu_row_height(row: TerminalMenuRow) -> f32 {
        match row {
            TerminalMenuRow::Separator => SEPARATOR_HEIGHT,
            TerminalMenuRow::Item { .. } => ROW_HEIGHT,
        }
    }

    /// Sends text to the focused terminal, creating a session when none exists.
    pub(crate) fn run_text_in_terminal(&mut self, text: &str) -> Result<(), String> {
        if self.terminals.is_empty() {
            self.spawn_terminal_session()?;
        }
        let index = self
            .focused_terminal
            .min(self.terminals.len().saturating_sub(1));
        let Some(session) = self.terminals.get(index) else {
            return Err(self.t("terminal.unavailable").to_owned());
        };
        session.input(text.as_bytes().to_vec());
        self.terminal_visible = true;
        self.terminal_problems = false;
        self.terminal_focused = true;
        self.resize_terminal();
        Ok(())
    }

    pub(crate) fn terminal_run_active_file(&mut self) -> Result<(), String> {
        let path = self.documents[self.active_document()]
            .buffer
            .path()
            .map(std::path::Path::to_path_buf)
            .ok_or_else(|| self.t("terminal.menu.no_active_file").to_owned())?;
        let command = terminal_file_command(&path, cfg!(windows));
        self.run_text_in_terminal(&command)
    }

    pub(crate) fn terminal_run_selected_text(&mut self) -> Result<(), String> {
        let lines = self
            .editor()
            .selected_texts()
            .into_iter()
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            return Err(self.t("terminal.menu.no_selection").to_owned());
        }
        self.run_text_in_terminal(&terminal_selection_text(&lines))
    }

    pub(crate) fn terminal_show_running_tasks(&mut self) -> Result<(), String> {
        if self.task_cancel.is_none() {
            return Err(self.t("terminal.menu.no_running_task").to_owned());
        }
        self.terminal_visible = true;
        self.terminal_problems = false;
        self.terminal_focused = false;
        self.resize_terminal();
        Ok(())
    }

    /// Stops the running task and starts it again once the worker reports completion.
    pub(crate) fn terminal_restart_running_task(&mut self) -> Result<(), String> {
        let Some(label) = self.task_running_label.clone() else {
            return Err(self.t("terminal.menu.no_running_task").to_owned());
        };
        self.task_restart_pending = Some(label);
        self.cancel_task();
        Ok(())
    }

    /// Opens the command palette filtered to the workspace's task entries.
    pub(crate) fn terminal_pick_task(&mut self) {
        self.open_command_palette();
        if let Some(palette) = self.command_palette.as_mut() {
            palette.query = "Run Task:".into();
        }
    }
}

/// Builds the shell input that runs `path`. PowerShell needs the call operator
/// before a quoted path; POSIX shells take the quoted path directly.
pub(crate) fn terminal_file_command(path: &std::path::Path, windows: bool) -> String {
    let display = path.display().to_string();
    if windows {
        format!("& '{}'\r", display.replace('\'', "''"))
    } else {
        format!("'{}'\r", display.replace('\'', "'\\''"))
    }
}

/// Converts selected lines to terminal input, one carriage return per line break.
pub(crate) fn terminal_selection_text(lines: &[String]) -> String {
    let joined = lines.join("\n").replace("\r\n", "\n").replace('\n', "\r");
    format!("{joined}\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{Key, NamedKey};

    fn temp_app() -> (crate::test_support::TempDir, App) {
        let temp = crate::test_support::TempDir::new();
        let app = App::new_with_options(temp.path().to_path_buf(), None, true);
        (temp, app)
    }

    #[test]
    fn terminal_menu_lists_the_reference_groups_in_order() {
        let (_temp, app) = temp_app();
        let rows = app.terminal_menu_rows();
        let commands = rows
            .iter()
            .filter_map(|row| match row {
                TerminalMenuRow::Item { command, .. } => Some(*command),
                TerminalMenuRow::Separator => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            commands,
            [
                TerminalMenuCommand::NewTerminal,
                TerminalMenuCommand::SplitTerminal,
                TerminalMenuCommand::NewTerminalWindow,
                TerminalMenuCommand::RunTask,
                TerminalMenuCommand::RunBuildTask,
                TerminalMenuCommand::RunActiveFile,
                TerminalMenuCommand::RunSelectedText,
                TerminalMenuCommand::ShowRunningTasks,
                TerminalMenuCommand::RestartRunningTask,
                TerminalMenuCommand::TerminateTask,
                TerminalMenuCommand::ConfigureTasks,
                TerminalMenuCommand::ConfigureDefaultBuildTask,
            ]
        );
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(row, TerminalMenuRow::Separator))
                .count(),
            3
        );
    }

    #[test]
    fn idle_terminal_menu_disables_split_selection_and_task_control_rows() {
        let (_temp, app) = temp_app();
        assert_eq!(
            app.terminal_menu_items(),
            [
                (TerminalMenuCommand::NewTerminal, true),
                (TerminalMenuCommand::SplitTerminal, false),
                (TerminalMenuCommand::NewTerminalWindow, false),
                (TerminalMenuCommand::RunTask, true),
                (TerminalMenuCommand::RunBuildTask, true),
                (TerminalMenuCommand::RunActiveFile, false),
                (TerminalMenuCommand::RunSelectedText, false),
                (TerminalMenuCommand::ShowRunningTasks, false),
                (TerminalMenuCommand::RestartRunningTask, false),
                (TerminalMenuCommand::TerminateTask, false),
                (TerminalMenuCommand::ConfigureTasks, true),
                (TerminalMenuCommand::ConfigureDefaultBuildTask, true),
            ]
        );
    }

    #[test]
    fn running_task_enables_task_control_rows() {
        let (_temp, mut app) = temp_app();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        app.task_cancel = Some(cancel);
        app.task_running_label = Some("build".into());
        let items = app.terminal_menu_items();
        assert!(items.contains(&(TerminalMenuCommand::ShowRunningTasks, true)));
        assert!(items.contains(&(TerminalMenuCommand::RestartRunningTask, true)));
        assert!(items.contains(&(TerminalMenuCommand::TerminateTask, true)));
    }

    #[test]
    fn restart_requests_a_rerun_of_the_running_task_after_cancelling_it() {
        let (_temp, mut app) = temp_app();
        assert!(app.terminal_restart_running_task().is_err());

        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        app.task_cancel = Some(cancel.clone());
        app.task_running_label = Some("build".into());
        assert!(app.terminal_restart_running_task().is_ok());
        assert!(cancel.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(app.task_restart_pending.as_deref(), Some("build"));
    }

    #[test]
    fn show_running_tasks_requires_a_running_task() {
        let (_temp, mut app) = temp_app();
        assert!(app.terminal_show_running_tasks().is_err());
    }

    #[test]
    fn run_selected_text_and_active_file_fail_without_a_selection_or_file() {
        let (_temp, mut app) = temp_app();
        assert!(app.terminal_run_selected_text().is_err());
        assert!(app.terminal_run_active_file().is_err());
    }

    #[test]
    fn hit_testing_uses_the_drawn_row_layout() {
        let (_temp, app) = temp_app();
        assert_eq!(
            app.terminal_menu_command_at(120.0, 100.0 + 13.0, 100.0, 100.0),
            Some((TerminalMenuCommand::NewTerminal, true))
        );
        // The first separator sits after three 26px rows plus the 4px top padding.
        assert_eq!(
            app.terminal_menu_command_at(120.0, 100.0 + 86.0, 100.0, 100.0),
            None
        );
        assert_eq!(
            app.terminal_menu_command_at(99.0, 100.0 + 13.0, 100.0, 100.0),
            None
        );
    }

    #[test]
    fn keyboard_moves_the_terminal_selection() {
        let (_temp, mut app) = temp_app();
        app.workbench_menu = Some(crate::app::menu_bar::WorkbenchMenu::Terminal);
        assert!(app.workbench_menu_key(&Key::Named(NamedKey::ArrowDown)));
        assert_eq!(app.terminal_menu_selection, 1);
        assert!(app.workbench_menu_key(&Key::Named(NamedKey::ArrowUp)));
        assert_eq!(app.terminal_menu_selection, 0);
        assert!(app.workbench_menu_key(&Key::Named(NamedKey::Escape)));
        assert!(app.workbench_menu.is_none());
    }

    #[test]
    fn file_command_quotes_paths_for_the_target_shell() {
        assert_eq!(
            terminal_file_command(std::path::Path::new("C:\\a b\\it's.py"), true),
            "& 'C:\\a b\\it''s.py'\r"
        );
        assert_eq!(
            terminal_file_command(std::path::Path::new("/tmp/a b/it's.sh"), false),
            "'/tmp/a b/it'\\''s.sh'\r"
        );
    }

    #[test]
    fn selection_text_turns_line_breaks_into_terminal_returns() {
        assert_eq!(
            terminal_selection_text(&["a\nb".to_owned(), "c".to_owned()]),
            "a\rb\rc\r"
        );
        assert_eq!(
            terminal_selection_text(&["one\r\ntwo".to_owned()]),
            "one\rtwo\r"
        );
    }
}
