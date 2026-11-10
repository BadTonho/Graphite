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
                SIDEBAR_WIDTH,
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
