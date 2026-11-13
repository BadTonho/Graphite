use super::*;

const ROW_HEIGHT: f32 = 28.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const MIN_MENU_WIDTH: f32 = 336.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionMenuCommand {
    SelectAll,
    ExpandSelection,
    ShrinkSelection,
    CopyLineUp,
    CopyLineDown,
    MoveLineUp,
    MoveLineDown,
    DuplicateSelection,
    AddCursorAbove,
    AddCursorBelow,
    AddCursorsToLineEnds,
    AddNextOccurrence,
    AddPreviousOccurrence,
    SelectAllOccurrences,
    SwitchMultiCursorModifier,
    ColumnSelectionMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionMenuRow {
    Separator,
    Item {
        command: SelectionMenuCommand,
        label: &'static str,
        shortcut: &'static str,
        enabled: bool,
        checked: bool,
    },
}

impl App {
    pub(crate) fn selection_menu_rows(&self) -> Vec<SelectionMenuRow> {
        use SelectionMenuCommand as C;
        use SelectionMenuRow::{Item as I, Separator as S};
        let editor = self.editor();
        let available = self.documents.get(self.active_document()).is_some();
        let writable = available && !editor.is_read_only();
        let row = |command, label, shortcut, enabled, checked| I {
            command,
            label,
            shortcut,
            enabled,
            checked,
        };
        vec![
            row(
                C::SelectAll,
                "selection.select_all",
                "Ctrl+A",
                available,
                false,
            ),
            row(
                C::ExpandSelection,
                "selection.expand",
                "Shift+Alt+RightArrow",
                available,
                false,
            ),
            row(
                C::ShrinkSelection,
                "selection.shrink",
                "Shift+Alt+LeftArrow",
                available,
                false,
            ),
            S,
            row(
                C::CopyLineUp,
                "selection.copy_line_up",
                "Shift+Alt+UpArrow",
                writable,
                false,
            ),
            row(
                C::CopyLineDown,
                "selection.copy_line_down",
                "Shift+Alt+DownArrow",
                writable,
                false,
            ),
            row(
                C::MoveLineUp,
                "selection.move_line_up",
                "Alt+UpArrow",
                writable,
                false,
            ),
            row(
                C::MoveLineDown,
                "selection.move_line_down",
                "Alt+DownArrow",
                writable,
                false,
            ),
            row(
                C::DuplicateSelection,
                "selection.duplicate",
                "",
                writable,
                false,
            ),
            S,
            row(
                C::AddCursorAbove,
                "selection.add_cursor_above",
                "Ctrl+Alt+UpArrow",
                available,
                false,
            ),
            row(
                C::AddCursorBelow,
                "selection.add_cursor_below",
                "Ctrl+Alt+DownArrow",
                available,
                false,
            ),
            row(
                C::AddCursorsToLineEnds,
                "selection.add_cursors_line_ends",
                "Shift+Alt+I",
                available,
                false,
            ),
            row(
                C::AddNextOccurrence,
                "selection.add_next_occurrence",
                "Ctrl+D",
                available,
                false,
            ),
            row(
                C::AddPreviousOccurrence,
                "selection.add_previous_occurrence",
                "Ctrl+Shift+D",
                available,
                false,
            ),
            row(
                C::SelectAllOccurrences,
                "selection.select_all_occurrences",
                "Ctrl+Shift+L",
                available,
                false,
            ),
            S,
            row(
                C::SwitchMultiCursorModifier,
                match self.view_preferences.multi_cursor_modifier {
                    config::MultiCursorModifier::Control => "selection.switch_to_alt_click",
                    config::MultiCursorModifier::Alt => "selection.switch_to_ctrl_click",
                },
                "",
                true,
                false,
            ),
            row(
                C::ColumnSelectionMode,
                "selection.column_mode",
                "",
                true,
                self.view_preferences.column_selection_mode,
            ),
        ]
    }

    pub(crate) fn selection_menu_width(&self, char_width: f32, available_width: f32) -> f32 {
        let content_width = self
            .selection_menu_rows()
            .iter()
            .filter_map(|row| match row {
                SelectionMenuRow::Item {
                    label, shortcut, ..
                } => Some(
                    34.0 + self.t(label).chars().count() as f32 * char_width
                        + if shortcut.is_empty() {
                            20.0
                        } else {
                            shortcut.chars().count() as f32 * char_width + 24.0
                        },
                ),
                SelectionMenuRow::Separator => None,
            })
            .fold(MIN_MENU_WIDTH, f32::max);
        content_width.min(available_width.max(0.0))
    }

    pub(crate) fn selection_menu_command_at(
        &self,
        x: f32,
        y: f32,
        popup_x: f32,
        popup_y: f32,
        width: f32,
    ) -> Option<(SelectionMenuCommand, bool)> {
        if x < popup_x || x >= popup_x + width || y < popup_y + 4.0 {
            return None;
        }
        let mut top = popup_y + 4.0;
        for row in self.selection_menu_rows() {
            let height = selection_menu_row_height(row);
            if y >= top && y < top + height {
                return match row {
                    SelectionMenuRow::Separator => None,
                    SelectionMenuRow::Item {
                        command, enabled, ..
                    } => Some((command, enabled)),
                };
            }
            top += height;
        }
        None
    }

    pub(crate) fn selection_menu_pointer_selection(&mut self, y: f32, popup_y: f32) {
        let mut top = popup_y + 4.0;
        let mut index = 0;
        for row in self.selection_menu_rows() {
            let height = selection_menu_row_height(row);
            if y >= top && y < top + height {
                if matches!(row, SelectionMenuRow::Item { .. }) {
                    self.selection_menu_selection = index;
                }
                return;
            }
            top += height;
            if matches!(row, SelectionMenuRow::Item { .. }) {
                index += 1;
            }
        }
    }

    pub(crate) fn selection_menu_move_selection(&mut self, delta: isize) {
        let count = self
            .selection_menu_rows()
            .iter()
            .filter(|row| matches!(row, SelectionMenuRow::Item { .. }))
            .count();
        self.selection_menu_selection = if count == 0 {
            0
        } else {
            self.selection_menu_selection
                .saturating_add_signed(delta)
                .min(count - 1)
        };
    }

    pub(crate) fn selection_menu_items(&self) -> Vec<(SelectionMenuCommand, bool, bool)> {
        self.selection_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                SelectionMenuRow::Item {
                    command,
                    enabled,
                    checked,
                    ..
                } => Some((command, enabled, checked)),
                SelectionMenuRow::Separator => None,
            })
            .collect()
    }

    pub(crate) fn activate_selection_menu_accessibility_item(&mut self, index: usize) {
        if let Some((command, true, _)) = self.selection_menu_items().get(index).copied() {
            self.execute_selection_menu_command(command);
            self.close_workbench_menu();
        }
    }

    pub(crate) fn execute_selection_menu_command(&mut self, command: SelectionMenuCommand) {
        use SelectionMenuCommand as C;
        match command {
            C::SelectAll => self.run_ui_command("editor.action.selectAll"),
            C::ExpandSelection => self.run_ui_command("editor.action.smartSelect.expand"),
            C::ShrinkSelection => self.run_ui_command("editor.action.smartSelect.shrink"),
            C::CopyLineUp => self.run_ui_command("editor.action.copyLinesUpAction"),
            C::CopyLineDown => self.run_ui_command("editor.action.copyLinesDownAction"),
            C::MoveLineUp => self.run_ui_command("editor.action.moveLinesUpAction"),
            C::MoveLineDown => self.run_ui_command("editor.action.moveLinesDownAction"),
            C::DuplicateSelection => self.run_ui_command("editor.action.duplicateSelection"),
            C::AddCursorAbove => self.run_ui_command("editor.action.insertCursorAbove"),
            C::AddCursorBelow => self.run_ui_command("editor.action.insertCursorBelow"),
            C::AddCursorsToLineEnds => self.run_ui_command("editor.action.insertCursorAtEndOfEachLineSelected"),
            C::AddNextOccurrence => {
                self.run_ui_command("editor.action.addSelectionToNextFindMatch")
            }
            C::AddPreviousOccurrence => {
                self.run_ui_command("editor.action.addSelectionToPreviousFindMatch")
            }
            C::SelectAllOccurrences => self.run_ui_command("editor.action.selectHighlights"),
            C::SwitchMultiCursorModifier => self.toggle_multi_cursor_modifier(),
            C::ColumnSelectionMode => self.toggle_column_selection_mode(),
        }
    }

    pub(crate) fn toggle_multi_cursor_modifier(&mut self) {
        self.view_preferences.multi_cursor_modifier =
            match self.view_preferences.multi_cursor_modifier {
                config::MultiCursorModifier::Control => config::MultiCursorModifier::Alt,
                config::MultiCursorModifier::Alt => config::MultiCursorModifier::Control,
            };
        self.save_view_preferences_for_command();
    }

    pub(crate) fn multi_cursor_modifier_pressed(&self) -> bool {
        match self.view_preferences.multi_cursor_modifier {
            config::MultiCursorModifier::Control => self.modifiers.control_key(),
            config::MultiCursorModifier::Alt => self.modifiers.alt_key(),
        }
    }

    pub(crate) fn toggle_column_selection_mode(&mut self) {
        self.view_preferences.column_selection_mode = !self.view_preferences.column_selection_mode;
        self.column_selection_anchor = None;
        self.save_view_preferences_for_command();
    }

    pub(crate) fn add_cursors_vertical(&mut self, down: bool) {
        let Some((width, height, line_height, char_width)) = self.gpu.as_ref().map(|gpu| {
            (
                gpu.renderer.size().0,
                gpu.renderer.size().1,
                gpu.renderer.line_height(),
                gpu.renderer.char_width(),
            )
        }) else {
            return;
        };
        let word_wrap = self.word_wrap;
        let indent = self.wrapping_indent;
        let document = self.active_document();
        let tab_size = self.documents[document].indentation.tab_size;
        let rect = self.focused_group_rect(width, height, line_height);
        let columns =
            (((rect.width - self.gutter_width(char_width)).max(char_width)) / char_width) as usize;
        let mut editor = self.editor_mut();
        let old = editor.selections();
        let result =
            vertical_cursor_selections(&editor, &old, down, columns, tab_size, word_wrap, indent);
        editor.set_selections(result);
    }

    pub(crate) fn add_cursors_to_line_ends(&mut self) {
        let mut editor = self.editor_mut();
        let existing = editor.selections();
        let mut cursors = Vec::new();
        for selection in &existing {
            let start = selection.anchor.min(selection.head);
            let end = selection.anchor.max(selection.head);
            let first_line = editor.rope().char_to_line(start);
            let last_line = if end > start {
                editor.rope().char_to_line(end.saturating_sub(1))
            } else {
                first_line
            };
            for line in first_line..=last_line {
                let rope_line = editor.rope().line(line);
                let content_len = rope_line
                    .to_string()
                    .trim_end_matches(['\n', '\r'])
                    .chars()
                    .count();
                let offset = editor.rope().line_to_char(line) + content_len;
                cursors.push(graphite_text::Selection {
                    anchor: offset,
                    head: offset,
                });
            }
        }
        editor.set_selections(cursors);
    }
}

pub(crate) fn selection_menu_row_height(row: SelectionMenuRow) -> f32 {
    match row {
        SelectionMenuRow::Separator => SEPARATOR_HEIGHT,
        SelectionMenuRow::Item { .. } => ROW_HEIGHT,
    }
}

pub(crate) fn selection_menu_height(rows: &[SelectionMenuRow]) -> f32 {
    rows.iter()
        .copied()
        .map(selection_menu_row_height)
        .sum::<f32>()
        + 8.0
}

pub(crate) fn vertical_cursor_selections(
    buffer: &Buffer,
    selections: &[graphite_text::Selection],
    down: bool,
    columns: usize,
    tab_size: usize,
    word_wrap: bool,
    indent: config::WrapIndent,
) -> Vec<graphite_text::Selection> {
    let last_row = if word_wrap {
        super::visual_lines::document_position(
            buffer,
            buffer.len_chars(),
            columns,
            tab_size,
            indent,
        )
        .0
    } else {
        buffer.len_lines().saturating_sub(1)
    };
    let mut result = selections.to_vec();
    for selection in selections {
        let (row, column) = if word_wrap {
            super::visual_lines::document_position(
                buffer,
                selection.head,
                columns,
                tab_size,
                indent,
            )
        } else {
            let line = buffer.rope().char_to_line(selection.head);
            (line, selection.head - buffer.rope().line_to_char(line))
        };
        let target_row = if down {
            row.saturating_add(1).min(last_row)
        } else {
            row.saturating_sub(1)
        };
        if target_row == row {
            continue;
        }
        let offset = if word_wrap {
            super::visual_lines::document_offset(
                buffer, target_row, column, columns, tab_size, indent,
            )
        } else {
            let start = buffer.rope().line_to_char(target_row);
            let content_len = buffer
                .rope()
                .line(target_row)
                .to_string()
                .trim_end_matches(['\n', '\r'])
                .chars()
                .count();
            start + column.min(content_len)
        };
        result.push(graphite_text::Selection {
            anchor: offset,
            head: offset,
        });
    }
    result
}

pub(crate) fn rectangular_selections(
    buffer: &Buffer,
    anchor: (usize, usize),
    head: (usize, usize),
    columns: usize,
    tab_size: usize,
    word_wrap: bool,
    indent: config::WrapIndent,
) -> Vec<graphite_text::Selection> {
    let last_row = if word_wrap {
        super::visual_lines::document_position(
            buffer,
            buffer.len_chars(),
            columns,
            tab_size,
            indent,
        )
        .0
    } else {
        buffer.len_lines().saturating_sub(1)
    };
    let anchor_row = anchor.0.min(last_row);
    let head_row = head.0.min(last_row);
    let first = anchor_row.min(head_row);
    let last = anchor_row.max(head_row);
    let left = anchor.1.min(head.1);
    let right = anchor.1.max(head.1);
    (first..=last)
        .map(|row| {
            let offset = |column| {
                if word_wrap {
                    super::visual_lines::document_offset(
                        buffer, row, column, columns, tab_size, indent,
                    )
                } else {
                    let line = row.min(buffer.len_lines().saturating_sub(1));
                    let start = buffer.rope().line_to_char(line);
                    let content_len = buffer
                        .rope()
                        .line(line)
                        .to_string()
                        .trim_end_matches(['\n', '\r'])
                        .chars()
                        .count();
                    start + column.min(content_len)
                }
            };
            let left_offset = offset(left);
            let right_offset = offset(right);
            graphite_text::Selection {
                anchor: if anchor.1 <= head.1 {
                    left_offset
                } else {
                    right_offset
                },
                head: if anchor.1 <= head.1 {
                    right_offset
                } else {
                    left_offset
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (crate::test_support::TempDir, App) {
        let temp = crate::test_support::TempDir::new();
        let app = App::new_with_options(temp.path().to_path_buf(), None, true);
        (temp, app)
    }

    #[test]
    fn selection_menu_matches_reference_order_and_grouping() {
        let (_temp, app) = app();
        let rows = app.selection_menu_rows();
        let commands = rows
            .iter()
            .filter_map(|row| match row {
                SelectionMenuRow::Item { command, .. } => Some(*command),
                SelectionMenuRow::Separator => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            commands,
            [
                SelectionMenuCommand::SelectAll,
                SelectionMenuCommand::ExpandSelection,
                SelectionMenuCommand::ShrinkSelection,
                SelectionMenuCommand::CopyLineUp,
                SelectionMenuCommand::CopyLineDown,
                SelectionMenuCommand::MoveLineUp,
                SelectionMenuCommand::MoveLineDown,
                SelectionMenuCommand::DuplicateSelection,
                SelectionMenuCommand::AddCursorAbove,
                SelectionMenuCommand::AddCursorBelow,
                SelectionMenuCommand::AddCursorsToLineEnds,
                SelectionMenuCommand::AddNextOccurrence,
                SelectionMenuCommand::AddPreviousOccurrence,
                SelectionMenuCommand::SelectAllOccurrences,
                SelectionMenuCommand::SwitchMultiCursorModifier,
                SelectionMenuCommand::ColumnSelectionMode,
            ]
        );
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(row, SelectionMenuRow::Separator))
                .count(),
            3
        );
        assert!(app.selection_menu_width(8.0, 1000.0) >= MIN_MENU_WIDTH);
        assert_eq!(app.selection_menu_width(8.0, 190.0), 190.0);
    }

    #[test]
    fn menu_hit_testing_and_keyboard_selection_share_the_same_items() {
        let (_temp, mut app) = app();
        assert_eq!(
            app.selection_menu_command_at(140.0, 48.0, 100.0, 30.0, 400.0),
            Some((SelectionMenuCommand::SelectAll, true))
        );
        assert_eq!(
            app.selection_menu_command_at(99.0, 48.0, 100.0, 30.0, 400.0),
            None
        );
        assert_eq!(
            app.selection_menu_command_at(140.0, 124.5, 100.0, 30.0, 400.0),
            None
        );
        app.selection_menu_move_selection(1);
        assert_eq!(
            app.selection_menu_items()[app.selection_menu_selection].0,
            SelectionMenuCommand::ExpandSelection
        );
    }

    #[test]
    fn cursor_modifier_and_column_mode_persist_with_the_active_profile() {
        let (temp, mut app) = app();
        app.profile_root = temp.path().join("profile");
        app.toggle_multi_cursor_modifier();
        app.toggle_column_selection_mode();
        let loaded = config::load_view_layout_preferences(&app.profile_root);
        assert_eq!(
            loaded.multi_cursor_modifier,
            config::MultiCursorModifier::Alt
        );
        assert!(loaded.column_selection_mode);
        assert_eq!(
            app.selection_menu_rows().iter().find_map(|row| match row {
                SelectionMenuRow::Item {
                    command: SelectionMenuCommand::SwitchMultiCursorModifier,
                    label,
                    ..
                } => Some(*label),
                _ => None,
            }),
            Some("selection.switch_to_ctrl_click")
        );
    }

    #[test]
    fn vertical_cursors_follow_visual_rows_and_column_selections_cover_the_rectangle() {
        let mut document = Buffer::new();
        let mut view = graphite_text::SelectionState::default();
        let mut buffer = document.edit(&mut view, graphite_text::ViewId(1));
        buffer.insert("abcdefgh\nxy\n123456");
        let start = [graphite_text::Selection { anchor: 1, head: 1 }];
        let vertical =
            vertical_cursor_selections(&buffer, &start, true, 4, 4, true, config::WrapIndent::None);
        assert_eq!(vertical[1].head, 5);

        let rectangle = rectangular_selections(
            &buffer,
            (0, 1),
            (2, 3),
            4,
            4,
            false,
            config::WrapIndent::None,
        );
        assert_eq!(rectangle.len(), 3);
        assert_eq!(
            rectangle,
            vec![
                graphite_text::Selection { anchor: 1, head: 3 },
                graphite_text::Selection {
                    anchor: 10,
                    head: 11
                },
                graphite_text::Selection {
                    anchor: 13,
                    head: 15
                },
            ]
        );

        let wrapped_rectangle = rectangular_selections(
            &buffer,
            (0, 1),
            (1, 3),
            4,
            4,
            true,
            config::WrapIndent::None,
        );
        assert_eq!(wrapped_rectangle.len(), 2);
        assert_eq!(
            wrapped_rectangle[0],
            graphite_text::Selection { anchor: 1, head: 3 }
        );
        assert_eq!(
            wrapped_rectangle[1],
            graphite_text::Selection { anchor: 5, head: 7 }
        );
    }
}
