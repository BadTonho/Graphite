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

fn draw_stepped_edge(
    renderer: &mut Renderer,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: Color,
) {
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

fn draw_ref_badge(
    renderer: &mut Renderer,
    label: &str,
    right_x: f32,
    y: f32,
    row_h: f32,
    cell_width: f32,
    bg_hex: u32,
    fg_hex: u32,
) -> f32 {
    let text_w = label.chars().count() as f32 * cell_width;
    let pad_x = 6.0;
    let pill_w = text_w + pad_x * 2.0;
    let pill_h = (row_h * 0.78).max(14.0);
    let pill_x = right_x - pill_w;
    let pill_y = y + (row_h - pill_h) * 0.5 - row_h * 0.08;
    renderer.draw_rect(pill_x, pill_y, pill_w, pill_h, rgb(bg_hex));
    renderer.draw_text(label, pill_x + pad_x, y, rgb(fg_hex));
    pill_x - 4.0
}

impl App {
    pub(crate) fn scm_sidebar_layout_for(
        &self,
        window_height: f32,
        line_height: f32,
        cell_width: f32,
    ) -> super::scm_ui::ScmPanelLayout {
        let row_h = line_height.max(18.0);
        let sidebar_width = self.sidebar_layout_width_px();
        let visible_height =
            (window_height - self.activity_bar_vertical_offset() - line_height).max(0.0);
        super::scm_ui::scm_panel_layout_with_graph(
            &self.scm_snapshot,
            sidebar_width,
            visible_height,
            row_h,
            cell_width,
            self.scm_scroll,
            self.scm_repositories_collapsed,
            self.scm_staged_collapsed,
            self.scm_changes_collapsed,
            &self.vsix_scm_rows(),
            true,
            self.scm_graph_collapsed,
            self.scm_graph_rows.len(),
            self.scm_graph_scroll,
            self.scm_graph_height,
        )
    }

    pub(crate) fn scm_graph_layout(&self) -> Option<super::scm_ui::ScmPanelLayout> {
        if self.zen_mode || !self.scm_visible {
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
        if self.scm_graph_collapsed {
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
        self.scm_graph_height = Some((list_end_y - local_y).max(row_h * 2.0));
    }

    pub(crate) fn scroll_scm_graph(&mut self, lines: i32) {
        let max_scroll = self.scm_graph_rows.len().saturating_sub(2);
        self.scm_graph_scroll =
            (self.scm_graph_scroll as i32 + lines).clamp(0, max_scroll as i32) as usize;
    }

    pub(crate) fn open_scm_commit_diff(&mut self, hash: &str) {
        let Some(proxy) = self.event_proxy.clone() else {
            let root = self.scm_workspace_root();
            match scm_graph::commit_details(&root, hash) {
                Ok(text) => self.open_read_only_view(PathBuf::from("commit.diff"), &text),
                Err(error) => self.status = error,
            }
            return;
        };
        let root = self.scm_workspace_root();
        let hash = hash.to_owned();
        self.status = self.t("scm.status.running").into();
        std::thread::spawn(move || {
            let result = scm_graph::commit_details(&root, &hash);
            let _ = proxy.send_event(UserEvent::ScmHistory(result));
        });
    }

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
        renderer.draw_text(self.t("scm.graph.title"), row.chevron.x + row.chevron.width, y, text);

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
        let Some(graph_row) = self.scm_graph_rows.get(index) else {
            return;
        };
        let y = row.bounds.y;
        let row_top = y - row_h * 0.1;
        let row_bottom = row_top + row_h;
        let cy = row_top + row_h * 0.5;
        let selected = self
            .scm_graph_selected_hash
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
            let outer = 8.0;
            let inner = 4.0;
            renderer.draw_rect(
                node_x - outer * 0.5,
                cy - outer * 0.5,
                outer,
                outer,
                node_color,
            );
            renderer.draw_rect(
                node_x - inner * 0.5,
                cy - inner * 0.5,
                inner,
                inner,
                rgb(Theme::color(Theme::SIDEBAR, 0x252526)),
            );
        } else {
            let dot = 6.0;
            renderer.draw_rect(node_x - dot * 0.5, cy - dot * 0.5, dot, dot, node_color);
        }

        // Draw ref pills on the right edge (up to 2 badges to keep the message readable).
        let mut right_x = sidebar_width - PAD;
        for reference in graph_row.commit.refs.iter().take(2).rev() {
            let (label, bg_hex, fg_hex) = match reference {
                GraphRef::HeadBranch(name) => (name.as_str(), 0x007acc, 0xffffff),
                GraphRef::LocalBranch(name) => (name.as_str(), 0x3a3d41, 0xcccccc),
                GraphRef::RemoteBranch(name) => (name.as_str(), 0x6b3fa0, 0xffffff),
                GraphRef::Tag(name) => (name.as_str(), 0x4b4b2a, 0xe5e5a0),
            };
            if right_x - (label.chars().count() as f32 * cell_width + 16.0)
                > lane_x(graph_row.max_lanes) + cell_width * 8.0
            {
                right_x = draw_ref_badge(
                    renderer,
                    label,
                    right_x,
                    y,
                    row_h,
                    cell_width,
                    bg_hex,
                    fg_hex,
                );
            }
        }

        let text_x = lane_x(graph_row.max_lanes.max(1)) + 4.0;
        let max_chars = ((right_x - text_x) / cell_width).floor().max(0.0) as usize;
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
        renderer.draw_text(&truncated, text_x, y, fg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::source_control::scm_ui::{scm_panel_layout_with_graph, ScmListRowKind};

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
        );
        assert!(expanded.graph_divider.is_some());
        assert!(expanded
            .list_rows
            .iter()
            .any(|r| matches!(r.kind, ScmListRowKind::GraphHeader { collapsed: false, count: 3 })));
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
}
