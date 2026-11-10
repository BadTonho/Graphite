use super::editor_groups::GroupRect;
use super::*;

pub(crate) fn ruler_positions(
    columns: &[usize],
    text_x: f32,
    char_width: f32,
    width: f32,
) -> Vec<f32> {
    columns
        .iter()
        .map(|column| text_x + *column as f32 * char_width)
        .filter(|x| *x < width)
        .collect()
}

impl App {
    pub(crate) fn sidebar_width_px(&self) -> f32 {
        effective_sidebar_boundary(
            self.sidebar_width,
            self.sidebar_scale_factor,
            self.sidebar_window_width_px,
            if self.outline_visible { 240.0 } else { 0.0 },
        )
    }

    pub(crate) fn resize_sidebar_from_pointer(&mut self, pointer_x: f32) {
        let scale = self.sidebar_scale_factor;
        let raw_width = (pointer_x - ACTIVITY_BAR_WIDTH) / scale;
        let available = max_sidebar_content_width(
            scale,
            self.sidebar_window_width_px,
            if self.outline_visible { 240.0 } else { 0.0 },
        );
        self.sidebar_width = if available >= config::MIN_SIDEBAR_WIDTH {
            config::clamp_sidebar_width(raw_width).min(available)
        } else {
            raw_width.clamp(0.0, available)
        };
    }

    pub(crate) fn visible_lines(&self, renderer: &Renderer) -> usize {
        let (width, height) = renderer.size();
        let rect = self.focused_group_rect(width, height, renderer.line_height());
        (((rect.height - renderer.line_height() * 1.5).max(0.0) / renderer.line_height()) as usize)
            .max(1)
    }

    pub(crate) fn group_rectangles(
        &self,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> Vec<GroupRect> {
        let sidebar_width = self.sidebar_width_px();
        let panel = if self.terminal_visible {
            terminal_panel_height(height)
        } else if self.phase6_panel_visible() {
            line_height * 9.0
        } else {
            0.0
        };
        let (sidebar, right_sidebar) = if self.zen_mode {
            (0.0, 0.0)
        } else {
            (
                sidebar_width,
                if self.outline_visible { 240.0 } else { 0.0 },
            )
        };
        let width = (width - right_sidebar).max(0.0);
        let centered = self.centered_editor || self.zen_mode;
        let editor_width = if centered {
            let cw = self
                .gpu
                .as_ref()
                .map(|gpu| gpu.renderer.char_width())
                .unwrap_or(8.0);
            (cw * 80.0 + self.gutter_width(cw)).min(width)
        } else {
            width - sidebar
        };
        let x = if centered {
            sidebar + (width - editor_width) / 2.0
        } else {
            sidebar
        };
        self.editor_groups.rectangles(GroupRect {
            id: 0,
            x,
            y: 0.0,
            width: editor_width.max(0.0),
            height: (height - panel - if self.zen_mode { 0.0 } else { line_height }).max(0.0),
        })
    }

    pub(super) fn focused_group_rect(
        &self,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> GroupRect {
        self.group_rectangles(width, height, line_height)
            .into_iter()
            .find(|rect| rect.id == self.editor_groups.focused())
            .expect("focused group has a rectangle")
    }

    pub(crate) fn keep_cursor_visible(
        &mut self,
        visible: usize,
        width: f32,
        height: f32,
        lh: f32,
        cw: f32,
    ) {
        let line = if self.word_wrap {
            let rect = self.focused_group_rect(width, height, lh);
            let columns = (((rect.width - self.gutter_width(cw)).max(cw)) / cw) as usize;
            let tab = &self.documents[self.active_document()];
            super::visual_lines::document_position(
                &tab.buffer,
                self.editor().cursor().char_idx,
                columns,
                tab.indentation.tab_size,
                self.wrapping_indent,
            )
            .0
        } else {
            self.editor().cursor_line_col().0
        };
        if line < self.editor_groups.active_view_mut().scroll_line {
            self.editor_groups.active_view_mut().scroll_line = line;
        } else if line >= self.editor_groups.active_view_mut().scroll_line + visible {
            self.editor_groups.active_view_mut().scroll_line = line + 1 - visible;
        }
    }

    pub(crate) fn gutter_width(&self, char_w: f32) -> f32 {
        let digits = self.documents[self.active_document()]
            .buffer
            .len_lines()
            .to_string()
            .len()
            .max(3);
        digits as f32 * char_w + PAD * 2.0
    }
}

fn max_sidebar_content_width(scale: f32, window_width_px: f32, right_sidebar_width_px: f32) -> f32 {
    let window_width = window_width_px / scale;
    let activity_width = ACTIVITY_BAR_WIDTH / scale;
    let maximum_by_percentage = window_width * 0.70 - activity_width;
    let maximum_by_editor = window_width - activity_width - right_sidebar_width_px / scale - 320.0;
    maximum_by_percentage
        .min(maximum_by_editor)
        .clamp(0.0, config::MAX_PERSISTED_SIDEBAR_WIDTH)
}

fn effective_sidebar_boundary(
    preferred_logical: f32,
    scale: f32,
    window_width_px: f32,
    right_sidebar_width_px: f32,
) -> f32 {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let available = max_sidebar_content_width(scale, window_width_px, right_sidebar_width_px);
    let min = if available >= config::MIN_SIDEBAR_WIDTH {
        config::MIN_SIDEBAR_WIDTH
    } else {
        0.0
    };
    ACTIVITY_BAR_WIDTH + preferred_logical.clamp(min, available) * scale
}

#[cfg(test)]
mod sidebar_width_tests {
    use super::*;

    #[test]
    fn sidebar_boundary_preserves_default_bounds_and_editor_space() {
        assert_eq!(effective_sidebar_boundary(260.0, 1.0, 1400.0, 0.0), 308.0);
        assert_eq!(effective_sidebar_boundary(500.0, 1.0, 800.0, 0.0), 480.0);
        assert_eq!(effective_sidebar_boundary(450.0, 2.0, 1600.0, 0.0), 948.0);
        assert_eq!(effective_sidebar_boundary(260.0, 1.0, 350.0, 0.0), 48.0);
    }

    #[test]
    fn sidebar_maximum_is_seventy_percent_and_preserves_editor_space() {
        assert_eq!(max_sidebar_content_width(1.0, 1920.0, 0.0), 1296.0);
        assert_eq!(effective_sidebar_boundary(2000.0, 1.0, 1920.0, 0.0), 1344.0);
        assert_eq!(max_sidebar_content_width(2.0, 1600.0, 0.0), 456.0);
        assert_eq!(max_sidebar_content_width(1.0, 1600.0, 240.0), 992.0);
        assert_eq!(max_sidebar_content_width(1.0, 600.0, 0.0), 232.0);
    }

    #[test]
    fn dragging_sidebar_clamps_to_preference_limits_and_available_editor_space() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.resize_sidebar_from_pointer(100.0);
        assert_eq!(app.sidebar_width, config::MIN_SIDEBAR_WIDTH);
        assert_eq!(app.sidebar_width_px(), ACTIVITY_BAR_WIDTH + 180.0);

        app.resize_sidebar_from_pointer(900.0);
        assert_eq!(app.sidebar_width, 792.0);
        assert_eq!(app.sidebar_width_px(), 840.0);

        app.sidebar_window_width_px = 600.0;
        app.resize_sidebar_from_pointer(498.0);
        assert_eq!(app.sidebar_width, 232.0);
        assert_eq!(app.sidebar_width_px(), 280.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rulers_only_include_columns_inside_editor_width() {
        assert_eq!(
            ruler_positions(&[80, 100, 120], 100.0, 8.0, 950.0),
            [740.0, 900.0]
        );
    }
}
