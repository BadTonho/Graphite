use super::*;

/// How often GTK is pumped on Linux while a web page is open.
const GTK_PUMP_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);
/// How often the loop checks background work for deadlines while it runs.
const BACKGROUND_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

/// How soon the loop must run again on its own, if at all, for open pages and background work.
/// Background threads wake the loop when they have something (`app::wake`, extension host and
/// task events), so the interval only catches deadlines and timeouts. GTK, which shows pages on
/// Linux, has no events of its own here and is pumped often enough for pages to stay smooth.
fn background_check_interval(
    linux: bool,
    webview_open: bool,
    background_busy: bool,
) -> Option<std::time::Duration> {
    if linux && webview_open {
        Some(GTK_PUMP_INTERVAL)
    } else if background_busy || webview_open {
        Some(BACKGROUND_CHECK_INTERVAL)
    } else {
        None
    }
}

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
        let draft_deadline = self.draft_tick(std::time::Instant::now());
        self.offer_draft_review();
        self.poll_extension_host(std::time::Instant::now());
        self.poll_plugins();
        self.sync_graphite_detail_webview();
        self.sync_sidebar_webviews();
        self.sync_custom_editor_webviews();
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
            if let Some(hover) = self.debug.last_hover_result.take() {
                self.hover_text = Some(hover);
            }
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
        self.run_pending_post_debug_task();
        let host_pending = self.extension_host.enabled
            && (self.extension_host.supervisor.as_ref().is_some_and(|host| {
                host.state == graphite_extension_host::HostState::Starting
                    || !host.pending.is_empty()
            }) || !self.extension_host.incoming.is_empty());
        let autosave_deadline = self
            .autosave_deadlines
            .values()
            .map(|(_, deadline)| *deadline)
            .chain(self.plugin_auto_update_deadline())
            .chain(draft_deadline)
            .min();
        let webview_open = self.webview.is_some() || self.graphite_detail_webview.is_some();
        let background_busy = self.saving.pending()
            || debug_pending
            || host_pending
            || self.plugins.pending()
            || self.task_cancel.is_some();
        event_loop.set_control_flow(
            if let Some(interval) =
                background_check_interval(cfg!(target_os = "linux"), webview_open, background_busy)
            {
                ControlFlow::WaitUntil(std::time::Instant::now() + interval)
            } else if self.scm_busy_state().is_some() {
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
                ControlFlow::WaitUntil(
                    std::time::Instant::now() + crate::app::scm_progress::SPINNER_STEP,
                )
            } else if let Some(deadline) = autosave_deadline {
                ControlFlow::WaitUntil(deadline)
            } else {
                ControlFlow::Wait
            },
        );
        if self.exit_requested && !self.drafts_busy() {
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
                .unwrap_or_else(|err| {
                    crate::app::error_log::record(
                        crate::app::error_log::ErrorSeverity::Critical,
                        "window.create",
                        format!("failed to create window: {err}"),
                    );
                    panic!("failed to create window: {err}");
                }),
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
        .unwrap_or_else(|err| {
            crate::app::error_log::record(
                crate::app::error_log::ErrorSeverity::Critical,
                "renderer.init",
                format!("failed to init renderer: {err}"),
            );
            panic!("failed to init renderer: {err}");
        });
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
        self.sidebar_window_width_px = renderer.size().0;
        self.gpu = Some(Gpu {
            window,
            renderer,
            accessibility,
            accessibility_actions,
        });
        self.start_background_update_poller();
        self.open_pending_update_changelog();
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
                let previous_y = self.mouse.y as f32;
                self.mouse = position;
                if self.quick_input_open() {
                    // The hovered row is highlighted, so the box repaints as the pointer moves.
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                }
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
                    self.terminal_toolbar_menu_pointer_moved(position.x as f32, position.y as f32);
                    self.explorer_context_menu_pointer_moved(position.x as f32, position.y as f32);
                    self.plugin_context_menu_pointer_moved(position.x as f32, position.y as f32);
                    self.scm_graph_menu_pointer_moved(position.x as f32, position.y as f32);
                    self.scm_more_menu_pointer_moved(position.x as f32, position.y as f32);
                    if self.bar_drag_pointer_moved(position.x as f32, position.y as f32) {
                        if let Some(gpu) = &self.gpu {
                            gpu.window.request_redraw();
                        }
                    }
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
                let pointer_y = position.y as f32;
                if self.secondary_resize_dragging {
                    self.resize_secondary_sidebar_from_pointer(pointer_x);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                } else if self.sidebar_resize_dragging {
                    self.resize_sidebar_from_pointer(pointer_x);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                } else if self.panel_resize_dragging {
                    self.resize_terminal_panel_from_pointer(pointer_y);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                } else if self.scm_graph_resize_dragging {
                    self.resize_scm_graph_from_pointer(pointer_y);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                } else {
                    let separator_x = self.sidebar_width_px();
                    let over_secondary_separator = !self.zen_mode
                        && self.is_over_secondary_resize_handle(pointer_x, pointer_y);
                    let was_over_secondary_separator = !self.zen_mode
                        && self.is_over_secondary_resize_handle(previous_x, previous_y);
                    let over_separator = over_secondary_separator
                        || !self.zen_mode
                            && (pointer_x - separator_x).abs() <= 4.0
                            && pointer_y
                                < self.gpu.as_ref().map_or(f32::MAX, |gpu| {
                                    gpu.renderer.size().1 - gpu.renderer.line_height()
                                });
                    let over_panel_separator = self
                        .is_over_terminal_resize_handle(pointer_x, pointer_y)
                        || self.is_over_scm_graph_resize_handle(pointer_x, pointer_y);
                    let was_over_panel_separator = self
                        .is_over_terminal_resize_handle(previous_x, previous_y)
                        || self.is_over_scm_graph_resize_handle(previous_x, previous_y);
                    // The hover card of a Graph commit follows the pointer from row to row.
                    let graph_hover_changed = self.gpu.as_ref().is_some_and(|gpu| {
                        let window = gpu.renderer.size();
                        let (lh, cw) = (gpu.renderer.line_height(), gpu.renderer.char_width());
                        self.scm_tip_changed(
                            (pointer_x, pointer_y),
                            (previous_x, previous_y),
                            window,
                            lh,
                            cw,
                        )
                    });
                    if let Some(gpu) = &self.gpu {
                        gpu.window.set_cursor(if over_panel_separator {
                            winit::window::CursorIcon::RowResize
                        } else if over_separator {
                            winit::window::CursorIcon::ColResize
                        } else if self.commit_hover_link_at(pointer_x, pointer_y) {
                            winit::window::CursorIcon::Pointer
                        } else {
                            winit::window::CursorIcon::Default
                        });
                        if over_separator
                            || was_over_secondary_separator
                            || (previous_x - separator_x).abs() <= 4.0
                            || over_panel_separator
                            || was_over_panel_separator
                            || self.activity_bar_menu.is_some()
                            || self.explorer_context_menu.is_some()
                            || self.plugins.context_menu.is_some()
                            || self.scm_graph_menu.is_some()
                            || self.scm_more_menu.is_some()
                            || graph_hover_changed
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
                // A press that reaches the window was not on a page, so the keyboard belongs to
                // the window again.
                self.return_focus_from_webviews();
                if self.error_notification_click(self.mouse.x as f32, self.mouse.y as f32) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                if self.settings_editor_click(self.mouse.x as f32, self.mouse.y as f32) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                if self.titlebar_handle_press(self.mouse.x as f32, self.mouse.y as f32) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                if let Some(sash) = self.sash_at(self.mouse.x as f32, self.mouse.y as f32) {
                    if self.sash_double_click(sash, std::time::Instant::now()) {
                        self.reset_sash(sash);
                        if let Some(gpu) = &self.gpu {
                            gpu.window.request_redraw();
                        }
                        return;
                    }
                }
                if !self.zen_mode
                    && self
                        .is_over_secondary_resize_handle(self.mouse.x as f32, self.mouse.y as f32)
                {
                    self.secondary_resize_dragging = true;
                    self.selecting = false;
                    return;
                }
                let separator_x = self.sidebar_width_px();
                if !self.zen_mode && (self.mouse.x as f32 - separator_x).abs() <= 4.0 {
                    self.sidebar_resize_dragging = true;
                    self.selecting = false;
                    return;
                }
                if self.is_over_terminal_resize_handle(self.mouse.x as f32, self.mouse.y as f32) {
                    self.panel_resize_dragging = true;
                    self.selecting = false;
                    return;
                }
                if self.is_over_scm_graph_resize_handle(self.mouse.x as f32, self.mouse.y as f32) {
                    self.scm_graph_resize_dragging = true;
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
                if !phase6_hit && self.modifiers.control_key() && !self.modifiers.alt_key() {
                    let in_editor = self.gpu.as_ref().is_some_and(|gpu| {
                        let (width, height) = gpu.renderer.size();
                        let rect =
                            self.focused_group_rect(width, height, gpu.renderer.line_height());
                        let (x, y) = (self.mouse.x as f32, self.mouse.y as f32);
                        x >= rect.x
                            && x < rect.x + rect.width
                            && y >= rect.y
                            && y < rect.y + rect.height
                    });
                    // Ctrl+click follows a link an extension reported under the cursor.
                    if in_editor && !self.open_document_link_at_cursor() {
                        self.run_code_lens_at_line_end();
                    }
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
                let mx = self.mouse.x as f32;
                let my = self.mouse.y as f32;
                self.scm_graph_menu = None;
                self.scm_more_menu = None;
                self.plugins.context_menu = None;
                let opened = self.activity_bar_right_click(mx, my)
                    || self.explorer_right_click(mx, my)
                    || self.plugin_right_click(mx, my)
                    || self.scm_graph_right_click(mx, my)
                    || self.scm_file_right_click(mx, my);
                if opened
                    || self.activity_bar_menu.is_none()
                    || self.explorer_context_menu.is_none()
                    || self.plugins.context_menu.is_none()
                {
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
                if self.bar_drag_mouse_up(self.mouse.x as f32, self.mouse.y as f32) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                }
                if self.explorer_drag.is_some() {
                    let row = self.explorer_row_at_mouse();
                    self.finish_explorer_drag(row);
                }
                if self.titlebar_pointer_released() {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.set_cursor(winit::window::CursorIcon::Default);
                        gpu.window.request_redraw();
                    }
                } else if self.panel_resize_dragging {
                    self.panel_resize_dragging = false;
                    self.save_view_preferences_for_command();
                    if let Some(gpu) = &self.gpu {
                        gpu.window.set_cursor(winit::window::CursorIcon::Default);
                        gpu.window.request_redraw();
                    }
                } else if self.scm_graph_resize_dragging {
                    self.scm_graph_resize_dragging = false;
                    if let Some(gpu) = &self.gpu {
                        gpu.window.set_cursor(winit::window::CursorIcon::Default);
                        gpu.window.request_redraw();
                    }
                } else if self.secondary_resize_dragging {
                    self.secondary_resize_dragging = false;
                    if self.profiles.is_some() {
                        if let Some(width) = self.secondary_sidebar_user_width {
                            if let Err(error) =
                                config::save_secondary_sidebar_width(&self.profile_root, width)
                            {
                                self.status =
                                    format!("Secondary side bar width could not be saved: {error}");
                                error_log::record(
                                    error_log::ErrorSeverity::Low,
                                    "layout.secondary_sidebar",
                                    "Could not save the secondary side bar width.",
                                );
                            }
                        }
                    }
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
                if let Some((width, lh)) = self
                    .gpu
                    .as_ref()
                    .map(|gpu| (gpu.renderer.size().0, gpu.renderer.line_height()))
                {
                    let pointer = (self.mouse.x as f32, self.mouse.y as f32);
                    if self.quick_input_scroll(lines, pointer, width, lh) {
                        if let Some(gpu) = &self.gpu {
                            gpu.window.request_redraw();
                        }
                        return;
                    }
                }
                if self.handle_chat_wheel(self.mouse.x as f32, self.mouse.y as f32, lines) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                if self.settings_editor_scroll(lines) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                if self.diff_view.is_some() {
                    self.scroll_diff(lines.round() as isize);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                if self.multi_diff.is_some() {
                    self.multi_diff_scroll_rows(lines.round() as isize);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                if self.merge_editor.is_some() && self.diff_view.is_none() {
                    self.merge_scroll_rows(lines.round() as isize);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
                let over_panel = self.terminal_visible
                    && self.view_preferences.panel_visible
                    && self.gpu.as_ref().is_some_and(|gpu| {
                        let (width, height) = gpu.renderer.size();
                        let panel = self.panel_bounds(width, height, gpu.renderer.line_height());
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
                let debug_geometry = (self.debug_panel_visible
                    && self.view_preferences.panel_visible
                    && !over_sidebar)
                    .then(|| {
                        let bounds = self.panel_bounds(window_width, window_height, line_height);
                        let inside = (self.mouse.x as f32) >= bounds.x
                            && (self.mouse.x as f32) < bounds.x + bounds.width
                            && (self.mouse.y as f32) >= bounds.y
                            && (self.mouse.y as f32) < bounds.y + bounds.height;
                        inside.then(|| {
                            super::debug_panel::DebugPanelGeometry::new(&bounds, line_height)
                        })
                    })
                    .flatten();
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
                    let over_graph = !self.scm_graph_collapsed
                        && self
                            .scm_graph_layout()
                            .and_then(|layout| layout.graph_divider)
                            .is_some_and(|divider| {
                                self.sidebar_local_y(self.mouse.y as f32) >= divider.y
                            });
                    if over_graph {
                        self.scroll_scm_graph(lines as i32);
                    } else {
                        self.scm_scroll =
                            (self.scm_scroll as i32 + lines as i32).clamp(0, max as i32) as usize;
                    }
                } else if over_plugins {
                    self.scroll_plugins(lines as i32);
                } else if over_sidebar && !self.scm_visible && !self.plugins.visible {
                    let delta_rows = if lines.round() == 0.0 && lines != 0.0 {
                        lines.signum() as isize
                    } else {
                        lines.round() as isize
                    };
                    self.explorer_scroll_by(delta_rows);
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                } else if self.plugins.active_graphite_detail.is_some() {
                    self.scroll_graphite_detail(lines as i32);
                } else if self.plugins.active_installed_detail.is_some() {
                    self.scroll_installed_detail(lines as i32);
                } else if let Some(geometry) = debug_geometry {
                    self.debug_panel_scroll(&geometry, self.mouse.x as f32, lines);
                } else if over_process_panel {
                    let max_scroll = self.process_metrics.len().saturating_sub(9);
                    self.process_scroll = (self.process_scroll as i32 + lines as i32)
                        .clamp(0, max_scroll as i32)
                        as usize;
                } else if over_panel && !self.terminal_problems {
                    if let Some(terminal) = self.terminals.get(self.focused_terminal) {
                        terminal.scroll((lines * 3.0) as i32);
                    }
                } else if self.active_notebook().is_some() {
                    self.notebook_scroll(lines);
                } else {
                    // A sideways swipe, or the wheel with Shift held, scrolls the text sideways.
                    let char_width = self
                        .gpu
                        .as_ref()
                        .map_or(8.0, |gpu| gpu.renderer.char_width());
                    let mut sideways = match delta {
                        MouseScrollDelta::LineDelta(x, _) => -x * 3.0,
                        MouseScrollDelta::PixelDelta(p) => -(p.x as f32) / char_width,
                    };
                    let mut lines = lines;
                    if self.modifiers.shift_key() {
                        sideways += lines;
                        lines = 0.0;
                    }
                    if sideways != 0.0 {
                        self.scroll_columns_by(sideways.round() as isize);
                    }
                    let tab = &self.documents[self.active_document()];
                    let max = if self.word_wrap() {
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
                if self.settings_editor_key(&event.logical_key, event.text.as_deref()) {
                    if let Some(gpu) = &self.gpu {
                        gpu.window.request_redraw();
                    }
                    return;
                }
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
            // `about_to_wait`, which follows every event, reads the queues.
            UserEvent::Wake => crate::app::wake::handled(),
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
                self.status = match &result {
                    Ok(0) => "Task completed successfully".into(),
                    Ok(code) => format!("Task exited with code {code}"),
                    Err(error) => format!("Task failed: {error}"),
                };
                self.finish_debug_pre_launch(&result);
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
            UserEvent::KernelWake => self.poll_kernels(),
            UserEvent::MultiDiffReady { set, result } => {
                self.show_multi_diff(set, result);
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::ScmHistory(result) => self.show_scm_history(result),
            UserEvent::GitHubAuth(event) => self.handle_github_auth_event(event),
            UserEvent::Chat(event) => self.handle_chat_ui_event(event),
            UserEvent::Sync(event) => self.handle_sync_event(event),
            UserEvent::Update(event) => self.handle_update_event(event),
            UserEvent::ErrorRecorded(item) => self.push_error_notification(item),
            UserEvent::ScmSnapshot(root, snapshot) => {
                // A result for a folder that is no longer the one shown is out of date.
                if root == self.scm_workspace_root() {
                    self.accept_scm_snapshot(snapshot);
                }
                if self.scm_refresh_gate.finished() {
                    self.refresh_scm_async();
                }
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::ScmGraph(root, result) => {
                if self.scm_refresh_gate.finished() {
                    self.refresh_scm_async();
                }
                if root != self.scm_workspace_root() {
                    // Stale: it was loaded for the folder that was open before.
                } else if let Ok(rows) = result {
                    self.scm_graph_rows = rows;
                } else {
                    self.scm_graph_rows.clear();
                }
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::ScmOperationCompleted { message, snapshot } => {
                self.end_scm_busy();
                self.status = message;
                if self.scm_commit_pending {
                    self.scm_commit_pending = false;
                    if snapshot
                        .as_ref()
                        .is_some_and(|s| s.has_commits && !s.files.iter().any(|f| f.staged))
                    {
                        self.scm_commit_message.clear();
                    }
                }
                if let Some(snapshot) = snapshot {
                    self.accept_scm_snapshot(snapshot);
                }
                self.refresh_scm_graph_async();
                self.diff_loading = false;
                self.check_cli_waiters();
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            UserEvent::CloneCompleted(result) => {
                self.finish_clone(result);
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
                self.diff_scroll = 0;
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

#[cfg(test)]
mod wait_tests {
    use super::*;

    #[test]
    fn the_loop_sleeps_instead_of_polling_one_hundred_times_a_second() {
        // Windows: an open page or background work needs only the deadline check.
        assert_eq!(
            background_check_interval(false, true, false),
            Some(BACKGROUND_CHECK_INTERVAL)
        );
        assert_eq!(
            background_check_interval(false, false, true),
            Some(BACKGROUND_CHECK_INTERVAL)
        );
        assert!(BACKGROUND_CHECK_INTERVAL >= std::time::Duration::from_millis(100));
        // Linux pumps GTK while a page is open.
        assert_eq!(
            background_check_interval(true, true, false),
            Some(GTK_PUMP_INTERVAL)
        );
        assert_eq!(
            background_check_interval(true, false, true),
            Some(BACKGROUND_CHECK_INTERVAL)
        );
        // Nothing open and nothing running: wait for input.
        assert_eq!(background_check_interval(false, false, false), None);
        assert_eq!(background_check_interval(true, false, false), None);
    }
}
