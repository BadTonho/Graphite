use super::*;

impl App {
    pub(crate) fn toggle_terminal(&mut self) {
        if !self.terminal_visible {
            if self.terminal.is_none() {
                let Some(proxy) = self.event_proxy.clone() else {
                    self.status = "Terminal unavailable in this app context".into();
                    return;
                };
                let wake = Arc::new(move || {
                    let _ = proxy.send_event(UserEvent::TerminalWake);
                });
                match TerminalSession::start(
                    self.tree.root(),
                    config::terminal_shell().as_deref(),
                    wake,
                ) {
                    Ok(session) => {
                        self.terminal = Some(session);
                        self.terminal_status = None;
                    }
                    Err(error) => {
                        self.terminal_status = Some(error.clone());
                        self.status = error;
                    }
                }
            }
            self.terminal_visible = self.terminal.is_some();
        } else {
            self.terminal_visible = false;
        }
        self.terminal_focused = self.terminal_visible;
        self.terminal_problems = false;
        self.resize_terminal();
    }

    pub(crate) fn resize_terminal(&mut self) {
        if !self.terminal_visible {
            return;
        }
        let Some(gpu) = &self.gpu else { return };
        let lh = gpu.renderer.line_height().max(1.0);
        let cw = gpu.renderer.char_width().max(1.0);
        let (w, h) = gpu.renderer.size();
        let panel_h = (h * 0.32).clamp(150.0, 300.0);
        let rows = ((panel_h - lh * 1.5) / lh).max(1.0) as u16;
        let columns = ((w - SIDEBAR_WIDTH - PAD * 2.0) / cw).max(1.0) as u16;
        if self.terminal_size != (rows, columns) {
            if let Some(terminal) = &self.terminal {
                terminal.resize(rows, columns, cw as u16, lh as u16);
                self.terminal_size = (rows, columns);
            }
        }
    }
}
