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
        let Some(proxy) = self.event_proxy.clone() else {
            return Err(self.t("terminal.unavailable").to_owned());
        };
        let wake = Arc::new(move || {
            let _ = proxy.send_event(UserEvent::TerminalWake);
        });
        let session =
            TerminalSession::start_profile(self.tree.root(), &profile, wake, self.translator)?;
        let count = self
            .terminal_names
            .iter()
            .filter(|name| name.starts_with(&profile.name))
            .count()
            + 1;
        self.terminal_names
            .push(format!("{} {count}", profile.name));
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

    pub(crate) fn create_terminal_tab(&mut self) {
        match self.spawn_terminal_session() {
            Ok(index) => {
                self.terminal_visible = true;
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
        let sidebar_width = self.sidebar_width_px();
        let panel_h = (h * 0.32).clamp(150.0, 300.0);
        let rows = ((panel_h - lh * 1.5) / lh).max(1.0) as u16;
        let columns = (((w - sidebar_width - PAD * 2.0)
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
}
