use super::*;

impl App {
    pub(crate) fn click(&mut self) {
        let Some(gpu) = &self.gpu else { return };
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let (window_width, window_height) = gpu.renderer.size();
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);

        if let Some(diff) = self.diff_view.as_mut() {
            if my < lh * 1.5 {
                diff.inline = !diff.inline;
            }
            return;
        }
        if my >= window_height - lh {
            match self.status_segment_at(mx, cw) {
                Some(super::phase4_tools::StatusSegment::Screencast) => {
                    self.toggle_screencast_mode()
                }
                Some(super::phase4_tools::StatusSegment::Indentation) => {
                    let document = self.active_document();
                    let mut target = self.documents[document].indentation;
                    target.insert_spaces = !target.insert_spaces;
                    let result = self.editor_mut().convert_indentation(target);
                    if result.is_ok() {
                        self.documents[document].indentation = target;
                        self.status = if target.insert_spaces {
                            "Indentation converted to spaces"
                        } else {
                            "Indentation converted to tabs"
                        }
                        .into();
                    }
                }
                Some(super::phase4_tools::StatusSegment::LineEnding) => {
                    let document = self.active_document();
                    self.documents[document].line_ending =
                        self.documents[self.active_document()].line_ending.toggle();
                    self.status = format!(
                        "Line endings: {}",
                        self.documents[self.active_document()].line_ending.label()
                    );
                }
                Some(super::phase4_tools::StatusSegment::Encoding) => {
                    let document = self.active_document();
                    self.documents[document].encoding =
                        self.documents[self.active_document()].encoding.next();
                    self.status = format!(
                        "Encoding for next save: {}",
                        self.documents[self.active_document()].encoding.label()
                    );
                }
                Some(super::phase4_tools::StatusSegment::Branch) => self.open_branch_picker(),
                None => {}
            }
            return;
        }

        if self.outline_visible && !self.zen_mode && mx >= window_width - 240.0 {
            if let Some(language) = self.documents[self.active_document()].language {
                let text = self.documents[self.active_document()].buffer.text();
                let row = ((my / lh) as isize - 1).max(0) as usize;
                if let Some(header) = graphite_syntax::declaration_headers(language, &text).get(row) {
                    self.editor_mut().set_cursor_line_col(header.line, 0);
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
                            self.cycle_terminal_profile();
                        } else if offset < cw * 38.0 {
                            self.create_terminal_tab();
                        } else if offset < cw * 45.0 {
                            self.toggle_terminal_split();
                        } else if offset < cw * 52.0 {
                            self.close_focused_terminal();
                        } else if offset < cw * 54.0 {
                            self.scroll_terminal_tabs(-1);
                        } else if offset < cw * 57.0 {
                            self.scroll_terminal_tabs(1);
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
                                self.select_terminal(index);
                            }
                        }
                    }
                } else if self.terminal_problems {
                    let row = ((my - panel_top - lh * 1.5) / lh).max(0.0) as usize;
                    if let Some((document, diagnostic)) =
                        self.diagnostic_entries().get(row).cloned()
                    {
                        self.remember_diagnostic(document, &diagnostic);
                        self.push_navigation_location();
                        self.activate_editor_document(document);
                        let line = (diagnostic.range.start.line as usize).min(
                            self.documents[document]
                                .buffer
                                .len_lines()
                                .saturating_sub(1),
                        );
                        let col = super::drawing_helpers::utf16_column_to_chars(
                            &self.documents[document].buffer.line(line),
                            diagnostic.range.start.character,
                        );
                        self.editor_mut().set_cursor_line_col(line, col);
                        self.reveal_cursor = true;
                        self.push_navigation_location();
                        self.request_quick_fixes_for(diagnostic);
                    }
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
                    self.scm_selected = row;
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
                    self.editor_mut().add_cursor(offset);
                    self.selecting = false;
                } else {
                    self.editor_mut().set_selection(offset, offset);
                    self.selecting = true;
                }
                return;
            }
            let line = self.editor_groups.active_view_mut().scroll_line
                + ((my - rect.y - lh * 1.5).max(0.0) / lh) as usize;
            let col = (((mx - text_x) / cw).round().max(0.0)) as usize;
            if self.modifiers.alt_key() {
                let mut b = self.editor_mut();
                let line = line.min(b.len_lines() - 1);
                let width = b.line(line).trim_end_matches(['\n', '\r']).chars().count();
                let offset = b.line_start_char(line) + col.min(width);
                b.add_cursor(offset);
                drop(b);
                self.selecting = false;
                return;
            }
            self.editor_mut().set_cursor_line_col(line, col);
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
            self.editor_mut().set_selection(anchor, head);
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
        self.editor_mut().set_selection(anchor, head);
    }
}
