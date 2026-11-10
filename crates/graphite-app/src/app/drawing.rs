use super::*;

impl App {
    pub(crate) fn draw(&mut self) {
        self.resize_terminal();
        let sidebar_width = self.sidebar_layout_width_px();
        let Some(mut gpu) = self.gpu.take() else {
            return;
        };
        let r = &mut gpu.renderer;
        let (w, h) = r.size();
        let (lh, cw) = (r.line_height(), r.char_width());
        let editor_left = if self.activity_bar_preferences.primary_sidebar_right {
            self.activity_bar_width_px()
        } else {
            self.sidebar_width_px()
        };
        let editor_right = if self.activity_bar_preferences.primary_sidebar_right {
            self.sidebar_width_px() - if self.outline_visible { 240.0 } else { 0.0 }
        } else {
            w - if self.outline_visible { 240.0 } else { 0.0 }
        };
        let editor_width = (editor_right - editor_left).max(0.0);

        let visible = self.visible_lines(r);
        if self.reveal_cursor {
            self.keep_cursor_visible(visible, w, h, lh, cw);
            self.reveal_cursor = false;
        }
        self.refresh_highlights();
        gpu.accessibility
            .update_if_active(|| self.grouped_accessibility_tree(w, h, lh));

        // Sidebar views share one transform so activity position and side affect
        // Explorer, Source Control and Plugins consistently.
        r.push_translation(
            self.sidebar_translation(w),
            self.activity_bar_vertical_offset(),
        );
        // Sidebar
        if !self.plugins.visible {
            r.draw_rect(
                ACTIVITY_BAR_WIDTH,
                0.0,
                sidebar_width - ACTIVITY_BAR_WIDTH,
                h,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            r.draw_text(
                &self.tree.root_name().to_uppercase(),
                ACTIVITY_BAR_WIDTH + PAD,
                lh * 0.25,
                rgb(Theme::color(Theme::DIM, 0x616161)),
            );
            for (i, row) in self.tree.rows.iter().enumerate() {
                let y = lh * 1.5 + i as f32 * lh;
                if y > h {
                    break;
                }
                let is_open = row.entry.path.as_path()
                    == self.documents[self.active_document()]
                        .buffer
                        .path()
                        .unwrap_or(std::path::Path::new(""));
                if is_open {
                    r.draw_rect(
                        ACTIVITY_BAR_WIDTH,
                        y,
                        sidebar_width - ACTIVITY_BAR_WIDTH,
                        lh,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                let indent = ACTIVITY_BAR_WIDTH + PAD + row.depth as f32 * cw * 2.0;
                let chevron_slot = cw * 1.8;
                let icon_slot = lh;
                let icon_size = (lh * 0.85).round();
                let chevron_size = (lh * 0.72).round();
                if row.entry.is_dir {
                    let chevron = if row.expanded {
                        crate::file_icons::CHEVRON_DOWN
                    } else {
                        crate::file_icons::CHEVRON_RIGHT
                    };
                    r.draw_icon(
                        chevron,
                        indent + (chevron_slot - chevron_size) * 0.5,
                        y + (lh - chevron_size) * 0.5,
                        chevron_size,
                        rgb(Theme::color(Theme::DIM, 0x616161)),
                    );
                }
                let icon_x = indent + chevron_slot;
                r.draw_icon(
                    row.icon.glyph,
                    icon_x + (icon_slot - icon_size) * 0.5,
                    y + (lh - icon_size) * 0.5,
                    icon_size,
                    rgb(row.icon.color),
                );
                r.draw_text(
                    &row.entry.name,
                    icon_x + icon_slot + 4.0,
                    y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if self.zen_mode {
            r.draw_rect(0.0, 0.0, w, h, rgb(Theme::color(Theme::BG, 0xffffff)));
        } else {
            self.draw_plugins_sidebar(r, h, lh, cw);
            if self.scm_visible {
                self.draw_scm_sidebar(r, h, lh, cw);
            }
            self.draw_plugin_activity_sidebar(r, h, lh, cw);
        }
        r.pop_translation();

        let rects = self.group_rectangles(w, h, lh);
        self.draw_editor_groups(r, &rects, lh, cw);
        let focused_rect = self.focused_group_rect(w, h, lh);
        let text_x = focused_rect.x + self.gutter_width(cw);
        let (cur_line, cur_col) = self.editor().cursor_line_col();
        // Status bar
        r.set_clip(None);
        r.draw_rect(
            0.0,
            h - lh,
            w,
            lh,
            rgb(Theme::color(Theme::STATUS, 0x005a9e)),
        );
        let prefix = self.status_bar_prefix();
        r.draw_text(&prefix, 0.0, h - lh, rgb(0xffffff));
        let mut status_x = prefix.chars().count() as f32 * cw;
        for (_, label) in self.status_bar_segments() {
            status_x += cw * 3.0;
            r.draw_text(&label, status_x, h - lh, rgb(0xffffff));
            status_x += label.chars().count() as f32 * cw;
        }
        if let Some(quick) = &self.quick_open {
            let count = quick.paths.len().min(8);
            let height = lh * (count as f32 + 1.5);
            let y = lh * 1.5;
            r.draw_rect(
                editor_left,
                y,
                editor_width,
                height,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            r.draw_text(
                &self.tf(
                    "overlay.quick_open.prompt",
                    &[
                        ("query", &quick.query),
                        ("preedit", &self.editor_groups.active_view().ime_preedit),
                    ],
                ),
                editor_left + PAD,
                y + PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            let start = quick
                .index
                .saturating_sub(7)
                .min(quick.paths.len().saturating_sub(8));
            for (row, (i, path)) in quick
                .paths
                .iter()
                .enumerate()
                .skip(start)
                .take(8)
                .enumerate()
            {
                let item_y = y + lh * (row as f32 + 1.0);
                if i == quick.index {
                    r.draw_rect(
                        editor_left,
                        item_y,
                        editor_width,
                        lh,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                let relative = path.strip_prefix(self.tree.root()).unwrap_or(path);
                let icon = crate::file_icons::for_path(path);
                let icon_size = (lh * 0.85).round();
                r.draw_icon(
                    icon.glyph,
                    editor_left + PAD,
                    item_y + (lh - icon_size) * 0.5,
                    icon_size,
                    rgb(icon.color),
                );
                r.draw_text(
                    &relative.display().to_string(),
                    editor_left + PAD + lh + 4.0,
                    item_y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if let Some(palette) = &self.command_palette {
            let rows = self.command_palette_rows();
            let height = lh * (rows.len().min(8) as f32 + 1.5);
            let y = lh * 1.5;
            let x = editor_left;
            let width = editor_width;
            r.draw_rect(
                x,
                y,
                width,
                height,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            let prompt = match &palette.extension_prompt {
                Some((title, true)) => {
                    format!("{} > {}", title, "•".repeat(palette.query.chars().count()))
                }
                Some((title, false)) => format!("{} > {}", title, palette.query),
                None => format!("> {}", palette.query),
            };
            r.draw_text(
                &prompt,
                x + PAD,
                y + PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            let selected = palette
                .items
                .iter()
                .filter(|item| {
                    let query = palette.query.to_lowercase();
                    query.is_empty()
                        || item.title.to_lowercase().contains(&query)
                        || item.id.to_lowercase().contains(&query)
                })
                .nth(palette.selected)
                .map(|item| item.id.as_str());
            for (index, item) in rows.iter().enumerate() {
                let row_y = y + lh * (index as f32 + 1.0);
                if Some(item.id.as_str()) == selected {
                    r.draw_rect(
                        x,
                        row_y,
                        width,
                        lh,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                r.draw_text(
                    &format!(
                        "{}  [{}]{}",
                        item.title,
                        item.source,
                        if item.enabled { "" } else { " (disabled)" }
                    ),
                    x + PAD,
                    row_y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if let Some(search) = &self.local_search {
            let height = if search.replace { lh * 3.5 } else { lh * 2.5 };
            let y = h - lh - height;
            r.draw_rect(
                editor_left,
                y,
                editor_width,
                height,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            let preedit = if search.edit_replacement {
                ""
            } else {
                &self.editor_groups.active_view().ime_preedit
            };
            let label = format!(
                "{} {}",
                if search.edit_replacement { " " } else { ">" },
                self.tf(
                    "overlay.find.prompt",
                    &[
                        ("query", &search.query),
                        ("preedit", &preedit),
                        ("message", &search.message),
                    ],
                )
            );
            r.draw_text(
                &label,
                editor_left + PAD,
                y + PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            if search.replace {
                let preedit = if search.edit_replacement {
                    &self.editor_groups.active_view().ime_preedit
                } else {
                    ""
                };
                let replacement = format!(
                    "{} {}",
                    if search.edit_replacement { ">" } else { " " },
                    self.tf(
                        "overlay.find.replace",
                        &[("replacement", &search.replacement), ("preedit", &preedit),],
                    )
                );
                r.draw_text(
                    &replacement,
                    editor_left + PAD,
                    y + lh + PAD,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
            let on = self.t("common.on");
            let off = self.t("common.off");
            let opts = self.tf(
                "overlay.find.options",
                &[
                    ("regex", &if search.options.regex { on } else { off }),
                    (
                        "case",
                        &if search.options.case_sensitive {
                            on
                        } else {
                            off
                        },
                    ),
                    ("word", &if search.options.whole_word { on } else { off }),
                    (
                        "preserve",
                        &if search.options.preserve_case {
                            on
                        } else {
                            off
                        },
                    ),
                    ("selection", &if search.scope.is_some() { on } else { off }),
                ],
            );
            r.draw_text(
                &opts,
                editor_left + PAD,
                y + height - lh,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
        }
        if let Some(query) = &self.search_query {
            let label = self.tf(
                "overlay.project_search.prompt",
                &[
                    ("query", query),
                    ("preedit", &self.editor_groups.active_view().ime_preedit),
                    ("count", &self.search_results.len()),
                ],
            );
            let height = lh * (self.search_results.len().min(6) as f32 + 1.2);
            r.draw_rect(
                editor_left,
                h - lh - height,
                editor_width,
                height,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            r.draw_text(
                &label,
                editor_left + PAD,
                h - lh - height + PAD,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            let start = self
                .search_index
                .saturating_sub(5)
                .min(self.search_results.len().saturating_sub(6));
            for (visible_i, (i, hit)) in self
                .search_results
                .iter()
                .enumerate()
                .skip(start)
                .take(6)
                .enumerate()
            {
                let y = h - lh - height + lh * (visible_i as f32 + 1.0);
                if i == self.search_index {
                    r.draw_rect(
                        editor_left,
                        y,
                        editor_width,
                        lh,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                let rel = hit
                    .path
                    .strip_prefix(self.tree.root())
                    .unwrap_or(&hit.path)
                    .display();
                let row = format!("{}:{}  {}", rel, hit.line + 1, hit.text);
                r.draw_text(
                    &row,
                    editor_left + PAD,
                    y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if let Some(query) = &self.snippet_query {
            let matches: Vec<_> = self
                .snippets
                .iter()
                .filter(|s| s.prefix.starts_with(query) && self.snippet_scope_matches(s))
                .take(6)
                .collect();
            let y = h - lh * (matches.len() as f32 + 1.0);
            r.draw_rect(
                editor_left,
                y,
                editor_width,
                lh * (matches.len() as f32 + 1.0),
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            r.draw_text(
                &self.tf(
                    "overlay.snippet.prompt",
                    &[
                        ("query", query),
                        ("preedit", &self.editor_groups.active_view().ime_preedit),
                    ],
                ),
                editor_left + PAD,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            for (i, s) in matches.iter().enumerate() {
                r.draw_text(
                    &s.prefix,
                    editor_left + PAD,
                    y + lh * (i as f32 + 1.0),
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }

        if !self.completion_items.is_empty() {
            let visible_items = self
                .completion_items
                .iter()
                .take(7)
                .enumerate()
                .collect::<Vec<_>>();
            let box_height = lh * visible_items.len() as f32;
            let y = (focused_rect.y
                + lh * (cur_line.saturating_sub(self.editor_groups.active_view_mut().scroll_line)
                    as f32
                    + 1.5)
                + 2.0)
                .min(focused_rect.y + focused_rect.height - box_height);
            let x = (text_x + cur_col as f32 * cw).min(focused_rect.x + focused_rect.width - 300.0);
            r.draw_rect(
                x,
                y,
                300.0,
                box_height,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            for (i, item) in visible_items {
                let row_y = y + i as f32 * lh;
                if i == self.completion_index {
                    r.draw_rect(
                        x,
                        row_y,
                        300.0,
                        lh,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                r.draw_text(
                    &item.label,
                    x + PAD,
                    row_y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if let Some(hover) = self.hover_text.as_deref().filter(|text| !text.is_empty()) {
            let lines = hover.lines().take(6).collect::<Vec<_>>();
            let box_h = lh * lines.len() as f32 + PAD;
            let y = (focused_rect.y + lh * 1.6).min(focused_rect.y + focused_rect.height - box_h);
            r.draw_rect(
                text_x,
                y,
                (focused_rect.x + focused_rect.width - text_x - PAD).max(0.0),
                box_h,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            for (i, line) in lines.iter().enumerate() {
                r.draw_text(
                    line,
                    text_x + PAD,
                    y + i as f32 * lh + PAD * 0.5,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if self.quick_fix_visible {
            let items = self
                .quick_fix_actions
                .iter()
                .take(6)
                .enumerate()
                .collect::<Vec<_>>();
            let lines = items.len().max(1);
            let box_h = lh * (lines as f32 + 1.0);
            let y = (focused_rect.y + lh * 1.6).min(focused_rect.y + focused_rect.height - box_h);
            let x = (text_x + PAD).min(focused_rect.x + focused_rect.width - 360.0);
            r.draw_rect(
                x,
                y,
                360.0,
                box_h,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            r.draw_text(
                self.t("overlay.quick_fix.title"),
                x + PAD,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            if items.is_empty() {
                r.draw_text(
                    self.t("overlay.quick_fix.empty"),
                    x + PAD,
                    y + lh,
                    rgb(Theme::color(Theme::DIM, 0x858585)),
                );
            }
            for (index, action) in items {
                let title = match action {
                    lsp_types::CodeActionOrCommand::CodeAction(action) => action.title.as_str(),
                    lsp_types::CodeActionOrCommand::Command(command) => command.title.as_str(),
                };
                let row_y = y + lh * (index as f32 + 1.0);
                if index == self.quick_fix_index {
                    r.draw_rect(
                        x,
                        row_y,
                        360.0,
                        lh,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                r.draw_text(
                    title,
                    x + PAD,
                    row_y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if self.terminal_visible {
            self.draw_terminal_panel(r, w, h, lh, cw);
        }
        if self.zen_mode {
            r.draw_rect(0.0, h - lh, w, lh, rgb(Theme::color(Theme::BG, 0xffffff)));
        }
        self.draw_phase4_overlays(r, w, h, lh);
        self.draw_phase6_overlays(r, w, h, lh);
        if !self.zen_mode {
            let split = self.sidebar_width_px();
            let hovered =
                !self.sidebar_resize_dragging && (self.mouse.x as f32 - split).abs() <= 4.0;
            r.draw_rect(
                split - 0.5,
                0.0,
                1.0,
                h,
                rgb(Theme::color(0x3f3f46, 0xd0d0d0)),
            );
            if hovered || self.sidebar_resize_dragging {
                r.draw_rect(
                    split - 1.5,
                    0.0,
                    3.0,
                    h,
                    rgb(Theme::color(Theme::STATUS, 0x007acc)),
                );
            }
        }

        if !self.zen_mode {
            self.draw_activity_bar(r, h);
        }

        self.draw_workbench_menu(r);

        r.set_clear_color(rgb(Theme::color(Theme::BG, 0xffffff)));
        r.present();
        self.gpu = Some(gpu);
    }
}
