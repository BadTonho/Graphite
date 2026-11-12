use super::*;

impl ApplicationHandler<UserEvent> for App {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let saving = self.saving.pending();
        self.poll_saves(std::time::Instant::now());
        self.autosave_tick();
        if saving {
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
        self.collect_closed_documents();
        self.poll_extension_host(std::time::Instant::now());
        self.poll_plugins();
        self.sync_graphite_detail_webview();
        if let (Some(view), Some(gpu)) = (&self.webview, &self.gpu) {
            view.pump();
            let _ = view.set_bounds(&gpu.window);
        }
        if let Some(view) = &self.graphite_detail_webview {
            view.pump();
        }
        if self.markdown_preview.is_some() && self.webview_handle.is_none() {
            if let Some(view) = &self.webview {
                for message in view.take_messages() {
                    if let Ok(message) = serde_json::from_str::<serde_json::Value>(&message) {
                        if message["type"] == "markdown-scroll" {
                            if let Some(ratio) = message["ratio"].as_f64() {
                                let total = self.documents[self.active_document()]
                                    .buffer
                                    .len_lines()
                                    .saturating_sub(1);
                                let line = (ratio.clamp(0.0, 1.0) * total as f64).round() as usize;
                                self.editor_groups.active_view_mut().scroll_line = line;
                                self.preview_source_scroll = Some(line);
                                if let Some(gpu) = &self.gpu {
                                    gpu.window.request_redraw();
                                }
                            }
                        }
                    }
                }
                let source_line = self.editor_groups.active_view().scroll_line;
                if self.preview_source_scroll != Some(source_line) {
                    let total = self.documents[self.active_document()]
                        .buffer
                        .len_lines()
                        .saturating_sub(1)
                        .max(1);
                    let ratio = (source_line as f64 / total as f64).clamp(0.0, 1.0);
                    let _ = view.evaluate_script(&format!(
                        "window.postMessage({{type:'markdown-scroll-to',ratio:{ratio}}},'*')"
                    ));
                    self.preview_source_scroll = Some(source_line);
                }
            }
        }
        let debug_pending = self.debug.session.is_some();
        if debug_pending && self.debug.poll(self.tree.root()).unwrap_or(false) {
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
        let host_pending = self.extension_host.enabled
            && (self.extension_host.supervisor.as_ref().is_some_and(|host| {
                host.state == graphite_extension_host::HostState::Starting
                    || !host.pending.is_empty()
            }) || !self.extension_host.incoming.is_empty());
        let autosave_deadline = self
            .autosave_deadlines
            .values()
            .map(|(_, deadline)| *deadline)
            .min();
        event_loop.set_control_flow(
            if self.saving.pending()
                || debug_pending
                || host_pending
                || self.plugins.pending()
                || self.task_cancel.is_some()
                || self.webview.is_some()
                || self.graphite_detail_webview.is_some()
            {
                ControlFlow::WaitUntil(
                    std::time::Instant::now() + std::time::Duration::from_millis(10),
                )
            } else if let Some(deadline) = autosave_deadline {
                ControlFlow::WaitUntil(deadline)
            } else {
                ControlFlow::Wait
            },
        );
        if self.exit_requested {
            event_loop.exit();
        }
        self.refresh_command_context();
        self.reap_commands();
        self.check_cli_waiters();
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window_started = std::time::Instant::now();
        let window = Arc::new(
            event_loop
                .create_window(
                    crate::branding::window_attributes()
                        .with_visible(false)
                        .with_inner_size(LogicalSize::new(1200.0, 760.0)),
                )
                .expect("failed to create window"),
        );
        self.startup_metrics.push(self.tf(
            "startup.window_creation",
            &[(
                "duration",
                &format!("{:.1}", window_started.elapsed().as_secs_f64() * 1000.0),
            )],
        ));
        let scale = window.scale_factor() as f32;
        let renderer_started = std::time::Instant::now();
        let renderer = Renderer::new(
            window.clone(),
            self.font_size * scale * f32::from(self.view_preferences.zoom_percent) / 100.0,
        )
        .expect("failed to init renderer");
        self.startup_metrics.push(self.tf(
            "startup.renderer_init",
            &[(
                "duration",
                &format!("{:.1}", renderer_started.elapsed().as_secs_f64() * 1000.0),
            )],
        ));
        let accessibility_actions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let accessibility = accesskit_winit::Adapter::with_direct_handlers(
            event_loop,
            &window,
            AccessibilityActivation,
            AccessibilityActionQueue {
                queue: accessibility_actions.clone(),
                proxy: self
                    .event_proxy
                    .clone()
                    .expect("event loop proxy is required"),
            },
            AccessibilityDeactivation,
        );
        window.set_visible(true);
        window.set_ime_allowed(true);
        self.sidebar_scale_factor = scale;
        self.sidebar_window_width_px = renderer.size().0 as f32;
        self.gpu = Some(Gpu {
            window,
            renderer,
            accessibility,
            accessibility_actions,
        });
        self.refresh_scm_async();
        if let Some(request) = self.initial_cli_request.take() {
            self.apply_cli_request(request, None);
        }
        if let Some(path) = std::env::var_os("GRAPHITE_STARTUP_FILE") {
            self.open_file(PathBuf::from(path));
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(gpu) = &mut self.gpu {
            gpu.accessibility.process_event(&gpu.window, &event);
        }
        self.process_accessibility_actions();
        match event {
            WindowEvent::Focused(false) => self.autosave_window_lost_focus(),
            WindowEvent::CloseRequested => {
                self.run_ui_command("workbench.action.quit");
            }
            WindowEvent::Resized(size) => {
                self.sidebar_window_width_px = size.width as f32;
                if let Some(gpu) = &mut self.gpu {
                    gpu.renderer.resize(size.width, size.height);
                }
                self.reveal_cursor = true;
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.sidebar_scale_factor = scale_factor as f32;
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let previous_x = self.mouse.x as f32;
                self.mouse = position;
                if let Some(window_width) = self.gpu.as_ref().map(|gpu| gpu.renderer.size().0) {
                    self.workbench_menu_pointer_moved(position.x as f32, position.y as f32);
                    self.activity_bar_menu_pointer_moved(
                        position.x as f32,
                        position.y as f32,
                        window_width,
                    );
                    let window_height = self.gpu.as_ref().map_or(0.0, |gpu| gpu.renderer.size().1);
                    self.settings_menu_pointer_moved(
                        position.x as f32,
                        position.y as f32,
                        window_width,
                        window_height,
                    );
                    self.profile_menu_pointer_moved(position.x as f32, position.y as f32);
                }
                if self.titlebar_pointer_moved(position.x as f32, position.y as f32) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                if let Some(cursor) = self.titlebar_cursor(position.x as f32, position.y as f32) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.set_cursor(cursor);
                        gpu.window.request_redraw();
                    }
                    return;
                }
                let pointer_x = position.x as f32;
                if self.sidebar_resize_dragging {
                    self.resize_sidebar_from_pointer(pointer_x);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                } else {
                    let separator_x = self.sidebar_width_px();
                    let over_separator = !self.zen_mode
                        && (pointer_x - separator_x).abs() <= 4.0
                        && (position.y as f32)
                            < self.gpu.as_ref().map_or(f32::MAX, |gpu| {
                                gpu.renderer.size().1 as f32 - gpu.renderer.line_height()
                            });
                    if let Some(gpu) = &self.gpu {
                        gpu.window.set_cursor(if over_separator {
                            winit::window::CursorIcon::ColResize
                        } else {
                            winit::window::CursorIcon::Default
                        });
                        if over_separator
                            || (previous_x - separator_x).abs() <= 4.0
                            || self.activity_bar_menu.is_some()
                        {
                            gpu.window.request_redraw();
                        }
                    }
                    let editor_state = self.editor_state();
                    self.drag_select();
                    if editor_state != self.editor_state() {
                        self.invalidate_lsp_requests();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.titlebar_handle_press(self.mouse.x as f32, self.mouse.y as f32) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                let separator_x = self.sidebar_width_px();
                if !self.zen_mode && (self.mouse.x as f32 - separator_x).abs() <= 4.0 {
                    self.sidebar_resize_dragging = true;
                    self.selecting = false;
                    return;
                }
                self.editor_groups
                    .active_view_mut()
                    .smart_selection_history
                    .clear();
                let editor_state = self.editor_state();
                let panel_geometry = self.gpu.as_ref().map(|gpu| {
                    (
                        gpu.renderer.size().0,
                        gpu.renderer.size().1,
                        gpu.renderer.line_height(),
                    )
                });
                let phase6_hit = panel_geometry.is_some_and(|(width, height, line_height)| {
                    self.phase6_panel_click(
                        self.mouse.x as f32,
                        self.mouse.y as f32,
                        width,
                        height,
                        line_height,
                    )
                });
                if !phase6_hit {
                    self.click();
                }
                if editor_state != self.editor_state() {
                    self.invalidate_lsp_requests();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                self.dismiss_workbench_menu();
                let opened =
                    self.activity_bar_right_click(self.mouse.x as f32, self.mouse.y as f32);
                if opened || self.activity_bar_menu.is_none() {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self.explorer_drag.is_some() {
                    let row = self.explorer_row_at_mouse();
                    self.finish_explorer_drag(row);
                }
                if self.titlebar_pointer_released() {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.set_cursor(winit::window::CursorIcon::Default);
                        gpu.window.request_redraw();
                    }
                } else if self.sidebar_resize_dragging {
                    self.sidebar_resize_dragging = false;
                    self.sidebar_width = config::clamp_sidebar_width(self.sidebar_width);
                    if self.profiles.is_some() {
                        if let Err(error) =
                            config::save_sidebar_width(&self.profile_root, self.sidebar_width)
                        {
                            self.status = format!("Sidebar width could not be saved: {error}");
                        }
                    }
                    if let Some(gpu) = &self.gpu {
                        gpu.window.set_cursor(winit::window::CursorIcon::Default);
                        gpu.window.request_redraw();
                    }
                } else {
                    self.selecting = false;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * 3.0,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / 20.0,
                };
                let over_panel = self.terminal_visible
                    && self.view_preferences.panel_visible
                    && self.gpu.as_ref().is_some_and(|gpu| {
                        let (width, height) = gpu.renderer.size();
                        let panel = self.panel_bounds(
                            width as f32,
                            height as f32,
                            gpu.renderer.line_height(),
                        );
                        (self.mouse.x as f32) >= panel.x
                            && (self.mouse.x as f32) < panel.x + panel.width
                            && (self.mouse.y as f32) >= panel.y
                            && (self.mouse.y as f32) < panel.y + panel.height
                    });
                let over_process_panel = self.process_panel_visible
                    && self.gpu.as_ref().is_some_and(|gpu| {
                        let (_, height) = gpu.renderer.size();
                        self.mouse.y as f32 >= gpu.renderer.line_height()
                            && (self.mouse.y as f32) < height - gpu.renderer.line_height() * 2.0
                    });
                let (window_width, window_height, line_height) =
                    self.gpu.as_ref().map_or((0.0, 0.0, 20.0), |gpu| {
                        let (width, height) = gpu.renderer.size();
                        (width, height, gpu.renderer.line_height())
                    });
                let over_sidebar = !self.zen_mode
                    && self.primary_sidebar_contains(
                        self.mouse.x as f32,
                        self.mouse.y as f32,
                        window_width,
                        window_height,
                        line_height,
                    );
                let over_plugins = self.plugins.visible && over_sidebar;
                let over_scm = self.scm_visible && over_sidebar;
                if over_scm {
                    let staged = self
                        .scm_snapshot
                        .files
                        .iter()
                        .filter(|file| file.staged)
                        .count();
                    let changed = self
                        .scm_snapshot
                        .files
                        .iter()
                        .filter(|file| file.unstaged || file.untracked || file.conflicted)
                        .count();
                    let total_rows = changed + usize::from(staged > 0) + staged + 1;
                    let visible_rows = self.gpu.as_ref().map_or(1, |gpu| {
                        let line = gpu.renderer.line_height().max(18.0);
                        let repo_rows = if self.scm_repositories_collapsed {
                            2.0
                        } else {
                            4.0
                        };
                        let visible =
                            (gpu.renderer.size().1 - line * (repo_rows + 2.50) - line * 1.2) / line;
                        visible.max(1.0) as usize
                    });
                    let max = total_rows.saturating_sub(visible_rows);
                    self.scm_scroll =
                        (self.scm_scroll as i32 + lines as i32).clamp(0, max as i32) as usize;
                } else if over_plugins {
                    self.scroll_plugins(lines as i32);
                } else if self.plugins.active_graphite_detail.is_some() {
                    self.scroll_graphite_detail(lines as i32);
                } else if over_process_panel {
                    let max_scroll = self.process_metrics.len().saturating_sub(9);
                    self.process_scroll = (self.process_scroll as i32 + lines as i32)
                        .clamp(0, max_scroll as i32)
                        as usize;
                } else if over_panel && !self.terminal_problems {
                    if let Some(terminal) = self.terminals.get(self.focused_terminal) {
                        terminal.scroll((lines * 3.0) as i32);
                    }
                } else {
                    let tab = &self.documents[self.active_document()];
                    let max = if self.word_wrap {
                        if let Some(gpu) = &self.gpu {
                            let (width, height) = gpu.renderer.size();
                            let cw = gpu.renderer.char_width();
                            let rect =
                                self.focused_group_rect(width, height, gpu.renderer.line_height());
                            let columns =
                                (((rect.width - self.gutter_width(cw)).max(cw)) / cw) as usize;
                            super::visual_lines::document_position(
                                &tab.buffer,
                                tab.buffer.len_chars(),
                                columns,
                                tab.indentation.tab_size,
                                self.wrapping_indent,
                            )
                            .0
                        } else {
                            0
                        }
                    } else {
                        tab.buffer.len_lines().saturating_sub(1)
                    } as f32;
                    self.editor_groups.active_view_mut().scroll_line =
                        (self.editor_groups.active_view_mut().scroll_line as f32 + lines)
                            .clamp(0.0, max) as usize;
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.note_screencast_shortcut(&event);
                let editor_state = self.editor_state();

                self.handle_key(&event);

                if editor_state != self.editor_state() {
                    self.reveal_cursor = true;
                    self.invalidate_lsp_requests();
                }
                self.sync_lsp_active();
            }
            WindowEvent::Ime(winit::event::Ime::Enabled) => self.ime_preedit(String::new()),
            WindowEvent::Ime(winit::event::Ime::Preedit(text, _)) => self.ime_preedit(text),
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.ime_commit(&text);

                self.reveal_cursor = true;
            }
            WindowEvent::Ime(winit::event::Ime::Disabled) => self.ime_preedit(String::new()),
            WindowEvent::RedrawRequested => self.draw(),
            _ => return,
        }
        if self.exit_requested {
            self.process_cancel.store(true, Ordering::Release);
            self.screencast_cancel.store(true, Ordering::Release);
            self.lsp_manager.shutdown();
            let _ = self.debug.stop();
            self.terminals.clear();
            event_loop.exit();
            return;
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        self.handle_user_event(event);
        self.refresh_command_context();
        self.reap_commands();
    }
}
impl App {
    pub(crate) fn handle_user_event(&mut self, event: UserEvent) {
        match event {
            UserEvent::ExtensionHostWake => self.poll_extension_host(std::time::Instant::now()),
            UserEvent::SettingsImported(settings) => self.apply_imported_settings(*settings),
            UserEvent::SnippetsLoaded { snippets, warnings } => {
                self.context_keys
                    .close_widget(super::context_keys::WidgetKind::Snippet);
                self.set_user_snippets(snippets);
                self.snippet_load_warnings = warnings;
                self.snippet_query = Some(String::new());
                self.status = self
                    .snippet_load_warnings
                    .first()
                    .cloned()
                    .unwrap_or_else(|| self.t("snippet.status").into());
            }
            UserEvent::LspCommandEvent(event) => self.apply_lsp_ui_event(*event, true),
            UserEvent::CommandCompleted {
                execution,
                event,
                result,
            } => {
                self.accept_command_event(execution, event, result);
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::AccessibilityWake => {
                let editor_state = self.editor_state();
                self.process_accessibility_actions();
                if editor_state != self.editor_state() {
                    self.reveal_cursor = true;
                    self.invalidate_lsp_requests();
                }
                self.sync_lsp_active();
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::LspWake => {
                self.process_lsp_events();
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::TerminalWake => {
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::TaskOutput(line) => {
                self.task_output.push(line);
                if self.task_output.len() > 2_000 {
                    self.task_output.drain(..self.task_output.len() - 2_000);
                }
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::TaskProcessStarted(pid) => self.ports.register_process(pid),
            UserEvent::TaskFinished {
                result,
                diagnostics,
            } => {
                self.task_cancel = None;
                self.task_running_label = None;
                self.task_diagnostics = diagnostics;
                self.status = match result {
                    Ok(0) => "Task completed successfully".into(),
                    Ok(code) => format!("Task exited with code {code}"),
                    Err(error) => format!("Task failed: {error}"),
                };
                if let Some(label) = self.task_restart_pending.take() {
                    if let Err(error) = self.run_task(&label) {
                        self.status = error;
                    }
                }
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::ScmRefresh => {
                self.refresh_scm_async();
                self.sync_external_changes();
            }
            UserEvent::FileEvents(changes) => self.forward_vsix_file_events(&changes),
            UserEvent::ExternalChangeCheck => self.sync_external_changes(),
            UserEvent::ScmHistory(result) => self.show_scm_history(result),
            UserEvent::GitHubAuth(event) => self.handle_github_auth_event(event),
            UserEvent::ScmSnapshot(snapshot) => {
                self.accept_scm_snapshot(snapshot);
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::ScmOperationCompleted { message, snapshot } => {
                self.status = message;
                if let Some(snapshot) = snapshot {
                    self.accept_scm_snapshot(snapshot);
                }
                self.diff_loading = false;
                self.check_cli_waiters();
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::BranchCheckoutCompleted {
                message,
                snapshot,
                documents,
            } => {
                self.status = message;
                self.accept_scm_snapshot(snapshot);
                if !documents.is_empty() {
                    self.reload_after_branch_checkout(documents);
                }
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::DiffReady(diff) => {
                self.diff_view = Some(diff);
                self.diff_loading = false;
                self.status = self.t("diff.opened").into();
                self.check_cli_waiters();
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::ConflictReady(conflict) => {
                self.conflict_view = Some(conflict);
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::ProcessSnapshot(metrics) => {
                self.process_metrics = metrics;
                self.process_scroll = self
                    .process_scroll
                    .min(self.process_metrics.len().saturating_sub(9));
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::ScreencastTick => {
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::CliRequest {
                request,
                acknowledge,
            } => {
                self.apply_cli_request(request, Some(acknowledge));
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::ProjectSearch {
                query,
                generation,
                results,
            } => {
                if self.accept_project_search(&query, generation, results) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                }
            }
            UserEvent::QuickOpen {
                query,
                generation,
                paths,
            } => {
                if self.accept_quick_open(&query, generation, paths) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                }
            }
            UserEvent::GoSymbolsReady { generation, items } => {
                self.accept_go_symbols(generation, items);
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
        }
    }
}
