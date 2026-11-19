use super::*;

impl App {
    pub(crate) fn toggle_terminal(&mut self) {
        if !self.terminal_visible && self.terminals.is_empty() {
            if let Err(error) = self.spawn_terminal_session() {
                self.terminal_status = Some(error.clone());
                self.status = error;
                return;
            }
        }
        self.terminal_visible = !self.terminal_visible;
        if self.terminal_visible {
            self.view_preferences.panel_visible = true;
        }
        self.terminal_focused = self.terminal_visible && !self.terminal_problems;
        self.terminal_problems = false;
        self.resize_terminal();
    }

    pub(crate) fn spawn_terminal_session(&mut self) -> Result<usize, String> {
        let Some(profile) = self
            .terminal_profiles
            .get(self.selected_terminal_profile)
            .cloned()
        else {
            return Err(self
                .terminal_status
                .clone()
                .unwrap_or_else(|| self.t("terminal.no_profiles").to_owned()));
        };
        self.spawn_terminal_profile(&profile, None, None)
    }

    /// Starts a terminal for `profile` in `directory` (the workspace root by default). A fixed
    /// `title` names the tab; otherwise tabs are numbered after the profile.
    pub(crate) fn spawn_terminal_profile(
        &mut self,
        profile: &crate::config::TerminalProfile,
        directory: Option<&Path>,
        title: Option<&str>,
    ) -> Result<usize, String> {
        let wake: Arc<dyn Fn() + Send + Sync> = match self.event_proxy.clone() {
            Some(proxy) => Arc::new(move || {
                let _ = proxy.send_event(UserEvent::TerminalWake);
            }),
            // Tests have no event loop to wake; terminals are polled directly.
            #[cfg(test)]
            None => Arc::new(|| {}),
            #[cfg(not(test))]
            None => return Err(self.t("terminal.unavailable").to_owned()),
        };
        let session = TerminalSession::start_profile(
            directory.unwrap_or_else(|| self.tree.root()),
            profile,
            wake,
            self.translator,
        )?;
        let name = match title {
            Some(title) => title.to_owned(),
            None => {
                let count = self
                    .terminal_names
                    .iter()
                    .filter(|name| name.starts_with(&profile.name))
                    .count()
                    + 1;
                format!("{} {count}", profile.name)
            }
        };
        self.terminal_names.push(name);
        self.terminals.push(session);
        self.terminal_sizes.push((0, 0));
        let index = self.terminals.len() - 1;
        self.active_terminal = index;
        self.focused_terminal = index;
        self.terminal_tab_offset = index;
        self.terminal_status = None;
        self.resize_terminal();
        Ok(index)
    }

    /// Starts a virtual terminal tab whose bytes are pushed from an extension pseudoterminal.
    pub(crate) fn spawn_virtual_terminal(
        &mut self,
        title: &str,
    ) -> Result<(usize, std::sync::mpsc::Receiver<Vec<u8>>), String> {
        let wake: Arc<dyn Fn() + Send + Sync> = match self.event_proxy.clone() {
            Some(proxy) => Arc::new(move || {
                let _ = proxy.send_event(UserEvent::TerminalWake);
            }),
            #[cfg(test)]
            None => Arc::new(|| {}),
            #[cfg(not(test))]
            None => return Err(self.t("terminal.unavailable").to_owned()),
        };
        let (session, input_rx) = TerminalSession::start_virtual(wake)?;
        self.terminal_names.push(title.to_owned());
        self.terminals.push(session);
        self.terminal_sizes.push((0, 0));
        let index = self.terminals.len() - 1;
        self.active_terminal = index;
        self.focused_terminal = index;
        self.terminal_tab_offset = index;
        self.terminal_status = None;
        self.resize_terminal();
        Ok((index, input_rx))
    }

    pub(crate) fn create_terminal_tab(&mut self) {
        match self.spawn_terminal_session() {
            Ok(index) => {
                self.terminal_visible = true;
                self.view_preferences.panel_visible = true;
                self.terminal_problems = false;
                self.active_terminal = index;
                self.focused_terminal = index;
                self.terminal_focused = true;
                self.resize_terminal();
            }
            Err(error) => {
                self.terminal_status = Some(error.clone());
                self.status = error;
            }
        }
    }

    pub(crate) fn toggle_terminal_split(&mut self) {
        if self.split_terminal.is_some() {
            self.split_terminal = None;
            self.focused_terminal = self.active_terminal;
            self.resize_terminal();
            return;
        }
        if self.terminals.len() < 2 {
            if let Err(error) = self.spawn_terminal_session() {
                self.terminal_status = Some(error.clone());
                self.status = error;
                return;
            }
        }
        if self.terminals.len() >= 2 {
            self.split_terminal = Some((self.active_terminal + 1) % self.terminals.len());
            self.resize_terminal();
        }
    }

    pub(crate) fn select_terminal(&mut self, index: usize) {
        if index >= self.terminals.len() {
            return;
        }
        self.active_terminal = index;
        if self.split_terminal.is_some() {
            self.split_terminal = Some((index + 1) % self.terminals.len());
        } else {
            self.focused_terminal = index;
        }
        self.resize_terminal();
    }

    pub(crate) fn scroll_terminal_tabs(&mut self, direction: isize) {
        if direction < 0 {
            self.terminal_tab_offset = self.terminal_tab_offset.saturating_sub(1);
        } else {
            self.terminal_tab_offset =
                (self.terminal_tab_offset + 1).min(self.terminal_names.len().saturating_sub(1));
        }
    }

    pub(crate) fn close_focused_terminal(&mut self) {
        let index = self
            .focused_terminal
            .min(self.terminals.len().saturating_sub(1));
        self.close_terminal_at(index);
    }

    pub(crate) fn clear_focused_terminal(&mut self) {
        let index = self
            .focused_terminal
            .min(self.terminals.len().saturating_sub(1));
        if let Some(terminal) = self.terminals.get(index) {
            terminal.clear();
        }
    }

    pub(crate) fn close_terminal_at(&mut self, index: usize) {
        if index >= self.terminals.len() {
            return;
        }
        self.terminals.remove(index);
        self.terminal_names.remove(index);
        self.terminal_sizes.remove(index);
        let new_len = self.terminals.len();
        let remap = |target: usize| {
            if new_len == 0 {
                0
            } else if target > index {
                target - 1
            } else {
                target.min(new_len - 1)
            }
        };
        self.terminal_tab_offset = remap(self.terminal_tab_offset);
        if self.terminals.is_empty() {
            self.active_terminal = 0;
            self.focused_terminal = 0;
            self.split_terminal = None;
        } else {
            self.active_terminal = remap(self.active_terminal);
            self.focused_terminal = remap(self.focused_terminal);
            if self.terminals.len() < 2 {
                self.split_terminal = None;
            } else if self.split_terminal.is_some() {
                self.split_terminal = Some((self.active_terminal + 1) % self.terminals.len());
            }
        }
        self.terminal_focused = !self.terminals.is_empty();
        self.resize_terminal();
    }

    pub(crate) fn cycle_terminal_profile(&mut self) {
        if self.terminal_profiles.len() > 1 {
            self.selected_terminal_profile =
                (self.selected_terminal_profile + 1) % self.terminal_profiles.len();
        }
    }

    pub(crate) fn resize_terminal(&mut self) {
        if !self.terminal_visible || self.terminal_problems || self.terminals.is_empty() {
            return;
        }
        let Some(gpu) = &self.gpu else { return };
        let lh = gpu.renderer.line_height().max(1.0);
        let cw = gpu.renderer.char_width().max(1.0);
        let (w, h) = gpu.renderer.size();
        let panel_width = self.panel_bounds(w, h, lh).width;
        let panel_h = drawing_helpers::terminal_panel_height_with_delta(
            h,
            self.view_preferences.panel_height_delta,
        );
        let rows = ((panel_h - super::terminal_drawing::TERMINAL_HEADER_HEIGHT - 4.0) / lh).max(1.0)
            as u16;
        let columns = (((panel_width - super::terminal_drawing::TERMINAL_GUTTER_WIDTH - PAD)
            / if self.split_terminal.is_some() {
                2.0
            } else {
                1.0
            })
            / cw)
            .max(1.0) as u16;
        let mut visible = vec![self.active_terminal];
        if let Some(second) = self.split_terminal {
            if second < self.terminals.len() && second != self.active_terminal {
                visible.push(second);
            }
        }
        for index in visible {
            let Some(terminal) = self.terminals.get(index) else {
                continue;
            };
            if self.terminal_sizes.get(index).copied() != Some((rows, columns)) {
                terminal.resize(rows, columns, cw as u16, lh as u16);
                if let Some(size) = self.terminal_sizes.get_mut(index) {
                    *size = (rows, columns);
                }
            }
        }
        self.terminal_size = (rows, columns);
    }

    pub(crate) fn terminal_resize_handle_bounds(&self) -> Option<(f32, f32, f32)> {
        if self.zen_mode || !self.view_preferences.panel_visible || !self.terminal_visible {
            return None;
        }
        let gpu = self.gpu.as_ref()?;
        let (width, height) = gpu.renderer.size();
        let bounds = self.panel_bounds(width, height, gpu.renderer.line_height());
        Some((bounds.x, bounds.y, bounds.width))
    }

    pub(crate) fn is_over_terminal_resize_handle(&self, mx: f32, my: f32) -> bool {
        let Some((panel_x, panel_y, panel_width)) = self.terminal_resize_handle_bounds() else {
            return false;
        };
        mx >= panel_x && mx <= panel_x + panel_width && (my - panel_y).abs() <= 4.0
    }

    pub(crate) fn resize_terminal_panel_from_pointer(&mut self, pointer_y: f32) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let (_, height) = gpu.renderer.size();
        let line_height = gpu.renderer.line_height();
        self.resize_terminal_panel_for_window(pointer_y, height, line_height);
    }

    pub(crate) fn resize_terminal_panel_for_window(
        &mut self,
        pointer_y: f32,
        window_height: f32,
        line_height: f32,
    ) {
        let max_height = (window_height * 0.85).max(300.0);
        let desired_height = (window_height - line_height - pointer_y).clamp(120.0, max_height);
        let base = (window_height * 0.32).clamp(150.0, 300.0);
        self.view_preferences.panel_height_delta =
            ((desired_height - base).round() as i16).clamp(-180, 800);
        self.resize_terminal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_terminal_resize_handle_increases_and_clamps_panel_height() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.view_preferences.panel_visible = true;
        app.terminal_visible = true;

        // Drag upward to y = 200 on a 900px tall window -> ~680px tall terminal.
        app.resize_terminal_panel_for_window(200.0, 900.0, 20.0);
        let grown = drawing_helpers::terminal_panel_height_with_delta(
            900.0,
            app.view_preferences.panel_height_delta,
        );
        assert!(grown > 600.0);

        // Dragging near the bottom clamps to at least 120px.
        app.resize_terminal_panel_for_window(880.0, 900.0, 20.0);
        let shrunk = drawing_helpers::terminal_panel_height_with_delta(
            900.0,
            app.view_preferences.panel_height_delta,
        );
        assert_eq!(shrunk, 120.0);
    }
}
