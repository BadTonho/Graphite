//! Horizontal scrolling of an editor view that does not wrap its lines.
//!
//! A view keeps how many columns its text is scrolled to the left (`View::scroll_col`). The text
//! area is the group's width without the line-number column and the minimap, so it changes when
//! the group is resized, for example when a side bar is dragged wider. The scroll then follows:
//! it keeps the caret in view when the caret moves and never rests past the end of the longest
//! visible line, so widening the editor brings the text back.

use super::drawing_helpers::display_line;
use super::editor_groups::GroupRect;
use super::*;

/// The scroll that shows column `cursor` in a text area `columns` wide, moving `scroll` as little
/// as possible.
pub(crate) fn reveal_column(scroll: usize, cursor: usize, columns: usize) -> usize {
    let columns = columns.max(1);
    if cursor < scroll {
        cursor
    } else if cursor + 1 > scroll + columns {
        cursor + 1 - columns
    } else {
        scroll
    }
}

/// The largest scroll that still shows something: the end of a line `longest` columns wide, with
/// room for the caret after it, at the right edge of a text area `columns` wide.
pub(crate) fn max_scroll(longest: usize, columns: usize) -> usize {
    (longest + 1).saturating_sub(columns.max(1))
}

/// Columns the text of a line takes once tabs are expanded, without its line break.
fn line_columns(text: &str, tab_size: usize) -> usize {
    display_line(text.trim_end_matches(['\r', '\n']), tab_size)
        .chars()
        .count()
}

impl App {
    /// Columns of text a group's text area shows: its width minus the line numbers and minimap.
    pub(crate) fn text_area_columns(&self, rect: GroupRect, cw: f32) -> usize {
        let minimap = if self.view_preferences.minimap_visible {
            72.0_f32.min(rect.width * 0.22)
        } else {
            0.0
        };
        let width = (rect.width - self.gutter_width(cw) - minimap).max(cw);
        ((width / cw) as usize).max(1)
    }

    /// Where column 0 of the focused view's text starts on screen, after horizontal scrolling.
    pub(crate) fn text_origin_x(&self, rect: GroupRect, cw: f32) -> f32 {
        let scrolled = if self.word_wrap() {
            0
        } else {
            self.editor_groups.active_view().scroll_col
        };
        rect.x + self.gutter_width(cw) - scrolled as f32 * cw
    }

    /// Scrolls the focused view sideways until the caret is inside the text area.
    pub(crate) fn reveal_cursor_column(&mut self, rect: GroupRect, cw: f32) {
        if self.word_wrap() {
            self.editor_groups.active_view_mut().scroll_col = 0;
            return;
        }
        let columns = self.text_area_columns(rect, cw);
        let tab_size = self.documents[self.active_document()].indentation.tab_size;
        let (line, _) = self.editor().cursor_line_col();
        let before_caret: String = {
            let buffer = &self.documents[self.active_document()].buffer;
            let start = buffer.line_start_char(line);
            let head = self.editor().cursor().char_idx.max(start);
            buffer.line(line).chars().take(head - start).collect()
        };
        let cursor = line_columns(&before_caret, tab_size);
        let view = self.editor_groups.active_view_mut();
        view.scroll_col = reveal_column(view.scroll_col, cursor, columns);
    }

    /// Keeps every view's scroll inside what its lines can show. Runs each frame, so a group that
    /// gets wider (a side bar closed or dragged narrower) shows its text again at once.
    pub(crate) fn clamp_horizontal_scroll(&mut self, width: f32, height: f32, lh: f32, cw: f32) {
        let rects = self.group_rectangles(width, height, lh);
        let minimap = self.view_preferences.minimap_visible;
        let setting = self.word_wrap_setting;
        let tabs = &self.documents;
        self.editor_groups.for_each_view_mut(|group, view| {
            if view.scroll_col == 0 {
                return;
            }
            let (Some(rect), Some(tab)) = (
                rects.iter().find(|rect| rect.id == group),
                tabs.get(view.document),
            ) else {
                return;
            };
            let wraps = super::word_wrap::resolve_wrap(
                setting,
                view.wrap_choice,
                super::word_wrap::wraps_by_default(tab.buffer.path(), tab.language),
            );
            if wraps {
                view.scroll_col = 0;
                return;
            }
            let minimap_width = if minimap {
                72.0_f32.min(rect.width * 0.22)
            } else {
                0.0
            };
            let gutter = tab.buffer.len_lines().to_string().len().max(3) as f32 * cw + PAD * 2.0;
            let columns = (((rect.width - gutter - minimap_width).max(cw)) / cw) as usize;
            let tab_size = tab.indentation.tab_size;
            let buffer = &tab.buffer;
            let first = view.scroll_line.min(buffer.len_lines().saturating_sub(1));
            let shown = (rect.height / lh) as usize + 1;
            let mut longest = 0;
            for line in first..(first + shown).min(buffer.len_lines()) {
                longest = longest.max(line_columns(&buffer.line(line), tab_size));
            }
            view.scroll_col = view.scroll_col.min(max_scroll(longest, columns));
        });
    }

    /// Scrolls the focused view sideways by `columns` (negative is left). The result is clamped
    /// on the next frame, so a view cannot be scrolled into empty space.
    pub(crate) fn scroll_columns_by(&mut self, columns: isize) {
        if self.word_wrap() {
            return;
        }
        let view = self.editor_groups.active_view_mut();
        view.scroll_col = (view.scroll_col as isize + columns).max(0) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_caret_is_revealed_with_the_least_movement() {
        // Inside the text area: no change.
        assert_eq!(reveal_column(0, 10, 40), 0);
        assert_eq!(reveal_column(5, 44, 40), 5);
        // Past the right edge: the caret ends up in the last column.
        assert_eq!(reveal_column(0, 40, 40), 1);
        assert_eq!(reveal_column(0, 100, 40), 61);
        // Before the left edge: the caret ends up in the first column.
        assert_eq!(reveal_column(30, 12, 40), 12);
        // A zero-width area still keeps the caret in view.
        assert_eq!(reveal_column(0, 7, 0), 7);
    }

    #[test]
    fn scrolling_stops_at_the_end_of_the_longest_line() {
        assert_eq!(max_scroll(100, 40), 61);
        assert_eq!(max_scroll(30, 40), 0);
        assert_eq!(max_scroll(39, 40), 0);
        assert_eq!(max_scroll(40, 40), 1);
        assert_eq!(max_scroll(0, 0), 0);
    }

    fn app_with_text(text: &str) -> (App, crate::test_support::TempDir) {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.sidebar_window_width_px = 1000.0;
        app.view_preferences.minimap_visible = false;
        // A new file is plain text and would wrap by default; these tests are about not wrapping.
        app.word_wrap_setting = Some(false);
        app.editor_mut().insert(text);
        (app, dir)
    }

    #[test]
    fn the_scroll_follows_the_caret_and_the_group_width() {
        let long = "a".repeat(300);
        let (mut app, _dir) = app_with_text(&long);
        let (w, h, lh, cw) = (1000.0, 700.0, 20.0, 10.0);
        let rect = app.focused_group_rect(w, h, lh);
        let columns = app.text_area_columns(rect, cw);

        // The caret is at the end of the line: it is revealed in the last column.
        app.reveal_cursor_column(rect, cw);
        assert_eq!(
            app.editor_groups.active_view().scroll_col,
            300 + 1 - columns
        );
        // Text starts left of the text area by the scroll, so the caret is in view.
        let caret_x = app.text_origin_x(rect, cw) + 300.0 * cw;
        assert!(caret_x >= rect.x + app.gutter_width(cw));
        assert!(caret_x < rect.x + rect.width);

        // Opening the secondary side bar makes the group narrower: the caret is no longer in
        // view until the caret moves, and revealing it scrolls further.
        let before = app.editor_groups.active_view().scroll_col;
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        let narrow = app.focused_group_rect(w, h, lh);
        assert!(narrow.width < rect.width);
        app.reveal_cursor_column(narrow, cw);
        assert!(app.editor_groups.active_view().scroll_col > before);

        // Closing it again widens the group; the per-frame clamp keeps the scroll valid, and a
        // very wide group has nothing left to scroll.
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        app.clamp_horizontal_scroll(w, h, lh, cw);
        assert_eq!(app.editor_groups.active_view().scroll_col, before);
        app.clamp_horizontal_scroll(5000.0, h, lh, cw);
        assert_eq!(app.editor_groups.active_view().scroll_col, 0);
    }

    #[test]
    fn word_wrap_never_scrolls_sideways_and_manual_scroll_is_clamped() {
        let long = "b".repeat(200);
        let (mut app, _dir) = app_with_text(&long);
        let (w, h, lh, cw) = (1000.0, 700.0, 20.0, 10.0);
        app.scroll_columns_by(9999);
        app.clamp_horizontal_scroll(w, h, lh, cw);
        let rect = app.focused_group_rect(w, h, lh);
        let columns = app.text_area_columns(rect, cw);
        assert_eq!(
            app.editor_groups.active_view().scroll_col,
            max_scroll(200, columns)
        );
        app.scroll_columns_by(-9999);
        assert_eq!(app.editor_groups.active_view().scroll_col, 0);

        app.scroll_columns_by(30);
        app.word_wrap_setting = Some(true);
        app.clamp_horizontal_scroll(w, h, lh, cw);
        assert_eq!(app.editor_groups.active_view().scroll_col, 0);
        app.scroll_columns_by(30);
        assert_eq!(app.editor_groups.active_view().scroll_col, 0);
        assert_eq!(app.text_origin_x(rect, cw), rect.x + app.gutter_width(cw));
    }
}
