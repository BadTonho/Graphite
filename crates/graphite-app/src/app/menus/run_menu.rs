use super::*;
use crate::debugger::DebugState;

const ROW_HEIGHT: f32 = 28.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
pub(crate) const RUN_MENU_WIDTH: f32 = 380.0;
pub(crate) const BREAKPOINT_SUBMENU_WIDTH: f32 = 250.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunMenuCommand {
    StartDebugging,
    RunWithoutDebugging,
    StopDebugging,
    RestartDebugging,
    OpenConfigurations,
    AddConfiguration,
    StepOver,
    StepInto,
    StepOut,
    Continue,
    ToggleBreakpoint,
    NewBreakpoint,
    ConditionalBreakpoint,
    EditBreakpoint,
    InlineBreakpoint,
    FunctionBreakpoint,
    Logpoint,
    TriggeredBreakpoint,
    EnableAllBreakpoints,
    DisableAllBreakpoints,
    RemoveAllBreakpoints,
    InstallDebuggers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunMenuRow {
    Separator,
    Item {
        command: RunMenuCommand,
        label: &'static str,
        shortcut: &'static str,
        enabled: bool,
        submenu: bool,
        unavailable_reason: Option<&'static str>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BreakpointPromptKind {
    Conditional,
    Edit,
    Logpoint,
}

#[derive(Clone, Debug)]
pub(crate) struct BreakpointPrompt {
    pub kind: BreakpointPromptKind,
    pub path: std::path::PathBuf,
    pub line: u32,
    pub value: String,
}

impl App {
    pub(crate) fn open_run_menu(&mut self) {
        self.debug.launch_configs.clear();
        let _ = self.debug.refresh_configs(self.tree.root());
        self.run_menu_selection = 0;
        self.run_breakpoint_submenu = false;
        self.run_breakpoint_selection = 0;
    }

    pub(crate) fn run_menu_rows(&self) -> Vec<RunMenuRow> {
        use RunMenuCommand as C;
        use RunMenuRow::{Item as I, Separator as S};
        let trusted = self
            .workspace_trust
            .as_ref()
            .is_some_and(|trust| trust.is_trusted(self.tree.root()));
        let has_session = self.debug.session.is_some();
        let paused = matches!(self.debug.state, DebugState::Paused { .. });
        let config_exists = self.tree.root().join(".vscode/launch.json").is_file();
        let has_config = self.debug.launch_config().is_some();
        let active_path = self.documents[self.active_document()].buffer.path();
        let has_saved_file = active_path.is_some();
        let any_breakpoints = self
            .debug
            .breakpoints
            .values()
            .any(|points| !points.is_empty());
        let row = |command, label, shortcut, enabled, submenu, unavailable_reason| I {
            command,
            label,
            shortcut,
            enabled,
            submenu,
            unavailable_reason,
        };
        vec![
            row(
                C::StartDebugging,
                "run.start_debugging",
                "F5",
                trusted && has_config && !has_session && !paused,
                false,
                (!trusted)
                    .then_some("run.reason.workspace_untrusted")
                    .or_else(|| {
                        (!has_config)
                            .then_some("run.reason.no_configuration")
                            .or_else(|| has_session.then_some("run.reason.session_active"))
                    }),
            ),
            row(
                C::RunWithoutDebugging,
                "run.without_debugging",
                "Ctrl+F5",
                false,
                false,
                Some("run.reason.not_supported"),
            ),
            row(
                C::StopDebugging,
                "run.stop_debugging",
                "Shift+F5",
                has_session,
                false,
                Some("run.reason.no_session"),
            ),
            row(
                C::RestartDebugging,
                "run.restart_debugging",
                "Ctrl+Shift+F5",
                has_session,
                false,
                Some("run.reason.no_session"),
            ),
            S,
            row(
                C::OpenConfigurations,
                "run.open_configurations",
                "",
                config_exists,
                false,
                Some("run.reason.no_configuration"),
            ),
            row(
                C::AddConfiguration,
                "run.add_configuration",
                "",
                true,
                false,
                None,
            ),
            S,
            row(
                C::StepOver,
                "run.step_over",
                "F10",
                paused,
                false,
                Some("run.reason.not_paused"),
            ),
            row(
                C::StepInto,
                "run.step_into",
                "F11",
                paused,
                false,
                Some("run.reason.not_paused"),
            ),
            row(
                C::StepOut,
                "run.step_out",
                "Shift+F11",
                paused,
                false,
                Some("run.reason.not_paused"),
            ),
            row(
                C::Continue,
                "run.continue",
                "F5",
                paused,
                false,
                Some("run.reason.not_paused"),
            ),
            S,
            row(
                C::ToggleBreakpoint,
                "run.toggle_breakpoint",
                "F9",
                trusted && has_saved_file,
                false,
                (!trusted)
                    .then_some("run.reason.workspace_untrusted")
                    .or_else(|| (!has_saved_file).then_some("run.reason.saved_file_required")),
            ),
            row(C::NewBreakpoint, "run.new_breakpoint", "", true, true, None),
            S,
            row(
                C::EnableAllBreakpoints,
                "run.enable_all_breakpoints",
                "",
                any_breakpoints && self.debug.breakpoints_disabled,
                false,
                Some(if any_breakpoints {
                    "run.reason.breakpoints_already_enabled"
                } else {
                    "run.reason.no_breakpoints"
                }),
            ),
            row(
                C::DisableAllBreakpoints,
                "run.disable_all_breakpoints",
                "",
                any_breakpoints && !self.debug.breakpoints_disabled,
                false,
                Some(if any_breakpoints {
                    "run.reason.breakpoints_already_disabled"
                } else {
                    "run.reason.no_breakpoints"
                }),
            ),
            row(
                C::RemoveAllBreakpoints,
                "run.remove_all_breakpoints",
                "",
                any_breakpoints,
                false,
                Some("run.reason.no_breakpoints"),
            ),
            S,
            row(
                C::InstallDebuggers,
                "run.install_debuggers",
                "",
                false,
                false,
                Some("run.reason.not_supported"),
            ),
        ]
    }

    pub(crate) fn run_menu_width(&self, char_width: f32, available_width: f32) -> f32 {
        self.measure_run_rows(
            &self.run_menu_rows(),
            char_width,
            available_width,
            RUN_MENU_WIDTH,
        )
    }

    pub(crate) fn run_breakpoint_menu_width(&self, char_width: f32, available_width: f32) -> f32 {
        self.measure_run_rows(
            &self.run_breakpoint_rows(),
            char_width,
            available_width,
            BREAKPOINT_SUBMENU_WIDTH,
        )
    }

    fn measure_run_rows(
        &self,
        rows: &[RunMenuRow],
        char_width: f32,
        available_width: f32,
        minimum: f32,
    ) -> f32 {
        rows.iter()
            .filter_map(|row| match row {
                RunMenuRow::Item {
                    label,
                    shortcut,
                    submenu,
                    ..
                } => Some(
                    30.0 + self.t(label).chars().count() as f32 * char_width
                        + if shortcut.is_empty() {
                            14.0 + if *submenu { 18.0 } else { 0.0 }
                        } else {
                            14.0 + shortcut.chars().count() as f32 * char_width
                                + 12.0
                                + if *submenu { 18.0 } else { 0.0 }
                        },
                ),
                RunMenuRow::Separator => None,
            })
            .fold(minimum, f32::max)
            .min(available_width.max(0.0))
    }

    /// Top edge of the breakpoint submenu: level with the row that opens it and kept inside
    /// the window.
    pub(crate) fn run_breakpoint_submenu_top(&self, menu_top: f32, window_height: f32) -> f32 {
        let mut row_top = menu_top + 4.0;
        let mut anchor = menu_top;
        for row in self.run_menu_rows() {
            match row {
                RunMenuRow::Separator => row_top += SEPARATOR_HEIGHT,
                RunMenuRow::Item { submenu, .. } => {
                    if submenu {
                        anchor = row_top - 4.0;
                        break;
                    }
                    row_top += ROW_HEIGHT;
                }
            }
        }
        let height = self
            .run_breakpoint_rows()
            .iter()
            .map(|row| match row {
                RunMenuRow::Separator => SEPARATOR_HEIGHT,
                RunMenuRow::Item { .. } => ROW_HEIGHT,
            })
            .sum::<f32>()
            + 8.0;
        anchor.min(window_height - height).max(menu_top)
    }

    pub(crate) fn run_breakpoint_submenu_position(
        &self,
        main_x: f32,
        main_width: f32,
        window_width: f32,
        submenu_width: f32,
    ) -> f32 {
        if main_x + main_width + submenu_width <= window_width {
            main_x + main_width
        } else {
            (main_x - submenu_width).max(0.0)
        }
    }

    pub(crate) fn run_breakpoint_rows(&self) -> Vec<RunMenuRow> {
        use RunMenuCommand as C;
        use RunMenuRow::Item as I;
        let trusted = self
            .workspace_trust
            .as_ref()
            .is_some_and(|trust| trust.is_trusted(self.tree.root()));
        let path = self.documents[self.active_document()].buffer.path();
        let line = self.editor().cursor_line_col().0 as u32 + 1;
        let saved = path.is_some();
        let has_breakpoint = path
            .and_then(|path| self.debug.breakpoints.get(path))
            .is_some_and(|points| {
                points
                    .iter()
                    .any(|point| point["line"].as_u64() == Some(line as u64))
            });
        let supported = trusted && saved;
        let row = |command, label, shortcut, enabled, reason| I {
            command,
            label,
            shortcut,
            enabled,
            submenu: false,
            unavailable_reason: reason,
        };
        vec![
            row(
                C::ConditionalBreakpoint,
                "run.conditional_breakpoint",
                "",
                supported,
                (!trusted)
                    .then_some("run.reason.workspace_untrusted")
                    .or_else(|| (!saved).then_some("run.reason.saved_file_required")),
            ),
            row(
                C::EditBreakpoint,
                "run.edit_breakpoint",
                "",
                supported && has_breakpoint,
                (!trusted)
                    .then_some("run.reason.workspace_untrusted")
                    .or_else(|| (!has_breakpoint).then_some("run.reason.breakpoint_required")),
            ),
            row(
                C::InlineBreakpoint,
                "run.inline_breakpoint",
                "Shift+F9",
                false,
                Some("run.reason.not_supported"),
            ),
            row(
                C::FunctionBreakpoint,
                "run.function_breakpoint",
                "",
                false,
                Some("run.reason.not_supported"),
            ),
            row(
                C::Logpoint,
                "run.logpoint",
                "",
                supported,
                (!trusted)
                    .then_some("run.reason.workspace_untrusted")
                    .or_else(|| (!saved).then_some("run.reason.saved_file_required")),
            ),
            row(
                C::TriggeredBreakpoint,
                "run.triggered_breakpoint",
                "",
                false,
                Some("run.reason.not_supported"),
            ),
        ]
    }

    pub(crate) fn run_menu_command_at(
        &self,
        x: f32,
        y: f32,
        popup_x: f32,
        popup_y: f32,
        width: f32,
    ) -> Option<(RunMenuCommand, bool, bool)> {
        if x < popup_x || x >= popup_x + width || y < popup_y + 4.0 {
            return None;
        }
        row_at(&self.run_menu_rows(), y - popup_y - 4.0)
    }

    pub(crate) fn run_breakpoint_command_at(
        &self,
        x: f32,
        y: f32,
        popup_x: f32,
        popup_y: f32,
        width: f32,
    ) -> Option<(RunMenuCommand, bool)> {
        if x < popup_x || x >= popup_x + width || y < popup_y + 4.0 {
            return None;
        }
        row_at(&self.run_breakpoint_rows(), y - popup_y - 4.0)
            .map(|(command, enabled, _)| (command, enabled))
    }

    pub(crate) fn run_menu_pointer_selection(&mut self, y: f32, popup_y: f32, submenu: bool) {
        let rows = if submenu {
            self.run_breakpoint_rows()
        } else {
            self.run_menu_rows()
        };
        if let Some((index, command)) = selectable_row_at(&rows, y - popup_y - 4.0) {
            if submenu {
                self.run_breakpoint_selection = index;
            } else {
                self.run_menu_selection = index;
                self.run_breakpoint_submenu = command == RunMenuCommand::NewBreakpoint;
                if self.run_breakpoint_submenu {
                    self.run_breakpoint_selection = 0;
                }
            }
        }
    }

    pub(crate) fn run_menu_move_selection(&mut self, delta: isize, submenu: bool) {
        let rows = if submenu {
            self.run_breakpoint_rows()
        } else {
            self.run_menu_rows()
        };
        let count = rows
            .iter()
            .filter(|row| matches!(row, RunMenuRow::Item { .. }))
            .count();
        let selected = if submenu {
            &mut self.run_breakpoint_selection
        } else {
            &mut self.run_menu_selection
        };
        *selected = if count == 0 {
            0
        } else {
            selected.saturating_add_signed(delta).min(count - 1)
        };
    }

    pub(crate) fn run_menu_selected(&self, submenu: bool) -> Option<(RunMenuCommand, bool, bool)> {
        let index = if submenu {
            self.run_breakpoint_selection
        } else {
            self.run_menu_selection
        };
        self.run_menu_items_for(submenu).get(index).copied()
    }

    fn run_menu_items_for(&self, submenu: bool) -> Vec<(RunMenuCommand, bool, bool)> {
        if submenu {
            self.run_breakpoint_rows()
        } else {
            self.run_menu_rows()
        }
        .into_iter()
        .filter_map(|row| match row {
            RunMenuRow::Item {
                command,
                enabled,
                submenu,
                ..
            } => Some((command, enabled, submenu)),
            RunMenuRow::Separator => None,
        })
        .collect()
    }

    pub(crate) fn activate_run_menu_accessibility_item(&mut self, index: usize, submenu: bool) {
        if let Some((command, enabled, has_submenu)) =
            self.run_menu_items_for(submenu).get(index).copied()
        {
            if !enabled {
                self.show_run_menu_unavailable_reason(command, submenu);
                return;
            }
            if has_submenu {
                self.run_breakpoint_submenu = true;
                self.run_breakpoint_selection = 0;
            } else {
                self.execute_run_menu_command(command);
                self.close_workbench_menu();
            }
        }
    }

    pub(crate) fn show_run_menu_unavailable_reason(
        &mut self,
        command: RunMenuCommand,
        submenu: bool,
    ) {
        let rows = if submenu {
            self.run_breakpoint_rows()
        } else {
            self.run_menu_rows()
        };
        if let Some(reason) = rows.into_iter().find_map(|row| match row {
            RunMenuRow::Item {
                command: row_command,
                unavailable_reason,
                ..
            } if row_command == command => unavailable_reason,
            _ => None,
        }) {
            self.status = self.t(reason).into();
        }
    }

    pub(crate) fn execute_run_menu_command(&mut self, command: RunMenuCommand) {
        use RunMenuCommand as C;
        match command {
            C::StartDebugging => self.run_ui_command("workbench.action.debug.start"),
            C::RunWithoutDebugging
            | C::InlineBreakpoint
            | C::FunctionBreakpoint
            | C::TriggeredBreakpoint
            | C::InstallDebuggers
            | C::NewBreakpoint => {}
            C::StopDebugging => self.run_ui_command("workbench.action.debug.stop"),
            C::RestartDebugging => self.run_ui_command("workbench.action.debug.restart"),
            C::OpenConfigurations => self.open_debug_configuration(false),
            C::AddConfiguration => self.open_debug_configuration(true),
            C::StepOver => self.run_ui_command("workbench.action.debug.stepOver"),
            C::StepInto => self.run_ui_command("workbench.action.debug.stepInto"),
            C::StepOut => self.run_ui_command("workbench.action.debug.stepOut"),
            C::Continue => self.run_ui_command("workbench.action.debug.continue"),
            C::ToggleBreakpoint => self.run_ui_command("editor.debug.action.toggleBreakpoint"),
            C::ConditionalBreakpoint => {
                self.begin_breakpoint_prompt(BreakpointPromptKind::Conditional)
            }
            C::EditBreakpoint => self.begin_breakpoint_prompt(BreakpointPromptKind::Edit),
            C::Logpoint => self.begin_breakpoint_prompt(BreakpointPromptKind::Logpoint),
            C::EnableAllBreakpoints => self.set_all_breakpoints_enabled(true),
            C::DisableAllBreakpoints => self.set_all_breakpoints_enabled(false),
            C::RemoveAllBreakpoints => self.remove_all_breakpoints(),
        }
    }

    pub(crate) fn open_debug_configuration(&mut self, create: bool) {
        let path = self.tree.root().join(".vscode/launch.json");
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                self.status = error.to_string();
                return;
            }
        }
        if create && !path.exists() {
            let contents = "{\n  \"version\": \"0.2.0\",\n  \"configurations\": [\n    {\n      \"name\": \"Debug\",\n      \"type\": \"adapter-id\",\n      \"request\": \"launch\",\n      \"adapter\": { \"command\": \"path/to/debug-adapter\" }\n    }\n  ]\n}\n";
            if let Err(error) = std::fs::write(&path, contents) {
                self.status = error.to_string();
                return;
            }
        }
        self.open_file(path);
    }

    fn begin_breakpoint_prompt(&mut self, kind: BreakpointPromptKind) {
        let Some(path) = self.documents[self.active_document()]
            .buffer
            .path()
            .map(ToOwned::to_owned)
        else {
            return;
        };
        let line = self.editor().cursor_line_col().0 as u32 + 1;
        let existing = self.debug.breakpoints.get(&path).and_then(|points| {
            points
                .iter()
                .find(|point| point["line"].as_u64() == Some(line as u64))
        });
        let value = match kind {
            BreakpointPromptKind::Conditional => existing
                .and_then(|point| point["condition"].as_str())
                .unwrap_or("")
                .to_owned(),
            BreakpointPromptKind::Logpoint => existing
                .and_then(|point| point["logMessage"].as_str())
                .unwrap_or("")
                .to_owned(),
            BreakpointPromptKind::Edit => existing
                .map(|point| {
                    let mut object = point.as_object().cloned().unwrap_or_default();
                    object.remove("line");
                    serde_json::Value::Object(object).to_string()
                })
                .unwrap_or_else(|| "{}".into()),
        };
        self.run_breakpoint_prompt = Some(BreakpointPrompt {
            kind,
            path,
            line,
            value,
        });
        self.close_workbench_menu();
    }

    pub(crate) fn handle_breakpoint_prompt_key(
        &mut self,
        key: &winit::keyboard::Key,
        text: Option<&str>,
    ) -> bool {
        let Some(prompt) = self.run_breakpoint_prompt.as_mut() else {
            return false;
        };
        match key {
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                self.run_breakpoint_prompt = None
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => {
                let prompt = self.run_breakpoint_prompt.take().unwrap();
                let value = match prompt.kind {
                    BreakpointPromptKind::Conditional => {
                        serde_json::json!({"condition":prompt.value})
                    }
                    BreakpointPromptKind::Logpoint => {
                        serde_json::json!({"logMessage":prompt.value})
                    }
                    BreakpointPromptKind::Edit => {
                        match serde_json::from_str::<serde_json::Value>(&prompt.value) {
                            Ok(value) if value.is_object() => value,
                            _ => {
                                self.status = self.t("run.invalid_breakpoint_json").into();
                                self.run_breakpoint_prompt = Some(prompt);
                                return true;
                            }
                        }
                    }
                };
                if let Err(error) = self.configure_breakpoint_at(&prompt.path, prompt.line, &value)
                {
                    self.status = error;
                    self.run_breakpoint_prompt = Some(prompt);
                }
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => {
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

    pub(crate) fn set_all_breakpoints_enabled(&mut self, enabled: bool) {
        self.debug.breakpoints_disabled = !enabled;
        if self.debug.session.is_some() {
            let files: Vec<_> = self.debug.breakpoints.keys().cloned().collect();
            for file in files {
                let points = self
                    .debug
                    .breakpoints
                    .get(&file)
                    .cloned()
                    .unwrap_or_default();
                if let Err(error) = self.debug.set_breakpoints(&file, points) {
                    self.status = error.to_string();
                    break;
                }
            }
        }
    }

    pub(crate) fn remove_all_breakpoints(&mut self) {
        let files: Vec<_> = self.debug.breakpoints.keys().cloned().collect();
        self.debug.breakpoints.clear();
        if self.debug.session.is_some() {
            for file in files {
                if let Err(error) = self.debug.set_breakpoints(&file, Vec::new()) {
                    self.status = error.to_string();
                    break;
                }
                self.debug.breakpoints.remove(&file);
            }
        }
        self.debug.breakpoints_disabled = false;
    }
}

fn row_at(rows: &[RunMenuRow], y: f32) -> Option<(RunMenuCommand, bool, bool)> {
    let mut top = 0.0;
    for row in rows {
        let height = if matches!(row, RunMenuRow::Separator) {
            SEPARATOR_HEIGHT
        } else {
            ROW_HEIGHT
        };
        if y >= top && y < top + height {
            return match row {
                RunMenuRow::Item {
                    command,
                    enabled,
                    submenu,
                    ..
                } => Some((*command, *enabled, *submenu)),
                RunMenuRow::Separator => None,
            };
        }
        top += height;
    }
    None
}

fn selectable_row_at(rows: &[RunMenuRow], y: f32) -> Option<(usize, RunMenuCommand)> {
    let mut top = 0.0;
    let mut index = 0;
    for row in rows {
        let height = if matches!(row, RunMenuRow::Separator) {
            SEPARATOR_HEIGHT
        } else {
            ROW_HEIGHT
        };
        if y >= top && y < top + height {
            return match row {
                RunMenuRow::Item { command, .. } => Some((index, *command)),
                RunMenuRow::Separator => None,
            };
        }
        top += height;
        if matches!(row, RunMenuRow::Item { .. }) {
            index += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{Key, NamedKey};

    fn app() -> (crate::test_support::TempDir, App) {
        let root = crate::test_support::TempDir::new();
        let app = App::new_with_options(root.path().to_path_buf(), None, true);
        (root, app)
    }

    fn row(app: &App, command: RunMenuCommand) -> Option<(bool, Option<&'static str>)> {
        app.run_menu_rows().into_iter().find_map(|row| match row {
            RunMenuRow::Item {
                command: candidate,
                enabled,
                unavailable_reason,
                ..
            } if candidate == command => Some((enabled, unavailable_reason)),
            _ => None,
        })
    }

    #[test]
    fn run_menu_matches_reference_order_and_marks_unsupported_features_disabled() {
        let (_temp, app) = app();
        let commands: Vec<_> = app
            .run_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                RunMenuRow::Item { command, .. } => Some(command),
                RunMenuRow::Separator => None,
            })
            .collect();
        assert_eq!(
            commands,
            vec![
                RunMenuCommand::StartDebugging,
                RunMenuCommand::RunWithoutDebugging,
                RunMenuCommand::StopDebugging,
                RunMenuCommand::RestartDebugging,
                RunMenuCommand::OpenConfigurations,
                RunMenuCommand::AddConfiguration,
                RunMenuCommand::StepOver,
                RunMenuCommand::StepInto,
                RunMenuCommand::StepOut,
                RunMenuCommand::Continue,
                RunMenuCommand::ToggleBreakpoint,
                RunMenuCommand::NewBreakpoint,
                RunMenuCommand::EnableAllBreakpoints,
                RunMenuCommand::DisableAllBreakpoints,
                RunMenuCommand::RemoveAllBreakpoints,
                RunMenuCommand::InstallDebuggers,
            ]
        );
        assert_eq!(
            row(&app, RunMenuCommand::RunWithoutDebugging),
            Some((false, Some("run.reason.not_supported")))
        );
        assert_eq!(
            row(&app, RunMenuCommand::InstallDebuggers),
            Some((false, Some("run.reason.not_supported")))
        );
    }

    #[test]
    fn run_menu_width_clamps_to_window_and_submenu_flips_when_space_is_tight() {
        let (_temp, app) = app();
        assert!(app.run_menu_width(8.0, 1000.0) >= RUN_MENU_WIDTH);
        assert_eq!(app.run_menu_width(8.0, 190.0), 190.0);
        assert_eq!(
            app.run_breakpoint_submenu_position(600.0, 380.0, 900.0, 250.0),
            350.0
        );
        assert_eq!(
            app.run_breakpoint_submenu_position(30.0, 380.0, 1000.0, 250.0),
            410.0
        );
    }

    #[test]
    fn menu_hit_testing_uses_row_geometry_and_reports_disabled_items() {
        let (_temp, mut app) = app();
        let rows = app.run_menu_rows();
        let first_item_y = 4.0 + ROW_HEIGHT * 0.5;
        assert_eq!(
            app.run_menu_command_at(24.0, first_item_y, 20.0, 0.0, 400.0),
            Some((RunMenuCommand::StartDebugging, false, false))
        );

        // The second item is explicitly visible but disabled until a no-debug
        // execution path is implemented.
        assert_eq!(
            app.run_menu_command_at(24.0, first_item_y + ROW_HEIGHT, 20.0, 0.0, 400.0),
            Some((RunMenuCommand::RunWithoutDebugging, false, false))
        );

        let separator_y = 4.0 + ROW_HEIGHT * 4.0 + SEPARATOR_HEIGHT * 0.5;
        assert!(app
            .run_menu_command_at(24.0, separator_y, 20.0, 0.0, 400.0)
            .is_none());
        assert!(!rows.is_empty());

        let submenu_y = 4.0 + ROW_HEIGHT * 11.0 + SEPARATOR_HEIGHT * 3.0 + ROW_HEIGHT * 0.5;
        assert_eq!(
            app.run_menu_command_at(24.0, submenu_y, 20.0, 0.0, 400.0),
            Some((RunMenuCommand::NewBreakpoint, true, true))
        );

        app.show_run_menu_unavailable_reason(RunMenuCommand::RunWithoutDebugging, false);
        assert!(!app.status.is_empty());
    }

    #[test]
    fn adding_and_opening_debug_configuration_creates_a_starter_profile() {
        let (temp, mut app) = app();
        app.open_debug_configuration(true);
        let path = temp.path().join(".vscode/launch.json");
        assert!(path.is_file());
        let profiles = crate::debugger::load_launch_configs(temp.path()).unwrap();
        assert_eq!(profiles[0].name, "Debug");
        assert_eq!(
            app.documents[app.active_document()].buffer.path(),
            Some(path.canonicalize().unwrap().as_path())
        );
        app.open_debug_configuration(false);
        assert_eq!(
            app.documents[app.active_document()].buffer.path(),
            Some(path.canonicalize().unwrap().as_path())
        );
    }

    #[test]
    fn start_requires_trust_and_configuration() {
        let (temp, mut app) = app();
        app.open_run_menu();
        assert!(!row(&app, RunMenuCommand::StartDebugging).unwrap().0);
        let launch_path = temp.path().join(".vscode/launch.json");
        std::fs::create_dir_all(launch_path.parent().unwrap()).unwrap();
        std::fs::write(
            &launch_path,
            r#"{"configurations":[{"name":"Debug","type":"fixture","request":"launch","adapter":{"command":"adapter"}}]}"#,
        )
        .unwrap();
        app.open_run_menu();
        assert!(!row(&app, RunMenuCommand::StartDebugging).unwrap().0);
        app.workspace_trust = Some(crate::app::phase5::WorkspaceTrust::restricted(
            temp.path().join("trust.json"),
        ));
        app.open_run_menu();
        assert_eq!(
            row(&app, RunMenuCommand::StartDebugging),
            Some((false, Some("run.reason.workspace_untrusted")))
        );
        app.workspace_trust
            .as_mut()
            .unwrap()
            .trust(temp.path())
            .unwrap();
        app.open_run_menu();
        assert!(row(&app, RunMenuCommand::StartDebugging).unwrap().0);
        assert!(!row(&app, RunMenuCommand::Continue).unwrap().0);
    }

    #[test]
    fn conditional_breakpoint_prompt_and_bulk_actions_update_state() {
        let (temp, mut app) = app();
        let path = temp.path().join("main.rs");
        std::fs::write(&path, "fn main() {}\n").unwrap();
        app.open_file(path.clone());
        app.workspace_trust = Some(crate::app::phase5::WorkspaceTrust::restricted(
            temp.path().join("trust.json"),
        ));
        app.workspace_trust
            .as_mut()
            .unwrap()
            .trust(temp.path())
            .unwrap();
        app.begin_breakpoint_prompt(BreakpointPromptKind::Conditional);
        assert!(app.handle_breakpoint_prompt_key(&Key::Character("n".into()), Some("n")));
        assert!(app.handle_breakpoint_prompt_key(&Key::Named(NamedKey::Enter), None));
        assert_eq!(app.debug.breakpoints[&path][0]["condition"], "n");
        app.set_all_breakpoints_enabled(false);
        assert!(app.debug.breakpoints_disabled);
        app.set_all_breakpoints_enabled(true);
        assert!(!app.debug.breakpoints_disabled);
        app.remove_all_breakpoints();
        assert!(app.debug.breakpoints.is_empty());
    }

    #[test]
    fn breakpoint_submenu_is_level_with_its_parent_row() {
        let (_temp, app) = app();
        let above: f32 = app
            .run_menu_rows()
            .iter()
            .take_while(|row| !matches!(row, RunMenuRow::Item { submenu: true, .. }))
            .map(|row| match row {
                RunMenuRow::Separator => SEPARATOR_HEIGHT,
                RunMenuRow::Item { .. } => ROW_HEIGHT,
            })
            .sum();
        assert!(above > 0.0);
        assert_eq!(app.run_breakpoint_submenu_top(30.0, 900.0), 30.0 + above);
        // A short window pushes the submenu up, but never above the menu itself.
        let top = app.run_breakpoint_submenu_top(30.0, 60.0);
        assert_eq!(top, 30.0);
    }
}
