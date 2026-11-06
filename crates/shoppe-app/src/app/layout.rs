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
        let panel = if self.terminal_visible {
            terminal_panel_height(renderer.size().1)
        } else {
            0.0
        };
        let h = renderer.size().1 - panel - renderer.line_height() * 2.5;
        ((h / renderer.line_height()) as usize).max(1)
    }

    pub(crate) fn keep_cursor_visible(&mut self, visible: usize) {
        let (line, _) = self.tabs[self.active_tab].buffer.cursor_line_col();
        if line < self.tabs[self.active_tab].scroll_line {
            self.tabs[self.active_tab].scroll_line = line;
        } else if line >= self.tabs[self.active_tab].scroll_line + visible {
            self.tabs[self.active_tab].scroll_line = line + 1 - visible;
        }
    }

    pub(crate) fn gutter_width(&self, char_w: f32) -> f32 {
        let digits = self.tabs[self.active_tab]
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
