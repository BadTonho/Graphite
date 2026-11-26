use super::settings_menu::{SettingsSubcommand, SettingsSubmenu, SettingsSubmenuRow};
use super::*;

const ROW_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
pub(crate) const FILE_MENU_WIDTH: f32 = 420.0;
pub(crate) const FILE_SUBMENU_WIDTH: f32 = 360.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileMenuCommand {
    NewTextFile,
    NewFile,
    NewWindow,
    NewWindowProfile,
    OpenFile,
    OpenFolder,
    OpenWorkspace,
    OpenRecent,
    AddFolder,
    RemoveFolder,
    SaveWorkspaceAs,
    DuplicateWorkspace,
    Save,
    SaveAs,
    SaveAll,
    AutoSave,
    Preferences,
    Revert,
    CloseEditor,
    CloseWindow,
    Exit,
    Profiles,
    Settings,
    Extensions,
    KeyboardShortcuts,
    ConfigureSnippets,
    Tasks,
    Themes,
    DarkTheme,
    LightTheme,
    RecentPath(usize),
    SwitchProfile(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileMenuSubmenu {
    NewWindowProfile,
    OpenRecent,
    Preferences,
    Profiles,
    Themes,
}

impl FileMenuSubmenu {
    /// The submenu this one opens from, when it is not opened from the main menu.
    pub(crate) fn parent(self) -> Option<FileMenuSubmenu> {
        match self {
            Self::Profiles | Self::Themes => Some(Self::Preferences),
            Self::NewWindowProfile | Self::OpenRecent | Self::Preferences => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FileMenuRow {
    Separator,
    Item {
        command: FileMenuCommand,
        label: &'static str,
        shortcut: &'static str,
        submenu: Option<FileMenuSubmenu>,
        enabled: bool,
        checked: bool,
    },
}

impl App {
    pub(crate) fn new_untitled_document(&mut self) {
        let document = Document {
            buffer: Buffer::new(),
            language: None,
            highlights: None,
            sticky_headers_cache: None,
            highlighter: IncrementalHighlighter::new(),
            bracket_depths: Default::default(),
            diagnostics: Vec::new(),
            inlay_hints: Vec::new(),
            inlay_revision: None,
            provided_folds: None,
            schema_diagnostics: Vec::new(),
            schema_revision: None,
            lsp_revision: None,
            indentation: graphite_text::Indentation {
                insert_spaces: self.insert_spaces,
                tab_size: self.tab_size,
            },
            saved_fingerprint: None,
            recovery_label: None,
            recovery_pending_save: false,
            saved_format: (
                crate::file_format::Encoding::Utf8,
                crate::file_format::LineEnding::Lf,
            ),
            encoding: crate::file_format::Encoding::Utf8,
            line_ending: crate::file_format::LineEnding::Lf,
            notebook: None,
            custom_dirty: false,
        };
        let id = self.documents.push(document);
        self.activate_editor_document(id);
        self.status = self.t("file.new_text_created").into();
    }

    pub(crate) fn file_menu_rows(&self, submenu: Option<FileMenuSubmenu>) -> Vec<FileMenuRow> {
        use FileMenuCommand as C;
        use FileMenuSubmenu as S;
        let row = |command, label, shortcut, submenu, enabled, checked| FileMenuRow::Item {
            command,
            label,
            shortcut,
            submenu,
            enabled,
            checked,
        };
        match submenu {
            None => vec![
                row(C::NewTextFile, "file.new_text", "Ctrl+N", None, true, false),
                row(C::NewFile, "file.new_file", "", None, true, false),
                FileMenuRow::Separator,
                row(
                    C::NewWindow,
                    "file.new_window",
                    "Ctrl+Shift+N",
                    None,
                    true,
                    false,
                ),
                row(
                    C::NewWindowProfile,
                    "file.new_window_profile",
                    "",
                    Some(S::NewWindowProfile),
                    true,
                    false,
                ),
                FileMenuRow::Separator,
                row(C::OpenFile, "file.open_file", "Ctrl+O", None, true, false),
                row(
                    C::OpenFolder,
                    "file.open_folder",
                    "Ctrl+K Ctrl+O",
                    None,
                    true,
                    false,
                ),
                row(
                    C::OpenWorkspace,
                    "file.open_workspace",
                    "",
                    None,
                    true,
                    false,
                ),
                row(
                    C::OpenRecent,
                    "file.open_recent",
                    "",
                    Some(S::OpenRecent),
                    true,
                    false,
                ),
                FileMenuRow::Separator,
                row(C::AddFolder, "file.add_folder", "", None, true, false),
                row(
                    C::RemoveFolder,
                    "file.remove_folder",
                    "",
                    None,
                    self.tree.roots().len() > 1,
                    false,
                ),
                row(
                    C::SaveWorkspaceAs,
                    "file.save_workspace_as",
                    "",
                    None,
                    true,
                    false,
                ),
                row(
                    C::DuplicateWorkspace,
                    "file.duplicate_workspace",
                    "",
                    None,
                    true,
                    false,
                ),
                FileMenuRow::Separator,
                row(C::Save, "file.save", "Ctrl+S", None, true, false),
                row(C::SaveAs, "file.save_as", "Ctrl+Shift+S", None, true, false),
                row(C::SaveAll, "file.save_all", "Ctrl+K S", None, true, false),
                FileMenuRow::Separator,
                row(
                    C::AutoSave,
                    "file.auto_save",
                    "",
                    None,
                    true,
                    self.auto_save,
                ),
                row(
                    C::Preferences,
                    "file.preferences",
                    "",
                    Some(S::Preferences),
                    true,
                    false,
                ),
                FileMenuRow::Separator,
                row(
                    C::Revert,
                    "file.revert",
                    "",
                    None,
                    self.documents[self.active_document()]
                        .buffer
                        .path()
                        .is_some(),
                    false,
                ),
                row(
                    C::CloseEditor,
                    "file.close_editor",
                    "Ctrl+F4",
                    None,
                    true,
                    false,
                ),
                row(
                    C::CloseWindow,
                    "file.close_window",
                    "Alt+F4",
                    None,
                    true,
                    false,
                ),
                FileMenuRow::Separator,
                row(C::Exit, "file.exit", "", None, true, false),
            ],
            Some(S::Preferences) => vec![
                row(
                    C::Profiles,
                    "file.profiles",
                    "",
                    Some(S::Profiles),
                    true,
                    false,
                ),
                row(C::Settings, "file.settings", "", None, true, false),
                row(
                    C::Extensions,
                    "file.extensions",
                    "Ctrl+Shift+X",
                    None,
                    true,
                    false,
                ),
                row(
                    C::KeyboardShortcuts,
                    "file.keyboard_shortcuts",
                    "Ctrl+K Ctrl+S",
                    None,
                    true,
                    false,
                ),
                row(
                    C::ConfigureSnippets,
                    "file.configure_snippets",
                    "",
                    None,
                    true,
                    false,
                ),
                row(C::Tasks, "file.tasks", "", None, true, false),
                row(C::Themes, "file.themes", "", Some(S::Themes), true, false),
            ],
            Some(S::Profiles) => {
                let profiles = self.user_profiles().unwrap_or_default();
                let mut rows = profiles
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        row(
                            C::SwitchProfile(index),
                            "file.profile_entry",
                            "",
                            None,
                            true,
                            false,
                        )
                    })
                    .collect::<Vec<_>>();
                rows.push(FileMenuRow::Separator);
                rows.push(row(
                    C::Profiles,
                    "file.create_profile",
                    "",
                    None,
                    true,
                    false,
                ));
                rows
            }
            Some(S::Themes) => vec![
                row(
                    C::DarkTheme,
                    "file.theme_dark",
                    "",
                    None,
                    true,
                    DARK_THEME.load(Ordering::Relaxed),
                ),
                row(
                    C::LightTheme,
                    "file.theme_light",
                    "",
                    None,
                    true,
                    !DARK_THEME.load(Ordering::Relaxed),
                ),
            ],
            Some(S::NewWindowProfile) => self
                .user_profiles()
                .unwrap_or_default()
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    row(
                        C::SwitchProfile(index),
                        "file.new_window_profile_entry",
                        "",
                        None,
                        true,
                        false,
                    )
                })
                .collect(),
            Some(S::OpenRecent) => self
                .recent_workspaces
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    row(
                        C::RecentPath(index),
                        "file.recent_workspace_entry",
                        "",
                        None,
                        true,
                        false,
                    )
                })
                .collect(),
        }
    }

    pub(crate) fn file_menu_row_height(row: FileMenuRow) -> f32 {
        if matches!(row, FileMenuRow::Separator) {
            SEPARATOR_HEIGHT
        } else {
            ROW_HEIGHT
        }
    }

    /// The innermost open submenu (the nested one when there is one).
    pub(crate) fn file_menu_innermost(&self) -> Option<FileMenuSubmenu> {
        self.file_menu_nested.or(self.file_menu_submenu)
    }

    /// The panel that keyboard navigation acts on and the highlighted row inside it.
    fn file_menu_focus(&self) -> (Option<FileMenuSubmenu>, usize) {
        match self.file_menu_nested {
            Some(nested) => (Some(nested), self.file_menu_nested_selection),
            None => (self.file_menu_submenu, self.file_menu_selection),
        }
    }

    /// Opens `child` beside its row: nested inside the open submenu when it belongs to it,
    /// otherwise as the first-level submenu.
    pub(crate) fn open_file_submenu(&mut self, child: FileMenuSubmenu) {
        if child.parent().is_some() && child.parent() == self.file_menu_submenu {
            self.file_menu_nested = Some(child);
            self.file_menu_nested_selection = 0;
        } else {
            self.file_menu_submenu = Some(child);
            self.file_menu_nested = None;
            self.file_menu_selection = 0;
        }
    }

    /// Closes the innermost submenu and reports whether one was open.
    pub(crate) fn close_file_submenu(&mut self) -> bool {
        if self.file_menu_nested.take().is_some() {
            true
        } else if self.file_menu_submenu.take().is_some() {
            self.file_menu_selection = 0;
            true
        } else {
            false
        }
    }

    /// Horizontal positions of the first-level and nested submenus: `(x, nested x, width)`.
    /// Each opens to the right of its parent, or to the left when there is no room.
    pub(crate) fn file_panel_xs(
        popup_x: f32,
        popup_width: f32,
        window_width: f32,
    ) -> (f32, f32, f32) {
        let width = FILE_SUBMENU_WIDTH.min(window_width);
        let opened_right = popup_x + popup_width + width <= window_width;
        let submenu_x = if opened_right {
            popup_x + popup_width
        } else {
            (popup_x - width).max(0.0)
        };
        let nested_x = if opened_right {
            if submenu_x + width * 2.0 <= window_width {
                submenu_x + width
            } else {
                (submenu_x - width).max(0.0)
            }
        } else if submenu_x >= width {
            submenu_x - width
        } else {
            submenu_x + width
        };
        (submenu_x, nested_x, width)
    }

    pub(crate) fn file_menu_move_selection(&mut self, direction: isize) {
        let (panel, selected) = self.file_menu_focus();
        let count = match self.file_menu_nested {
            Some(nested) => self
                .file_nested_rows(nested)
                .iter()
                .filter(|row| matches!(row, SettingsSubmenuRow::Item { .. }))
                .count(),
            None => self
                .file_menu_rows(panel)
                .iter()
                .filter(|row| matches!(row, FileMenuRow::Item { .. }))
                .count(),
        };
        if count > 0 {
            let next = if direction < 0 {
                (selected + count - 1) % count
            } else {
                (selected + 1) % count
            };
            if self.file_menu_nested.is_some() {
                self.file_menu_nested_selection = next;
            } else {
                self.file_menu_selection = next;
            }
        }
    }

    pub(crate) fn file_menu_selected(
        &self,
    ) -> Option<(FileMenuCommand, Option<FileMenuSubmenu>, bool)> {
        self.file_menu_rows(self.file_menu_submenu)
            .into_iter()
            .filter_map(|row| match row {
                FileMenuRow::Item {
                    command,
                    submenu,
                    enabled,
                    ..
                } => Some((command, submenu, enabled)),
                FileMenuRow::Separator => None,
            })
            .nth(self.file_menu_selection)
    }

    /// Rows of a nested panel (Profiles or Themes): the same rows the settings gear menu shows,
    /// so both menus always offer the same profiles and themes.
    pub(crate) fn file_nested_rows(&self, open: FileMenuSubmenu) -> Vec<SettingsSubmenuRow> {
        let submenu = match open {
            FileMenuSubmenu::Profiles => SettingsSubmenu::Profiles,
            FileMenuSubmenu::Themes => SettingsSubmenu::Themes,
            _ => return Vec::new(),
        };
        self.settings_submenu_rows(submenu, &self.user_profiles().unwrap_or_default())
    }

    fn file_nested_row_height(row: &SettingsSubmenuRow) -> f32 {
        if matches!(row, SettingsSubmenuRow::Separator) {
            SEPARATOR_HEIGHT
        } else {
            ROW_HEIGHT
        }
    }

    /// Height of the panel that shows `open`, borders included.
    pub(crate) fn file_panel_height(&self, open: FileMenuSubmenu) -> f32 {
        let rows: f32 = if open.parent().is_some() {
            self.file_nested_rows(open)
                .iter()
                .map(Self::file_nested_row_height)
                .sum()
        } else {
            self.file_menu_rows(Some(open))
                .iter()
                .map(|row| Self::file_menu_row_height(*row))
                .sum()
        };
        rows + 8.0
    }

    /// The highlighted row of the nested panel and whether it can be chosen.
    pub(crate) fn file_nested_selected(&self) -> Option<(SettingsSubcommand, bool)> {
        let nested = self.file_menu_nested?;
        self.file_nested_rows(nested)
            .into_iter()
            .filter_map(|row| match row {
                SettingsSubmenuRow::Item {
                    command, enabled, ..
                } => Some((command, enabled)),
                SettingsSubmenuRow::Separator => None,
            })
            .nth(self.file_menu_nested_selection)
    }

    /// The row of the nested panel at the point and whether it can be chosen.
    pub(crate) fn file_nested_command_at(
        &self,
        x: f32,
        y: f32,
        nested_x: f32,
        window_height: f32,
    ) -> Option<(SettingsSubcommand, bool)> {
        let nested = self.file_menu_nested?;
        if x < nested_x || x >= nested_x + FILE_SUBMENU_WIDTH {
            return None;
        }
        let mut current = self.file_nested_top(window_height) + 4.0;
        for row in self.file_nested_rows(nested) {
            let height = Self::file_nested_row_height(&row);
            if y >= current && y < current + height {
                return match row {
                    SettingsSubmenuRow::Item {
                        command, enabled, ..
                    } => Some((command, enabled)),
                    SettingsSubmenuRow::Separator => None,
                };
            }
            current += height;
        }
        None
    }

    /// Top edge of a submenu: level with the row of `parent` that opens it and kept inside the
    /// window.
    fn file_child_top(
        &self,
        parent: Option<FileMenuSubmenu>,
        parent_top: f32,
        open: FileMenuSubmenu,
        window_height: f32,
    ) -> f32 {
        let mut row_top = parent_top + 4.0;
        let mut anchor = parent_top;
        for row in self.file_menu_rows(parent) {
            if matches!(row, FileMenuRow::Item { submenu: Some(child), .. } if child == open) {
                anchor = row_top - 4.0;
                break;
            }
            row_top += Self::file_menu_row_height(row);
        }
        let height = self.file_panel_height(open);
        anchor
            .min(window_height - height)
            .max(self.top_menu_height())
    }

    /// Top edge of the open first-level submenu.
    pub(crate) fn file_submenu_top(&self, window_height: f32) -> f32 {
        let menu_top = self.top_menu_height();
        match self.file_menu_submenu {
            Some(open) => self.file_child_top(None, menu_top, open, window_height),
            None => menu_top,
        }
    }

    /// Top edge of the open nested submenu.
    pub(crate) fn file_nested_top(&self, window_height: f32) -> f32 {
        match (self.file_menu_submenu, self.file_menu_nested) {
            (Some(parent), Some(open)) => self.file_child_top(
                Some(parent),
                self.file_submenu_top(window_height),
                open,
                window_height,
            ),
            _ => self.top_menu_height(),
        }
    }

    /// Top edge of the panel that shows `submenu` (`None` is the main menu).
    fn file_panel_top(&self, submenu: Option<FileMenuSubmenu>, window_height: f32) -> f32 {
        match submenu {
            None => self.top_menu_height(),
            Some(open) if Some(open) == self.file_menu_nested => {
                self.file_nested_top(window_height)
            }
            Some(_) => self.file_submenu_top(window_height),
        }
    }

    /// Index among the selectable rows of the panel showing `submenu` that is at `y`.
    pub(crate) fn file_menu_row_index_at(
        &self,
        y: f32,
        submenu: Option<FileMenuSubmenu>,
        window_height: f32,
    ) -> Option<usize> {
        let mut current = self.file_panel_top(submenu, window_height) + 4.0;
        let mut selected = 0;
        if let Some(nested) = submenu.filter(|open| open.parent().is_some()) {
            for row in self.file_nested_rows(nested) {
                let height = Self::file_nested_row_height(&row);
                if y >= current && y < current + height {
                    return matches!(row, SettingsSubmenuRow::Item { .. }).then_some(selected);
                }
                if matches!(row, SettingsSubmenuRow::Item { .. }) {
                    selected += 1;
                }
                current += height;
            }
            return None;
        }
        for row in self.file_menu_rows(submenu) {
            let height = Self::file_menu_row_height(row);
            if y >= current && y < current + height {
                return matches!(row, FileMenuRow::Item { .. }).then_some(selected);
            }
            if matches!(row, FileMenuRow::Item { .. }) {
                selected += 1;
            }
            current += height;
        }
        None
    }

    /// Highlights the row of the panel showing `submenu` that is at `y`.
    pub(crate) fn file_menu_pointer_selection(
        &mut self,
        y: f32,
        submenu: Option<FileMenuSubmenu>,
        window_height: f32,
    ) {
        if let Some(index) = self.file_menu_row_index_at(y, submenu, window_height) {
            if submenu.is_some() && submenu == self.file_menu_nested {
                self.file_menu_nested_selection = index;
            } else {
                self.file_menu_selection = index;
            }
        }
    }

    pub(crate) fn file_menu_command_at(
        &self,
        x: f32,
        y: f32,
        popup_x: f32,
        submenu_x: f32,
        nested_x: f32,
        window_height: f32,
    ) -> Option<(FileMenuCommand, Option<FileMenuSubmenu>, bool)> {
        let inside = |left: f32, width: f32| x >= left && x < left + width;
        if self.file_menu_nested.is_some() && inside(nested_x, FILE_SUBMENU_WIDTH) {
            // The nested panel has its own rows; see `file_nested_command_at`.
            return None;
        }
        let (panel, left, width) =
            if self.file_menu_submenu.is_some() && inside(submenu_x, FILE_SUBMENU_WIDTH) {
                (self.file_menu_submenu, submenu_x, FILE_SUBMENU_WIDTH)
            } else {
                (None, popup_x, FILE_MENU_WIDTH)
            };
        if !inside(left, width) {
            return None;
        }
        let mut current = self.file_panel_top(panel, window_height) + 4.0;
        for row in self.file_menu_rows(panel) {
            let height = Self::file_menu_row_height(row);
            if y >= current && y < current + height {
                if let FileMenuRow::Item {
                    command,
                    submenu,
                    enabled,
                    ..
                } = row
                {
                    return Some((command, submenu, enabled));
                }
                return None;
            }
            current += height;
        }
        None
    }

    pub(crate) fn execute_file_menu_command(&mut self, command: FileMenuCommand) {
        use FileMenuCommand as C;
        match command {
            C::NewTextFile => self.new_untitled_document(),
            C::NewFile => self.create_new_file_dialog(),
            C::NewWindow => self.spawn_window(None),
            C::NewWindowProfile => self.open_file_submenu(FileMenuSubmenu::NewWindowProfile),
            C::OpenFile => self.open_file_dialog(),
            C::OpenFolder => self.open_folder_dialog(),
            C::OpenWorkspace => self.open_workspace_dialog(),
            C::OpenRecent => self.open_file_submenu(FileMenuSubmenu::OpenRecent),
            C::AddFolder => self.add_folder_dialog(),
            C::RemoveFolder => self.remove_active_workspace_folder(),
            C::SaveWorkspaceAs => self.save_workspace_dialog(false),
            C::DuplicateWorkspace => self.save_workspace_dialog(true),
            C::Save => {
                if self.documents[self.active_document()]
                    .buffer
                    .path()
                    .is_some()
                {
                    self.run_ui_command("workbench.action.files.save");
                } else {
                    self.save_as_dialog();
                }
            }
            C::SaveAs => self.save_as_dialog(),
            C::SaveAll => self.run_ui_command("workbench.action.files.saveAll"),
            C::AutoSave => self.toggle_auto_save(),
            C::Preferences => self.open_file_submenu(FileMenuSubmenu::Preferences),
            C::Revert => self.revert_active_file(),
            C::CloseEditor => self.run_ui_command("workbench.action.closeActiveEditor"),
            C::CloseWindow | C::Exit => self.run_ui_command("workbench.action.quit"),
            C::Profiles => {
                if self.file_menu_innermost() == Some(FileMenuSubmenu::Profiles) {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.profile_root)
                        .save_file()
                    {
                        if let Some(name) = path.file_stem().and_then(|name| name.to_str()) {
                            if let Err(error) = self.create_user_profile(name) {
                                self.status = error;
                            }
                        }
                    }
                } else {
                    self.open_file_submenu(FileMenuSubmenu::Profiles);
                }
            }
            C::Settings => self.open_preference_file("settings.json"),
            C::Extensions => self.open_plugins(),
            C::KeyboardShortcuts => self.open_preference_file("keybindings.json"),
            C::ConfigureSnippets => self.open_snippet_configuration(),
            C::Tasks => self.open_task_configuration(),
            C::Themes => self.open_file_submenu(FileMenuSubmenu::Themes),
            C::DarkTheme => self.select_builtin_theme(true),
            C::LightTheme => self.select_builtin_theme(false),
            C::RecentPath(index) => self.open_recent_workspace(index),
            C::SwitchProfile(index) => {
                if let Ok(profiles) = self.user_profiles() {
                    if let Some(profile) = profiles.get(index).cloned() {
                        if self.file_menu_submenu == Some(FileMenuSubmenu::NewWindowProfile) {
                            self.spawn_window(Some(profile));
                        } else if let Err(error) = self.switch_user_profile(&profile) {
                            self.status = error;
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn activate_file_menu_accessibility_item(&mut self, index: usize) {
        if let Some(nested) = self.file_menu_nested {
            let chosen = self
                .file_nested_rows(nested)
                .into_iter()
                .filter_map(|row| match row {
                    SettingsSubmenuRow::Item {
                        command, enabled, ..
                    } => Some((command, enabled)),
                    SettingsSubmenuRow::Separator => None,
                })
                .nth(index);
            if let Some((command, true)) = chosen {
                self.execute_settings_subcommand(command);
                self.dismiss_workbench_menu();
            }
            return;
        }
        if let Some((command, submenu, enabled)) = self
            .file_menu_rows(self.file_menu_innermost())
            .into_iter()
            .filter_map(|row| match row {
                FileMenuRow::Item {
                    command,
                    submenu,
                    enabled,
                    ..
                } => Some((command, submenu, enabled)),
                FileMenuRow::Separator => None,
            })
            .nth(index)
        {
            if !enabled {
                return;
            }
            if let Some(submenu) = submenu {
                self.open_file_submenu(submenu);
            } else {
                self.execute_file_menu_command(command);
                self.dismiss_workbench_menu();
            }
        }
    }

    pub(crate) fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(self.tree.root())
            .pick_file()
        {
            self.open_file(path);
        }
    }

    pub(crate) fn open_folder_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(self.tree.root())
            .pick_folder()
        {
            self.remember_recent_workspace(path.clone());
            self.replace_workspace_roots(vec![path], None);
        }
    }

    fn open_workspace_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("VS Code Workspace", &["code-workspace"])
            .pick_file()
        {
            match Workspace::open_code_workspace(&path) {
                Ok(workspace) => {
                    self.replace_workspace_roots(workspace.roots().to_vec(), Some(path.clone()));
                    self.remember_recent_workspace(path);
                }
                Err(error) => {
                    self.status = self.tf("file.workspace_open_failed", &[("error", &error)])
                }
            }
        }
    }

    fn add_folder_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            match self.tree.add_workspace_root(path.clone()) {
                Ok(true) => {
                    self.status = self.tf("file.folder_added", &[("folder", &path.display())])
                }
                Ok(false) => self.status = self.t("file.folder_already_present").into(),
                Err(error) => self.status = error.to_string(),
            }
        }
    }

    fn remove_active_workspace_folder(&mut self) {
        let root = self.documents[self.active_document()]
            .buffer
            .path()
            .map(|path| self.tree.root_for(path).to_path_buf())
            .unwrap_or_else(|| {
                self.tree
                    .roots()
                    .last()
                    .cloned()
                    .unwrap_or_else(|| self.tree.root().to_path_buf())
            });
        if self.tree.remove_workspace_root(&root) {
            self.current_workspace_file = None;
            self.refresh_workspace_services();
            self.status = self.t("file.folder_removed").into();
        }
    }

    fn create_new_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(self.tree.root())
            .save_file()
        {
            if !path.exists() {
                if let Err(error) = std::fs::write(&path, []) {
                    self.status = error.to_string();
                    return;
                }
            }
            self.open_file(path);
        }
    }

    pub(crate) fn save_as_dialog(&mut self) {
        let current = self.documents[self.active_document()]
            .buffer
            .path()
            .map(Path::to_path_buf);
        let mut dialog = rfd::FileDialog::new();
        if let Some(path) = current.as_ref() {
            if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
                dialog = dialog.set_file_name(file_name.to_owned());
            }
        }
        let directory = current
            .as_deref()
            .and_then(Path::parent)
            .unwrap_or(self.tree.root());
        if let Some(path) = dialog.set_directory(directory).save_file() {
            if let Err(error) = self.save_active_document_as(&path) {
                self.status = self.tf("save.failed", &[("error", &error.to_string())]);
            }
        }
    }

    pub(crate) fn save_active_document_as(
        &mut self,
        path: &Path,
    ) -> Result<(), graphite_text::BufferError> {
        let document = self.active_document();
        if self.saving.document_pending(document) {
            crate::app::error_log::record(
                crate::app::error_log::ErrorSeverity::High,
                "document.save_as",
                "A save for this document is already in progress",
            );
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                self.t("drafts.save_pending").to_owned(),
            )
            .into());
        }
        let tab = &mut self.documents[document];
        if tab.buffer.is_read_only() {
            crate::app::error_log::record(
                crate::app::error_log::ErrorSeverity::High,
                "document.save_as",
                "The source document is read-only",
            );
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "document is read-only",
            )
            .into());
        }
        let text = tab.buffer.text();
        let bytes = crate::file_format::encode(&text, tab.encoding, tab.line_ending);
        if let Err(error) = Buffer::write_bytes_to_path(path, &bytes) {
            crate::app::error_log::record(
                crate::app::error_log::ErrorSeverity::High,
                "document.save_as",
                "Could not save the requested copy",
            );
            return Err(error);
        }
        tab.buffer.set_path(path);
        tab.buffer.mark_saved_snapshot(&text);
        tab.saved_format = (tab.encoding, tab.line_ending);
        tab.saved_fingerprint = Some(crate::file_format::fingerprint(&bytes));
        tab.recovery_label = None;
        tab.recovery_pending_save = false;
        tab.language = Language::from_path(path);
        tab.highlights = None;
        if crate::same_file_path(path, &self.profile_root.join("settings.json")) {
            self.drafts.reload_settings(&self.profile_root);
        }
        self.register_lsp_active();
        self.status = path.display().to_string();
        Ok(())
    }

    fn save_workspace_dialog(&mut self, duplicate: bool) {
        let name = if duplicate {
            "workspace-copy.code-workspace"
        } else {
            "workspace.code-workspace"
        };
        if let Some(mut path) = rfd::FileDialog::new().set_file_name(name).save_file() {
            if path.extension().is_none() {
                path.set_extension("code-workspace");
            }
            match self.tree.workspace_snapshot().save_code_workspace(&path) {
                Ok(()) => {
                    if !duplicate {
                        self.current_workspace_file = Some(path.clone());
                    }
                    self.remember_recent_workspace(path.clone());
                    self.status = self.tf("file.workspace_saved", &[("path", &path.display())]);
                }
                Err(error) => self.status = error,
            }
        }
    }

    pub(crate) fn replace_workspace_roots(
        &mut self,
        roots: Vec<PathBuf>,
        workspace_file: Option<PathBuf>,
    ) {
        if self.documents.iter().any(|document| document.is_dirty()) || self.saving.pending() {
            self.defer_workspace_roots(roots, workspace_file);
            return;
        }
        match Workspace::from_roots(roots) {
            Ok(workspace) => {
                self.stop_workspace_plugins();
                self.lsp_manager.change_workspace();
                self.tree.replace_workspace(workspace);
                self.current_workspace_file = workspace_file;
                self.refresh_workspace_services();
                self.status = self.t("file.workspace_opened").into();
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn refresh_workspace_services(&mut self) {
        self.reset_scm_for_workspace();
        if self
            .workspace_trust
            .as_ref()
            .is_some_and(|trust| trust.is_trusted(self.tree.root()))
        {
            self.start_workspace_trusted_plugins();
        }
        for document in self.documents.ids().collect::<Vec<_>>() {
            self.sync_lsp_document(document);
        }
    }

    pub(crate) fn open_preference_file(&mut self, name: &str) {
        let path = self.profile_root.join(name);
        if !path.exists() {
            if let Err(error) = std::fs::write(&path, b"{}\n") {
                self.status = error.to_string();
                return;
            }
        }
        self.open_file(path);
    }

    /// Opens `<workspace>/.vscode/settings.json`, creating an empty one first.
    pub(crate) fn open_workspace_settings_file(&mut self) {
        let path = self.tree.root().join(".vscode").join("settings.json");
        if !path.exists() {
            let created = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, b"{}\n"));
            if let Err(error) = created {
                self.status = error.to_string();
                return;
            }
        }
        self.open_file(path);
    }

    /// Shows the built-in keyboard shortcuts in a read-only view.
    pub(crate) fn open_default_keybindings_view(&mut self) {
        let path = self.profile_root.join("default-keybindings.json");
        let text = crate::keybindings::default_keybindings_json();
        match self.documents.id_for_path(&path) {
            Some(id) => {
                let _ = self.documents[id].buffer.reload_text(&text);
                self.activate_editor_document(id);
            }
            None => self.open_read_only_view(path, &text),
        }
    }

    pub(crate) fn open_snippet_configuration(&mut self) {
        if let Err(error) = std::fs::create_dir_all(self.profile_root.join("snippets")) {
            self.status = error.to_string();
            return;
        }
        self.open_preference_file("snippets.json");
    }

    pub(crate) fn open_task_configuration(&mut self) {
        let path = self.tree.root().join(".vscode").join("tasks.json");
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                self.status = error.to_string();
                return;
            }
        }
        if !path.exists() {
            let contents = b"{\n  \"version\": \"2.0.0\",\n  \"tasks\": []\n}\n";
            if let Err(error) = std::fs::write(&path, contents) {
                self.status = error.to_string();
                return;
            }
        }
        self.open_file(path);
    }

    pub(crate) fn set_theme(&mut self, dark: bool) {
        DARK_THEME.store(dark, std::sync::atomic::Ordering::Relaxed);
        config::save_dark_theme_in(&self.profile_root, dark);
        self.tree.refresh_icons();
    }

    pub(crate) fn spawn_window(&mut self, profile: Option<String>) {
        let _ = self.spawn_window_process(profile, None);
    }

    fn spawn_window_process(
        &mut self,
        profile: Option<String>,
        startup_file: Option<PathBuf>,
    ) -> bool {
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                self.status = error.to_string();
                return false;
            }
        };
        let mut command = std::process::Command::new(executable);
        let workspace_arg = if let Some(workspace_file) = &self.current_workspace_file {
            workspace_file.clone()
        } else if self.tree.roots().len() > 1 {
            let workspace_file = std::env::temp_dir().join(format!(
                "graphite-window-{}-{}.code-workspace",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_millis())
            ));
            if let Err(error) = self
                .tree
                .workspace_snapshot()
                .save_code_workspace(&workspace_file)
            {
                self.status = error;
                return false;
            }
            workspace_file
        } else {
            self.tree.root().to_path_buf()
        };
        command.arg(workspace_arg).env("GRAPHITE_NEW_WINDOW", "1");
        if let Some(profile) = profile {
            command.env("GRAPHITE_STARTUP_PROFILE", profile);
        }
        if let Some(path) = startup_file {
            command.env("GRAPHITE_STARTUP_FILE", path);
        }
        match command.spawn() {
            Ok(_) => true,
            Err(error) => {
                self.status = error.to_string();
                false
            }
        }
    }

    pub(crate) fn open_editor_in_new_window(&mut self, move_editor: bool) {
        let document = self.active_document();
        let Some(path) = self.documents[document]
            .buffer
            .path()
            .map(Path::to_path_buf)
        else {
            self.status = self.t("view.layout.unavailable").to_owned();
            return;
        };
        if self.documents[document].is_dirty() {
            self.save_batch(
                vec![document],
                super::saving::AfterSave::OpenEditorWindow { path, move_editor },
            );
        } else {
            self.complete_editor_window(path, move_editor);
        }
    }

    pub(crate) fn complete_editor_window(&mut self, path: PathBuf, move_editor: bool) {
        if self.spawn_window_process(None, Some(path)) && move_editor {
            self.close_active_tab();
        }
    }

    fn toggle_auto_save(&mut self) {
        self.auto_save = !self.auto_save;
        let contents = if self.auto_save {
            b"{\"autoSave\":true}\n" as &[u8]
        } else {
            b"{\"autoSave\":false}\n"
        };
        if let Err(error) = std::fs::write(self.profile_root.join("files.json"), contents) {
            self.status = error.to_string();
        }
    }

    pub(crate) fn revert_active_file(&mut self) {
        let document = self.active_document();
        if self.documents[document].buffer.path().is_none() {
            return;
        }
        if self.documents[document].is_dirty() || self.saving.document_pending(document) {
            self.pending_revert_confirmation = Some(document);
            self.close_window_confirm = true;
            self.status = self.t("file.revert_confirm_prompt").into();
            return;
        }
        self.reload_document_from_disk(document);
    }

    pub(crate) fn reload_document_from_disk(&mut self, document: DocumentId) {
        let Some(path) = self
            .documents
            .get(document)
            .and_then(|tab| tab.buffer.path())
            .map(Path::to_path_buf)
        else {
            return;
        };
        match crate::file_format::read(&path, &self.translator) {
            Ok(file) => self.reload_after_branch_checkout(vec![(path, Ok(file))]),
            Err(error) => self.status = error,
        }
    }

    pub(crate) fn open_recent_workspace(&mut self, index: usize) {
        let Some(path) = self.recent_workspaces.get(index).cloned() else {
            return;
        };
        if path.is_dir() {
            self.replace_workspace_roots(vec![path], None);
        } else if let Ok(workspace) = Workspace::open_code_workspace(&path) {
            self.replace_workspace_roots(workspace.roots().to_vec(), Some(path));
        } else {
            self.open_file(path);
        }
    }

    pub(crate) fn remember_recent_workspace(&mut self, path: PathBuf) {
        self.recent_workspaces.retain(|existing| existing != &path);
        self.recent_workspaces.insert(0, path);
        self.recent_workspaces.truncate(20);
        if let Ok(bytes) = serde_json::to_vec_pretty(&self.recent_workspaces) {
            let _ = std::fs::write(self.profile_root.join("recent-workspaces.json"), bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_menu_has_local_actions_with_separators_and_no_online_entries() {
        let temp = crate::test_support::TempDir::new();
        let app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let rows = app.file_menu_rows(None);
        assert!(rows.iter().any(|row| matches!(
            row,
            FileMenuRow::Item {
                command: FileMenuCommand::NewTextFile,
                ..
            }
        )));
        assert!(rows.iter().any(|row| matches!(
            row,
            FileMenuRow::Item {
                command: FileMenuCommand::AutoSave,
                checked: false,
                ..
            }
        )));
        assert!(
            rows.iter()
                .filter(|row| matches!(row, FileMenuRow::Separator))
                .count()
                >= 5
        );
        assert!(!rows.iter().any(|row| matches!(
            row,
            FileMenuRow::Item {
                label: "file.share" | "file.settings_sync",
                ..
            }
        )));
    }

    #[test]
    fn autosave_toggle_persists_per_profile() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profile_root = temp.path().join("profile");
        std::fs::create_dir_all(&app.profile_root).unwrap();
        app.toggle_auto_save();
        assert!(app.auto_save);
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(app.profile_root.join("files.json")).unwrap())
                .unwrap();
        assert_eq!(value["autoSave"], true);
        app.toggle_auto_save();
        assert!(!app.auto_save);
    }

    #[test]
    fn new_text_file_opens_a_separate_untitled_document() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let before = app.documents.len();
        app.new_untitled_document();
        assert_eq!(app.documents.len(), before + 1);
        assert!(app.documents[app.active_document()].buffer.path().is_none());
    }

    #[test]
    fn submenu_is_level_with_its_parent_row_and_stays_inside_the_window() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let menu_top = app.top_menu_height();
        app.file_menu_submenu = None;
        assert_eq!(app.file_submenu_top(900.0), menu_top);
        app.file_menu_submenu = Some(FileMenuSubmenu::OpenRecent);
        let above: f32 = app
            .file_menu_rows(None)
            .into_iter()
            .take_while(|row| {
                !matches!(
                    row,
                    FileMenuRow::Item {
                        submenu: Some(FileMenuSubmenu::OpenRecent),
                        ..
                    }
                )
            })
            .map(App::file_menu_row_height)
            .sum();
        assert!(above > 0.0);
        assert_eq!(app.file_submenu_top(900.0), menu_top + 4.0 + above - 4.0);
        // In a short window the submenu moves up so that it still fits, never above the menu.
        let height: f32 = app
            .file_menu_rows(Some(FileMenuSubmenu::OpenRecent))
            .into_iter()
            .map(App::file_menu_row_height)
            .sum::<f32>()
            + 8.0;
        let short = menu_top + height + 10.0;
        assert!(app.file_submenu_top(short) + height <= short);
        assert!(app.file_submenu_top(short) >= menu_top);
    }

    #[test]
    fn themes_and_profiles_open_nested_beside_preferences() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profile_root = temp.path().join("profile");
        app.open_file_submenu(FileMenuSubmenu::Preferences);
        app.open_file_submenu(FileMenuSubmenu::Themes);
        // The parent stays open; the nested submenu is the one keyboard navigation acts on.
        assert_eq!(app.file_menu_submenu, Some(FileMenuSubmenu::Preferences));
        assert_eq!(app.file_menu_nested, Some(FileMenuSubmenu::Themes));
        assert_eq!(app.file_menu_innermost(), Some(FileMenuSubmenu::Themes));
        assert_eq!(
            app.file_nested_selected(),
            Some((SettingsSubcommand::DarkTheme, true))
        );
        app.file_menu_move_selection(1);
        assert_eq!(
            app.file_nested_selected(),
            Some((SettingsSubcommand::LightTheme, true))
        );
        // Back goes one level at a time.
        assert!(app.close_file_submenu());
        assert_eq!(app.file_menu_nested, None);
        assert_eq!(app.file_menu_submenu, Some(FileMenuSubmenu::Preferences));
        assert!(app.close_file_submenu());
        assert!(!app.close_file_submenu());
        // Opening Profiles replaces Themes beside the same parent.
        app.open_file_submenu(FileMenuSubmenu::Preferences);
        app.open_file_submenu(FileMenuSubmenu::Themes);
        app.open_file_submenu(FileMenuSubmenu::Profiles);
        assert_eq!(app.file_menu_nested, Some(FileMenuSubmenu::Profiles));
        // A first-level submenu opened from the main menu discards the nested one.
        app.open_file_submenu(FileMenuSubmenu::OpenRecent);
        assert_eq!(app.file_menu_nested, None);
        assert_eq!(app.file_menu_submenu, Some(FileMenuSubmenu::OpenRecent));
    }

    #[test]
    fn nested_panel_is_hit_tested_level_with_its_own_row() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profile_root = temp.path().join("profile");
        app.open_file_submenu(FileMenuSubmenu::Preferences);
        app.open_file_submenu(FileMenuSubmenu::Themes);
        let (popup_x, window) = (0.0, (1400.0, 900.0));
        let (submenu_x, nested_x, width) = App::file_panel_xs(popup_x, FILE_MENU_WIDTH, window.0);
        assert_eq!(submenu_x, FILE_MENU_WIDTH);
        assert_eq!(nested_x, FILE_MENU_WIDTH + width);
        // The nested panel starts at the Themes row, not at the top of the menu.
        let nested_top = app.file_nested_top(window.1);
        assert!(nested_top > app.file_submenu_top(window.1));
        let first_row = nested_top + 4.0 + 5.0;
        let hit = app.file_nested_command_at(nested_x + 10.0, first_row, nested_x, window.1);
        assert_eq!(hit, Some((SettingsSubcommand::DarkTheme, true)));
        // The first-level lookup leaves the nested panel to `file_nested_command_at`.
        assert!(app
            .file_menu_command_at(
                nested_x + 10.0,
                first_row,
                popup_x,
                submenu_x,
                nested_x,
                window.1
            )
            .is_none());
        // The same point over the first-level panel hits a Preferences row instead.
        let hit = app.file_menu_command_at(
            submenu_x + 10.0,
            first_row,
            popup_x,
            submenu_x,
            nested_x,
            window.1,
        );
        assert_ne!(hit.map(|hit| hit.0), Some(FileMenuCommand::DarkTheme));
        // With no room on the right, both submenus open to the left of their parents.
        let (left_x, left_nested, _) = App::file_panel_xs(900.0, FILE_MENU_WIDTH, 1400.0);
        assert!(left_x < 900.0 && left_nested < left_x);
    }

    #[test]
    fn nested_profiles_and_themes_are_the_same_rows_as_the_settings_gear_menu() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profile_root = temp.path().join("profile");
        crate::config::install_bundled_themes(&app.profile_root);
        let profiles = app.user_profiles().unwrap_or_default();
        for (nested, gear) in [
            (FileMenuSubmenu::Themes, SettingsSubmenu::Themes),
            (FileMenuSubmenu::Profiles, SettingsSubmenu::Profiles),
        ] {
            assert_eq!(
                app.file_nested_rows(nested),
                app.settings_submenu_rows(gear, &profiles)
            );
        }
        // The bundled theme is offered, and choosing it from the File menu applies it.
        let simple = SettingsSubcommand::ColorTheme("Graphite Simple".into());
        assert!(app.file_nested_rows(FileMenuSubmenu::Themes).iter().any(
            |row| matches!(row, SettingsSubmenuRow::Item { command, .. } if *command == simple)
        ));
        app.open_file_submenu(FileMenuSubmenu::Preferences);
        app.open_file_submenu(FileMenuSubmenu::Themes);
        let index = app
            .file_nested_rows(FileMenuSubmenu::Themes)
            .into_iter()
            .filter(|row| matches!(row, SettingsSubmenuRow::Item { .. }))
            .position(
                |row| matches!(row, SettingsSubmenuRow::Item { command, .. } if command == simple),
            )
            .unwrap();
        app.file_menu_nested_selection = index;
        let (command, enabled) = app.file_nested_selected().unwrap();
        assert!(enabled);
        app.execute_settings_subcommand(command);
        assert!(
            std::fs::read_to_string(app.profile_root.join("settings.json"))
                .unwrap()
                .contains("Graphite Simple")
        );
        // The panel is as tall as its rows, so it is positioned like any other submenu.
        assert!(app.file_panel_height(FileMenuSubmenu::Themes) > 8.0);
    }
}
