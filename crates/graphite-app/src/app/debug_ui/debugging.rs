use super::*;
use crate::debugger::{DebugState, RequestKind};

impl App {
    pub(crate) fn start_debugging(&mut self) -> Result<(), String> {
        if !self.workspace_trusted() {
            return Err("Workspace is restricted; trust it before starting a debugger".into());
        }
        if self.debug.session.is_some() {
            if matches!(self.debug.state, DebugState::Paused { .. }) {
                self.debug
                    .continue_execution()
                    .map_err(|error| error.to_string())?;
            } else if matches!(
                self.debug.state,
                DebugState::Stopped | DebugState::Failed(_)
            ) {
                self.debug.stop().map_err(|error| error.to_string())?;
                return self.start_debugging();
            }
            self.debug_panel_visible = true;
            return Ok(());
        }
        self.debug.launch_configs.clear();
        self.debug
            .refresh_configs(self.tree.root())
            .map_err(|error| error.to_string())?;
        let config = self
            .debug
            .launch_config()
            .cloned()
            .ok_or("No launch configuration is available")?;
        if self.debug_pending_launch.is_some() {
            return Err("A debug session is already waiting for its preLaunchTask".into());
        }
        if self.vsix_debug_adapter_pending() {
            return Err("A debug adapter is already being created".into());
        }
        if let Some(label) = config.pre_launch_task.clone() {
            // The session starts from `finish_debug_pre_launch` once the task succeeds.
            self.task_file()?
                .get(&label)
                .ok_or_else(|| format!("preLaunchTask '{label}' was not found in tasks.json"))?;
            self.run_task(&label)?;
            self.debug_pending_launch = Some(config);
            self.debug_panel_visible = true;
            self.status = format!("Running preLaunchTask: {label}");
            return Ok(());
        }
        self.begin_debug_session(config)
    }

    /// Names of the `launch.json` configurations, without duplicates, in file order.
    pub(crate) fn launch_config_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for config in crate::debugger::load_launch_configs(self.tree.root()).unwrap_or_default() {
            if !names.contains(&config.name) {
                names.push(config.name);
            }
        }
        names
    }

    /// Chooses the configuration that F5 and the Run menu start.
    pub(crate) fn select_debug_configuration(&mut self, name: &str) -> Result<(), String> {
        self.debug
            .refresh_configs(self.tree.root())
            .map_err(|error| error.to_string())?;
        if !self.debug.select_config(name) {
            return Err(format!("No launch configuration named '{name}'"));
        }
        self.status = format!("Debug configuration: {name}");
        Ok(())
    }

    /// Selects `name` and starts it. A running session must be stopped first so a
    /// configuration chosen from the list is never silently ignored.
    pub(crate) fn start_debug_configuration(&mut self, name: &str) -> Result<(), String> {
        if self.debug.session.is_some()
            || self.debug_pending_launch.is_some()
            || self.vsix_debug_adapter_pending()
        {
            return Err(
                "A debug session is already active; stop it before starting another".into(),
            );
        }
        self.select_debug_configuration(name)?;
        self.start_debugging()
    }

    fn begin_debug_session(
        &mut self,
        mut config: crate::debugger::LaunchConfig,
    ) -> Result<(), String> {
        match self.resolve_debug_adapter(&mut config)? {
            super::debug_adapters::AdapterSource::Ready => {
                self.start_resolved_debug_session(config)
            }
            // The extension creates its adapter in the extension host; the session starts from
            // `finish_vsix_debug_adapter` when the answer arrives.
            super::debug_adapters::AdapterSource::Extension(owner) => {
                self.request_vsix_debug_adapter(config, &owner)?;
                self.debug_panel_visible = true;
                Ok(())
            }
        }
    }

    /// Starts the session of a configuration whose adapter is known.
    pub(crate) fn start_resolved_debug_session(
        &mut self,
        config: crate::debugger::LaunchConfig,
    ) -> Result<(), String> {
        self.debug.active_file = self.documents[self.active_document()]
            .buffer
            .path()
            .map(std::path::Path::to_path_buf);
        self.debug
            .start(config, self.tree.root())
            .map_err(|error| error.to_string())?;
        self.debug_panel_visible = true;
        Ok(())
    }

    /// Called when a task ends. Starts the debug session that was waiting for it, but only if
    /// the task succeeded; a failed `preLaunchTask` aborts the launch like in VS Code.
    pub(crate) fn finish_debug_pre_launch(&mut self, result: &Result<i32, String>) {
        let Some(config) = self.debug_pending_launch.take() else {
            return;
        };
        let label = config.pre_launch_task.clone().unwrap_or_default();
        match result {
            Ok(0) => {
                if let Err(error) = self.begin_debug_session(config) {
                    self.status = error;
                }
            }
            Ok(code) => {
                self.status = format!(
                    "Debugging not started: preLaunchTask '{label}' exited with code {code}"
                )
            }
            Err(error) => {
                self.status =
                    format!("Debugging not started: preLaunchTask '{label}' failed: {error}")
            }
        }
    }

    /// Runs the `postDebugTask` of a session that has just ended.
    pub(crate) fn run_pending_post_debug_task(&mut self) {
        let Some(label) = self.debug.take_post_debug_task() else {
            return;
        };
        if let Err(error) = self.run_task(&label) {
            self.status = format!("postDebugTask '{label}' did not run: {error}");
        }
    }

    pub(crate) fn stop_debugging(&mut self) -> Result<(), String> {
        if self.debug_pending_launch.take().is_some() {
            // Stopping while the preLaunchTask runs abandons the launch.
            self.cancel_task();
            self.status = "Debugging cancelled".into();
        }
        if self.cancel_vsix_debug_adapter() {
            // Stopping while the extension builds its adapter abandons the launch.
            self.status = "Debugging cancelled".into();
        }
        self.debug.stop().map_err(|error| error.to_string())
    }

    pub(crate) fn debug_continue(&mut self) -> Result<(), String> {
        self.debug
            .continue_execution()
            .map_err(|error| error.to_string())
    }

    /// Steps the paused thread. DAP step requests require its `threadId`.
    pub(crate) fn debug_step(&mut self, command: &str) -> Result<(), String> {
        match command {
            "next" => self.debug.step_over(),
            "stepIn" => self.debug.step_into(),
            "stepOut" => self.debug.step_out(),
            other => return Err(format!("Unknown debug step: {other}")),
        }
        .map_err(|error| error.to_string())
    }

    pub(crate) fn toggle_breakpoint(&mut self) -> Result<(), String> {
        if !self.workspace_trusted() {
            return Err(
                "Workspace is restricted; trust it before configuring debug breakpoints".into(),
            );
        }
        let path = self.documents[self.active_document()]
            .buffer
            .path()
            .ok_or("Breakpoints require a saved file")?
            .to_path_buf();
        let line = self.editor().cursor_line_col().0 as u32 + 1;
        let points = self
            .debug
            .breakpoints
            .entry(path.to_path_buf())
            .or_default();
        if let Some(index) = points
            .iter()
            .position(|point| point["line"].as_u64() == Some(line as u64))
        {
            points.remove(index);
        } else {
            points.push(serde_json::json!({"line":line}));
        }
        let points = points.clone();
        if self.debug.session.is_some() {
            self.debug
                .set_breakpoints(&path, points)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(crate) fn configure_breakpoint(&mut self, value: &serde_json::Value) -> Result<(), String> {
        let path = self.documents[self.active_document()]
            .buffer
            .path()
            .ok_or("Breakpoints require a saved file")?
            .to_path_buf();
        let line = self.editor().cursor_line_col().0 as u32 + 1;
        self.configure_breakpoint_at(&path, line, value)
    }

    pub(crate) fn configure_breakpoint_at(
        &mut self,
        path: &std::path::Path,
        line: u32,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        if !self.workspace_trusted() {
            return Err(
                "Workspace is restricted; trust it before configuring debug breakpoints".into(),
            );
        }
        let points = self
            .debug
            .breakpoints
            .entry(path.to_path_buf())
            .or_default();
        points.retain(|point| point["line"].as_u64() != Some(line as u64));
        let mut breakpoint = serde_json::Map::new();
        breakpoint.insert("line".into(), serde_json::json!(line));
        for key in ["condition", "hitCondition", "logMessage"] {
            if let Some(value) = value[key].as_str().filter(|value| !value.trim().is_empty()) {
                breakpoint.insert(key.into(), serde_json::json!(value));
            }
        }
        points.push(serde_json::Value::Object(breakpoint));
        let configured_points = points.clone();
        if self.debug.session.is_some() {
            self.debug
                .set_breakpoints(&path, configured_points)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn workspace_trusted(&self) -> bool {
        self.workspace_trust
            .as_ref()
            .is_some_and(|trust| trust.is_trusted(self.tree.root()))
    }

    pub(crate) fn evaluate_debug_input(&mut self) -> Result<(), String> {
        let expression = self.debug_input.trim().to_owned();
        if expression.is_empty() {
            return Ok(());
        }
        self.debug
            .evaluate_repl(&expression)
            .map_err(|error| error.to_string())?;
        self.debug_input.clear();
        Ok(())
    }

    /// Adds the console input as a watch expression, shown in the variables column.
    pub(crate) fn add_debug_watch_from_input(&mut self) -> Result<(), String> {
        let expression = self.debug_input.trim().to_owned();
        if expression.is_empty() {
            return Ok(());
        }
        self.debug
            .add_watch(&expression)
            .map_err(|error| error.to_string())?;
        self.debug_input.clear();
        Ok(())
    }

    pub(crate) fn set_debug_request_kind(&mut self, kind: RequestKind) {
        if let Some(config) = self.debug.launch_configs.first_mut() {
            config.request = kind;
        }
    }
}

pub(crate) fn register(registry: &mut super::command_registry::CommandRegistry) {
    use super::command_execution::CommandError;
    for (id, title, action) in [
        ("workbench.action.debug.start", "Start Debugging", 0),
        ("workbench.action.debug.stop", "Stop Debugging", 1),
        ("workbench.action.debug.continue", "Continue", 2),
        ("workbench.action.debug.stepOver", "Step Over", 3),
        ("workbench.action.debug.stepInto", "Step Into", 4),
        ("workbench.action.debug.stepOut", "Step Out", 5),
        ("workbench.action.debug.restart", "Restart Debugging", 6),
        ("editor.debug.action.toggleBreakpoint", "Toggle Breakpoint", 7),
        (
            "graphite.debug.configureBreakpoint",
            "Configure Breakpoint (JSON)",
            16,
        ),
        (
            "workbench.debug.action.openConfiguration",
            "Open Configurations",
            20,
        ),
        (
            "workbench.debug.action.addConfiguration",
            "Add Configuration",
            21,
        ),
        (
            "editor.debug.action.conditionalBreakpoint",
            "Conditional Breakpoint",
            22,
        ),
        ("editor.debug.action.editBreakpoint", "Edit Breakpoint", 23),
        ("editor.debug.action.addLogPoint", "Logpoint", 24),
        (
            "workbench.debug.viewlet.action.enableAllBreakpoints",
            "Enable All Breakpoints",
            25,
        ),
        (
            "workbench.debug.viewlet.action.disableAllBreakpoints",
            "Disable All Breakpoints",
            26,
        ),
        (
            "workbench.debug.viewlet.action.removeAllBreakpoints",
            "Remove All Breakpoints",
            27,
        ),
        ("graphite.debug.evaluate", "Evaluate Debug Expression", 8),
        ("graphite.tests.open", "Open Test Explorer", 9),
        ("graphite.ports.open", "Open Local Ports", 10),
        ("graphite.preview.open", "Open Preview", 11),
        ("graphite.preview.close", "Close Preview", 12),
        ("graphite.preview.zoomIn", "Zoom In Image Preview", 18),
        ("graphite.preview.zoomOut", "Zoom Out Image Preview", 19),
        ("graphite.tests.runSelected", "Run Selected Test", 13),
        ("graphite.tests.cancel", "Cancel Test Run", 14),
        ("graphite.ports.refresh", "Refresh Local Ports", 15),
        (
            "graphite.ports.stopSelected",
            "Stop Selected Graphite Process",
            17,
        ),
    ] {
        let handler: super::command_registry::Handler = match action {
            0 => |app, _| app.start_debugging().map_err(CommandError::Failed),
            1 => |app, _| app.stop_debugging().map_err(CommandError::Failed),
            2 => |app, _| app.debug_continue().map_err(CommandError::Failed),
            3 => |app, _| app.debug_step("next").map_err(CommandError::Failed),
            4 => |app, _| app.debug_step("stepIn").map_err(CommandError::Failed),
            5 => |app, _| app.debug_step("stepOut").map_err(CommandError::Failed),
            6 => |app, _| {
                app.debug
                    .restart()
                    .map_err(|error| CommandError::Failed(error.to_string()))
            },
            7 => |app, _| app.toggle_breakpoint().map_err(CommandError::Failed),
            8 => |app, _| app.evaluate_debug_input().map_err(CommandError::Failed),
            9 => |app, _| {
                app.tests_visible = !app.tests_visible;
                Ok(())
            },
            10 => |app, _| {
                app.ports_visible = !app.ports_visible;
                if app.ports_visible {
                    app.refresh_local_ports().map_err(CommandError::Failed)?;
                }
                Ok(())
            },
            11 => |app, _| app.open_active_preview().map_err(CommandError::Failed),
            12 => |app, _| {
                app.markdown_preview = None;
                app.image_preview = None;
                app.close_webview();
                Ok(())
            },
            13 => |app, _| app.run_selected_test().map_err(CommandError::Failed),
            14 => |app, _| {
                app.cancel_test_run();
                Ok(())
            },
            16 => |app, invocation| {
                app.configure_breakpoint(&invocation.request.arguments)
                    .map_err(CommandError::Failed)
            },
            17 => |app, _| {
                let process_id = app.selected_port_process.ok_or_else(|| {
                    CommandError::Failed("Select a listed Graphite process first".into())
                })?;
                app.stop_port_process(process_id)
                    .map_err(CommandError::Failed)?;
                app.selected_port_process = None;
                app.refresh_local_ports().map_err(CommandError::Failed)
            },
            18 => |app, _| {
                app.preview_zoom(1.2);
                Ok(())
            },
            19 => |app, _| {
                app.preview_zoom(1.0 / 1.2);
                Ok(())
            },
            20 => |app, _| {
                app.open_debug_configuration(false);
                Ok(())
            },
            21 => |app, _| {
                app.open_debug_configuration(true);
                Ok(())
            },
            22 => |app, _| {
                app.execute_run_menu_command(
                    super::run_menu::RunMenuCommand::ConditionalBreakpoint,
                );
                Ok(())
            },
            23 => |app, _| {
                app.execute_run_menu_command(super::run_menu::RunMenuCommand::EditBreakpoint);
                Ok(())
            },
            24 => |app, _| {
                app.execute_run_menu_command(super::run_menu::RunMenuCommand::Logpoint);
                Ok(())
            },
            25 => |app, _| {
                app.set_all_breakpoints_enabled(true);
                Ok(())
            },
            26 => |app, _| {
                app.set_all_breakpoints_enabled(false);
                Ok(())
            },
            27 => |app, _| {
                app.remove_all_breakpoints();
                Ok(())
            },
            _ => |app, _| app.refresh_local_ports().map_err(CommandError::Failed),
        };
        registry
            .register(super::command_registry::CommandDescriptor {
                id,
                title,
                category: if action < 9 || (20..=27).contains(&action) {
                    "Debug"
                } else if action == 9 {
                    "Testing"
                } else if action == 10 {
                    "Ports"
                } else {
                    "Preview"
                },
                targets: &[
                    super::command_registry::TargetKind::Global,
                    super::command_registry::TargetKind::View,
                ],
                enablement: "true",
                menu_when: None,
                arguments: if action == 16 {
                    super::command_registry::ArgumentKind::Text
                } else {
                    super::command_registry::ArgumentKind::None
                },
                widget: None,
                handler,
            })
            .expect("unique phase 6 command");
    }

    for (id, title) in [
        ("workbench.action.debug.run", "Run Without Debugging"),
        ("graphite.debug.inlineBreakpoint", "Inline Breakpoint"),
        ("graphite.debug.triggeredBreakpoint", "Triggered Breakpoint"),
        (
            "debug.installAdditionalDebuggers",
            "Install Additional Debuggers",
        ),
    ] {
        registry
            .register(super::command_registry::CommandDescriptor {
                id,
                title,
                category: "Debug",
                targets: &[
                    super::command_registry::TargetKind::Global,
                    super::command_registry::TargetKind::View,
                ],
                enablement: "false",
                menu_when: None,
                arguments: super::command_registry::ArgumentKind::None,
                widget: None,
                handler: |_, _| Ok(()),
            })
            .expect("unique unavailable debug command");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::LaunchConfig;
    use std::sync::{atomic::Ordering, Arc};

    fn config(pre: Option<&str>) -> LaunchConfig {
        serde_json::from_value(serde_json::json!({
            "name": "x", "type": "t", "request": "launch", "preLaunchTask": pre,
        }))
        .unwrap()
    }

    fn trusted_app(dir: &crate::test_support::TempDir) -> App {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.workspace_trust =
            Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
        app.run_ui_command("graphite.security.trustWorkspace");
        app
    }

    #[test]
    fn a_successful_pre_launch_task_starts_the_session() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.debug_pending_launch = Some(config(Some("build")));
        app.finish_debug_pre_launch(&Ok(0));
        assert!(app.debug_pending_launch.is_none());
        // No extension provides the fixture type, so reaching this error proves the launch was attempted.
        assert!(
            app.status
                .contains("No installed extension provides a debugger"),
            "status was: {}",
            app.status
        );
    }

    #[test]
    fn a_failing_pre_launch_task_aborts_the_launch() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        for (result, expected) in [
            (Ok(2), "exited with code 2"),
            (Err("Task cancelled".to_owned()), "failed: Task cancelled"),
        ] {
            app.debug_pending_launch = Some(config(Some("build")));
            app.finish_debug_pre_launch(&result);
            assert!(app.debug_pending_launch.is_none());
            assert!(
                app.status.contains("Debugging not started"),
                "{}",
                app.status
            );
            assert!(app.status.contains("'build'"), "{}", app.status);
            assert!(app.status.contains(expected), "{}", app.status);
            assert!(app.debug.session.is_none());
        }
    }

    #[test]
    fn a_finished_task_unrelated_to_debugging_changes_nothing() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.status = "unchanged".into();
        app.finish_debug_pre_launch(&Ok(0));
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn starting_with_an_unknown_pre_launch_task_reports_it_and_does_not_wait() {
        let dir = crate::test_support::TempDir::new();
        std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
        std::fs::write(
            dir.path().join(".vscode/launch.json"),
            r#"{"configurations":[{"name":"x","type":"t","request":"launch","preLaunchTask":"nope"}]}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".vscode/tasks.json"),
            r#"{"tasks":[{"label":"build","type":"process","command":"echo"}]}"#,
        )
        .unwrap();
        let mut app = trusted_app(&dir);
        let error = app.start_debugging().unwrap_err();
        assert!(
            error.contains("'nope'") && error.contains("not found"),
            "{error}"
        );
        assert!(app.debug_pending_launch.is_none());
        assert!(app.task_cancel.is_none(), "no task was started");
    }

    #[test]
    fn stopping_while_the_pre_launch_task_runs_abandons_the_launch() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        app.task_cancel = Some(cancel.clone());
        app.debug_pending_launch = Some(config(Some("build")));
        app.stop_debugging().unwrap();
        assert!(app.debug_pending_launch.is_none());
        assert!(cancel.load(Ordering::Acquire), "the task is cancelled");
        assert_eq!(app.status, "Debugging cancelled");
    }

    #[test]
    fn a_second_start_while_waiting_for_the_task_is_refused() {
        let dir = crate::test_support::TempDir::new();
        std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
        std::fs::write(
            dir.path().join(".vscode/launch.json"),
            r#"{"configurations":[{"name":"x","type":"t","request":"launch"}]}"#,
        )
        .unwrap();
        let mut app = trusted_app(&dir);
        app.debug_pending_launch = Some(config(Some("build")));
        assert!(app
            .start_debugging()
            .unwrap_err()
            .contains("already waiting"));
    }

    #[test]
    fn a_post_debug_task_that_cannot_run_is_reported() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.debug.queue_post_debug_task_for_test("clean");
        app.run_pending_post_debug_task();
        assert!(
            app.status.contains("postDebugTask 'clean' did not run"),
            "status was: {}",
            app.status
        );
        // It is handed out only once.
        app.status.clear();
        app.run_pending_post_debug_task();
        assert!(app.status.is_empty());
    }

    fn write_launch_json(dir: &crate::test_support::TempDir, body: &str) {
        std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
        std::fs::write(dir.path().join(".vscode/launch.json"), body).unwrap();
    }

    const TWO_CONFIGS: &str = r#"{"configurations":[
        {"name":"API","type":"node","request":"launch"},
        {"name":"Worker","type":"node","request":"launch"},
        {"name":"API","type":"node","request":"attach"}
    ]}"#;

    #[test]
    fn the_palette_offers_start_and_select_for_each_distinct_configuration() {
        let dir = crate::test_support::TempDir::new();
        write_launch_json(&dir, TWO_CONFIGS);
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_command_palette();
        let items: Vec<_> = app
            .command_palette
            .as_ref()
            .unwrap()
            .items
            .iter()
            .filter(|item| item.source == "Debug")
            .map(|item| item.id.clone())
            .collect();
        for id in [
            "graphite.debug.start:API",
            "graphite.debug.select:API",
            "graphite.debug.start:Worker",
            "graphite.debug.select:Worker",
        ] {
            assert_eq!(items.iter().filter(|item| *item == id).count(), 1, "{id}");
        }
        assert_eq!(items.len(), 4, "the duplicated name is listed once");
    }

    #[test]
    fn starting_from_the_palette_is_disabled_until_the_workspace_is_trusted() {
        let dir = crate::test_support::TempDir::new();
        write_launch_json(&dir, TWO_CONFIGS);
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.workspace_trust = Some(crate::app::phase5::WorkspaceTrust::restricted(
            dir.path().join("trust.json"),
        ));
        app.open_command_palette();
        let enabled = |app: &App, id: &str| {
            app.command_palette
                .as_ref()
                .unwrap()
                .items
                .iter()
                .find(|item| item.id == id)
                .map(|item| item.enabled)
        };
        assert_eq!(enabled(&app, "graphite.debug.start:API"), Some(false));
        assert_eq!(enabled(&app, "graphite.debug.select:API"), Some(true));
    }

    #[test]
    fn selecting_a_configuration_from_the_palette_changes_what_f5_starts() {
        let dir = crate::test_support::TempDir::new();
        write_launch_json(&dir, TWO_CONFIGS);
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_command_palette();
        app.command_palette.as_mut().unwrap().query = "select debug configuration: worker".into();
        app.execute_command_palette_selection();
        assert_eq!(app.status, "Debug configuration: Worker");
        assert_eq!(app.debug.launch_config().unwrap().name, "Worker");
    }

    #[test]
    fn an_unknown_configuration_name_is_refused() {
        let dir = crate::test_support::TempDir::new();
        write_launch_json(&dir, TWO_CONFIGS);
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let error = app.select_debug_configuration("Nope").unwrap_err();
        assert_eq!(error, "No launch configuration named 'Nope'");
        assert!(app.debug.selected_config.is_none());
    }

    #[test]
    fn starting_a_named_configuration_never_hijacks_a_live_session() {
        let dir = crate::test_support::TempDir::new();
        write_launch_json(&dir, TWO_CONFIGS);
        let mut app = trusted_app(&dir);
        app.debug_pending_launch = Some(config(Some("build")));
        let error = app.start_debug_configuration("Worker").unwrap_err();
        assert!(error.contains("already active"), "{error}");
        assert!(
            app.debug.selected_config.is_none(),
            "the refused request does not change the selection"
        );
    }

    #[test]
    fn starting_a_named_configuration_selects_it_before_starting() {
        let dir = crate::test_support::TempDir::new();
        write_launch_json(&dir, TWO_CONFIGS);
        let mut app = trusted_app(&dir);
        // The fixture type has no provider, so the start itself fails after the selection.
        let error = app.start_debug_configuration("Worker").unwrap_err();
        assert!(error.contains("No installed extension provides a debugger for type 'node'"));
        assert_eq!(app.debug.selected_config.as_deref(), Some("Worker"));
    }

    #[test]
    fn the_run_menu_can_start_a_configuration_that_relies_on_an_extension_adapter() {
        let dir = crate::test_support::TempDir::new();
        write_launch_json(&dir, TWO_CONFIGS);
        let mut app = trusted_app(&dir);
        app.open_run_menu();
        let start = app
            .run_menu_rows()
            .into_iter()
            .find_map(|row| match row {
                crate::app::run_menu::RunMenuRow::Item {
                    command: crate::app::run_menu::RunMenuCommand::StartDebugging,
                    enabled,
                    ..
                } => Some(enabled),
                _ => None,
            })
            .unwrap();
        assert!(
            start,
            "no explicit \"adapter\" is needed to enable Start Debugging"
        );
    }
}
