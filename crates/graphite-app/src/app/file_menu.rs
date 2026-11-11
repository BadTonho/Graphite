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
            diagnostics: Vec::new(),
            inlay_hints: Vec::new(),
            inlay_revision: None,
            schema_diagnostics: Vec::new(),
            schema_revision: None,
            lsp_revision: None,
            indentation: graphite_text::Indentation {
                insert_spaces: self.insert_spaces,
                tab_size: self.tab_size,
            },
            saved_format: (
                crate::file_format::Encoding::Utf8,
                crate::file_format::LineEnding::Lf,
            ),
            encoding: crate::file_format::Encoding::Utf8,
            line_ending: crate::file_format::LineEnding::Lf,
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

    pub(crate) fn file_menu_move_selection(&mut self, direction: isize) {
        let count = self
            .file_menu_rows(self.file_menu_submenu)
            .iter()
            .filter(|row| matches!(row, FileMenuRow::Item { .. }))
            .count();
        if count > 0 {
            self.file_menu_selection = if direction < 0 {
                (self.file_menu_selection + count - 1) % count
            } else {
                (self.file_menu_selection + 1) % count
            };
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

    pub(crate) fn file_menu_pointer_selection(&mut self, y: f32, submenu: Option<FileMenuSubmenu>) {
        let mut current = self.top_menu_height() + 4.0;
        let mut selected = 0;
        for row in self.file_menu_rows(submenu) {
            let height = Self::file_menu_row_height(row);
            if y >= current && y < current + height {
                if matches!(row, FileMenuRow::Item { .. }) {
                    self.file_menu_selection = selected;
                }
                return;
            }
            if matches!(row, FileMenuRow::Item { .. }) {
                selected += 1;
            }
            current += height;
        }
    }

    pub(crate) fn file_menu_command_at(
        &self,
        x: f32,
        y: f32,
        popup_x: f32,
        submenu_x: f32,
    ) -> Option<(FileMenuCommand, Option<FileMenuSubmenu>, bool)> {
        let (popup_x, rows) = if x >= submenu_x && self.file_menu_submenu.is_some() {
            (submenu_x, self.file_menu_rows(self.file_menu_submenu))
        } else {
            (popup_x, self.file_menu_rows(None))
        };
        if x < popup_x || x >= popup_x + FILE_MENU_WIDTH {
            return None;
        }
        let mut current = self.top_menu_height() + 4.0;
        for row in rows {
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
            C::NewWindowProfile => self.file_menu_submenu = Some(FileMenuSubmenu::NewWindowProfile),
            C::OpenFile => self.open_file_dialog(),
            C::OpenFolder => self.open_folder_dialog(),
            C::OpenWorkspace => self.open_workspace_dialog(),
            C::OpenRecent => self.file_menu_submenu = Some(FileMenuSubmenu::OpenRecent),
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
            C::Preferences => self.file_menu_submenu = Some(FileMenuSubmenu::Preferences),
            C::Revert => self.revert_active_file(),
            C::CloseEditor => self.run_ui_command("workbench.action.closeActiveEditor"),
            C::CloseWindow | C::Exit => self.run_ui_command("workbench.action.quit"),
            C::Profiles => {
                if self.file_menu_submenu == Some(FileMenuSubmenu::Profiles) {
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
                    self.file_menu_submenu = Some(FileMenuSubmenu::Profiles);
                }
            }
            C::Settings => self.open_preference_file("settings.json"),
            C::Extensions => self.open_plugins(),
            C::KeyboardShortcuts => self.open_preference_file("keybindings.json"),
            C::ConfigureSnippets => self.open_snippet_configuration(),
            C::Tasks => self.open_task_configuration(),
            C::Themes => self.file_menu_submenu = Some(FileMenuSubmenu::Themes),
            C::DarkTheme => self.set_theme(true),
            C::LightTheme => self.set_theme(false),
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
        if let Some((command, submenu, enabled)) = self
            .file_menu_rows(self.file_menu_submenu)
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
                self.file_menu_submenu = Some(submenu);
                self.file_menu_selection = 0;
            } else {
                self.execute_file_menu_command(command);
                self.dismiss_workbench_menu();
            }
        }
    }

    pub(super) fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(self.tree.root())
            .pick_file()
        {
            self.open_file(path);
        }
    }

    pub(super) fn open_folder_dialog(&mut self) {
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

    pub(super) fn save_as_dialog(&mut self) {
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
            let document = self.active_document();
            match self.documents[document].buffer.save_as(&path) {
                Ok(()) => {
                    self.documents[document].language = Language::from_path(&path);
                    self.documents[document].highlights = None;
                    self.register_lsp_active();
                    self.status = path.display().to_string();
                }
                Err(error) => {
                    self.status = self.tf("save.failed", &[("error", &error.to_string())])
                }
            }
        }
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

    pub(super) fn replace_workspace_roots(
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
        self.scm_snapshot = crate::scm::snapshot(self.tree.root());
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

    pub(super) fn open_preference_file(&mut self, name: &str) {
        let path = self.profile_root.join(name);
        if !path.exists() {
            if let Err(error) = std::fs::write(&path, b"{}\n") {
                self.status = error.to_string();
                return;
            }
        }
        self.open_file(path);
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
    }

    pub(super) fn spawn_window(&mut self, profile: Option<String>) {
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

    fn revert_active_file(&mut self) {
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

    pub(crate) fn autosave_tick(&mut self) {
        if !self.auto_save {
            self.autosave_deadlines.clear();
            return;
        }
        let now = std::time::Instant::now();
        let mut due = Vec::new();
        for document in self.documents.iter() {
            if !document.is_dirty()
                || document.buffer.path().is_none()
                || document.buffer.is_read_only()
            {
                self.autosave_deadlines.remove(&document.id);
                continue;
            }
            let revision = document.buffer.revision();
            let deadline = self
                .autosave_deadlines
                .entry(document.id)
                .or_insert((revision, now + std::time::Duration::from_secs(1)));
            if deadline.0 != revision {
                *deadline = (revision, now + std::time::Duration::from_secs(1));
            } else if now >= deadline.1 && !self.saving.document_pending(document.id) {
                due.push(document.id);
                deadline.1 = now + std::time::Duration::from_secs(3600);
            }
        }
        for document in due {
            self.save_document(document);
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
}
