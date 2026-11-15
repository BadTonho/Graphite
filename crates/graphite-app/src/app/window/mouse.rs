use super::scm_remote_ui::RemoteOp;
use super::scm_ui::{ScmAction, ScmListRowKind, ScmPrimaryAction};
use super::terminal_panel::terminal_drawing::{
    TerminalHeaderTab, TerminalToolbarAction, TERMINAL_HEADER_HEIGHT,
};
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

        let prev_terminal_toolbar_menu = self.terminal_toolbar_menu.as_ref().map(|m| m.kind);
        if self.terminal_toolbar_menu.is_some() {
            if self.terminal_toolbar_menu_contains(mx, my) {
                self.terminal_toolbar_menu_click(mx, my);
                return;
            }
            self.terminal_toolbar_menu = None;
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

        if self.click_scm_confirm(mx, my, window_width as f32, window_height as f32) {
            return;
        }
        if self.click_scm_publish_modal(mx, my, window_width as f32, window_height as f32) {
            return;
        }
        if self.click_explorer_delete_confirm(mx, my, window_width as f32, window_height as f32) {
            return;
        }
        if self.scm_more_menu.is_some() {
            self.scm_more_menu_click(mx, my);
            return;
        }
        if self.scm_graph_menu.is_some() {
            // A click outside only closes the menu; one inside runs its action.
            self.scm_graph_menu_click(mx, my);
            return;
        }
        if self.explorer_context_menu.is_some() {
            if self.explorer_context_menu_contains(mx, my) {
                self.explorer_context_menu_click(mx, my);
                return;
            }
            self.explorer_context_menu = None;
        }

        if self.activity_bar_menu.is_some() {
            self.handle_activity_bar_menu_click(mx, my, window_width as f32);
            return;
        }

        if self.multi_diff.is_some() {
            self.multi_diff_click(mx, my);
            return;
        }
        if self.merge_editor.is_some() && self.diff_view.is_none() {
            self.merge_handle_click(mx, my);
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
                    self.run_extension_status_item(index)
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

        if self.handle_chat_click(mx, my, window_width, window_height, lh) {
            return;
        }

        if self.outline_visible
            && self.view_preferences.secondary_sidebar_visible
            && !self.zen_mode
            && !(self.chat.visible && self.chat.surface == super::chat::ChatSurface::SideBar)
            && mx >= window_width - 240.0
        {
            let first_row = if self.outline.filter_on_type {
                2.5
            } else {
                1.5
            };
            let row = ((my / lh) - first_row).floor();
            if row >= 0.0 {
                let row = row as usize;
                let depth = self.outline_rows().get(row).map_or(0, |entry| entry.depth);
                let chevron_start = window_width - 240.0 + PAD + depth as f32 * 2.0 * cw;
                self.outline_click(row, mx < chevron_start + cw * 2.0);
            }
            return;
        }

        if self.terminal_visible && self.view_preferences.panel_visible {
            let panel = self.panel_bounds(window_width, window_height, lh);
            if mx >= panel.x
                && mx < panel.x + panel.width
                && my >= panel.y
                && my < panel.y + panel.height
            {
                self.handle_terminal_panel_click(
                    mx,
                    my,
                    panel.x,
                    panel.y,
                    panel.width,
                    lh,
                    cw,
                    prev_terminal_toolbar_menu,
                );
                return;
            }
        }

        if self.primary_sidebar_contains(mx, my, window_width, window_height, lh) {
            if self.active_plugin_activity.is_some()
                && self.plugin_activity_sidebar_click(mx, my, window_width, lh)
            {
                return;
            }
            if self.scm_visible {
                let local_x = self.sidebar_local_x(mx, window_width);
                let local_y = self.sidebar_local_y(my);
                let row_h = lh.max(18.0);
                if self.shows_scm_welcome_view() {
                    self.click_scm_welcome_view(
                        local_x,
                        local_y,
                        self.sidebar_layout_width_px(),
                        row_h,
                        cw,
                    );
                    return;
                }
                let layout = self.scm_sidebar_layout_for(window_height, lh, cw);
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
                    self.scm_extension_input = None;
                    return;
                }
                if self.scm_primary_action() == ScmPrimaryAction::Commit
                    && super::scm_ui::commit_arrow(layout.commit_button)
                        .contains(local_x, local_y)
                {
                    let arrow = super::scm_ui::commit_arrow(layout.commit_button);
                    let (anchor_x, anchor_y) = (mx - (local_x - arrow.x), my + (arrow.y + arrow.height - local_y));
                    self.open_scm_commit_menu((anchor_x, anchor_y));
                    return;
                }
                if layout.commit_button.contains(local_x, local_y) {
                    match self.scm_primary_action() {
                        ScmPrimaryAction::PublishRepository => {
                            self.open_publish_repo_modal();
                            return;
                        }
                        ScmPrimaryAction::PublishBranch => {
                            self.run_scm_remote(RemoteOp::Push);
                            return;
                        }
                        ScmPrimaryAction::Sync { .. } => {
                            self.run_scm_remote(RemoteOp::Sync);
                            return;
                        }
                        ScmPrimaryAction::Commit => {}
                    }
                    if !self.scm_commit_message.trim().is_empty() {
                        self.run_ui_command("graphite.scm.commit");
                        return;
                    }
                }
                self.scm_commit_prompt = false;
                self.scm_extension_input = None;
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
                        ScmListRowKind::Extension(extension) => {
                            let row = *row;
                            self.click_extension_scm_row(&row, extension, local_x, local_y);
                        }
                        ScmListRowKind::Folder {
                            staged,
                            first,
                            depth,
                            ..
                        } => {
                            if let Some(file) = self.scm_snapshot.files.get(first) {
                                let folder = super::scm_ui::folder_path(&file.path, depth);
                                if row.action.contains(local_x, local_y) {
                                    self.stage_folder(staged, &folder);
                                } else {
                                    self.toggle_scm_folder(staged, folder);
                                }
                            }
                        }
                        ScmListRowKind::GraphHeader { .. } => {
                            let [fetch, pull, push, refresh] =
                                super::source_control::scm_graph_ui::graph_header_buttons(
                                    row.action, cw,
                                );
                            if fetch.contains(local_x, local_y) {
                                self.run_scm_remote(super::scm_remote_ui::RemoteOp::Fetch);
                            } else if pull.contains(local_x, local_y) {
                                self.run_scm_remote(super::scm_remote_ui::RemoteOp::Pull);
                            } else if push.contains(local_x, local_y) {
                                self.run_scm_remote(super::scm_remote_ui::RemoteOp::Push);
                            } else if refresh.contains(local_x, local_y) {
                                self.refresh_scm_async();
                            } else {
                                self.scm_graph_collapsed = !self.scm_graph_collapsed;
                            }
                        }
                        ScmListRowKind::GraphCommit { index } => {
                            if let Some(hash) = self
                                .scm_graph_rows
                                .get(index)
                                .map(|r| r.commit.hash.clone())
                            {
                                self.scm_graph_selected_hash = Some(hash.clone());
                                self.open_scm_commit_diff(&hash);
                            }
                        }
                        ScmListRowKind::File { index, staged } => {
                            self.scm_selected = index;
                            if !staged
                                && super::scm_ui::discard_button(row.action, cw)
                                    .contains(local_x, local_y)
                            {
                                if let Some(target) = self.discard_target_at(index) {
                                    self.confirm_discard(vec![target]);
                                }
                            } else if row.action.contains(mx, my) {
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
            let sidebar_lh = lh;
            let first_row_y = sidebar_lh * 1.5;
            if my >= first_row_y {
                let row = ((my - first_row_y) / sidebar_lh) as usize + self.explorer_scroll;
                if row >= self.tree.rows.len() {
                    self.activate_virtual_explorer_row(row - self.tree.rows.len());
                    return;
                }
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
            if self.active_notebook().is_some() {
                self.notebook_click(mx, my, rect);
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
            let body_row = ((my - rect.y - tab_strip).max(0.0) / lh) as usize;
            // A click on the lens row above a line runs its lens.
            if let Some(lens_line) = self.editor_row_lens_line(body_row) {
                if self.run_code_lens_on_line(lens_line) {
                    return;
                }
            }
            let line = self.editor_line_at_row(body_row);
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

    #[allow(clippy::too_many_arguments)]
    fn handle_terminal_panel_click(
        &mut self,
        mx: f32,
        my: f32,
        panel_x: f32,
        panel_top: f32,
        panel_width: f32,
        lh: f32,
        cw: f32,
        prev_terminal_toolbar_menu: Option<
            super::terminal_panel::terminal_toolbar_menu::TerminalToolbarMenuKind,
        >,
    ) {
        if self.modifiers.control_key()
            && my >= panel_top + TERMINAL_HEADER_HEIGHT + 4.0
            && self.open_terminal_link_at(mx, my, panel_x, panel_top, lh, cw)
        {
            return;
        }
        if my < panel_top + TERMINAL_HEADER_HEIGHT {
            let layout = self.terminal_header_layout(panel_x, panel_top, panel_width, cw);
            if let Some(hit) = layout.tabs.iter().find(|rect| rect.contains(mx, my)) {
                match hit.item {
                    TerminalHeaderTab::Problems => {
                        self.terminal_problems = true;
                        self.terminal_focused = false;
                    }
                    TerminalHeaderTab::Terminal => {
                        self.terminal_problems = false;
                        self.terminal_focused = true;
                    }
                    TerminalHeaderTab::Output => {
                        self.terminal_problems = false;
                        self.terminal_focused = false;
                        self.run_ui_command("workbench.action.showOutputChannels");
                    }
                    TerminalHeaderTab::DebugConsole => {
                        self.debug_panel_visible = !self.debug_panel_visible;
                    }
                    TerminalHeaderTab::Ports => {
                        self.run_ui_command("graphite.ports.open");
                    }
                    TerminalHeaderTab::Comments => {
                        self.terminal_problems = true;
                        self.terminal_focused = false;
                    }
                }
                return;
            }
            if let Some(hit) = layout.actions.iter().find(|rect| rect.contains(mx, my)) {
                match hit.item {
                    TerminalToolbarAction::NewTerminal => {
                        self.terminal_problems = false;
                        self.terminal_focused = true;
                        self.run_ui_command("graphite.terminal.new");
                    }
                    TerminalToolbarAction::CycleProfile => {
                        if prev_terminal_toolbar_menu
                            != Some(super::terminal_panel::terminal_toolbar_menu::TerminalToolbarMenuKind::ProfilePicker)
                        {
                            let (window_width, window_height) = self
                                .gpu
                                .as_ref()
                                .map_or((panel_x + panel_width, panel_top + 200.0), |gpu| {
                                    gpu.renderer.size()
                                });
                            self.open_terminal_profile_menu(
                                (hit.x, hit.y, hit.w, hit.h),
                                window_width,
                                window_height,
                            );
                        }
                    }
                    TerminalToolbarAction::SplitTerminal => {
                        self.terminal_problems = false;
                        self.terminal_focused = true;
                        self.run_ui_command("graphite.terminal.split");
                    }
                    TerminalToolbarAction::KillTerminal => {
                        self.run_ui_command("graphite.terminal.close");
                    }
                    TerminalToolbarAction::MoreActions => {
                        if prev_terminal_toolbar_menu
                            != Some(super::terminal_panel::terminal_toolbar_menu::TerminalToolbarMenuKind::MoreActions)
                        {
                            let (window_width, window_height) = self
                                .gpu
                                .as_ref()
                                .map_or((panel_x + panel_width, panel_top + 200.0), |gpu| {
                                    gpu.renderer.size()
                                });
                            self.open_terminal_more_actions_menu(
                                (hit.x, hit.y, hit.w, hit.h),
                                window_width,
                                window_height,
                            );
                        }
                    }
                    TerminalToolbarAction::ToggleMaximize => {
                        self.view_preferences.panel_height_delta =
                            if self.view_preferences.panel_height_delta > 0 {
                                0
                            } else {
                                220
                            };
                        self.save_view_preferences_for_command();
                        self.resize_terminal();
                    }
                    TerminalToolbarAction::ClosePanel => {
                        self.terminal_visible = false;
                        self.terminal_focused = false;
                    }
                    TerminalToolbarAction::PrevSessionTabs => {
                        self.run_ui_command("graphite.terminal.scrollLeft");
                    }
                    TerminalToolbarAction::NextSessionTabs => {
                        self.run_ui_command("graphite.terminal.scrollRight");
                    }
                    TerminalToolbarAction::SelectSession(index) => {
                        self.terminal_problems = false;
                        self.terminal_focused = true;
                        self.run_ui_command_args(
                            "graphite.terminal.select",
                            serde_json::json!({"index": index}),
                        );
                    }
                }
            }
            return;
        }
        if self.terminal_problems {
            let row = ((my - panel_top - TERMINAL_HEADER_HEIGHT - 4.0) / lh).max(0.0) as usize;
            self.run_ui_command_args("graphite.problems.open", serde_json::json!({"index": row}));
        } else {
            self.terminal_focused = true;
            self.focused_terminal = if let Some(second) = self.split_terminal {
                let split_x = panel_x + panel_width * 0.5;
                if mx >= split_x {
                    second
                } else {
                    self.active_terminal
                }
            } else {
                self.active_terminal
            };
        }
    }
}
