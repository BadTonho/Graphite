use super::scm_ui::{scm_panel_layout, ScmAction, ScmListRowKind};
use super::*;

impl App {
    pub(crate) fn click(&mut self) {
        let Some((lh, cw, window_width, window_height)) = self.gpu.as_ref().map(|gpu| {
            let (width, height) = gpu.renderer.size();
            (
                gpu.renderer.line_height(),
                gpu.renderer.char_width(),
                width,
                height,
            )
        }) else {
            return;
        };
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);
        if self.handle_github_dialog_click(mx, my, window_width, window_height, cw) {
            return;
        }
        let sidebar_width = self.sidebar_layout_width_px();

        if self.help_modal.is_some() {
            self.help_modal_click(mx, my);
            return;
        }

        if self.command_center.is_some() {
            if self.command_center_popup_contains(mx, my) {
                self.command_center_click(mx, my);
                return;
            }
            self.close_command_center();
        }

        if self.profile_menu.is_some() {
            if self.profile_menu_contains(mx, my) {
                self.profile_menu_click(mx, my);
                return;
            }
            if !self.zen_mode && self.activity_bar_contains(mx, my) {
                self.activity_bar_click_at(mx, my);
                return;
            }
            self.profile_menu = None;
        }

        if self.settings_menu.is_some() {
            if self.settings_menu_contains(mx, my, window_width as f32, window_height as f32) {
                self.settings_menu_click(mx, my, window_width as f32, window_height as f32);
                return;
            }
            if !self.zen_mode && self.activity_bar_contains(mx, my) {
                self.activity_bar_click_at(mx, my);
                return;
            }
            self.settings_menu = None;
        }

        if self.workbench_menu_click(mx, my) {
            return;
        }
        self.column_selection_anchor = None;

        if self.activity_bar_menu.is_some() {
            self.handle_activity_bar_menu_click(mx, my, window_width as f32);
            return;
        }

        if self.diff_view.is_some() {
            if my < lh * 1.5 {
                self.run_ui_command("graphite.diff.toggleLayout");
            }
            return;
        }
        if self.view_preferences.status_bar_visible && !self.zen_mode && my >= window_height - lh {
            match self.status_segment_at(mx, cw) {
                Some(super::phase4_tools::StatusSegment::Extension(index)) => {
                    self.run_vsix_status_item(index)
                }
                Some(super::phase4_tools::StatusSegment::Screencast) => {
                    self.run_ui_command("workbench.action.toggleScreencastMode")
                }
                Some(super::phase4_tools::StatusSegment::Indentation) => {
                    self.run_ui_command("editor.action.toggleIndentation");
                }
                Some(super::phase4_tools::StatusSegment::LineEnding) => {
                    self.run_ui_command("graphite.editor.toggleLineEnding")
                }
                Some(super::phase4_tools::StatusSegment::Encoding) => {
                    self.run_ui_command("graphite.editor.nextEncoding")
                }
                Some(super::phase4_tools::StatusSegment::Branch) => {
                    self.run_ui_command("graphite.scm.openBranchPicker")
                }
                None => {}
            }
            return;
        }

        if !self.zen_mode && self.activity_bar_contains(mx, my) {
            self.activity_bar_click_at(mx, my);
            return;
        }

        if self.plugins.visible
            && !self.zen_mode
            && self.primary_sidebar_contains(mx, my, window_width, window_height, lh)
        {
            self.plugin_click();
            return;
        }
        if self.plugins.visible {
            self.plugins.focused = false;
        }

        if self.graphite_detail_click(mx, my) {
            return;
        }

        if self.outline_visible
            && self.view_preferences.secondary_sidebar_visible
            && !self.zen_mode
            && mx >= window_width - 240.0
        {
            if let Some(language) = self.documents[self.active_document()].language {
                let text = self.documents[self.active_document()].buffer.text();
                let row = ((my / lh) as isize - 1).max(0) as usize;
                if let Some(header) = graphite_syntax::declaration_headers(language, &text).get(row)
                {
                    self.run_ui_command_args(
                        "graphite.editor.setCursor",
                        serde_json::json!({"line":header.line,"column":0}),
                    );
                    self.reveal_cursor = true;
                }
            }
            return;
        }

        if self.terminal_visible && self.view_preferences.panel_visible {
            let panel = self.panel_bounds(window_width, window_height, lh);
            let panel_top = panel.y;
            if mx >= panel.x
                && mx < panel.x + panel.width
                && my >= panel_top
                && my < panel_top + panel.height
            {
                if self.modifiers.control_key()
                    && my >= panel_top + lh * 1.4
                    && self.open_terminal_link_at(mx, my, panel.x, panel_top, lh, cw)
                {
                    return;
                }
                if my < panel_top + lh * 1.4 {
                    let panel_offset = mx - panel.x;
                    self.terminal_problems = panel_offset < cw * 9.0;
                    self.terminal_focused = !self.terminal_problems;
                    if !self.terminal_problems && panel_offset >= cw * 19.0 {
                        let offset = panel_offset;
                        if offset < cw * 34.0 {
                            self.run_ui_command("graphite.terminal.nextProfile");
                        } else if offset < cw * 38.0 {
                            self.run_ui_command("graphite.terminal.new");
                        } else if offset < cw * 45.0 {
                            self.run_ui_command("graphite.terminal.split");
                        } else if offset < cw * 52.0 {
                            self.run_ui_command("graphite.terminal.close");
                        } else if offset < cw * 54.0 {
                            self.run_ui_command("graphite.terminal.scrollLeft");
                        } else if offset < cw * 57.0 {
                            self.run_ui_command("graphite.terminal.scrollRight");
                        } else {
                            let mut x = panel.x + cw * 57.0;
                            let mut selected = None;
                            for (index, name) in self
                                .terminal_names
                                .iter()
                                .enumerate()
                                .skip(self.terminal_tab_offset)
                            {
                                let tab_width = (name.chars().count() as f32 + 2.0) * cw * 1.25;
                                if x + tab_width > panel.x + panel.width - PAD {
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
                                    "graphite.terminal.select",
                                    serde_json::json!({"index":index}),
                                );
                            }
                        }
                    }
                } else if self.terminal_problems {
                    let row = ((my - panel_top - lh * 1.5) / lh).max(0.0) as usize;
                    self.run_ui_command_args(
                        "graphite.problems.open",
                        serde_json::json!({"index":row}),
                    );
                } else {
                    self.terminal_focused = true;
                    self.focused_terminal = if let Some(second) = self.split_terminal {
                        let split_x = sidebar_width + (window_width - sidebar_width) / 2.0;
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

        if self.primary_sidebar_contains(mx, my, window_width, window_height, lh) {
            if self.scm_visible {
                let local_x = self.sidebar_local_x(mx, window_width);
                let local_y = self.sidebar_local_y(my);
                let row_h = self.sidebar_density_line_height(lh).max(18.0);
                let layout = scm_panel_layout(
                    &self.scm_snapshot,
                    sidebar_width,
                    window_height,
                    row_h,
                    cw,
                    self.scm_scroll,
                    self.scm_repositories_collapsed,
                    self.scm_staged_collapsed,
                    self.scm_changes_collapsed,
                );
                if !self.scm_repositories_collapsed {
                    if layout.repo_sync.contains(local_x, local_y) {
                        self.run_scm_remote(super::scm_remote_ui::RemoteOp::Sync);
                        return;
                    }
                    if layout.repo_pull_request.contains(local_x, local_y) {
                        self.open_pull_request();
                        return;
                    }
                    if layout.repo_more.contains(local_x, local_y) {
                        self.open_scm_more_menu();
                        return;
                    }
                }
                if layout.repository_header.contains(local_x, local_y) {
                    self.scm_repositories_collapsed = !self.scm_repositories_collapsed;
                    return;
                }
                if layout.commit_field.contains(local_x, local_y) {
                    self.scm_commit_prompt = true;
                    return;
                }
                if layout.commit_button.contains(local_x, local_y)
                    && !self.scm_commit_message.trim().is_empty()
                {
                    self.run_ui_command("graphite.scm.commit");
                    return;
                }
                self.scm_commit_prompt = false;
                if layout.branch_row.contains(local_x, local_y) && !self.scm_repositories_collapsed
                {
                    if layout.refresh.contains(local_x, local_y) {
                        self.refresh_scm_async();
                    } else if layout.branch_picker.contains(local_x, local_y)
                        || local_x < layout.branch_picker.x
                    {
                        self.open_branch_picker();
                    }
                    return;
                }
                if let Some(row) = layout
                    .list_rows
                    .iter()
                    .find(|row| row.bounds.contains(local_x, local_y))
                {
                    match row.kind {
                        ScmListRowKind::Section { staged, .. } => {
                            if !staged {
                                let (history, stash) =
                                    super::scm_ui::changes_header_buttons(row.action, cw);
                                if history.contains(local_x, local_y) {
                                    self.open_scm_history();
                                    return;
                                }
                                if stash.contains(local_x, local_y) {
                                    self.run_scm_remote(super::scm_remote_ui::RemoteOp::Stash);
                                    return;
                                }
                            }
                            if row.action.contains(local_x, local_y) {
                                self.perform_scm_action(if staged {
                                    ScmAction::UnstageAll
                                } else {
                                    ScmAction::StageAll
                                });
                            } else if staged {
                                self.scm_staged_collapsed = !self.scm_staged_collapsed;
                            } else {
                                self.scm_changes_collapsed = !self.scm_changes_collapsed;
                            }
                        }
                        ScmListRowKind::File { index, staged } => {
                            self.scm_selected = index;
                            if row.action.contains(mx, my) {
                                self.perform_scm_action(if staged {
                                    ScmAction::Unstage
                                } else {
                                    ScmAction::Stage
                                });
                            } else {
                                self.run_ui_command_args(
                                    "graphite.scm.select",
                                    serde_json::json!({"index":index}),
                                );
                                let file = &self.scm_snapshot.files[index];
                                let path = self.tree.root().join(&file.path);
                                self.open_file(path);
                            }
                        }
                    }
                    return;
                }
                return;
            }
            let sidebar_lh = self.sidebar_density_line_height(lh);
            let first_row_y = sidebar_lh * 1.5;
            if my >= first_row_y {
                let row = ((my - first_row_y) / sidebar_lh) as usize + self.explorer_scroll;
                if let Some(entry) = self.tree.rows.get(row).map(|row| row.entry.path.clone()) {
                    self.explorer_select(entry.clone());
                    self.explorer_drag = Some(entry);
                }
                if let Some(path) = self.tree.activate(row) {
                    self.open_file_preview(path);
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
            self.explorer_focused = false;
            let tab_strip = self.editor_tab_strip_height(lh);
            if self.view_preferences.minimap_visible
                && mx >= rect.x + rect.width - 72.0_f32.min(rect.width * 0.22)
                && my >= rect.y + tab_strip
            {
                let document = self.active_document();
                let lines = self.documents[document].buffer.len_lines();
                let visible = (rect.height / lh).max(1.0) as usize;
                let ratio = ((my - rect.y - tab_strip) / (rect.height - tab_strip).max(1.0))
                    .clamp(0.0, 1.0);
                self.editor_groups.active_view_mut().scroll_line =
                    ((lines.saturating_sub(visible)) as f32 * ratio) as usize;
                self.reveal_cursor = false;
                return;
            }
            if tab_strip > 0.0 && my < rect.y + tab_strip {
                let mut x = rect.x;
                let views: Vec<(graphite_text::ViewId, DocumentId)> = self
                    .editor_groups
                    .views(rect.id)
                    .unwrap()
                    .0
                    .iter()
                    .map(|view| (view.id, view.document))
                    .collect();
                for (view_id, document) in views {
                    let Some(document) = self.resolve_document(document) else {
                        continue;
                    };
                    let width = super::group_drawing::tab_width(
                        &self.documents[document],
                        cw,
                        &self.translator,
                    );
                    if mx >= x && mx < x + width {
                        let (close_left, close_right) =
                            super::group_drawing::tab_close_area(x, width, cw);
                        if mx >= close_left && mx < close_right {
                            self.close_editor_tab(view_id);
                        } else {
                            self.activate_editor_document(document);
                        }
                        return;
                    }
                    x += width;
                }
                self.focus_editor_group(rect.id);
                self.graphite_detail_tab_click(rect.id, mx);
                return;
            }
            let text_x = rect.x + self.gutter_width(cw);
            if self.word_wrap {
                let row = self.editor_groups.active_view_mut().scroll_line
                    + ((my - rect.y - tab_strip).max(0.0) / lh) as usize;
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
                if self.multi_cursor_modifier_pressed() {
                    self.run_ui_command_args(
                        "graphite.editor.addCursor",
                        serde_json::json!({"index":offset}),
                    );
                    self.selecting = false;
                } else {
                    if self.view_preferences.column_selection_mode {
                        self.column_selection_anchor = Some((row, col));
                    }
                    self.run_ui_command_args(
                        "graphite.editor.setSelection",
                        serde_json::json!({"anchor":offset,"head":offset}),
                    );
                    self.selecting = true;
                }
                return;
            }
            let line = self.editor_line_at_row(((my - rect.y - tab_strip).max(0.0) / lh) as usize);
            let col = (((mx - text_x) / cw).round().max(0.0)) as usize;
            if self.multi_cursor_modifier_pressed() {
                let b = self.editor();
                let line = line.min(b.len_lines() - 1);
                let width = b.line(line).trim_end_matches(['\n', '\r']).chars().count();
                let offset = b.line_start_char(line) + col.min(width);
                drop(b);
                self.run_ui_command_args(
                    "graphite.editor.addCursor",
                    serde_json::json!({"index":offset}),
                );
                self.selecting = false;
                return;
            }
            if self.view_preferences.column_selection_mode {
                self.column_selection_anchor = Some((line, col));
            }
            self.run_ui_command_args(
                "graphite.editor.setCursor",
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
        let tab_strip = self.editor_tab_strip_height(lh);
        let text_x = rect.x + self.gutter_width(cw);
        if self.view_preferences.column_selection_mode {
            if let Some((anchor_row, anchor_column)) = self.column_selection_anchor {
                let row = self.editor_line_at_row(
                    ((self.mouse.y as f32 - rect.y - tab_strip).max(0.0) / lh) as usize,
                );
                let column = (((self.mouse.x as f32 - text_x) / cw).round().max(0.0)) as usize;
                let columns = (((rect.x + rect.width - text_x).max(cw)) / cw) as usize;
                let document = self.active_document();
                let tab_size = self.documents[document].indentation.tab_size;
                let indent = self.wrapping_indent;
                let wrap = self.word_wrap;
                let mut editor = self.editor_mut();
                let selections = super::selection_menu::rectangular_selections(
                    &editor,
                    (anchor_row, anchor_column),
                    (row, column),
                    columns,
                    tab_size,
                    wrap,
                    indent,
                );
                editor.set_selections(selections);
                return;
            }
        }
        if self.word_wrap {
            let row = self.editor_groups.active_view_mut().scroll_line
                + ((self.mouse.y as f32 - rect.y - tab_strip).max(0.0) / lh) as usize;
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
                "graphite.editor.setSelection",
                serde_json::json!({"anchor":anchor,"head":head}),
            );
            return;
        }
        let line = self.editor_line_at_row(
            ((self.mouse.y as f32 - rect.y - tab_strip).max(0.0) / lh) as usize,
        );
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
            "graphite.editor.setSelection",
            serde_json::json!({"anchor":anchor,"head":head}),
        );
    }
}
