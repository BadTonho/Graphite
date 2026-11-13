//! Vertical layout of the cells of a notebook, shared by drawing, scrolling and hit testing.

use super::model::CellKind;
use super::session::{CellState, Session};

/// Space after each cell, in lines.
const GAP: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CellLayout {
    /// Distance from the top of the notebook.
    pub top: f32,
    pub height: f32,
    /// Lines the input takes (1 when collapsed).
    pub input_lines: usize,
    /// Lines the outputs take (0 when there are none or they are collapsed).
    pub output_lines: usize,
}

impl CellLayout {
    /// Where the input starts, below the header line.
    pub(crate) fn input_top(&self, line_height: f32) -> f32 {
        self.top + line_height
    }

    pub(crate) fn output_top(&self, line_height: f32) -> f32 {
        self.top + line_height * (1.0 + self.input_lines as f32)
    }

    pub(crate) fn bottom(&self) -> f32 {
        self.top + self.height
    }
}

pub(crate) fn visible_output_lines(state: &CellState, line_height: f32) -> usize {
    if state.cell.kind != CellKind::Code {
        return 0;
    }
    let blocks = state.blocks();
    let rows: usize = blocks.iter().map(|block| block.rows(line_height)).sum();
    match rows {
        0 => 0,
        // A collapsed output leaves one line saying so.
        _ if state.output_collapsed => 1,
        rows => rows,
    }
}

/// Positions of all cells for a given line height.
pub(crate) fn layout(session: &Session, line_height: f32) -> Vec<CellLayout> {
    let mut top = 0.0;
    let hidden = session.hidden_cells();
    session
        .cells
        .iter()
        .enumerate()
        .map(|(index, state)| {
            if hidden[index] {
                return CellLayout {
                    top,
                    height: 0.0,
                    input_lines: 0,
                    output_lines: 0,
                };
            }
            let input_lines = if state.input_collapsed {
                1
            } else {
                state.buffer.len_lines().max(1)
            };
            let output_lines = visible_output_lines(state, line_height);
            let height = line_height * (1.0 + input_lines as f32 + output_lines as f32 + GAP);
            let cell = CellLayout {
                top,
                height,
                input_lines,
                output_lines,
            };
            top += height;
            cell
        })
        .collect()
}

/// The cell at `y` (measured from the top of the notebook).
pub(crate) fn cell_at(layouts: &[CellLayout], y: f32) -> Option<usize> {
    layouts
        .iter()
        .position(|cell| y >= cell.top && y < cell.bottom())
}

/// The scroll offset that brings `target` fully into a view of `viewport` pixels, changing
/// `scroll` as little as possible. A cell taller than the view is shown from its top.
pub(crate) fn scroll_to_show(
    layouts: &[CellLayout],
    target: usize,
    scroll: f32,
    viewport: f32,
) -> f32 {
    let Some(cell) = layouts.get(target) else {
        return scroll;
    };
    if cell.top < scroll || cell.height > viewport {
        cell.top
    } else if cell.bottom() > scroll + viewport {
        cell.bottom() - viewport
    } else {
        scroll
    }
}

/// Keeps `scroll` inside the notebook.
pub(crate) fn clamp_scroll(layouts: &[CellLayout], scroll: f32, viewport: f32) -> f32 {
    let total = layouts.last().map_or(0.0, CellLayout::bottom);
    scroll.clamp(0.0, (total - viewport).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::super::model::{Cell, Notebook};
    use super::*;
    use serde_json::json;

    fn session(cells: Vec<Cell>) -> Session {
        let mut notebook = Notebook::new();
        notebook.cells = cells;
        Session::from_notebook(notebook)
    }

    #[test]
    fn a_cell_is_a_header_its_input_its_outputs_and_a_gap() {
        let mut with_output = Cell::new(CellKind::Code, "a\nb");
        with_output.outputs = vec![json!({"output_type": "stream", "name": "stdout", "text": "x\ny\nz\n"})];
        let session = session(vec![with_output, Cell::new(CellKind::Markdown, "")]);
        let layouts = layout(&session, 10.0);
        assert_eq!(layouts[0].input_lines, 2);
        assert_eq!(layouts[0].output_lines, 3);
        assert_eq!(layouts[0].top, 0.0);
        assert_eq!(layouts[0].height, 10.0 * (1.0 + 2.0 + 3.0 + 0.6));
        assert_eq!(layouts[1].top, layouts[0].bottom());
        // An empty cell still has one input line.
        assert_eq!(layouts[1].input_lines, 1);
        assert_eq!(layouts[0].input_top(10.0), 10.0);
        assert_eq!(layouts[0].output_top(10.0), 30.0);
    }

    #[test]
    fn collapsed_inputs_and_outputs_shrink_the_cell() {
        let mut cell = Cell::new(CellKind::Code, "a\nb\nc");
        cell.outputs = vec![json!({"output_type": "stream", "name": "stdout", "text": "x\n"})];
        let mut session = session(vec![cell]);
        let full = layout(&session, 10.0)[0].height;
        session.toggle_input_collapsed();
        assert_eq!(layout(&session, 10.0)[0].input_lines, 1);
        session.toggle_output_collapsed();
        let collapsed = layout(&session, 10.0)[0];
        assert_eq!(collapsed.output_lines, 1);
        assert!(collapsed.height < full);
    }


    #[test]
    fn an_image_output_takes_the_rows_of_its_height() {
        use base64::Engine;
        let image = image::RgbaImage::from_pixel(40, 65, image::Rgba([1, 2, 3, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        let mut cell = Cell::new(CellKind::Code, "plot()");
        cell.outputs = vec![
            json!({"output_type": "stream", "name": "stdout", "text": "a\nb\n"}),
            json!({"output_type": "display_data", "metadata": {},
                   "data": {"image/png": base64::engine::general_purpose::STANDARD.encode(bytes)}}),
        ];
        let session = session(vec![cell]);
        let layouts = layout(&session, 10.0);
        // Two text lines and an image of 65 pixels, which is seven 10-pixel rows.
        assert_eq!(layouts[0].output_lines, 2 + 7);
    }

    #[test]
    fn the_unused_first_cell_of_an_interactive_window_takes_no_room() {
        use super::super::interactive::InteractiveKind;
        let mut session = Session::new_interactive(InteractiveKind::Window);
        let layouts = layout(&session, 10.0);
        assert_eq!(layouts[0].height, 0.0);
        assert_eq!(cell_at(&layouts, 0.0), None);
        session.edit_input(|editor| editor.insert("1"));
        session.submit_input();
        assert!(layout(&session, 10.0)[0].height > 0.0);
    }
    #[test]
    fn text_cells_never_show_outputs() {
        let mut cell = Cell::new(CellKind::Markdown, "# t");
        cell.outputs = vec![json!({"output_type": "stream", "name": "stdout", "text": "x\n"})];
        let session = session(vec![cell]);
        assert_eq!(layout(&session, 10.0)[0].output_lines, 0);
    }

    #[test]
    fn hit_testing_finds_the_cell_at_a_height() {
        let session = session(vec![Cell::new(CellKind::Code, "a"), Cell::new(CellKind::Code, "b")]);
        let layouts = layout(&session, 10.0);
        assert_eq!(cell_at(&layouts, 0.0), Some(0));
        assert_eq!(cell_at(&layouts, layouts[0].bottom() - 0.1), Some(0));
        assert_eq!(cell_at(&layouts, layouts[0].bottom()), Some(1));
        assert_eq!(cell_at(&layouts, 10_000.0), None);
        assert_eq!(cell_at(&layouts, -1.0), None);
    }

    #[test]
    fn scrolling_shows_the_target_with_the_least_movement() {
        let session = session((0..10).map(|_| Cell::new(CellKind::Code, "x")).collect());
        let layouts = layout(&session, 10.0);
        let height = layouts[0].height;
        let viewport = height * 3.0;
        // Already visible: unchanged.
        assert_eq!(scroll_to_show(&layouts, 1, 0.0, viewport), 0.0);
        // Below the view: its bottom lines up with the bottom of the view.
        assert_eq!(scroll_to_show(&layouts, 5, 0.0, viewport), layouts[5].bottom() - viewport);
        // Above the view: its top lines up with the top.
        assert_eq!(scroll_to_show(&layouts, 0, 50.0, viewport), 0.0);
        // Taller than the view: from its top.
        assert_eq!(scroll_to_show(&layouts, 2, 0.0, height * 0.5), layouts[2].top);
        assert_eq!(scroll_to_show(&layouts, 99, 7.0, viewport), 7.0);
    }

    #[test]
    fn scroll_is_clamped_to_the_notebook() {
        let session = session(vec![Cell::new(CellKind::Code, "x"), Cell::new(CellKind::Code, "y")]);
        let layouts = layout(&session, 10.0);
        let total = layouts[1].bottom();
        assert_eq!(clamp_scroll(&layouts, -5.0, 20.0), 0.0);
        assert_eq!(clamp_scroll(&layouts, 1000.0, 20.0), total - 20.0);
        assert_eq!(clamp_scroll(&layouts, 1000.0, 10_000.0), 0.0);
        assert_eq!(clamp_scroll(&[], 5.0, 20.0), 0.0);
    }
}
