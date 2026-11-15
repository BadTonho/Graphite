//! Keyboard navigation of the explorer tree: moving the selection, expanding and collapsing
//! folders, and opening the selected file.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExplorerMove {
    Up,
    Down,
    PageUp,
    PageDown,
    First,
    Last,
}

impl App {
    /// How many explorer rows fit in the side bar.
    fn explorer_page(&self) -> usize {
        let Some((height, line_height)) = self
            .gpu
            .as_ref()
            .map(|gpu| (gpu.renderer.size().1 as f32, gpu.renderer.line_height()))
        else {
            return 20;
        };
        let row = line_height;
        (((height - row * 1.5) / row) as usize).max(1)
    }

    fn explorer_selected_row(&self) -> Option<usize> {
        let selected = self.explorer_selected.as_ref()?;
        self.tree
            .rows
            .iter()
            .position(|row| &row.entry.path == selected)
    }

    /// Selects a row and scrolls the list just enough to show it.
    fn explorer_select_row(&mut self, index: usize) {
        let Some(path) = self.tree.rows.get(index).map(|row| row.entry.path.clone()) else {
            return;
        };
        self.explorer_select(path);
        let page = self.explorer_page();
        if index < self.explorer_scroll {
            self.explorer_scroll = index;
        } else if index >= self.explorer_scroll + page {
            self.explorer_scroll = index + 1 - page;
        }
    }

    /// Moves the selection. With nothing selected, the first move selects the first row.
    pub(crate) fn explorer_move(&mut self, movement: ExplorerMove) {
        let count = self.tree.rows.len();
        if count == 0 {
            return;
        }
        let page = self.explorer_page();
        let Some(current) = self.explorer_selected_row() else {
            self.explorer_select_row(0);
            return;
        };
        let target = match movement {
            ExplorerMove::Up => current.saturating_sub(1),
            ExplorerMove::Down => (current + 1).min(count - 1),
            ExplorerMove::PageUp => current.saturating_sub(page),
            ExplorerMove::PageDown => (current + page).min(count - 1),
            ExplorerMove::First => 0,
            ExplorerMove::Last => count - 1,
        };
        self.explorer_select_row(target);
    }

    /// Expands the selected folder; on an expanded folder moves to its first entry.
    pub(crate) fn explorer_expand(&mut self) {
        let Some(index) = self.explorer_selected_row() else {
            return;
        };
        let row = &self.tree.rows[index];
        if !row.entry.is_dir {
            return;
        }
        if !row.expanded {
            let path = row.entry.path.clone();
            self.tree.set_expanded(&path, true);
        } else if self
            .tree
            .rows
            .get(index + 1)
            .is_some_and(|next| next.depth > row.depth)
        {
            self.explorer_select_row(index + 1);
        }
    }

    /// Collapses the selected folder; anywhere else moves to the parent folder.
    pub(crate) fn explorer_collapse(&mut self) {
        let Some(index) = self.explorer_selected_row() else {
            return;
        };
        let row = &self.tree.rows[index];
        if row.entry.is_dir && row.expanded {
            let path = row.entry.path.clone();
            self.tree.set_expanded(&path, false);
        } else {
            self.explorer_focus_parent();
        }
    }

    pub(crate) fn explorer_focus_parent(&mut self) {
        let Some(index) = self.explorer_selected_row() else {
            return;
        };
        let depth = self.tree.rows[index].depth;
        if let Some(parent) = (0..index)
            .rev()
            .find(|&candidate| self.tree.rows[candidate].depth < depth)
        {
            self.explorer_select_row(parent);
        }
    }

    pub(crate) fn explorer_collapse_all(&mut self) {
        self.tree.collapse_all();
        self.explorer_scroll = 0;
        if let Some(selected) = self.explorer_selected.clone() {
            if self.explorer_selected_row().is_none() {
                // The selection was inside a folder that closed: keep the nearest visible folder.
                let root = self
                    .tree
                    .rows
                    .iter()
                    .position(|row| selected.starts_with(&row.entry.path));
                match root {
                    Some(index) => self.explorer_select_row(index),
                    None => self.explorer_selected = None,
                }
            }
        }
    }

    /// Opens the selected file as a preview tab, or toggles the selected folder.
    pub(crate) fn explorer_activate_selected(&mut self) {
        let Some(index) = self.explorer_selected_row() else {
            return;
        };
        if let Some(path) = self.tree.activate(index) {
            self.open_file_preview(path);
        }
    }

    /// Toggles the selected folder without opening a file.
    pub(crate) fn explorer_activate_selected_folder(&mut self) {
        let Some(index) = self.explorer_selected_row() else {
            return;
        };
        if self.tree.rows[index].entry.is_dir {
            self.tree.activate(index);
        }
    }

    pub(crate) fn explorer_scroll_by(&mut self, rows: isize) {
        let last = self.explorer_row_count().saturating_sub(1);
        self.explorer_scroll = self.explorer_scroll.saturating_add_signed(rows).min(last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    /// `dir/` holds `inner.txt`; there are also `file_00.txt` to `file_29.txt` beside it.
    fn app() -> (App, TempDir) {
        let dir = TempDir::new();
        std::fs::create_dir(dir.path().join("dir")).unwrap();
        std::fs::write(dir.path().join("dir/inner.txt"), "inner").unwrap();
        for index in 0..30 {
            std::fs::write(dir.path().join(format!("file_{index:02}.txt")), "x").unwrap();
        }
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.tree.refresh();
        (app, dir)
    }

    fn selected_name(app: &App) -> String {
        app.explorer_selected
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    #[test]
    fn the_first_move_selects_the_first_row_and_moves_stop_at_the_ends() {
        let (mut app, _dir) = app();
        app.run_ui_command("list.focusDown");
        assert_eq!(selected_name(&app), "dir");
        assert!(app.explorer_focused);
        app.run_ui_command("list.focusUp");
        assert_eq!(selected_name(&app), "dir", "already first");
        app.run_ui_command("list.focusDown");
        assert_eq!(selected_name(&app), "file_00.txt");
        app.run_ui_command("list.focusLast");
        assert_eq!(selected_name(&app), "file_29.txt");
        app.run_ui_command("list.focusDown");
        assert_eq!(selected_name(&app), "file_29.txt", "already last");
        app.run_ui_command("list.focusFirst");
        assert_eq!(selected_name(&app), "dir");
    }

    #[test]
    fn paging_moves_by_a_screenful_and_keeps_the_selection_in_view() {
        let (mut app, _dir) = app();
        app.run_ui_command("list.focusFirst");
        app.run_ui_command("list.focusPageDown");
        let page = app.explorer_page();
        assert_eq!(app.explorer_selected_row(), Some(page));
        let row = app.explorer_selected_row().unwrap();
        assert!(row >= app.explorer_scroll && row < app.explorer_scroll + page);
        app.run_ui_command("list.focusLast");
        let last = app.explorer_selected_row().unwrap();
        assert!(last >= app.explorer_scroll && last < app.explorer_scroll + page);
        app.run_ui_command("list.focusPageUp");
        let row = app.explorer_selected_row().unwrap();
        assert_eq!(row, last - page);
        assert!(row >= app.explorer_scroll && row < app.explorer_scroll + page);
    }

    #[test]
    fn expand_and_collapse_walk_into_and_out_of_a_folder() {
        let (mut app, _dir) = app();
        app.run_ui_command("list.focusFirst");
        let rows_before = app.tree.rows.len();
        app.run_ui_command("list.expand");
        assert_eq!(app.tree.rows.len(), rows_before + 1);
        app.run_ui_command("list.expand");
        assert_eq!(
            selected_name(&app),
            "inner.txt",
            "an expanded folder moves inside"
        );
        app.run_ui_command("list.collapse");
        assert_eq!(selected_name(&app), "dir", "a file moves to its folder");
        app.run_ui_command("list.collapse");
        assert_eq!(app.tree.rows.len(), rows_before);
        app.run_ui_command("list.expand");
        app.run_ui_command("list.focusDown");
        app.run_ui_command("list.focusParent");
        assert_eq!(selected_name(&app), "dir");
    }

    #[test]
    fn select_toggles_a_folder_and_opens_a_file() {
        let (mut app, _dir) = app();
        app.run_ui_command("list.focusFirst");
        let before = app.tree.rows.len();
        app.run_ui_command("list.select");
        assert_eq!(app.tree.rows.len(), before + 1);
        app.run_ui_command("list.toggleExpand");
        assert_eq!(app.tree.rows.len(), before);
        app.run_ui_command("list.focusDown");
        app.run_ui_command("list.select");
        let path = app.documents[app.active_document()]
            .buffer
            .path()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned());
        assert_eq!(path.as_deref(), Some("file_00.txt"));
    }

    #[test]
    fn collapse_all_closes_every_folder_and_fixes_the_selection() {
        let (mut app, _dir) = app();
        app.run_ui_command("list.focusFirst");
        app.run_ui_command("list.expand");
        app.run_ui_command("list.focusDown");
        assert_eq!(selected_name(&app), "inner.txt");
        app.run_ui_command("list.collapseAll");
        assert_eq!(selected_name(&app), "dir");
        assert!(!app
            .tree
            .rows
            .iter()
            .any(|row| row.entry.name == "inner.txt"));
    }

    #[test]
    fn the_up_and_down_commands_drive_quick_open_while_it_is_open() {
        let (mut app, dir) = app();
        app.quick_open = Some(crate::app::quick_open::QuickOpen {
            query: String::new(),
            paths: vec![
                dir.path().join("file_00.txt"),
                dir.path().join("file_01.txt"),
            ],
            index: 0,
        });
        app.run_ui_command("list.focusDown");
        assert_eq!(app.quick_open.as_ref().unwrap().index, 1);
        app.run_ui_command("list.focusUp");
        assert_eq!(app.quick_open.as_ref().unwrap().index, 0);
        assert!(
            app.explorer_selected.is_none(),
            "the explorer was not touched"
        );
    }

    #[test]
    fn the_arrow_keys_work_when_the_explorer_has_the_focus() {
        let (mut app, _dir) = app();
        app.explorer_focused = true;
        app.refresh_command_context();
        assert!(app.handle_keybinding_stroke("down"));
        assert_eq!(selected_name(&app), "dir");
        assert!(app.handle_keybinding_stroke("right"));
        assert!(app
            .tree
            .rows
            .iter()
            .any(|row| row.entry.name == "inner.txt"));
        assert!(app.handle_keybinding_stroke("left"));
        assert!(!app
            .tree
            .rows
            .iter()
            .any(|row| row.entry.name == "inner.txt"));
        assert!(app.handle_keybinding_stroke("end"));
        assert_eq!(selected_name(&app), "file_29.txt");
    }

    #[test]
    fn list_multi_selection_and_type_to_find_work_in_explorer() {
        let (mut app, _dir) = app();
        app.run_ui_command("list.focusFirst");
        app.run_ui_command("list.expandSelectionDown");
        assert_eq!(app.explorer_multi_selected.len(), 2);

        app.run_ui_command("list.selectAll");
        assert_eq!(app.explorer_multi_selected.len(), app.tree.rows.len());

        app.run_ui_command("list.clear");
        assert!(app.explorer_multi_selected.is_empty());

        app.explorer_type_find("file_15");
        assert_eq!(selected_name(&app), "file_15.txt");
    }
}

impl App {
    pub(crate) fn explorer_select_all(&mut self) {
        self.explorer_multi_selected = self
            .tree
            .rows
            .iter()
            .map(|row| row.entry.path.clone())
            .collect();
    }

    pub(crate) fn explorer_clear_selection(&mut self) {
        self.explorer_multi_selected.clear();
        self.explorer_type_filter.clear();
    }

    pub(crate) fn explorer_expand_selection(&mut self, down: bool) {
        if let Some(current) = self.explorer_selected.clone() {
            self.explorer_multi_selected.insert(current);
        }
        self.explorer_move(if down {
            ExplorerMove::Down
        } else {
            ExplorerMove::Up
        });
        if let Some(next) = self.explorer_selected.clone() {
            self.explorer_multi_selected.insert(next);
        }
    }

    pub(crate) fn explorer_type_find(&mut self, query: &str) {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return;
        }
        self.explorer_type_filter = trimmed.to_owned();
        let needle = trimmed.to_ascii_lowercase();
        let start = self
            .explorer_selected_row()
            .map_or(0, |index| (index + 1) % self.tree.rows.len().max(1));
        for offset in 0..self.tree.rows.len() {
            let index = (start + offset) % self.tree.rows.len();
            if self.tree.rows[index]
                .entry
                .name
                .to_ascii_lowercase()
                .contains(&needle)
            {
                self.explorer_select_row(index);
                return;
            }
        }
    }
}
