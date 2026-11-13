use super::*;

const ROW_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
pub(crate) const EDIT_MENU_WIDTH: f32 = 340.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditMenuCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Find,
    Replace,
    FindInFiles,
    ReplaceInFiles,
    ToggleLineComment,
    ToggleBlockComment,
    EmmetExpand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditMenuRow {
    Separator,
    Item {
        command: EditMenuCommand,
        label: &'static str,
        shortcut: &'static str,
        enabled: bool,
    },
}

impl App {
    pub(crate) fn edit_menu_rows(&self) -> Vec<EditMenuRow> {
        use EditMenuCommand as C;
        let row = |command, label, shortcut, enabled| EditMenuRow::Item {
            command,
            label,
            shortcut,
            enabled,
        };
        let editor = self.editor();
        let has_selection = editor.selections().iter().any(|s| s.anchor != s.head);
        let writable = !editor.is_read_only();
        let comments = self.active_comment_tokens();
        vec![
            row(C::Undo, "edit.undo", "Ctrl+Z", editor.can_undo()),
            row(C::Redo, "edit.redo", "Ctrl+Y", editor.can_redo()),
            EditMenuRow::Separator,
            row(C::Cut, "edit.cut", "Ctrl+X", writable && has_selection),
            row(C::Copy, "edit.copy", "Ctrl+C", has_selection),
            row(C::Paste, "edit.paste", "Ctrl+V", writable),
            EditMenuRow::Separator,
            row(C::Find, "edit.find", "Ctrl+F", true),
            row(C::Replace, "edit.replace", "Ctrl+H", writable),
            EditMenuRow::Separator,
            row(C::FindInFiles, "edit.find_in_files", "Ctrl+Shift+F", true),
            row(
                C::ReplaceInFiles,
                "edit.replace_in_files",
                "Ctrl+Shift+H",
                true,
            ),
            EditMenuRow::Separator,
            row(
                C::ToggleLineComment,
                "edit.toggle_line_comment",
                "Ctrl+;",
                writable && comments.as_ref().is_some_and(|c| c.line.is_some()),
            ),
            row(
                C::ToggleBlockComment,
                "edit.toggle_block_comment",
                "Shift+Alt+A",
                writable && comments.as_ref().is_some_and(|c| c.block.is_some()),
            ),
            EditMenuRow::Separator,
            row(
                C::EmmetExpand,
                "edit.emmet_expand",
                "Tab",
                writable && self.can_expand_emmet_at_cursor(),
            ),
        ]
    }

    pub(crate) fn edit_menu_command_at(
        &self,
        x: f32,
        y: f32,
        popup_x: f32,
        popup_y: f32,
    ) -> Option<(EditMenuCommand, bool)> {
        if x < popup_x || x >= popup_x + EDIT_MENU_WIDTH || y < popup_y + 4.0 {
            return None;
        }
        let mut row_y = popup_y + 4.0;
        for row in self.edit_menu_rows() {
            let height = Self::edit_menu_row_height(row);
            if y >= row_y && y < row_y + height {
                return match row {
                    EditMenuRow::Separator => None,
                    EditMenuRow::Item {
                        command, enabled, ..
                    } => Some((command, enabled)),
                };
            }
            row_y += height;
        }
        None
    }

    pub(crate) fn edit_menu_move_selection(&mut self, delta: isize) {
        let rows = self.edit_menu_rows();
        let count = rows
            .iter()
            .filter(|row| matches!(row, EditMenuRow::Item { .. }))
            .count();
        if count == 0 {
            self.edit_menu_selection = 0;
            return;
        }
        self.edit_menu_selection = self
            .edit_menu_selection
            .saturating_add_signed(delta)
            .min(count - 1);
    }

    pub(crate) fn edit_menu_selected(&self) -> Option<(EditMenuCommand, bool)> {
        self.edit_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                EditMenuRow::Item {
                    command, enabled, ..
                } => Some((command, enabled)),
                EditMenuRow::Separator => None,
            })
            .nth(self.edit_menu_selection)
    }

    pub(crate) fn edit_menu_pointer_selection(&mut self, y: f32, popup_y: f32) {
        let mut row_y = popup_y + 4.0;
        let mut index = 0;
        for row in self.edit_menu_rows() {
            let height = Self::edit_menu_row_height(row);
            if y >= row_y && y < row_y + height {
                if matches!(row, EditMenuRow::Item { .. }) {
                    self.edit_menu_selection = index;
                }
                return;
            }
            row_y += height;
            if matches!(row, EditMenuRow::Item { .. }) {
                index += 1;
            }
        }
    }

    pub(crate) fn activate_edit_menu_accessibility_item(&mut self, index: usize) {
        if let Some((command, true)) = self
            .edit_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                EditMenuRow::Item {
                    command, enabled, ..
                } => Some((command, enabled)),
                EditMenuRow::Separator => None,
            })
            .nth(index)
        {
            self.execute_edit_menu_command(command);
            self.close_workbench_menu();
        }
    }

    pub(crate) fn execute_edit_menu_command(&mut self, command: EditMenuCommand) {
        use EditMenuCommand as C;
        match command {
            C::Undo => self.run_ui_command("undo"),
            C::Redo => self.run_ui_command("redo"),
            C::Cut => self.run_ui_command("editor.action.clipboardCutAction"),
            C::Copy => self.run_ui_command("editor.action.clipboardCopyAction"),
            C::Paste => self.run_ui_command("editor.action.clipboardPasteAction"),
            C::Find => self.run_ui_command("actions.find"),
            C::Replace => self.run_ui_command("editor.action.startFindReplaceAction"),
            C::FindInFiles => self.open_project_search_mode(false),
            C::ReplaceInFiles => self.open_project_search_mode(true),
            C::ToggleLineComment => self.toggle_line_comment(),
            C::ToggleBlockComment => self.toggle_block_comment(),
            C::EmmetExpand => self.run_ui_command("editor.emmet.action.expandAbbreviation"),
        }
    }

    pub(crate) fn edit_menu_label(&self, key: &'static str) -> String {
        self.t(key).to_owned()
    }

    pub(crate) fn edit_menu_row_height(row: EditMenuRow) -> f32 {
        match row {
            EditMenuRow::Separator => SEPARATOR_HEIGHT,
            EditMenuRow::Item { .. } => ROW_HEIGHT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_menu_has_reference_groups_and_disables_unavailable_editor_actions() {
        let temp = crate::test_support::TempDir::new();
        let app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let rows = app.edit_menu_rows();
        let commands = rows
            .iter()
            .filter_map(|row| match row {
                EditMenuRow::Item { command, .. } => Some(*command),
                EditMenuRow::Separator => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            commands,
            [
                EditMenuCommand::Undo,
                EditMenuCommand::Redo,
                EditMenuCommand::Cut,
                EditMenuCommand::Copy,
                EditMenuCommand::Paste,
                EditMenuCommand::Find,
                EditMenuCommand::Replace,
                EditMenuCommand::FindInFiles,
                EditMenuCommand::ReplaceInFiles,
                EditMenuCommand::ToggleLineComment,
                EditMenuCommand::ToggleBlockComment,
                EditMenuCommand::EmmetExpand,
            ]
        );
        assert!(matches!(rows[0], EditMenuRow::Item { enabled: false, .. }));
        assert!(matches!(rows[3], EditMenuRow::Item { enabled: false, .. }));
        assert!(matches!(rows[4], EditMenuRow::Item { enabled: false, .. }));
        assert!(matches!(rows[5], EditMenuRow::Item { enabled: true, .. }));
        assert!(
            rows.iter()
                .filter(|row| matches!(row, EditMenuRow::Separator))
                .count()
                >= 4
        );
    }

    #[test]
    fn edit_menu_hit_testing_uses_the_drawn_row_layout() {
        let temp = crate::test_support::TempDir::new();
        let app = App::new_with_options(temp.path().to_path_buf(), None, true);
        assert_eq!(
            app.edit_menu_command_at(140.0, 40.0, 100.0, 30.0),
            Some((EditMenuCommand::Undo, false))
        );
        assert_eq!(app.edit_menu_command_at(99.0, 40.0, 100.0, 30.0), None);
        assert_eq!(app.edit_menu_command_at(140.0, 90.0, 100.0, 30.0), None);
    }
}
