use super::go_menu::GoPickerKind;
use super::*;
use winit::keyboard::{Key, NamedKey};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CommandCenterMode {
    #[default]
    Files,
    Commands,
    WorkspaceSymbols,
    EditorSymbols,
    LineColumn,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CommandCenter {
    pub query: String,
    pub mode: CommandCenterMode,
}

impl CommandCenter {
    fn parsed_query(query: &str) -> (CommandCenterMode, &str) {
        match query.chars().next() {
            Some('>') => (CommandCenterMode::Commands, query[1..].trim_start()),
            Some('#') => (CommandCenterMode::WorkspaceSymbols, query[1..].trim_start()),
            Some('@') => (CommandCenterMode::EditorSymbols, query[1..].trim_start()),
            Some(':') => (CommandCenterMode::LineColumn, query[1..].trim_start()),
            _ => (CommandCenterMode::Files, query),
        }
    }
}

impl App {
    pub(crate) fn open_command_center(&mut self) {
        if self.command_center.is_some() {
            return;
        }
        self.close_workbench_menu();
        self.profile_menu = None;
        self.settings_menu = None;
        self.go_picker = None;
        self.command_palette = None;
        self.quick_open = None;
        self.command_center = Some(CommandCenter::default());
        self.update_command_center_mode();
    }

    pub(crate) fn close_command_center(&mut self) {
        let Some(center) = self.command_center.take() else {
            return;
        };
        match center.mode {
            CommandCenterMode::Files => {
                self.quick_open = None;
                self.search_generation = self.search_generation.wrapping_add(1);
                self.search_cancel
                    .store(self.search_generation, Ordering::Relaxed);
            }
            CommandCenterMode::Commands => self.command_palette = None,
            CommandCenterMode::WorkspaceSymbols
            | CommandCenterMode::EditorSymbols
            | CommandCenterMode::LineColumn => {
                self.go_picker = None;
                self.go_picker_generation = self.go_picker_generation.wrapping_add(1);
            }
        }
    }

    fn update_command_center_mode(&mut self) {
        let Some(center) = self.command_center.as_ref() else {
            return;
        };
        let query = center.query.clone();
        let (mode, inner_query) = CommandCenter::parsed_query(&query);
        let inner_query = inner_query.to_owned();
        let changed = center.mode != mode;
        if changed {
            self.quick_open = None;
            self.command_palette = None;
            self.go_picker = None;
            if mode == CommandCenterMode::Files {
                self.search_generation = self.search_generation.wrapping_add(1);
                self.search_cancel
                    .store(self.search_generation, Ordering::Relaxed);
            }
            if mode == CommandCenterMode::WorkspaceSymbols
                || mode == CommandCenterMode::EditorSymbols
                || mode == CommandCenterMode::LineColumn
            {
                self.go_picker_generation = self.go_picker_generation.wrapping_add(1);
            }
        }
        if let Some(center) = self.command_center.as_mut() {
            center.mode = mode;
        }
        match mode {
            CommandCenterMode::Files => {
                if self.quick_open.is_none() {
                    self.start_quick_open();
                }
                if let Some(quick) = self.quick_open.as_mut() {
                    if quick.query != inner_query {
                        quick.query = inner_query;
                        quick.index = 0;
                        self.request_quick_open();
                    }
                }
            }
            CommandCenterMode::Commands => {
                if self.command_palette.is_none() {
                    self.open_command_palette();
                }
                if let Some(palette) = self.command_palette.as_mut() {
                    palette.query = inner_query;
                    palette.selected = 0;
                }
            }
            CommandCenterMode::WorkspaceSymbols
            | CommandCenterMode::EditorSymbols
            | CommandCenterMode::LineColumn => {
                let kind = match mode {
                    CommandCenterMode::WorkspaceSymbols => GoPickerKind::WorkspaceSymbols,
                    CommandCenterMode::EditorSymbols => GoPickerKind::EditorSymbols,
                    CommandCenterMode::LineColumn => GoPickerKind::LineColumn,
                    _ => unreachable!(),
                };
                if self
                    .go_picker
                    .as_ref()
                    .is_none_or(|picker| picker.kind != kind)
                {
                    self.open_go_picker(kind);
                }
                if let Some(picker) = self.go_picker.as_mut() {
                    picker.query = inner_query;
                    picker.selected = 0;
                }
            }
        }
    }

    pub(crate) fn handle_command_center_key(&mut self, key: &Key, text: Option<&str>) -> bool {
        if self.command_center.is_none() {
            return false;
        }
        if self.modifiers.control_key() || self.modifiers.alt_key() {
            self.close_command_center();
            return false;
        }
        match key {
            Key::Named(NamedKey::Escape) => self.close_command_center(),
            Key::Named(NamedKey::Backspace) => {
                if let Some(center) = self.command_center.as_mut() {
                    center.query.pop();
                }
                self.update_command_center_mode();
            }
            Key::Named(NamedKey::ArrowDown) => self.command_center_move_selection(1),
            Key::Named(NamedKey::ArrowUp) => self.command_center_move_selection(-1),
            Key::Named(NamedKey::Enter) => self.command_center_accept(),
            _ => {
                let Some(text) = text.filter(|text| !text.chars().any(char::is_control)) else {
                    return true;
                };
                if let Some(center) = self.command_center.as_mut() {
                    center.query.push_str(text);
                }
                self.update_command_center_mode();
            }
        }
        true
    }

    pub(crate) fn command_center_ime_commit(&mut self, text: &str) {
        if let Some(center) = self.command_center.as_mut() {
            center.query.push_str(text);
        }
        self.update_command_center_mode();
    }

    fn command_center_move_selection(&mut self, delta: isize) {
        let Some(mode) = self.command_center.as_ref().map(|center| center.mode) else {
            return;
        };
        match mode {
            CommandCenterMode::Files => {
                if let Some(quick) = self.quick_open.as_mut() {
                    let count = quick.paths.len();
                    if count > 0 {
                        quick.index = if delta < 0 {
                            (quick.index + count - 1) % count
                        } else {
                            (quick.index + 1) % count
                        };
                    }
                }
            }
            CommandCenterMode::Commands => {
                if let Some(palette) = self.command_palette.as_mut() {
                    let count = palette
                        .items
                        .iter()
                        .filter(|item| command_palette::matches(item, &palette.query))
                        .count();
                    if count > 0 {
                        palette.selected = if delta < 0 {
                            (palette.selected + count - 1) % count
                        } else {
                            (palette.selected + 1) % count
                        };
                    }
                }
            }
            CommandCenterMode::WorkspaceSymbols
            | CommandCenterMode::EditorSymbols
            | CommandCenterMode::LineColumn => {
                let count = self.go_picker_rows().len();
                if let Some(picker) = self.go_picker.as_mut() {
                    if count > 0 {
                        picker.selected = if delta < 0 {
                            (picker.selected + count - 1) % count
                        } else {
                            (picker.selected + 1) % count
                        };
                    }
                }
            }
        }
    }

    pub(crate) fn command_center_accept(&mut self) {
        let Some(mode) = self.command_center.as_ref().map(|center| center.mode) else {
            return;
        };
        match mode {
            CommandCenterMode::Files => {
                self.command_center = None;
                self.execute_quick_open_command("quickOpen.accept");
            }
            CommandCenterMode::Commands => {
                self.command_center = None;
                self.execute_command_palette_selection();
            }
            CommandCenterMode::WorkspaceSymbols
            | CommandCenterMode::EditorSymbols
            | CommandCenterMode::LineColumn => {
                self.command_center = None;
                self.accept_go_picker();
            }
        }
    }

    pub(crate) fn command_center_rows(&self) -> Vec<String> {
        if self.command_center.is_none() {
            return Vec::new();
        }
        self.quick_input_model()
            .map(|model| model.rows.into_iter().map(|row| row.accessible).collect())
            .unwrap_or_default()
    }

    pub(crate) fn command_center_selected_index(&self) -> Option<usize> {
        self.command_center.as_ref()?;
        self.quick_input_model()?.selected
    }

    pub(crate) fn command_center_activate_index(&mut self, index: usize) {
        if self.command_center.is_some() {
            self.quick_input_accept_row(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_select_access_modes_and_keep_the_query() {
        assert_eq!(
            CommandCenter::parsed_query("src/main"),
            (CommandCenterMode::Files, "src/main")
        );
        assert_eq!(
            CommandCenter::parsed_query("> save"),
            (CommandCenterMode::Commands, "save")
        );
        assert_eq!(
            CommandCenter::parsed_query("# Widget"),
            (CommandCenterMode::WorkspaceSymbols, "Widget")
        );
        assert_eq!(
            CommandCenter::parsed_query("@main"),
            (CommandCenterMode::EditorSymbols, "main")
        );
        assert_eq!(
            CommandCenter::parsed_query(":12:4"),
            (CommandCenterMode::LineColumn, "12:4")
        );
    }

    #[test]
    fn opening_shows_file_suggestions_and_escape_cancels_them() {
        let dir = crate::test_support::TempDir::new();
        std::fs::write(dir.path().join("hello.rs"), "fn main() {}\n").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_command_center();
        assert_eq!(
            app.command_center.as_ref().unwrap().mode,
            CommandCenterMode::Files
        );
        app.quick_open.as_mut().unwrap().paths = vec![dir.path().join("hello.rs")];
        assert!(app.command_center_rows()[0].ends_with("hello.rs"));
        assert!(app.handle_command_center_key(&Key::Named(NamedKey::Escape), None));
        assert!(app.command_center.is_none());
        assert!(app.quick_open.is_none());
    }

    #[test]
    fn typing_prefix_switches_to_commands_and_enter_executes_selection() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_command_center();
        app.handle_command_center_key(&Key::Character(">".into()), Some(">"));
        assert_eq!(
            app.command_center.as_ref().unwrap().mode,
            CommandCenterMode::Commands
        );
        assert!(app.command_palette.is_some());
    }

    #[test]
    fn symbol_and_line_prefixes_reuse_existing_go_picker_modes() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_command_center();
        app.handle_command_center_key(&Key::Character("#".into()), Some("#"));
        assert_eq!(
            app.command_center.as_ref().unwrap().mode,
            CommandCenterMode::WorkspaceSymbols
        );
        assert_eq!(
            app.go_picker.as_ref().unwrap().kind,
            GoPickerKind::WorkspaceSymbols
        );
        app.handle_command_center_key(&Key::Named(NamedKey::Backspace), None);
        app.handle_command_center_key(&Key::Character("@".into()), Some("@"));
        assert_eq!(
            app.command_center.as_ref().unwrap().mode,
            CommandCenterMode::EditorSymbols
        );
        assert_eq!(
            app.go_picker.as_ref().unwrap().kind,
            GoPickerKind::EditorSymbols
        );
        app.handle_command_center_key(&Key::Named(NamedKey::Backspace), None);
        app.handle_command_center_key(&Key::Character(":".into()), Some(":"));
        assert_eq!(
            app.command_center.as_ref().unwrap().mode,
            CommandCenterMode::LineColumn
        );
        assert_eq!(
            app.go_picker.as_ref().unwrap().kind,
            GoPickerKind::LineColumn
        );
    }

    #[test]
    fn navigation_buttons_follow_history_availability() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(!app.can_navigate_history(-1));
        assert!(!app.can_navigate_history(1));
        app.push_navigation_location();
        app.new_untitled_document();
        app.push_navigation_location();
        assert!(app.can_navigate_history(-1));
        assert!(!app.can_navigate_history(1));
        app.navigate_history(-1);
        assert!(!app.can_navigate_history(-1));
        assert!(app.can_navigate_history(1));
    }
}
