use super::*;

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window_started = std::time::Instant::now();
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("VsCodeShoppe")
                        .with_visible(false)
                        .with_inner_size(LogicalSize::new(1200.0, 760.0)),
                )
                .expect("failed to create window"),
        );
        self.startup_metrics.push(format!(
            "Native window creation: {:.1} ms",
            window_started.elapsed().as_secs_f64() * 1000.0
        ));
        let scale = window.scale_factor() as f32;
        let renderer_started = std::time::Instant::now();
        let renderer =
            Renderer::new(window.clone(), self.font_size * scale).expect("failed to init renderer");
        self.startup_metrics.push(format!(
            "GPU renderer initialization: {:.1} ms",
            renderer_started.elapsed().as_secs_f64() * 1000.0
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
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(gpu) = &mut self.gpu {
            gpu.accessibility.process_event(&gpu.window, &event);
        }
        self.process_accessibility_actions();
        match event {
            WindowEvent::CloseRequested => {
                self.request_window_close();
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.renderer.resize(size.width, size.height);
                }
                self.reveal_cursor = true;
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = position;
                let editor_state = self.editor_state();
                self.drag_select();
                if editor_state != self.editor_state() {
                    self.invalidate_lsp_requests();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.smart_selection_history.clear();
                let editor_state = self.editor_state();
                self.click();
                if editor_state != self.editor_state() {
                    self.invalidate_lsp_requests();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.selecting = false,
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * 3.0,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / 20.0,
                };
                let over_panel = self.terminal_visible
                    && self.gpu.as_ref().is_some_and(|gpu| {
                        self.mouse.y as f32
                            >= gpu.renderer.size().1
                                - gpu.renderer.line_height()
                                - terminal_panel_height(gpu.renderer.size().1)
                    });
                let over_process_panel = self.process_panel_visible
                    && self.gpu.as_ref().is_some_and(|gpu| {
                        let (_, height) = gpu.renderer.size();
                        self.mouse.y as f32 >= gpu.renderer.line_height()
                            && (self.mouse.y as f32) < height - gpu.renderer.line_height() * 2.0
                    });
                if over_process_panel {
                    let max_scroll = self.process_metrics.len().saturating_sub(9);
                    self.process_scroll = (self.process_scroll as i32 + lines as i32)
                        .clamp(0, max_scroll as i32)
                        as usize;
                } else if over_panel && !self.terminal_problems {
                    if let Some(terminal) = self.terminals.get(self.focused_terminal) {
                        terminal.scroll((lines * 3.0) as i32);
                    }
                } else {
                    let tab = &self.tabs[self.active_tab];
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
                    self.tabs[self.active_tab].scroll_line =
                        (self.tabs[self.active_tab].scroll_line as f32 + lines).clamp(0.0, max)
                            as usize;
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.note_screencast_shortcut(&event);
                let editor_state = self.editor_state();
                let before = self.begin_editor_event();
                self.handle_key(&event);
                self.end_editor_event(before);
                if editor_state != self.editor_state() {
                    self.reveal_cursor = true;
                    self.invalidate_lsp_requests();
                }
                self.sync_lsp_active();
            }
            WindowEvent::Ime(winit::event::Ime::Enabled) => self.ime_preedit(String::new()),
            WindowEvent::Ime(winit::event::Ime::Preedit(text, _)) => self.ime_preedit(text),
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                let before = self.begin_editor_event();
                self.ime_commit(&text);
                self.end_editor_event(before);
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
            self.terminals.clear();
            event_loop.exit();
            return;
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
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
            UserEvent::ScmRefresh => self.refresh_scm_async(),
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
                self.status = "Diff view opened".into();
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
        }
    }
}
