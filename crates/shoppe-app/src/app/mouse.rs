use super::*;

impl App {
    pub(crate) fn click(&mut self) {
        let Some(gpu) = &self.gpu else { return };
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let (window_width, window_height) = gpu.renderer.size();
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
            let Some(rect) = self
                .group_rectangles(window_width, window_height, lh)
                .into_iter()
                .find(|rect| {
                    mx >= rect.x
                        && mx < rect.x + rect.width
                        && my >= rect.y
                        && my < rect.y + rect.height
                })
            else {
                return;
            };
            self.focus_editor_group(rect.id);
            self.terminal_focused = false;
            if my < rect.y + lh * 1.5 {
                let mut x = rect.x;
                let documents: Vec<_> = self
                    .editor_groups
                    .views(rect.id)
                    .unwrap()
                    .0
                    .iter()
                    .map(|view| view.document)
                    .collect();
                for document in documents {
                    let width = super::group_drawing::tab_width(&self.tabs[document], cw);
                    if mx >= x && mx < x + width {
                        self.activate_editor_document(document);
                        return;
                    }
                    x += width;
                }
                return;
            }
            let text_x = rect.x + self.gutter_width(cw);
            if self.word_wrap {
                let row = self.tabs[self.active_tab].scroll_line
                    + ((my - rect.y - lh * 1.5).max(0.0) / lh) as usize;
                let col = (((mx - text_x) / cw).round().max(0.0)) as usize;
                let columns = (((rect.x + rect.width - text_x).max(cw)) / cw) as usize;
                let tab = &mut self.tabs[self.active_tab];
                let offset = super::visual_lines::document_offset(
                    &tab.buffer,
                    row,
                    col,
                    columns,
                    tab.indentation.tab_size,
                    self.wrapping_indent,
                );
                if self.modifiers.alt_key() {
                    tab.buffer.add_cursor(offset);
                    self.selecting = false;
                } else {
                    tab.buffer.set_selection(offset, offset);
                    self.selecting = true;
                }
                return;
            }
            let line = self.tabs[self.active_tab].scroll_line
                + ((my - rect.y - lh * 1.5).max(0.0) / lh) as usize;
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
        let (width, height) = gpu.renderer.size();
        let rect = self.focused_group_rect(width, height, lh);
        let text_x = rect.x + self.gutter_width(cw);
        if self.word_wrap {
            let row = self.tabs[self.active_tab].scroll_line
                + ((self.mouse.y as f32 - rect.y - lh * 1.5).max(0.0) / lh) as usize;
            let col = (((self.mouse.x as f32 - text_x) / cw).round().max(0.0)) as usize;
            let columns = (((rect.x + rect.width - text_x).max(cw)) / cw) as usize;
            let tab = &mut self.tabs[self.active_tab];
            let head = super::visual_lines::document_offset(
                &tab.buffer,
                row,
                col,
                columns,
                tab.indentation.tab_size,
                self.wrapping_indent,
            );
            let anchor = tab
                .buffer
                .selection()
                .map(|s| s.anchor)
                .unwrap_or(tab.buffer.cursor().char_idx);
            tab.buffer.set_selection(anchor, head);
            return;
        }
        let line = (self.tabs[self.active_tab].scroll_line
            + ((self.mouse.y as f32 - rect.y - lh * 1.5).max(0.0) / lh) as usize)
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
