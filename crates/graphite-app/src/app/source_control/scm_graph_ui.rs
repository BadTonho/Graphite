//! Rendering and interaction for the Source Control Graph section.

use super::scm_icons::{draw_scm_icon, ScmIcon};
use super::scm_ui::{ScmListRow, ScmRect};
use super::*;
use crate::vcs::scm_graph::{self, GraphEdge, GraphRef};

const LANE_PALETTE: [u32; 6] = [
    0x3794ff, // blue
    0xc586c0, // purple
    0x4ec9b0, // teal
    0xdcdcaa, // yellow
    0xce9178, // orange
    0x89d185, // green
];

fn lane_color(color_index: usize) -> Color {
    rgb(LANE_PALETTE[color_index % LANE_PALETTE.len()])
}

/// Returns `[fetch, pull, push, refresh]` rects placed right-to-left on the Graph header row.
pub(crate) fn graph_header_buttons(right_action: ScmRect, cell_width: f32) -> [ScmRect; 4] {
    let gap = (cell_width * 0.45).max(4.0);
    let refresh = right_action;
    let push = ScmRect {
        x: refresh.x - gap - refresh.width,
        ..refresh
    };
    let pull = ScmRect {
        x: push.x - gap - push.width,
        ..push
    };
    let fetch = ScmRect {
        x: pull.x - gap - pull.width,
        ..pull
    };
    [fetch, pull, push, refresh]
}

/// How many more commits are asked for each time the end of the Graph is reached.
pub(crate) const GRAPH_PAGE: usize = 100;

/// Whether scrolling to `scroll` is near the end of the `loaded` commits while git may still have
/// more (the last request was filled to its `limit`).
pub(crate) fn graph_needs_more(loaded: usize, limit: usize, scroll: usize) -> bool {
    loaded >= limit && scroll + 12 >= loaded
}

/// The horizontal extent of a disc of `radius`, one `(offset from the center row, half width)` per
/// pixel row. Used to draw circles with rectangles.
pub(crate) fn disc_rows(radius: f32) -> Vec<(f32, f32)> {
    let rows = radius.ceil().max(1.0) as i32;
    (-rows..=rows)
        .filter_map(|row| {
            let dy = row as f32;
            let squared = radius * radius - dy * dy;
            (squared >= 0.0).then(|| (dy, squared.sqrt()))
        })
        .collect()
}

fn draw_disc(renderer: &mut Renderer, cx: f32, cy: f32, radius: f32, color: Color) {
    for (dy, half) in disc_rows(radius) {
        renderer.draw_rect(cx - half, cy + dy - 0.5, half * 2.0, 1.0, color);
    }
}

fn draw_stepped_edge(renderer: &mut Renderer, x0: f32, y0: f32, x1: f32, y1: f32, color: Color) {
    let thickness = 1.6;
    if (x0 - x1).abs() < 0.5 {
        renderer.draw_rect(
            x0 - thickness * 0.5,
            y0.min(y1),
            thickness,
            (y1 - y0).abs(),
            color,
        );
        return;
    }
    let mid_y = (y0 + y1) * 0.5;
    renderer.draw_rect(
        x0 - thickness * 0.5,
        y0.min(mid_y),
        thickness,
        (mid_y - y0).abs() + thickness * 0.5,
        color,
    );
    renderer.draw_rect(
        x0.min(x1),
        mid_y - thickness * 0.5,
        (x1 - x0).abs() + thickness * 0.5,
        thickness,
        color,
    );
    renderer.draw_rect(
        x1 - thickness * 0.5,
        mid_y.min(y1),
        thickness,
        (y1 - mid_y).abs(),
        color,
    );
}

fn compact_ref_label(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars || max_chars <= 3 {
        label.to_owned()
    } else {
        let mut s: String = label.chars().take(max_chars - 2).collect();
        s.push_str("..");
        s
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ref_badge(
    renderer: &mut Renderer,
    label: &str,
    right_x: f32,
    row_top: f32,
    row_h: f32,
    cell_width: f32,
    bg_hex: u32,
    fg_hex: u32,
) -> f32 {
    let lh = renderer.line_height();
    let text_w = label.chars().count() as f32 * cell_width;
    let pad_x = 5.0;
    let pill_w = text_w + pad_x * 2.0;
    let pill_h = (row_h * 0.72).max(15.0);
    let pill_x = right_x - pill_w;
    let pill_y = row_top + (row_h - pill_h) * 0.5;
    let text_y = pill_y + (pill_h - lh) * 0.5;
    renderer.draw_rect(pill_x, pill_y, pill_w, pill_h, rgb(bg_hex));
    renderer.draw_text(label, pill_x + pad_x, text_y, rgb(fg_hex));
    pill_x - 4.0
}

impl App {
    pub(crate) fn scm_graph_visible_entries(&self) -> Vec<super::scm_ui::ScmListRowKind> {
        let mut entries = Vec::with_capacity(self.scm.graph_rows.len());
        for (commit_index, row) in self.scm.graph_rows.iter().enumerate() {
            entries.push(super::scm_ui::ScmListRowKind::GraphCommit {
                index: commit_index,
            });
            if self.scm.graph_expanded.contains(&row.commit.hash) {
                if let Some(files) = self.scm.graph_commit_files.get(&row.commit.hash) {
                    for file_index in 0..files.len() {
                        entries.push(super::scm_ui::ScmListRowKind::GraphCommitFile {
                            commit_index,
                            file_index,
                        });
                    }
                }
            }
        }
        entries
    }

    pub(crate) fn scm_sidebar_layout_for(
        &self,
        window_height: f32,
        line_height: f32,
        cell_width: f32,
    ) -> super::scm_ui::ScmPanelLayout {
        let row_h = (line_height + 4.0).max(22.0);
        let sidebar_width = self.sidebar_layout_width_px();
        let visible_height =
            (window_height - self.activity_bar_vertical_offset() - line_height).max(0.0);
        let graph_entries = self.scm_graph_visible_entries();
        super::scm_ui::scm_panel_layout_with_graph_entries(
            &self.scm.snapshot,
            sidebar_width,
            visible_height,
            row_h,
            cell_width,
            self.scm.scroll,
            self.scm.repositories_collapsed,
            self.scm.staged_collapsed,
            self.scm.changes_collapsed,
            &self.vsix_scm_rows(),
            true,
            self.scm.graph_collapsed,
            self.scm.graph_rows.len(),
            self.scm.graph_scroll,
            self.scm.graph_height,
            &super::scm_ui::ScmListView {
                tree: self.scm.view_as_tree,
                collapsed_folders: Some(&self.scm.collapsed_folders),
            },
            Some(&graph_entries),
        )
    }

    pub(crate) fn scm_graph_layout(&self) -> Option<super::scm_ui::ScmPanelLayout> {
        if self.zen_mode || !self.scm.visible {
            return None;
        }
        let gpu = self.gpu.as_ref()?;
        let (_, height) = gpu.renderer.size();
        Some(self.scm_sidebar_layout_for(
            height,
            gpu.renderer.line_height(),
            gpu.renderer.char_width(),
        ))
    }

    pub(crate) fn is_over_scm_graph_resize_handle(&self, mx: f32, my: f32) -> bool {
        if self.scm.graph_collapsed {
            return false;
        }
        let Some(gpu) = &self.gpu else {
            return false;
        };
        let window_width = gpu.renderer.size().0;
        let local_x = self.sidebar_local_x(mx, window_width);
        let local_y = self.sidebar_local_y(my);
        self.scm_graph_layout()
            .and_then(|layout| layout.graph_divider)
            .is_some_and(|divider| divider.contains(local_x, local_y))
    }

    pub(crate) fn resize_scm_graph_from_pointer(&mut self, pointer_y: f32) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let (_, height) = gpu.renderer.size();
        let lh = gpu.renderer.line_height();
        let row_h = lh.max(18.0);
        let visible_height = (height - self.activity_bar_vertical_offset() - lh).max(0.0);
        let list_end_y = visible_height - row_h * 0.25;
        let local_y = self.sidebar_local_y(pointer_y);
        self.scm.graph_height = Some((list_end_y - local_y).max(row_h * 2.0));
    }

    /// The Graph commit under a window point, when the Source Control view shows the Graph.
    pub(crate) fn scm_graph_commit_at(
        &self,
        x: f32,
        y: f32,
        window: (f32, f32),
        lh: f32,
        cw: f32,
    ) -> Option<usize> {
        use crate::app::activity_bar::Activity;
        if self.zen_mode
            || !self.view_preferences.primary_sidebar_visible
            || self.active_activity() != Activity::SourceControl
            || self.active_plugin_activity.is_some()
            || self.scm.graph_collapsed
            || self.shows_scm_welcome_view()
            || !self.primary_sidebar_contains(x, y, window.0, window.1, lh)
        {
            return None;
        }
        let local_x = self.sidebar_local_x(x, window.0);
        let local_y = self.sidebar_local_y(y);
        let layout = self.scm_sidebar_layout_for(window.1, lh, cw);
        layout.list_rows.iter().find_map(|row| match row.kind {
            super::scm_ui::ScmListRowKind::GraphCommit { index }
                if row.bounds.contains(local_x, local_y) =>
            {
                Some(index)
            }
            _ => None,
        })
    }

    pub(crate) fn scroll_scm_graph(&mut self, lines: i32) {
        let total = self.scm_graph_visible_entries().len();
        let max_scroll = total.saturating_sub(2);
        self.scm.graph_scroll =
            (self.scm.graph_scroll as i32 + lines).clamp(0, max_scroll as i32) as usize;
        if graph_needs_more(
            self.scm.graph_rows.len(),
            self.scm.graph_limit,
            self.scm.graph_scroll,
        ) {
            self.scm.graph_limit += GRAPH_PAGE;
            self.refresh_scm_graph_async();
        }
    }

    /// Expands or collapses the changed-file list under a commit in the Graph.
    pub(crate) fn toggle_scm_graph_commit(&mut self, index: usize) {
        let Some(hash) = self
            .scm
            .graph_rows
            .get(index)
            .map(|row| row.commit.hash.clone())
        else {
            return;
        };
        self.scm.graph_selected_hash = Some(hash.clone());
        if self.scm.graph_expanded.remove(&hash) {
            return;
        }
        if !self.scm.graph_commit_files.contains_key(&hash) {
            let root = self.scm_workspace_root();
            match scm_graph::load_commit_files(&root, &hash) {
                Ok(files) => {
                    self.scm.graph_commit_files.insert(hash.clone(), files);
                }
                Err(error) => {
                    self.status = error;
                    return;
                }
            }
        }
        self.scm.graph_expanded.insert(hash);
    }

    /// Opens a side-by-side DiffView comparing a commit's changed file against its parent.
    pub(crate) fn open_scm_graph_file_diff(&mut self, commit_index: usize, file_index: usize) {
        let Some(commit) = self
            .scm
            .graph_rows
            .get(commit_index)
            .map(|r| r.commit.clone())
        else {
            return;
        };
        let Some(file) = self
            .scm
            .graph_commit_files
            .get(&commit.hash)
            .and_then(|files| files.get(file_index))
            .cloned()
        else {
            return;
        };
        self.scm.graph_selected_hash = Some(commit.hash.clone());
        self.scm.graph_selected_file = Some((commit.hash.clone(), file.path.clone()));

        let root = self.scm_workspace_root();
        let file_name = std::path::Path::new(&file.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&file.path)
            .to_owned();
        let title = format!("{file_name} ({})", commit.short_hash);
        let left_name = format!("{}^ ({})", commit.short_hash, file.path);
        let right_name = format!("{} ({})", commit.short_hash, file.path);

        let Some(proxy) = self.event_proxy.clone() else {
            match scm_graph::load_commit_file_comparison(&root, &commit.hash, &file.path) {
                Ok((left, right)) => {
                    self.diff_view = Some(super::scm_ui::DiffView {
                        title,
                        left_name,
                        right_name,
                        left,
                        right,
                        inline: false,
                        collapse_unchanged: false,
                        selected_change: 0,
                    });
                }
                Err(error) => self.status = error,
            }
            return;
        };

        self.diff_loading = true;
        std::thread::Builder::new()
            .name("graphite-commit-file-diff".into())
            .spawn(move || {
                match scm_graph::load_commit_file_comparison(&root, &commit.hash, &file.path) {
                    Ok((left, right)) => {
                        let _ = proxy.send_event(UserEvent::DiffReady(super::scm_ui::DiffView {
                            title,
                            left_name,
                            right_name,
                            left,
                            right,
                            inline: false,
                            collapse_unchanged: false,
                            selected_change: 0,
                        }));
                    }
                    Err(message) => {
                        let _ = proxy.send_event(UserEvent::ScmOperationCompleted {
                            message,
                            snapshot: None,
                        });
                    }
                }
            })
            .ok();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_scm_graph_header(
        &self,
        renderer: &mut Renderer,
        row: &ScmListRow,
        collapsed: bool,
        row_h: f32,
        cell_width: f32,
        text: Color,
        dim: Color,
    ) {
        let y = row.bounds.y;
        let chevron = if collapsed {
            crate::file_icons::CHEVRON_RIGHT
        } else {
            crate::file_icons::CHEVRON_DOWN
        };
        let icon_size = (row_h * 0.65).round();
        renderer.draw_icon(
            chevron,
            row.chevron.x + (row.chevron.width - icon_size) * 0.5,
            y + (row.chevron.height - icon_size) * 0.5,
            icon_size,
            dim,
        );
        renderer.draw_text(
            self.t("scm.graph.title"),
            row.chevron.x + row.chevron.width,
            y,
            text,
        );

        let [fetch, pull, push, refresh] = graph_header_buttons(row.action, cell_width);
        let mx = self.gpu.as_ref().map_or(self.mouse.x as f32, |gpu| {
            self.sidebar_local_x(self.mouse.x as f32, gpu.renderer.size().0)
        });
        let my = self.sidebar_local_y(self.mouse.y as f32);
        let hover_bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        let buttons = [
            (fetch, ScmIcon::Fetch),
            (pull, ScmIcon::Pull),
            (push, ScmIcon::Push),
            (refresh, ScmIcon::Refresh),
        ];
        for (rect, icon) in buttons {
            let hovered = rect.contains(mx, my);
            if hovered {
                renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, hover_bg);
            }
            let fg = if hovered {
                rgb(Theme::color(Theme::TEXT, 0xffffff))
            } else {
                dim
            };
            draw_scm_icon(renderer, icon, rect, fg);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_scm_graph_commit_row(
        &self,
        renderer: &mut Renderer,
        row: &ScmListRow,
        index: usize,
        row_h: f32,
        cell_width: f32,
        sidebar_width: f32,
        text: Color,
        dim: Color,
    ) {
        let Some(graph_row) = self.scm.graph_rows.get(index) else {
            return;
        };
        let y = row.bounds.y;
        let row_top = y - row_h * 0.1;
        let row_bottom = row_top + row_h;
        let cy = row_top + row_h * 0.5;
        let selected = self
            .scm
            .graph_selected_hash
            .as_deref()
            .is_some_and(|h| h == graph_row.commit.hash);
        if selected {
            renderer.draw_rect(
                ACTIVITY_BAR_WIDTH,
                row_top,
                sidebar_width - ACTIVITY_BAR_WIDTH,
                row_h,
                rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
            );
        }

        let base_x = ACTIVITY_BAR_WIDTH + PAD + 8.0;
        let lane_step = 11.0;
        let lane_x = |col: usize| base_x + col as f32 * lane_step;

        let mut draw_edges = |edges: &[GraphEdge], y_start: f32, y_end: f32| {
            for edge in edges {
                draw_stepped_edge(
                    renderer,
                    lane_x(edge.from_col),
                    y_start,
                    lane_x(edge.to_col),
                    y_end,
                    lane_color(edge.color_index),
                );
            }
        };
        draw_edges(&graph_row.upper_edges, row_top, cy);
        draw_edges(&graph_row.lower_edges, cy, row_bottom);

        let node_x = lane_x(graph_row.lane);
        let node_color = lane_color(graph_row.color_index);
        if graph_row.commit.is_head {
            // The checked out commit is a ring around the color of the background.
            draw_disc(renderer, node_x, cy, 5.0, node_color);
            draw_disc(
                renderer,
                node_x,
                cy,
                2.4,
                rgb(Theme::color(Theme::SIDEBAR, 0x252526)),
            );
        } else {
            draw_disc(renderer, node_x, cy, 3.4, node_color);
        }

        let text_x = lane_x(graph_row.max_lanes.max(1)) + 8.0;
        let min_subject_x = text_x + cell_width * 14.0;
        let mut right_x = sidebar_width - PAD;
        let total_refs = graph_row.commit.refs.len();
        let mut drawn_refs = 0usize;
        for reference in graph_row.commit.refs.iter().take(2).rev() {
            let (raw_label, bg_hex, fg_hex) = match reference {
                GraphRef::HeadBranch(name) => (name.as_str(), 0x007acc, 0xffffff),
                GraphRef::LocalBranch(name) => (name.as_str(), 0x3a3d41, 0xcccccc),
                GraphRef::RemoteBranch(name) => (name.as_str(), 0x6b3fa0, 0xffffff),
                GraphRef::Tag(name) => (name.as_str(), 0x4b4b2a, 0xe5e5a0),
            };
            let label = compact_ref_label(raw_label, 12);
            let pill_w = label.chars().count() as f32 * cell_width + 10.0;
            if right_x - pill_w - 6.0 > min_subject_x {
                right_x = draw_ref_badge(
                    renderer, &label, right_x, row_top, row_h, cell_width, bg_hex, fg_hex,
                );
                drawn_refs += 1;
            }
        }
        if total_refs > drawn_refs && drawn_refs > 0 {
            let extra = format!("+{}", total_refs - drawn_refs);
            let extra_w = extra.chars().count() as f32 * cell_width + 10.0;
            if right_x - extra_w - 6.0 > min_subject_x {
                right_x = draw_ref_badge(
                    renderer, &extra, right_x, row_top, row_h, cell_width, 0x3a3d41, 0xcccccc,
                );
            }
        }

        let max_chars = ((right_x - 6.0 - text_x) / cell_width).floor().max(0.0) as usize;
        let subject = &graph_row.commit.subject;
        let truncated: String = if subject.chars().count() > max_chars && max_chars > 3 {
            let mut s: String = subject.chars().take(max_chars - 3).collect();
            s.push_str("...");
            s
        } else {
            subject.chars().take(max_chars).collect()
        };
        let fg = if graph_row.commit.is_head {
            rgb(Theme::color(Theme::TEXT, 0xffffff))
        } else {
            let _ = dim;
            text
        };
        let text_y = row_top + (row_h - renderer.line_height()) * 0.5;
        renderer.draw_text(&truncated, text_x, text_y, fg);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_scm_graph_commit_file_row(
        &self,
        renderer: &mut Renderer,
        row: &super::scm_ui::ScmListRow,
        commit_index: usize,
        file_index: usize,
        row_h: f32,
        cell_width: f32,
        sidebar_width: f32,
        text: Color,
        dim: Color,
    ) {
        let Some(graph_row) = self.scm.graph_rows.get(commit_index) else {
            return;
        };
        let Some(file) = self
            .scm
            .graph_commit_files
            .get(&graph_row.commit.hash)
            .and_then(|files| files.get(file_index))
        else {
            return;
        };
        let y = row.bounds.y;
        let row_top = y - row_h * 0.1;
        let row_bottom = row_top + row_h;
        let selected = self
            .scm
            .graph_selected_file
            .as_ref()
            .is_some_and(|(h, p)| *h == graph_row.commit.hash && *p == file.path);
        if selected {
            renderer.draw_rect(
                ACTIVITY_BAR_WIDTH,
                row_top,
                sidebar_width - ACTIVITY_BAR_WIDTH,
                row_h,
                rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
            );
        }

        // Continue the commit's outgoing lane lines straight down through its file rows.
        let base_x = ACTIVITY_BAR_WIDTH + PAD + 8.0;
        let lane_step = 11.0;
        let lane_x = |col: usize| base_x + col as f32 * lane_step;
        for edge in &graph_row.lower_edges {
            let x = lane_x(edge.to_col);
            renderer.draw_rect(
                x - 1.0,
                row_top,
                2.0,
                row_bottom - row_top,
                lane_color(edge.color_index),
            );
        }

        let content_x = lane_x(graph_row.max_lanes.max(1)) + 8.0;
        let icon = crate::file_icons::for_path(std::path::Path::new(&file.path));
        let icon_size = (row_h * 0.65).round();
        let icon_y = row_top + (row_h - icon_size) * 0.5;
        renderer.draw_icon(icon.glyph, content_x, icon_y, icon_size, rgb(icon.color));

        let status_color = match file.status {
            'A' => rgb(0x73c991),
            'D' => rgb(0xf48771),
            'R' | 'C' => rgb(0x4ea1ff),
            _ => rgb(0xe2c08d),
        };
        let status_x = sidebar_width - PAD - cell_width;
        let text_y = row_top + (row_h - renderer.line_height()) * 0.5;
        let mut status_buf = [0u8; 4];
        let status_str = file.status.encode_utf8(&mut status_buf);
        renderer.draw_text(status_str, status_x, text_y, status_color);

        let name_x = content_x + icon_size + 5.0;
        let avail_w = (status_x - 6.0 - name_x).max(0.0);
        let p = std::path::Path::new(&file.path);
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or(&file.path);
        let parent = p
            .parent()
            .and_then(|d| d.to_str())
            .filter(|d| !d.is_empty());

        let name_clipped = Self::help_menu_visible_label(name, cell_width, avail_w);
        renderer.draw_text(&name_clipped, name_x, text_y, text);
        let name_w = name_clipped.chars().count() as f32 * cell_width;
        if let Some(dir) = parent {
            let dir_x = name_x + name_w + cell_width;
            let dir_avail = (status_x - 6.0 - dir_x).max(0.0);
            if dir_avail > cell_width * 3.0 {
                let dir_clipped = Self::help_menu_visible_label(dir, cell_width, dir_avail);
                renderer.draw_text(&dir_clipped, dir_x, text_y, dim);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::source_control::scm_ui::{
        scm_panel_layout_with_graph, ScmListRowKind, ScmListView,
    };

    #[test]
    fn discs_are_round_and_symmetric() {
        let rows = disc_rows(3.4);
        let widest = rows.iter().map(|(_, half)| *half).fold(0.0_f32, f32::max);
        assert!((widest - 3.4).abs() < 1e-3, "widest row is the middle one");
        assert_eq!(rows.first().map(|r| r.0), Some(-3.0));
        assert_eq!(rows.last().map(|r| r.0), Some(3.0));
        for (dy, half) in &rows {
            let mirrored = rows.iter().find(|(other, _)| *other == -dy).unwrap();
            assert!((mirrored.1 - half).abs() < 1e-4);
        }
        // A corner of the bounding square is outside the disc.
        assert!(rows
            .iter()
            .all(|(dy, half)| dy * dy + half * half <= 3.4 * 3.4 + 1e-3));
    }

    #[test]
    fn no_commit_is_under_the_pointer_when_the_graph_is_not_shown() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert_eq!(
            app.scm_graph_commit_at(60.0, 400.0, (1200.0, 760.0), 18.0, 8.0),
            None
        );
        app.scm.graph_collapsed = true;
        assert_eq!(
            app.scm_graph_commit_at(60.0, 400.0, (1200.0, 760.0), 18.0, 8.0),
            None
        );
    }

    #[test]
    fn more_commits_are_requested_only_near_the_end_of_a_full_page() {
        assert!(graph_needs_more(100, 100, 90));
        assert!(!graph_needs_more(100, 100, 10), "far from the end");
        assert!(
            !graph_needs_more(40, 100, 35),
            "git has no more: the page was not full"
        );
        assert!(graph_needs_more(200, 200, 188));
    }

    #[test]
    fn graph_header_buttons_are_ordered_fetch_pull_push_refresh_without_overlap() {
        let right = ScmRect {
            x: 240.0,
            y: 120.0,
            width: 18.0,
            height: 22.0,
        };
        let [fetch, pull, push, refresh] = graph_header_buttons(right, 8.0);
        assert!(fetch.x + fetch.width <= pull.x);
        assert!(pull.x + pull.width <= push.x);
        assert!(push.x + push.width <= refresh.x);
    }

    #[test]
    fn scm_panel_layout_includes_graph_header_and_commit_rows_when_expanded() {
        let snapshot = crate::scm::Snapshot::default();
        let expanded = scm_panel_layout_with_graph(
            &snapshot,
            280.0,
            600.0,
            22.0,
            8.0,
            0,
            false,
            false,
            false,
            &[],
            true,
            false,
            3,
            0,
            None,
            &ScmListView::LIST,
        );
        assert!(expanded.graph_divider.is_some());
        assert!(expanded.list_rows.iter().any(|r| matches!(
            r.kind,
            ScmListRowKind::GraphHeader {
                collapsed: false,
                count: 3
            }
        )));
        assert_eq!(
            expanded
                .list_rows
                .iter()
                .filter(|r| matches!(r.kind, ScmListRowKind::GraphCommit { .. }))
                .count(),
            3
        );

        let collapsed = scm_panel_layout_with_graph(
            &snapshot,
            280.0,
            600.0,
            22.0,
            8.0,
            0,
            false,
            false,
            false,
            &[],
            true,
            true,
            3,
            0,
            None,
            &ScmListView::LIST,
        );
        assert_eq!(
            collapsed
                .list_rows
                .iter()
                .filter(|r| matches!(r.kind, ScmListRowKind::GraphCommit { .. }))
                .count(),
            0
        );
    }

    #[test]
    fn clicking_commit_expands_files_and_clicking_file_opens_diff_view() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success());
        };
        git(&["init"]);
        git(&["config", "user.name", "Test"]);
        git(&["config", "user.email", "test@example.com"]);
        std::fs::write(root.join("note.txt"), "before\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "first"]);
        std::fs::write(root.join("note.txt"), "after\n").unwrap();
        git(&["commit", "-am", "second"]);

        let mut app = App::new_with_options(root.to_path_buf(), None, true);
        app.scm.graph_rows = scm_graph::load_graph_commits(root, 10).unwrap();
        assert_eq!(app.scm_graph_visible_entries().len(), 2);

        app.toggle_scm_graph_commit(0);
        let expanded = app.scm_graph_visible_entries();
        assert_eq!(
            expanded,
            vec![
                ScmListRowKind::GraphCommit { index: 0 },
                ScmListRowKind::GraphCommitFile {
                    commit_index: 0,
                    file_index: 0,
                },
                ScmListRowKind::GraphCommit { index: 1 },
            ]
        );

        app.open_scm_graph_file_diff(0, 0);
        let diff = app.diff_view.as_ref().expect("diff view should open");
        assert_eq!(diff.left.replace("\r\n", "\n"), "before\n");
        assert_eq!(diff.right.replace("\r\n", "\n"), "after\n");

        app.toggle_scm_graph_commit(0);
        assert_eq!(app.scm_graph_visible_entries().len(), 2);
    }
}
