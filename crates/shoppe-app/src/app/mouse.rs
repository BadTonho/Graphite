use super::*;

impl App {
    pub(crate) fn click(&mut self) {
        if self.plugins.visible {
            self.plugin_click();
            return;
        }
        let Some(gpu) = &self.gpu else { return };
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let (window_width, window_height) = gpu.renderer.size();
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);

        if self.diff_view.is_some() {
            if my < lh * 1.5 {
                self.run_ui_command("shoppe.diff.toggleLayout");
            }
            return;
        }
        if my >= window_height - lh {
            match self.status_segment_at(mx, cw) {
                Some(super::phase4_tools::StatusSegment::Screencast) => {
                    self.run_ui_command("workbench.action.toggleScreencastMode")
                }
                Some(super::phase4_tools::StatusSegment::Indentation) => {
                    self.run_ui_command("editor.action.toggleIndentation");
                }
                Some(super::phase4_tools::StatusSegment::LineEnding) => {
                    self.run_ui_command("shoppe.editor.toggleLineEnding")
                }
                Some(super::phase4_tools::StatusSegment::Encoding) => {
                    self.run_ui_command("shoppe.editor.nextEncoding")
                }
                Some(super::phase4_tools::StatusSegment::Branch) => {
                    self.run_ui_command("shoppe.scm.openBranchPicker")
                }
                None => {}
            }
            return;
        }

        if self.outline_visible && !self.zen_mode && mx >= window_width - 240.0 {
            if let Some(language) = self.documents[self.active_document()].language {
                let text = self.documents[self.active_document()].buffer.text();
                let row = ((my / lh) as isize - 1).max(0) as usize;
                if let Some(header) = graphite_syntax::declaration_headers(language, &text).get(row) {
                    self.run_ui_command_args(
                        "shoppe.editor.setCursor",
                        serde_json::json!({"line":header.line,"column":0}),
                    );
                    self.reveal_cursor = true;
                }
            }
            return;
        }

        if self.terminal_visible {
            let panel_top = gpu.renderer.size().1
                - gpu.renderer.line_height()
                - terminal_panel_height(gpu.renderer.size().1);
            if my >= panel_top {
                if my < panel_top + lh * 1.4 {
                    self.terminal_problems = mx < SIDEBAR_WIDTH + cw * 9.0;
                    self.terminal_focused = !self.terminal_problems;
                    if !self.terminal_problems && mx >= SIDEBAR_WIDTH + cw * 19.0 {
                        let offset = mx - SIDEBAR_WIDTH;
                        if offset < cw * 34.0 {
                            self.run_ui_command("shoppe.terminal.nextProfile");
                        } else if offset < cw * 38.0 {
                            self.run_ui_command("shoppe.terminal.new");
                        } else if offset < cw * 45.0 {
                            self.run_ui_command("shoppe.terminal.split");
                        } else if offset < cw * 52.0 {
                            self.run_ui_command("shoppe.terminal.close");
                        } else if offset < cw * 54.0 {
                            self.run_ui_command("shoppe.terminal.scrollLeft");
                        } else if offset < cw * 57.0 {
                            self.run_ui_command("shoppe.terminal.scrollRight");
                        } else {
                            let mut x = SIDEBAR_WIDTH + cw * 57.0;
                            let mut selected = None;
                            for (index, name) in self
                                .terminal_names
                                .iter()
                                .enumerate()
                                .skip(self.terminal_tab_offset)
                            {
                                let tab_width = (name.chars().count() as f32 + 2.0) * cw * 1.25;
                                if x + tab_width > window_width - PAD {
                                    break;
                                }
                                if mx >= x && mx < x + tab_width {
                                    selected = Some(index);
                                    break;
                                }
                                x += tab_width;
                            }
                            if let Some(index) = selected {
                                self.run_ui_command_args(
                                    "shoppe.terminal.select",
                                    serde_json::json!({"index":index}),
                                );
                            }
                        }
                    }
                } else if self.terminal_problems {
                    let row = ((my - panel_top - lh * 1.5) / lh).max(0.0) as usize;
                    self.run_ui_command_args(
                        "shoppe.problems.open",
                        serde_json::json!({"index":row}),
                    );
                } else {
                    self.terminal_focused = true;
                    self.focused_terminal = if let Some(second) = self.split_terminal {
                        let split_x = SIDEBAR_WIDTH + (window_width - SIDEBAR_WIDTH) / 2.0;
                        if mx >= split_x {
                            second
                        } else {
                            self.active_terminal
                        }
                    } else {
                        self.active_terminal
                    };
                }
                return;
            }
        }

        if mx < SIDEBAR_WIDTH {
            if self.scm_visible {
                let row = ((my - lh * 3.25) / lh).max(0.0) as usize;
                if my >= lh * 3.25 && row < self.scm_snapshot.files.len() {
                    self.run_ui_command_args("shoppe.scm.select", serde_json::json!({"index":row}));
                }
                return;
            }
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
                    let Some(document) = self.resolve_document(document) else {
                        continue;
                    };
                    let width = super::group_drawing::tab_width(&self.documents[document], cw);
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
                let row = self.editor_groups.active_view_mut().scroll_line
                    + ((my - rect.y - lh * 1.5).max(0.0) / lh) as usize;
                let col = (((mx - text_x) / cw).round().max(0.0)) as usize;
                let columns = (((rect.x + rect.width - text_x).max(cw)) / cw) as usize;
                let document = self.active_document();
                let tab = &mut self.documents[document];
                let offset = super::visual_lines::document_offset(
                    &tab.buffer,
                    row,
                    col,
                    columns,
                    tab.indentation.tab_size,
                    self.wrapping_indent,
                );
                if self.modifiers.alt_key() {
                    self.run_ui_command_args(
                        "shoppe.editor.addCursor",
                        serde_json::json!({"index":offset}),
                    );
                    self.selecting = false;
                } else {
                    self.run_ui_command_args(
                        "shoppe.editor.setSelection",
                        serde_json::json!({"anchor":offset,"head":offset}),
                    );
                    self.selecting = true;
                }
                return;
            }
            let line = self.editor_groups.active_view_mut().scroll_line
                + ((my - rect.y - lh * 1.5).max(0.0) / lh) as usize;
            let col = (((mx - text_x) / cw).round().max(0.0)) as usize;
            if self.modifiers.alt_key() {
                let b = self.editor();
                let line = line.min(b.len_lines() - 1);
                let width = b.line(line).trim_end_matches(['\n', '\r']).chars().count();
                let offset = b.line_start_char(line) + col.min(width);
                drop(b);
                self.run_ui_command_args(
                    "shoppe.editor.addCursor",
                    serde_json::json!({"index":offset}),
                );
                self.selecting = false;
                return;
            }
            self.run_ui_command_args(
                "shoppe.editor.setCursor",
                serde_json::json!({"line":line,"column":col}),
            );
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
            let row = self.editor_groups.active_view_mut().scroll_line
                + ((self.mouse.y as f32 - rect.y - lh * 1.5).max(0.0) / lh) as usize;
            let col = (((self.mouse.x as f32 - text_x) / cw).round().max(0.0)) as usize;
            let columns = (((rect.x + rect.width - text_x).max(cw)) / cw) as usize;
            let document = self.active_document();
            let tab = &mut self.documents[document];
            let head = super::visual_lines::document_offset(
                &tab.buffer,
                row,
                col,
                columns,
                tab.indentation.tab_size,
                self.wrapping_indent,
            );
            let anchor = self
                .editor()
                .selection()
                .map(|s| s.anchor)
                .unwrap_or(self.editor().cursor().char_idx);
            self.run_ui_command_args(
                "shoppe.editor.setSelection",
                serde_json::json!({"anchor":anchor,"head":head}),
            );
            return;
        }
        let line = (self.editor_groups.active_view_mut().scroll_line
            + ((self.mouse.y as f32 - rect.y - lh * 1.5).max(0.0) / lh) as usize)
            .min(self.documents[self.active_document()].buffer.len_lines() - 1);
        let col = (((self.mouse.x as f32 - text_x) / cw).round().max(0.0)) as usize;
        let head = self.documents[self.active_document()]
            .buffer
            .line_start_char(line)
            + col.min(
                self.documents[self.active_document()]
                    .buffer
                    .line(line)
                    .trim_end_matches(['\n', '\r'])
                    .chars()
                    .count(),
            );
        let anchor = self
            .editor()
            .selection()
            .map(|s| s.anchor)
            .unwrap_or(self.editor().cursor().char_idx);
        self.run_ui_command_args(
            "shoppe.editor.setSelection",
            serde_json::json!({"anchor":anchor,"head":head}),
        );
    }
}
