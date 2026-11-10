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
        if self.debug.launch_configs.is_empty() {
            self.debug
                .refresh_configs(self.tree.root())
                .map_err(|error| error.to_string())?;
        }
        let config = self
            .debug
            .launch_configs
            .first()
            .cloned()
            .ok_or("No launch configuration is available")?;
        self.debug
            .start(config, self.tree.root())
            .map_err(|error| error.to_string())?;
        self.debug_panel_visible = true;
        Ok(())
    }

    pub(crate) fn stop_debugging(&mut self) -> Result<(), String> {
        self.debug.stop().map_err(|error| error.to_string())
    }

    pub(crate) fn debug_continue(&mut self) -> Result<(), String> {
        self.debug
            .continue_execution()
            .map_err(|error| error.to_string())
    }

    pub(crate) fn debug_request(&mut self, command: &str) -> Result<(), String> {
        self.debug
            .request(command, serde_json::json!({}))
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
        let points = self.debug.breakpoints.entry(path.clone()).or_default();
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
        let points = self.debug.breakpoints.entry(path.clone()).or_default();
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
        let frame = self.debug.frames.first().map(|frame| frame.id);
        self.debug
            .evaluate_watch(&expression, frame)
            .map_err(|error| error.to_string())?;
        self.debug.watches.push(expression);
        self.debug_input.clear();
        Ok(())
    }

    pub(crate) fn set_debug_request_kind(&mut self, kind: RequestKind) {
        if let Some(config) = self.debug.launch_configs.first_mut() {
            config.request = kind;
        }
    }
}

pub(super) fn register(registry: &mut super::command_registry::CommandRegistry) {
    use super::command_execution::CommandError;
    for (id, title, action) in [
        ("workbench.action.debug.start", "Start Debugging", 0),
        ("workbench.action.debug.stop", "Stop Debugging", 1),
        ("workbench.action.debug.continue", "Continue", 2),
        ("workbench.action.debug.stepOver", "Step Over", 3),
        ("workbench.action.debug.stepInto", "Step Into", 4),
        ("workbench.action.debug.stepOut", "Step Out", 5),
        ("workbench.action.debug.restart", "Restart Debugging", 6),
        ("graphite.debug.toggleBreakpoint", "Toggle Breakpoint", 7),
        (
            "graphite.debug.configureBreakpoint",
            "Configure Breakpoint (JSON)",
            16,
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
            3 => |app, _| app.debug_request("next").map_err(CommandError::Failed),
            4 => |app, _| app.debug_request("stepIn").map_err(CommandError::Failed),
            5 => |app, _| app.debug_request("stepOut").map_err(CommandError::Failed),
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
            _ => |app, _| app.refresh_local_ports().map_err(CommandError::Failed),
        };
        registry
            .register(super::command_registry::CommandDescriptor {
                id,
                title,
                category: if action < 9 {
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
}
