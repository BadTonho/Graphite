use super::*;

impl App {
    pub(crate) fn draw(&mut self) {
        self.resize_terminal();
        let Some(mut gpu) = self.gpu.take() else {
            return;
        };
        let r = &mut gpu.renderer;
        let (w, h) = r.size();
        let (lh, cw) = (r.line_height(), r.char_width());

        let visible = self.visible_lines(r);
        self.keep_cursor_visible(visible);
        self.refresh_highlights();
        let accessible_panel = self.accessible_panel_text();
        let titles: Vec<_> = self
            .tabs
            .iter()
            .map(|t| {
                t.buffer
                    .path()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".into())
            })
            .collect();
        gpu.accessibility.update_if_active(|| {
            accessibility_tree(
                &titles,
                &self.tabs[self.active_tab].buffer.text(),
                accessible_panel.as_deref(),
            )
        });

        // Sidebar
        r.draw_rect(
            0.0,
            0.0,
            SIDEBAR_WIDTH,
            h,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        r.draw_text(
            &self.tree.root_name().to_uppercase(),
            PAD,
            lh * 0.25,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        for (i, row) in self.tree.rows.iter().enumerate() {
            let y = lh * 1.5 + i as f32 * lh;
            if y > h {
                break;
            }
            let is_open = row.entry.path.as_path()
                == self.tabs[self.active_tab]
                    .buffer
                    .path()
                    .unwrap_or(std::path::Path::new(""));
            if is_open {
                r.draw_rect(
                    0.0,
                    y,
                    SIDEBAR_WIDTH,
                    lh,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                );
            }
            let marker = match (row.entry.is_dir, row.expanded) {
                (true, true) => "v ",
                (true, false) => "> ",
                _ => "  ",
            };
            let label = format!("{marker}{}", row.entry.name);
            r.draw_text(
                &label,
                PAD + row.depth as f32 * cw * 2.0,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
        }

        // Editor
        let gutter = self.gutter_width(cw);
        let text_x = SIDEBAR_WIDTH + gutter;
        let mut tab_x = SIDEBAR_WIDTH;
        for (i, tab) in self.tabs.iter().enumerate() {
            let label = tab
                .buffer
                .path()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into());
            let label = format!(
                " {}{} ",
                label,
                if tab.buffer.is_dirty() { " ●" } else { "" }
            );
            let color = if i == self.active_tab {
                Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)
            } else {
                Theme::color(Theme::SIDEBAR, 0xf3f3f3)
            };
            r.draw_rect(
                tab_x,
                0.0,
                label.chars().count() as f32 * cw * 1.8,
                lh * 1.5,
                rgb(color),
            );
            r.draw_text(
                &label,
                tab_x + PAD,
                lh * 0.25,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            tab_x += label.chars().count() as f32 * cw * 1.8;
        }
        let (cur_line, cur_col) = self.tabs[self.active_tab].buffer.cursor_line_col();
        let last = (self.tabs[self.active_tab].scroll_line + visible)
            .min(self.tabs[self.active_tab].buffer.len_lines());
        for line in self.tabs[self.active_tab].scroll_line..last {
            let y = lh * 1.5 + (line - self.tabs[self.active_tab].scroll_line) as f32 * lh;
            if let Some(selection) = self.tabs[self.active_tab].buffer.selection() {
                let line_start = self.tabs[self.active_tab].buffer.line_start_char(line);
                let line_end = line_start
                    + self.tabs[self.active_tab]
                        .buffer
                        .line(line)
                        .trim_end_matches(['\n', '\r'])
                        .chars()
                        .count();
                let sel_start = selection.anchor.min(selection.head);
                let sel_end = selection.anchor.max(selection.head);
                let a = sel_start.max(line_start);
                let b = sel_end.min(line_end);
                if a < b {
                    r.draw_rect(
                        text_x + (a - line_start) as f32 * cw,
                        y,
                        (b - a) as f32 * cw,
                        lh,
                        rgb(0x264f78),
                    );
                }
            }
            let color = if line == cur_line {
                Theme::color(Theme::TEXT, 0x1e1e1e)
            } else {
                Theme::color(Theme::DIM, 0x616161)
            };
            let num = format!(
                "{:>width$}",
                line + 1,
                width = ((gutter - PAD * 2.0) / cw) as usize
            );
            if self.tabs[self.active_tab]
                .diagnostics
                .iter()
                .any(|d| d.range.start.line as usize == line)
            {
                r.draw_rect(SIDEBAR_WIDTH + 2.0, y + lh * 0.3, 5.0, 5.0, rgb(0xf14c4c));
            }
            r.draw_text(&num, SIDEBAR_WIDTH + PAD, y, rgb(color));
            draw_code_line(
                r,
                &self.tabs[self.active_tab].buffer.line(line),
                spans_for(&self.tabs[self.active_tab].highlights, line),
                text_x,
                y,
                cw,
                self.tab_size,
            );
        }
        if (self.tabs[self.active_tab].scroll_line..last).contains(&cur_line) {
            let prefix: String = self.tabs[self.active_tab]
                .buffer
                .line(cur_line)
                .chars()
                .take(cur_col)
                .collect();
            let col = display_line(&prefix, self.tab_size).chars().count();
            let y = lh * 1.5 + (cur_line - self.tabs[self.active_tab].scroll_line) as f32 * lh;
            r.draw_rect(
                text_x + col as f32 * cw,
                y,
                2.0,
                lh,
                rgb(Theme::color(Theme::CURSOR, 0x202020)),
            );
        }

        // Status bar
        let dirty = if self.tabs[self.active_tab].buffer.is_dirty() {
            " ●"
        } else {
            ""
        };
        let status = format!(
            " {}{}   Ln {}, Col {}",
            self.status,
            dirty,
            cur_line + 1,
            cur_col + 1
        );
        r.draw_rect(
            0.0,
            h - lh,
            w,
            lh,
            rgb(Theme::color(Theme::STATUS, 0x005a9e)),
        );
        r.draw_text(&status, 0.0, h - lh, rgb(0xffffff));
        if let Some(query) = &self.search_query {
            let label = format!(
                "Search: {query}  ({} matches; Up/Down, Enter, Esc)",
                self.search_results.len()
            );
            let height = lh * (self.search_results.len().min(6) as f32 + 1.2);
            r.draw_rect(
                SIDEBAR_WIDTH,
                h - lh - height,
                w - SIDEBAR_WIDTH,
                height,
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            r.draw_text(
                &label,
                SIDEBAR_WIDTH + PAD,
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
                        SIDEBAR_WIDTH,
                        y,
                        w - SIDEBAR_WIDTH,
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
                    SIDEBAR_WIDTH + PAD,
                    y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if let Some(query) = &self.snippet_query {
            let matches: Vec<_> = self
                .snippets
                .iter()
                .filter(|s| s.prefix.starts_with(query))
                .take(6)
                .collect();
            let y = h - lh * (matches.len() as f32 + 1.0);
            r.draw_rect(
                SIDEBAR_WIDTH,
                y,
                w - SIDEBAR_WIDTH,
                lh * (matches.len() as f32 + 1.0),
                rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
            );
            r.draw_text(
                &format!("Snippet prefix: {query}"),
                SIDEBAR_WIDTH + PAD,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            for (i, s) in matches.iter().enumerate() {
                r.draw_text(
                    &s.prefix,
                    SIDEBAR_WIDTH + PAD,
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
            let y = (lh
                * (cur_line.saturating_sub(self.tabs[self.active_tab].scroll_line) as f32 + 1.5)
                + 2.0)
                .min(h - box_height - lh);
            let x = (text_x + cur_col as f32 * cw).min(w - 300.0);
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
            let y = (lh * 1.6).min(h - box_h - lh);
            r.draw_rect(
                text_x,
                y,
                (w - text_x - PAD).max(120.0),
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
        if self.terminal_visible {
            self.draw_terminal_panel(r, w, h, lh, cw);
        }

        r.set_clear_color(rgb(Theme::color(Theme::BG, 0xffffff)));
        r.present();
        self.gpu = Some(gpu);
    }
}
