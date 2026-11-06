use super::*;

impl App {
    pub(crate) fn click(&mut self) {
        let Some(gpu) = &self.gpu else { return };
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);

        if self.terminal_visible {
            let panel_top = gpu.renderer.size().1
                - gpu.renderer.line_height()
                - terminal_panel_height(gpu.renderer.size().1);
            if my >= panel_top {
                if my < panel_top + lh * 1.4 {
                    self.terminal_problems = mx < SIDEBAR_WIDTH + cw * 9.0;
                    self.terminal_focused = !self.terminal_problems;
                } else if !self.terminal_problems {
                    self.terminal_focused = true;
                }
                return;
            }
        }

        if mx < SIDEBAR_WIDTH {
            let first_row_y = lh * 1.5;
            if my >= first_row_y {
                let row = ((my - first_row_y) / lh) as usize;
                if let Some(path) = self.tree.activate(row) {
                    self.open_file(path);
                }
            }
        } else {
            if my < lh * 1.5 {
                let mut x = SIDEBAR_WIDTH;
                for (i, tab) in self.tabs.iter().enumerate() {
                    let label = tab
                        .buffer
                        .path()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Untitled".into());
                    let width = (label.chars().count() as f32 + 4.0) * cw * 1.8;
                    if mx >= x && mx < x + width {
                        self.active_tab = i;
                        return;
                    }
                    x += width;
                }
                return;
            }
            let text_x = SIDEBAR_WIDTH + self.gutter_width(cw);
            let line =
                self.tabs[self.active_tab].scroll_line + ((my - lh * 1.5).max(0.0) / lh) as usize;
            let col = (((mx - text_x) / cw).round().max(0.0)) as usize;
            if self.modifiers.alt_key() {
                let b = &mut self.tabs[self.active_tab].buffer;
                let line = line.min(b.len_lines() - 1);
                let width = b.line(line).trim_end_matches(['\n', '\r']).chars().count();
                b.add_cursor(b.line_start_char(line) + col.min(width));
                self.selecting = false;
                return;
            }
            self.tabs[self.active_tab]
                .buffer
                .set_cursor_line_col(line, col);
            self.selecting = true;
        }
    }

    pub(crate) fn drag_select(&mut self) {
        if !self.selecting {
            return;
        }
        let Some(gpu) = &self.gpu else { return };
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let text_x = SIDEBAR_WIDTH + self.gutter_width(cw);
        let line = (self.tabs[self.active_tab].scroll_line
            + ((self.mouse.y as f32 - lh * 1.5).max(0.0) / lh) as usize)
            .min(self.tabs[self.active_tab].buffer.len_lines() - 1);
        let col = (((self.mouse.x as f32 - text_x) / cw).round().max(0.0)) as usize;
        let head = self.tabs[self.active_tab].buffer.line_start_char(line)
            + col.min(
                self.tabs[self.active_tab]
                    .buffer
                    .line(line)
                    .trim_end_matches(['\n', '\r'])
                    .chars()
                    .count(),
            );
        let anchor = self.tabs[self.active_tab]
            .buffer
            .selection()
            .map(|s| s.anchor)
            .unwrap_or(self.tabs[self.active_tab].buffer.cursor().char_idx);
        self.tabs[self.active_tab]
            .buffer
            .set_selection(anchor, head);
    }
}
