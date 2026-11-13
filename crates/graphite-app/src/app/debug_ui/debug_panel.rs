//! The debug panel: call stack, variables, watches, exception breakpoints and the console.
//!
//! Rows are built from the [`DebugRuntime`] by pure functions so that drawing, hit testing and
//! scrolling share one description of what is on screen.

use super::editor_groups::GroupRect;
use super::*;
use crate::debugger::{DebugRuntime, DebugState};
use std::ops::Range;

/// Columns of the panel, left to right.
pub(crate) const STACK: usize = 0;
pub(crate) const VARIABLES: usize = 1;
pub(crate) const CONSOLE: usize = 2;
const COLUMN_SHARE: [f32; 3] = [0.30, 0.36, 0.34];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DebugRowAction {
    SelectFrame(usize),
    ToggleVariable(i64),
    RemoveWatch(usize),
    ToggleException(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DebugRowStyle {
    Header,
    Normal,
    Dim,
    Error,
    Selected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DebugRow {
    pub text: String,
    pub style: DebugRowStyle,
    pub action: Option<DebugRowAction>,
    /// When set, the action only fires for clicks on the first `n` characters (a "×" button).
    pub hot_chars: Option<usize>,
}

impl DebugRow {
    fn new(text: impl Into<String>, style: DebugRowStyle) -> Self {
        Self {
            text: text.into(),
            style,
            action: None,
            hot_chars: None,
        }
    }

    fn action(mut self, action: DebugRowAction) -> Self {
        self.action = Some(action);
        self
    }
}

pub(crate) fn debug_columns(runtime: &DebugRuntime) -> [Vec<DebugRow>; 3] {
    [
        stack_rows(runtime),
        variable_rows(runtime),
        console_rows(runtime),
    ]
}

fn stack_rows(runtime: &DebugRuntime) -> Vec<DebugRow> {
    let mut rows = vec![DebugRow::new("CALL STACK", DebugRowStyle::Header)];
    if runtime.frames.is_empty() {
        let text = match runtime.state {
            DebugState::Running => "Running",
            DebugState::Initializing => "Starting…",
            _ => "Not paused",
        };
        rows.push(DebugRow::new(text, DebugRowStyle::Dim));
    }
    for (index, frame) in runtime.frames.iter().enumerate() {
        let selected = index == runtime.selected_frame;
        let place = frame
            .source
            .as_deref()
            .and_then(|path| path.file_name())
            .map(|name| format!("  {}:{}", name.to_string_lossy(), frame.line))
            .unwrap_or_default();
        rows.push(
            DebugRow::new(
                format!(
                    "{} {}{}",
                    if selected { "▶" } else { " " },
                    frame.name,
                    place
                ),
                if selected {
                    DebugRowStyle::Selected
                } else {
                    DebugRowStyle::Normal
                },
            )
            .action(DebugRowAction::SelectFrame(index)),
        );
    }
    rows.push(DebugRow::new(
        "EXCEPTION BREAKPOINTS",
        DebugRowStyle::Header,
    ));
    if runtime.exception_filters.is_empty() {
        rows.push(DebugRow::new(
            "None offered by the adapter",
            DebugRowStyle::Dim,
        ));
    }
    for (index, filter) in runtime.exception_filters.iter().enumerate() {
        rows.push(
            DebugRow::new(
                format!(
                    "[{}] {}",
                    if filter.enabled { "x" } else { " " },
                    filter.label
                ),
                DebugRowStyle::Normal,
            )
            .action(DebugRowAction::ToggleException(index)),
        );
    }
    rows
}

fn variable_rows(runtime: &DebugRuntime) -> Vec<DebugRow> {
    let mut rows = vec![DebugRow::new("VARIABLES", DebugRowStyle::Header)];
    let variables = runtime.variable_rows();
    if variables.is_empty() {
        rows.push(DebugRow::new("No variables", DebugRowStyle::Dim));
    }
    for variable in variables {
        let indent = "  ".repeat(variable.depth);
        let glyph = match (variable.reference > 0, variable.expanded) {
            (true, true) => "▾",
            (true, false) => "▸",
            (false, _) => " ",
        };
        let text = if variable.is_scope {
            format!("{glyph} {}", variable.name)
        } else {
            format!("{indent}{glyph} {} = {}", variable.name, variable.value)
        };
        let row = DebugRow::new(text, DebugRowStyle::Normal);
        rows.push(if variable.reference > 0 {
            row.action(DebugRowAction::ToggleVariable(variable.reference))
        } else {
            row
        });
    }
    rows.push(DebugRow::new("WATCH", DebugRowStyle::Header));
    if runtime.watches.is_empty() {
        rows.push(DebugRow::new(
            "Ctrl+Enter in the input adds a watch",
            DebugRowStyle::Dim,
        ));
    }
    for (index, watch) in runtime.watches.iter().enumerate() {
        let (text, style) = match &watch.result {
            Some(Ok(value)) => (
                format!("× {} = {value}", watch.expression),
                DebugRowStyle::Normal,
            ),
            Some(Err(error)) => (
                format!("× {}: {error}", watch.expression),
                DebugRowStyle::Error,
            ),
            None => (format!("× {} = …", watch.expression), DebugRowStyle::Dim),
        };
        let mut row = DebugRow::new(text, style).action(DebugRowAction::RemoveWatch(index));
        row.hot_chars = Some(1);
        rows.push(row);
    }
    rows
}

fn console_rows(runtime: &DebugRuntime) -> Vec<DebugRow> {
    let mut rows = vec![DebugRow::new("DEBUG CONSOLE", DebugRowStyle::Header)];
    for output in &runtime.output {
        let style = if output.category == "stderr" {
            DebugRowStyle::Error
        } else {
            DebugRowStyle::Normal
        };
        for line in output.text.lines() {
            if !runtime.console_filter.is_empty() && !line.contains(&runtime.console_filter) {
                continue;
            }
            rows.push(DebugRow::new(line.trim_end(), style));
        }
    }
    rows
}

/// Where the panel's columns and rows sit inside its bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DebugPanelGeometry {
    pub columns: [(f32, f32); 3],
    pub body_top: f32,
    pub body_rows: usize,
    pub input_top: f32,
    pub line_height: f32,
}

impl DebugPanelGeometry {
    pub(crate) fn new(bounds: &GroupRect, line_height: f32) -> Self {
        let body_top = bounds.y + line_height;
        let input_top = bounds.y + bounds.height - line_height * 1.8;
        let mut x = bounds.x;
        let mut columns = [(0.0, 0.0); 3];
        for (slot, share) in columns.iter_mut().zip(COLUMN_SHARE) {
            let width = bounds.width * share;
            *slot = (x, width);
            x += width;
        }
        Self {
            columns,
            body_top,
            body_rows: (((input_top - body_top) / line_height).floor().max(0.0)) as usize,
            input_top,
            line_height,
        }
    }

    pub(crate) fn column_at(&self, x: f32) -> Option<usize> {
        self.columns
            .iter()
            .position(|(left, width)| x >= *left && x < left + width)
    }
}

/// The rows of a column that fit on screen. The console follows the newest output, so its
/// scroll counts lines up from the end; the others count lines down from the top.
pub(crate) fn visible_range(
    column: usize,
    total: usize,
    rows: usize,
    scroll: usize,
) -> Range<usize> {
    let hidden = total.saturating_sub(rows);
    let scroll = scroll.min(hidden);
    if column == CONSOLE {
        let end = total - scroll;
        end.saturating_sub(rows)..end
    } else {
        scroll..(scroll + rows).min(total)
    }
}

/// The action under a click, if any.
pub(crate) fn action_at(
    columns: &[Vec<DebugRow>; 3],
    scroll: [usize; 3],
    geometry: &DebugPanelGeometry,
    x: f32,
    y: f32,
    char_width: f32,
) -> Option<DebugRowAction> {
    if y < geometry.body_top || y >= geometry.input_top {
        return None;
    }
    let column = geometry.column_at(x)?;
    let line = ((y - geometry.body_top) / geometry.line_height) as usize;
    let range = visible_range(
        column,
        columns[column].len(),
        geometry.body_rows,
        scroll[column],
    );
    let row = columns[column]
        .get(range.start + line)
        .filter(|_| line < range.len())?;
    let action = row.action.clone()?;
    if let Some(chars) = row.hot_chars {
        let text_x = geometry.columns[column].0 + PAD;
        if x - text_x >= chars as f32 * char_width {
            return None;
        }
    }
    Some(action)
}

impl App {
    pub(crate) fn debug_panel_geometry(
        &self,
        width: f32,
        height: f32,
        line_height: f32,
    ) -> DebugPanelGeometry {
        DebugPanelGeometry::new(&self.panel_bounds(width, height, line_height), line_height)
    }

    /// Handles a click inside the debug panel body. Returns whether an action ran.
    pub(crate) fn debug_panel_click(
        &mut self,
        geometry: &DebugPanelGeometry,
        x: f32,
        y: f32,
        char_width: f32,
    ) -> bool {
        let columns = debug_columns(&self.debug);
        let Some(action) = action_at(&columns, self.debug_scroll, geometry, x, y, char_width)
        else {
            return false;
        };
        let result = match action {
            DebugRowAction::SelectFrame(index) => self.debug.select_frame(index),
            DebugRowAction::ToggleVariable(reference) => self.debug.toggle_variable(reference),
            DebugRowAction::RemoveWatch(index) => {
                self.debug.remove_watch(index);
                Ok(())
            }
            DebugRowAction::ToggleException(index) => self.debug.toggle_exception_filter(index),
        };
        if let Err(error) = result {
            self.status = error.to_string();
        }
        true
    }

    /// Scrolls the column under `x` by `lines` (positive scrolls down).
    pub(crate) fn debug_panel_scroll(&mut self, geometry: &DebugPanelGeometry, x: f32, lines: f32) {
        let Some(column) = geometry.column_at(x) else {
            return;
        };
        let total = debug_columns(&self.debug)[column].len();
        let hidden = total.saturating_sub(geometry.body_rows) as i32;
        let delta = lines as i32;
        let delta = if column == CONSOLE { -delta } else { delta };
        self.debug_scroll[column] =
            (self.debug_scroll[column] as i32 + delta).clamp(0, hidden) as usize;
    }

    pub(crate) fn draw_debug_panel(&self, r: &mut Renderer, bounds: &GroupRect, line_height: f32) {
        let geometry = DebugPanelGeometry::new(bounds, line_height);
        let columns = debug_columns(&self.debug);
        let text = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x616161));
        let error = rgb(Theme::color(0xf48771, 0xa1260d));
        for (index, rows) in columns.iter().enumerate() {
            let (left, width) = geometry.columns[index];
            if index > 0 {
                r.set_clip(None);
                r.draw_rect(
                    left,
                    geometry.body_top,
                    1.0,
                    geometry.input_top - geometry.body_top,
                    rgb(Theme::color(0x3c3c3c, 0xd0d0d0)),
                );
            }
            r.set_clip(Some([
                left,
                geometry.body_top,
                left + width,
                geometry.input_top,
            ]));
            let range = visible_range(
                index,
                rows.len(),
                geometry.body_rows,
                self.debug_scroll[index],
            );
            for (line, row) in rows[range].iter().enumerate() {
                let y = geometry.body_top + line as f32 * line_height;
                if row.style == DebugRowStyle::Selected {
                    r.draw_rect(
                        left,
                        y,
                        width,
                        line_height,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                    );
                }
                let color = match row.style {
                    DebugRowStyle::Header | DebugRowStyle::Dim => dim,
                    DebugRowStyle::Error => error,
                    DebugRowStyle::Normal | DebugRowStyle::Selected => text,
                };
                r.draw_text(&row.text, left + PAD, y, color);
            }
        }
        r.set_clip(None);
        r.draw_text(
            &format!("> {}", self.debug_input),
            bounds.x + PAD,
            bounds.y + bounds.height - line_height * 1.7,
            text,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::{ExceptionFilter, StackFrame, WatchExpression};

    fn geometry() -> DebugPanelGeometry {
        // 1000 px wide, 11 lines of 20 px: one heading, eight body rows, the input line.
        DebugPanelGeometry::new(
            &GroupRect {
                id: 0,
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 220.0,
            },
            20.0,
        )
    }

    fn runtime_with_stop() -> DebugRuntime {
        let mut runtime = DebugRuntime::default();
        runtime.state = DebugState::Paused {
            thread_id: 1,
            reason: "breakpoint".into(),
        };
        runtime.frames = vec![
            StackFrame {
                id: 10,
                name: "main".into(),
                source: Some("src/a.rs".into()),
                line: 3,
            },
            StackFrame {
                id: 11,
                name: "caller".into(),
                source: None,
                line: 9,
            },
        ];
        runtime.exception_filters = vec![ExceptionFilter {
            id: "all".into(),
            label: "All Exceptions".into(),
            enabled: true,
        }];
        runtime.watches = vec![
            WatchExpression {
                expression: "a + 1".into(),
                result: Some(Ok("2".into())),
            },
            WatchExpression {
                expression: "bad".into(),
                result: Some(Err("not defined".into())),
            },
            WatchExpression {
                expression: "slow".into(),
                result: None,
            },
        ];
        runtime
    }

    #[test]
    fn the_stack_column_marks_the_selected_frame_and_lists_exception_filters() {
        let runtime = runtime_with_stop();
        let rows = &debug_columns(&runtime)[STACK];
        let texts: Vec<_> = rows.iter().map(|row| row.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "CALL STACK",
                "▶ main  a.rs:3",
                "  caller",
                "EXCEPTION BREAKPOINTS",
                "[x] All Exceptions"
            ]
        );
        assert_eq!(rows[1].style, DebugRowStyle::Selected);
        assert_eq!(rows[2].action, Some(DebugRowAction::SelectFrame(1)));
        assert_eq!(rows[4].action, Some(DebugRowAction::ToggleException(0)));
    }

    #[test]
    fn an_idle_panel_explains_itself_instead_of_showing_nothing() {
        let rows = debug_columns(&DebugRuntime::default());
        assert_eq!(rows[STACK][1].text, "Not paused");
        assert_eq!(rows[STACK][3].text, "None offered by the adapter");
        assert_eq!(rows[VARIABLES][1].text, "No variables");
        assert_eq!(rows[CONSOLE].len(), 1);
    }

    #[test]
    fn watch_rows_show_results_errors_and_pending_values() {
        let runtime = runtime_with_stop();
        let rows = &debug_columns(&runtime)[VARIABLES];
        let watch: Vec<_> = rows
            .iter()
            .skip_while(|row| row.text != "WATCH")
            .skip(1)
            .map(|row| (row.text.as_str(), row.style))
            .collect();
        assert_eq!(
            watch,
            [
                ("× a + 1 = 2", DebugRowStyle::Normal),
                ("× bad: not defined", DebugRowStyle::Error),
                ("× slow = …", DebugRowStyle::Dim)
            ]
        );
    }

    #[test]
    fn the_console_splits_multiline_output_and_flags_errors() {
        let mut runtime = DebugRuntime::default();
        runtime.output.push(crate::debugger::DebugOutput {
            category: "stdout".into(),
            text: "one\ntwo\n".into(),
        });
        runtime.output.push(crate::debugger::DebugOutput {
            category: "stderr".into(),
            text: "boom".into(),
        });
        let rows = &debug_columns(&runtime)[CONSOLE];
        let lines: Vec<_> = rows
            .iter()
            .map(|row| (row.text.as_str(), row.style))
            .collect();
        assert_eq!(
            lines,
            [
                ("DEBUG CONSOLE", DebugRowStyle::Header),
                ("one", DebugRowStyle::Normal),
                ("two", DebugRowStyle::Normal),
                ("boom", DebugRowStyle::Error)
            ]
        );
    }

    #[test]
    fn visible_ranges_clamp_and_the_console_follows_the_end() {
        assert_eq!(visible_range(STACK, 20, 8, 0), 0..8);
        assert_eq!(visible_range(STACK, 20, 8, 5), 5..13);
        assert_eq!(visible_range(STACK, 20, 8, 99), 12..20);
        assert_eq!(visible_range(STACK, 3, 8, 4), 0..3);
        assert_eq!(visible_range(CONSOLE, 20, 8, 0), 12..20);
        assert_eq!(visible_range(CONSOLE, 20, 8, 5), 7..15);
        assert_eq!(visible_range(CONSOLE, 20, 8, 99), 0..8);
        assert_eq!(visible_range(CONSOLE, 3, 8, 2), 0..3);
        assert_eq!(visible_range(STACK, 0, 0, 0), 0..0);
    }

    #[test]
    fn clicks_map_to_the_row_under_the_pointer() {
        let runtime = runtime_with_stop();
        let columns = debug_columns(&runtime);
        let geometry = geometry();
        let scroll = [0; 3];
        // Body row 2 (y 60..80) of the stack column is the second frame.
        assert_eq!(
            action_at(&columns, scroll, &geometry, 50.0, 70.0, 8.0),
            Some(DebugRowAction::SelectFrame(1))
        );
        // The header, the empty area below the rows, the heading and the input line do nothing.
        assert_eq!(
            action_at(&columns, scroll, &geometry, 50.0, 30.0, 8.0),
            None
        );
        assert_eq!(
            action_at(&columns, scroll, &geometry, 50.0, 150.0, 8.0),
            None
        );
        assert_eq!(action_at(&columns, scroll, &geometry, 50.0, 5.0, 8.0), None);
        assert_eq!(
            action_at(&columns, scroll, &geometry, 50.0, 205.0, 8.0),
            None
        );
        // The console column has no actions.
        assert_eq!(
            action_at(&columns, scroll, &geometry, 900.0, 50.0, 8.0),
            None
        );
    }

    #[test]
    fn scrolling_moves_which_row_a_click_hits() {
        let runtime = runtime_with_stop();
        let columns = debug_columns(&runtime);
        let mut small = geometry();
        small.body_rows = 2;
        // With two visible rows scrolled by two, the first visible row is the second frame.
        assert_eq!(
            action_at(&columns, [2, 0, 0], &small, 50.0, 30.0, 8.0),
            Some(DebugRowAction::SelectFrame(1))
        );
        // A row below the visible window is not clickable even though it exists.
        assert_eq!(
            action_at(&columns, [0, 0, 0], &small, 50.0, 70.0, 8.0),
            None
        );
    }

    #[test]
    fn only_the_cross_of_a_watch_row_removes_it() {
        let runtime = runtime_with_stop();
        let columns = debug_columns(&runtime);
        let geometry = geometry();
        let (left, _) = geometry.columns[VARIABLES];
        let variables = &columns[VARIABLES];
        let first_watch = variables
            .iter()
            .position(|row| row.text == "× a + 1 = 2")
            .unwrap();
        let y = geometry.body_top + first_watch as f32 * 20.0 + 5.0;
        assert_eq!(
            action_at(&columns, [0; 3], &geometry, left + PAD + 2.0, y, 8.0),
            Some(DebugRowAction::RemoveWatch(0))
        );
        assert_eq!(
            action_at(&columns, [0; 3], &geometry, left + PAD + 40.0, y, 8.0),
            None,
            "clicking the expression text must not delete the watch"
        );
    }

    #[test]
    fn column_geometry_covers_the_panel_without_gaps() {
        let geometry = geometry();
        assert_eq!(geometry.body_rows, 8);
        assert_eq!(geometry.column_at(0.0), Some(STACK));
        assert_eq!(geometry.column_at(299.0), Some(STACK));
        assert_eq!(geometry.column_at(301.0), Some(VARIABLES));
        assert_eq!(geometry.column_at(999.0), Some(CONSOLE));
        assert_eq!(geometry.column_at(1000.0), None);
    }
}
