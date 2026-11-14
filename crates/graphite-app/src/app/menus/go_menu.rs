use super::editor_groups::GroupRect;
use super::*;
use std::path::PathBuf;

const ROW_HEIGHT: f32 = 28.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const MIN_WIDTH: f32 = 326.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GoMenuCommand {
    Back,
    Forward,
    LastEdit,
    SwitchEditor,
    SwitchGroup,
    QuickOpen,
    WorkspaceSymbols,
    EditorSymbols,
    Definition,
    Declaration,
    TypeDefinition,
    Implementations,
    References,
    GoToLine,
    Bracket,
    NextProblem,
    PreviousProblem,
    NextChange,
    PreviousChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GoSubmenu {
    SwitchEditor,
    SwitchGroup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GoSubcommand {
    NextEditor,
    PreviousEditor,
    NextUsedEditor,
    PreviousUsedEditor,
    NextEditorInGroup,
    PreviousEditorInGroup,
    NextUsedEditorInGroup,
    PreviousUsedEditorInGroup,
    Group(usize),
    NextGroup,
    PreviousGroup,
    GroupLeft,
    GroupRight,
    GroupAbove,
    GroupBelow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GoMenuRow {
    Separator,
    Item {
        label: &'static str,
        shortcut: &'static str,
        command: GoMenuCommand,
        enabled: bool,
        submenu: Option<GoSubmenu>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GoSubmenuRow {
    Separator,
    Item {
        label: &'static str,
        shortcut: &'static str,
        command: GoSubcommand,
        enabled: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GoSymbolItem {
    pub path: PathBuf,
    pub line: usize,
    pub name: String,
    pub scope: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GoPickerKind {
    EditorSymbols,
    WorkspaceSymbols,
    LineColumn,
    Locations,
    /// Saved versions of a file: picking one restores it, compares it, or deletes it.
    HistoryRestore,
    HistoryCompare,
    HistoryDelete,
    /// The timeline of a file: picking an entry compares it with the file.
    Timeline,
    /// The installed kernels: picking one starts it for the active notebook.
    Kernel,
    /// Search through the cells of the active notebook: picking a line goes to it.
    NotebookFind,
}

#[derive(Clone, Debug)]
pub(crate) struct GoPicker {
    pub kind: GoPickerKind,
    pub query: String,
    pub items: Vec<GoSymbolItem>,
    pub selected: usize,
    pub loading: bool,
    pub generation: u64,
    pub message: Option<String>,
}

impl App {
    pub(crate) fn go_menu_width(&self, char_width: f32, available: f32) -> f32 {
        self.go_menu_rows()
            .iter()
            .filter_map(|row| match row {
                GoMenuRow::Item {
                    label, shortcut, ..
                } => Some(
                    38.0 + self.t(label).chars().count() as f32 * char_width
                        + shortcut.chars().count() as f32 * char_width
                        + if shortcut.is_empty() { 20.0 } else { 36.0 },
                ),
                GoMenuRow::Separator => None,
            })
            .fold(MIN_WIDTH, f32::max)
            .min(available.max(0.0))
    }
    pub(crate) fn go_submenu_width(
        &self,
        submenu: GoSubmenu,
        char_width: f32,
        available: f32,
    ) -> f32 {
        self.go_submenu_rows(submenu)
            .iter()
            .filter_map(|row| match row {
                GoSubmenuRow::Item {
                    label, shortcut, ..
                } => Some(
                    36.0 + self.t(label).chars().count() as f32 * char_width
                        + shortcut.chars().count() as f32 * char_width
                        + if shortcut.is_empty() { 16.0 } else { 34.0 },
                ),
                GoSubmenuRow::Separator => None,
            })
            .fold(MIN_WIDTH, f32::max)
            .min(available.max(0.0))
    }
    pub(crate) fn go_menu_command_at(
        &self,
        x: f32,
        y: f32,
        px: f32,
        py: f32,
        width: f32,
    ) -> Option<(GoMenuCommand, bool, Option<GoSubmenu>)> {
        if x < px || x >= px + width {
            return None;
        }
        let mut top = py + 4.0;
        for row in self.go_menu_rows() {
            let height = go_row_height(row);
            if y >= top && y < top + height {
                return match row {
                    GoMenuRow::Item {
                        command,
                        enabled,
                        submenu,
                        ..
                    } => Some((command, enabled, submenu)),
                    GoMenuRow::Separator => None,
                };
            }
            top += height;
        }
        None
    }
    pub(crate) fn go_submenu_command_at(
        &self,
        submenu: GoSubmenu,
        x: f32,
        y: f32,
        px: f32,
        py: f32,
        width: f32,
    ) -> Option<(GoSubcommand, bool)> {
        if x < px || x >= px + width {
            return None;
        }
        let mut top = py + 4.0;
        for row in self.go_submenu_rows(submenu) {
            let height = go_subrow_height(row);
            if y >= top && y < top + height {
                return match row {
                    GoSubmenuRow::Item {
                        command, enabled, ..
                    } => Some((command, enabled)),
                    GoSubmenuRow::Separator => None,
                };
            }
            top += height;
        }
        None
    }
    pub(crate) fn go_menu_move_selection(&mut self, delta: isize) {
        let count = self
            .go_menu_rows()
            .iter()
            .filter(|row| matches!(row, GoMenuRow::Item { .. }))
            .count();
        self.go_menu_selection = if count == 0 {
            0
        } else {
            self.go_menu_selection
                .saturating_add_signed(delta)
                .min(count - 1)
        };
    }
    pub(crate) fn go_submenu_move_selection(&mut self, delta: isize) {
        let Some(submenu) = self.go_submenu else {
            return;
        };
        let count = self
            .go_submenu_rows(submenu)
            .iter()
            .filter(|row| matches!(row, GoSubmenuRow::Item { .. }))
            .count();
        self.go_submenu_selection = if count == 0 {
            0
        } else {
            self.go_submenu_selection
                .saturating_add_signed(delta)
                .min(count - 1)
        };
    }
    pub(crate) fn go_menu_pointer_selection(&mut self, y: f32, py: f32) {
        let mut top = py + 4.0;
        let mut index = 0;
        for row in self.go_menu_rows() {
            let height = go_row_height(row);
            if y >= top && y < top + height {
                if matches!(row, GoMenuRow::Item { .. }) {
                    self.go_menu_selection = index;
                }
                return;
            }
            top += height;
            if matches!(row, GoMenuRow::Item { .. }) {
                index += 1;
            }
        }
    }
    pub(crate) fn go_menu_selected(&self) -> Option<(GoMenuCommand, bool, Option<GoSubmenu>)> {
        self.go_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                GoMenuRow::Item {
                    command,
                    enabled,
                    submenu,
                    ..
                } => Some((command, enabled, submenu)),
                GoMenuRow::Separator => None,
            })
            .nth(self.go_menu_selection)
    }
    pub(crate) fn go_submenu_selected(&self) -> Option<(GoSubcommand, bool)> {
        let submenu = self.go_submenu?;
        self.go_submenu_rows(submenu)
            .into_iter()
            .filter_map(|row| match row {
                GoSubmenuRow::Item {
                    command, enabled, ..
                } => Some((command, enabled)),
                GoSubmenuRow::Separator => None,
            })
            .nth(self.go_submenu_selection)
    }
    pub(crate) fn go_submenu_pointer_selection(&mut self, submenu: GoSubmenu, y: f32, py: f32) {
        let mut top = py + 4.0;
        let mut index = 0;
        for row in self.go_submenu_rows(submenu) {
            let height = go_subrow_height(row);
            if y >= top && y < top + height {
                if matches!(row, GoSubmenuRow::Item { .. }) {
                    self.go_submenu_selection = index;
                }
                return;
            }
            top += height;
            if matches!(row, GoSubmenuRow::Item { .. }) {
                index += 1;
            }
        }
    }
    pub(crate) fn go_submenu_anchor_y(&self, py: f32, _px: f32, _width: f32) -> f32 {
        let mut top = py + 4.0;
        for row in self.go_menu_rows() {
            if let GoMenuRow::Item {
                submenu: Some(current),
                ..
            } = row
            {
                if Some(current) == self.go_submenu {
                    return top;
                }
            }
            top += go_row_height(row);
        }
        py
    }
    pub(crate) fn activate_go_menu_accessibility_item(&mut self, index: usize) {
        let row = self
            .go_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                GoMenuRow::Item {
                    command,
                    enabled,
                    submenu,
                    ..
                } => Some((command, enabled, submenu)),
                GoMenuRow::Separator => None,
            })
            .nth(index);
        if let Some((command, true, submenu)) = row {
            if let Some(submenu) = submenu {
                self.go_submenu = Some(submenu);
                self.go_submenu_selection = 0;
            } else {
                self.go_menu_execute(command);
                self.close_workbench_menu();
            }
        }
    }
    pub(crate) fn activate_go_submenu_accessibility_item(&mut self, index: usize) {
        let row = self.go_submenu.and_then(|submenu| {
            self.go_submenu_rows(submenu)
                .into_iter()
                .filter_map(|row| match row {
                    GoSubmenuRow::Item {
                        command, enabled, ..
                    } => Some((command, enabled)),
                    GoSubmenuRow::Separator => None,
                })
                .nth(index)
        });
        if let Some((command, true)) = row {
            self.go_submenu_execute(command);
        }
    }
    pub(crate) fn go_menu_rows(&self) -> Vec<GoMenuRow> {
        use GoMenuCommand as C;
        use GoMenuRow::{Item as I, Separator as S};
        let document = self.documents.get(self.active_document());
        let has_document = document.is_some();
        let has_path = document.is_some_and(|document| document.buffer.path().is_some());
        let has_definition = document
            .and_then(|document| document.buffer.path())
            .is_some_and(|path| self.lsp_manager.supports_path(path));
        let (can_back, can_forward) = self.navigation_history_index.map_or((false, false), |i| {
            (i > 0, i + 1 < self.navigation_history.len())
        });
        let has_symbols = document.is_some_and(|document| {
            document.buffer.path().is_some() && document.language.is_some()
        });
        let has_problems = !self.diagnostic_entries().is_empty();
        vec![
            I {
                label: "go.back",
                shortcut: "Alt+LeftArrow",
                command: C::Back,
                enabled: can_back,
                submenu: None,
            },
            I {
                label: "go.forward",
                shortcut: "Alt+RightArrow",
                command: C::Forward,
                enabled: can_forward,
                submenu: None,
            },
            I {
                label: "go.last_edit",
                shortcut: "Ctrl+K Ctrl+Q",
                command: C::LastEdit,
                enabled: false,
                submenu: None,
            },
            S,
            I {
                label: "go.switch_editor",
                shortcut: "",
                command: C::SwitchEditor,
                enabled: true,
                submenu: Some(GoSubmenu::SwitchEditor),
            },
            I {
                label: "go.switch_group",
                shortcut: "",
                command: C::SwitchGroup,
                enabled: true,
                submenu: Some(GoSubmenu::SwitchGroup),
            },
            S,
            I {
                label: "go.to_file",
                shortcut: "Ctrl+P",
                command: C::QuickOpen,
                enabled: true,
                submenu: None,
            },
            I {
                label: "go.workspace_symbol",
                shortcut: "Ctrl+T",
                command: C::WorkspaceSymbols,
                enabled: !self.tree.roots().is_empty(),
                submenu: None,
            },
            S,
            I {
                label: "go.editor_symbol",
                shortcut: "Ctrl+Shift+O",
                command: C::EditorSymbols,
                enabled: has_symbols,
                submenu: None,
            },
            I {
                label: "go.definition",
                shortcut: "F12",
                command: C::Definition,
                enabled: has_path && has_definition,
                submenu: None,
            },
            I {
                label: "go.declaration",
                shortcut: "",
                command: C::Declaration,
                enabled: self.has_vsix_language_provider(
                    crate::app::plugins::VsixLanguageFeature::Declaration,
                ),
                submenu: None,
            },
            I {
                label: "go.type_definition",
                shortcut: "",
                command: C::TypeDefinition,
                enabled: self.has_vsix_language_provider(
                    crate::app::plugins::VsixLanguageFeature::TypeDefinition,
                ),
                submenu: None,
            },
            I {
                label: "go.implementations",
                shortcut: "Ctrl+F12",
                command: C::Implementations,
                enabled: self.has_vsix_language_provider(
                    crate::app::plugins::VsixLanguageFeature::Implementation,
                ),
                submenu: None,
            },
            I {
                label: "go.references",
                shortcut: "Shift+F12",
                command: C::References,
                enabled: self.has_vsix_language_provider(
                    crate::app::plugins::VsixLanguageFeature::References,
                ),
                submenu: None,
            },
            S,
            I {
                label: "go.line_column",
                shortcut: "Ctrl+G",
                command: C::GoToLine,
                enabled: has_document,
                submenu: None,
            },
            I {
                label: "go.bracket",
                shortcut: "Ctrl+Shift+]",
                command: C::Bracket,
                enabled: false,
                submenu: None,
            },
            S,
            I {
                label: "go.next_problem",
                shortcut: "F8",
                command: C::NextProblem,
                enabled: has_problems,
                submenu: None,
            },
            I {
                label: "go.previous_problem",
                shortcut: "Shift+F8",
                command: C::PreviousProblem,
                enabled: has_problems,
                submenu: None,
            },
            S,
            I {
                label: "go.next_change",
                shortcut: "Alt+F3",
                command: C::NextChange,
                enabled: false,
                submenu: None,
            },
            I {
                label: "go.previous_change",
                shortcut: "Shift+Alt+F3",
                command: C::PreviousChange,
                enabled: false,
                submenu: None,
            },
        ]
    }

    pub(crate) fn go_submenu_rows(&self, submenu: GoSubmenu) -> Vec<GoSubmenuRow> {
        use GoSubcommand as C;
        use GoSubmenuRow::{Item as I, Separator as S};
        match submenu {
            GoSubmenu::SwitchEditor => {
                let all = self.editor_groups.ordered_view_ids();
                let group = self
                    .editor_groups
                    .view_ids_in_group(self.editor_groups.focused())
                    .unwrap_or_default();
                vec![
                    I {
                        label: "go.next_editor",
                        shortcut: "Ctrl+PageDown",
                        command: C::NextEditor,
                        enabled: all.len() > 1,
                    },
                    I {
                        label: "go.previous_editor",
                        shortcut: "Ctrl+PageUp",
                        command: C::PreviousEditor,
                        enabled: all.len() > 1,
                    },
                    I {
                        label: "go.next_used_editor",
                        shortcut: "",
                        command: C::NextUsedEditor,
                        enabled: all.len() > 1,
                    },
                    I {
                        label: "go.previous_used_editor",
                        shortcut: "",
                        command: C::PreviousUsedEditor,
                        enabled: all.len() > 1,
                    },
                    S,
                    I {
                        label: "go.next_editor_in_group",
                        shortcut: "Ctrl+K Ctrl+PageDown",
                        command: C::NextEditorInGroup,
                        enabled: group.len() > 1,
                    },
                    I {
                        label: "go.previous_editor_in_group",
                        shortcut: "Ctrl+K Ctrl+PageUp",
                        command: C::PreviousEditorInGroup,
                        enabled: group.len() > 1,
                    },
                    I {
                        label: "go.next_used_editor_in_group",
                        shortcut: "",
                        command: C::NextUsedEditorInGroup,
                        enabled: group.len() > 1,
                    },
                    I {
                        label: "go.previous_used_editor_in_group",
                        shortcut: "",
                        command: C::PreviousUsedEditorInGroup,
                        enabled: group.len() > 1,
                    },
                ]
            }
            GoSubmenu::SwitchGroup => {
                let count = self.editor_groups.group_count();
                let mut rows = (1..=5)
                    .map(|number| I {
                        label: match number {
                            1 => "go.group_1",
                            2 => "go.group_2",
                            3 => "go.group_3",
                            4 => "go.group_4",
                            _ => "go.group_5",
                        },
                        shortcut: match number {
                            1 => "Ctrl+1",
                            2 => "Ctrl+2",
                            3 => "Ctrl+3",
                            4 => "Ctrl+4",
                            _ => "Ctrl+5",
                        },
                        command: C::Group(number),
                        enabled: number <= count,
                    })
                    .collect::<Vec<_>>();
                rows.extend([
                    S,
                    I {
                        label: "go.next_group",
                        shortcut: "",
                        command: C::NextGroup,
                        enabled: count > 1,
                    },
                    I {
                        label: "go.previous_group",
                        shortcut: "",
                        command: C::PreviousGroup,
                        enabled: count > 1,
                    },
                    S,
                    I {
                        label: "go.group_left",
                        shortcut: "Ctrl+K Ctrl+LeftArrow",
                        command: C::GroupLeft,
                        enabled: self.adjacent_group(GoSubcommand::GroupLeft).is_some(),
                    },
                    I {
                        label: "go.group_right",
                        shortcut: "Ctrl+K Ctrl+RightArrow",
                        command: C::GroupRight,
                        enabled: self.adjacent_group(GoSubcommand::GroupRight).is_some(),
                    },
                    I {
                        label: "go.group_above",
                        shortcut: "Ctrl+K Ctrl+UpArrow",
                        command: C::GroupAbove,
                        enabled: self.adjacent_group(GoSubcommand::GroupAbove).is_some(),
                    },
                    I {
                        label: "go.group_below",
                        shortcut: "Ctrl+K Ctrl+DownArrow",
                        command: C::GroupBelow,
                        enabled: self.adjacent_group(GoSubcommand::GroupBelow).is_some(),
                    },
                ]);
                rows
            }
        }
    }

    pub(crate) fn open_go_picker(&mut self, kind: GoPickerKind) {
        self.command_palette = None;
        self.quick_open = None;
        self.local_search = None;
        self.go_picker_generation = self.go_picker_generation.wrapping_add(1);
        let generation = self.go_picker_generation;
        let mut picker = GoPicker {
            kind,
            query: String::new(),
            items: Vec::new(),
            selected: 0,
            loading: kind == GoPickerKind::WorkspaceSymbols,
            generation,
            message: None,
        };
        if kind == GoPickerKind::EditorSymbols {
            picker.items = self.editor_symbol_items();
            if picker.items.is_empty() {
                picker.message = Some(self.t("go.no_symbols").into());
            }
        }
        self.go_picker = Some(picker);
        if kind == GoPickerKind::WorkspaceSymbols {
            self.request_workspace_symbols(generation);
        }
    }

    fn editor_symbol_items(&self) -> Vec<GoSymbolItem> {
        let document = &self.documents[self.active_document()];
        let Some(path) = document.buffer.path() else {
            return Vec::new();
        };
        let Some(language) = document.language else {
            return Vec::new();
        };
        let source = document.buffer.text();
        graphite_syntax::declaration_headers(language, &source)
            .into_iter()
            .map(|symbol| GoSymbolItem {
                path: path.to_path_buf(),
                line: symbol.line,
                name: symbol.text.clone(),
                scope: symbol.text,
            })
            .collect()
    }

    fn request_workspace_symbols(&mut self, generation: u64) {
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        let workspace = self.tree.workspace_snapshot();
        let opened = self
            .documents
            .iter()
            .filter_map(|document| {
                document
                    .buffer
                    .path()
                    .map(|path| (path.to_path_buf(), document.buffer.text()))
            })
            .collect::<std::collections::HashMap<_, _>>();
        std::thread::spawn(move || {
            let results = collect_workspace_symbols(&workspace, &opened);
            let _ = proxy.send_event(UserEvent::GoSymbolsReady {
                generation,
                items: results,
            });
        });
    }

    pub(crate) fn accept_go_symbols(&mut self, generation: u64, items: Vec<GoSymbolItem>) {
        let no_symbols = self.t("go.no_symbols").to_owned();
        if let Some(picker) = self.go_picker.as_mut().filter(|picker| {
            picker.kind == GoPickerKind::WorkspaceSymbols && picker.generation == generation
        }) {
            picker.items = items;
            picker.loading = false;
            if picker.items.is_empty() {
                picker.message = Some(no_symbols);
            }
        }
    }

    pub(crate) fn go_picker_rows(&self) -> Vec<GoSymbolItem> {
        let Some(picker) = &self.go_picker else {
            return Vec::new();
        };
        let query = picker.query.to_lowercase();
        picker
            .items
            .iter()
            .filter(|item| {
                query.is_empty()
                    || item.name.to_lowercase().contains(&query)
                    || item.path.to_string_lossy().to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    pub(crate) fn handle_go_picker(&mut self, event: &winit::event::KeyEvent) {
        if self.go_picker.is_none() {
            return;
        }
        match event.logical_key {
            Key::Named(NamedKey::Escape) => self.go_picker = None,
            Key::Named(NamedKey::ArrowDown) => {
                let count = self.go_picker_rows().len();
                if count > 0 {
                    if let Some(picker) = self.go_picker.as_mut() {
                        picker.selected = (picker.selected + 1) % count;
                    }
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                let count = self.go_picker_rows().len();
                if count > 0 {
                    if let Some(picker) = self.go_picker.as_mut() {
                        picker.selected = (picker.selected + count - 1) % count;
                    }
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some(picker) = self.go_picker.as_mut() {
                    picker.query.pop();
                    picker.selected = 0;
                }
            }
            Key::Named(NamedKey::Enter) => self.accept_go_picker(),
            _ if !self.modifiers.control_key() && !self.modifiers.alt_key() => {
                if let Some(text) = event
                    .text
                    .as_ref()
                    .filter(|text| !text.chars().any(char::is_control))
                {
                    if let Some(picker) = self.go_picker.as_mut() {
                        picker.query.push_str(text);
                        picker.selected = 0;
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn accept_go_picker(&mut self) {
        let Some(picker) = self.go_picker.take() else {
            return;
        };
        match picker.kind {
            GoPickerKind::LineColumn => {
                let parsed = parse_line_column(&picker.query);
                let Some((line, column)) = parsed else {
                    self.status = self.t("go.invalid_line_column").into();
                    self.go_picker = Some(picker);
                    return;
                };
                self.editor_mut()
                    .set_cursor_line_col(line.saturating_sub(1), column.saturating_sub(1));
                self.reveal_cursor = true;
            }
            GoPickerKind::Kernel => {
                if let Some(item) = self
                    .go_picker_rows_from(&picker)
                    .get(picker.selected)
                    .cloned()
                {
                    self.select_kernel(item.line);
                }
            }
            GoPickerKind::NotebookFind => {
                if let Some(item) = self
                    .go_picker_rows_from(&picker)
                    .get(picker.selected)
                    .cloned()
                {
                    self.notebook_find_goto(item.line, &picker.query);
                }
            }
            GoPickerKind::Timeline => {
                if let Some(item) = self
                    .go_picker_rows_from(&picker)
                    .get(picker.selected)
                    .cloned()
                {
                    self.compare_timeline_entry(item.path);
                }
            }
            GoPickerKind::HistoryRestore
            | GoPickerKind::HistoryCompare
            | GoPickerKind::HistoryDelete => {
                let Some(item) = self
                    .go_picker_rows_from(&picker)
                    .get(picker.selected)
                    .cloned()
                else {
                    return;
                };
                let Some(origin) = self.history_origin(&item.path) else {
                    self.status = self.t("history.no_origin").into();
                    return;
                };
                match picker.kind {
                    GoPickerKind::HistoryRestore => {
                        self.restore_history_version(&item.path, &origin)
                    }
                    GoPickerKind::HistoryCompare => self.compare_history_version(item.path, origin),
                    _ => self.delete_history_version(&item.path),
                }
            }
            _ => {
                if let Some(item) = self
                    .go_picker_rows_from(&picker)
                    .get(picker.selected)
                    .cloned()
                {
                    // Saved versions open read-only so they cannot overwrite the history.
                    let history = self.is_local_history_file(&item.path);
                    self.open_file(item.path);
                    if history {
                        let document = self.active_document();
                        self.documents[document].buffer.set_read_only(true);
                    }
                    self.editor_mut().set_cursor_line_col(item.line, 0);
                    self.reveal_cursor = true;
                }
            }
        }
    }

    fn go_picker_rows_from(&self, picker: &GoPicker) -> Vec<GoSymbolItem> {
        let query = picker.query.to_lowercase();
        picker
            .items
            .iter()
            .filter(|item| {
                query.is_empty()
                    || item.name.to_lowercase().contains(&query)
                    || item.path.to_string_lossy().to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    pub(crate) fn go_menu_execute(&mut self, command: GoMenuCommand) {
        match command {
            GoMenuCommand::Back => self.run_ui_command("workbench.action.navigateBack"),
            GoMenuCommand::Forward => self.run_ui_command("workbench.action.navigateForward"),
            GoMenuCommand::LastEdit
            | GoMenuCommand::Bracket
            | GoMenuCommand::NextChange
            | GoMenuCommand::PreviousChange => {}
            GoMenuCommand::SwitchEditor => {
                self.go_submenu = Some(GoSubmenu::SwitchEditor);
                self.go_submenu_selection = 0;
            }
            GoMenuCommand::SwitchGroup => {
                self.go_submenu = Some(GoSubmenu::SwitchGroup);
                self.go_submenu_selection = 0;
            }
            GoMenuCommand::QuickOpen => self.run_ui_command("workbench.action.quickOpen"),
            GoMenuCommand::WorkspaceSymbols => {
                self.run_ui_command("workbench.action.showAllSymbols")
            }
            GoMenuCommand::EditorSymbols => self.run_ui_command("workbench.action.gotoSymbol"),
            GoMenuCommand::Definition => self.run_ui_command("editor.action.revealDefinition"),
            GoMenuCommand::Implementations => {
                self.run_ui_command("editor.action.goToImplementation")
            }
            GoMenuCommand::References => self.run_ui_command("editor.action.goToReferences"),
            GoMenuCommand::Declaration => self.run_ui_command("editor.action.goToDeclaration"),
            GoMenuCommand::TypeDefinition => {
                self.run_ui_command("editor.action.goToTypeDefinition")
            }
            GoMenuCommand::GoToLine => self.run_ui_command("workbench.action.gotoLine"),
            GoMenuCommand::NextProblem => self.run_ui_command("editor.action.marker.next"),
            GoMenuCommand::PreviousProblem => self.run_ui_command("editor.action.marker.prev"),
        }
    }

    pub(crate) fn go_submenu_execute(&mut self, command: GoSubcommand) {
        use GoSubcommand as C;
        match command {
            C::NextEditor => self.switch_go_editor(false, false, false),
            C::PreviousEditor => self.switch_go_editor(true, false, false),
            C::NextUsedEditor => self.switch_go_editor(false, false, true),
            C::PreviousUsedEditor => self.switch_go_editor(true, false, true),
            C::NextEditorInGroup => self.switch_go_editor(false, true, false),
            C::PreviousEditorInGroup => self.switch_go_editor(true, true, false),
            C::NextUsedEditorInGroup => self.switch_go_editor(false, true, true),
            C::PreviousUsedEditorInGroup => self.switch_go_editor(true, true, true),
            C::Group(number) => self.focus_group_number(number),
            C::NextGroup => self.focus_group_step(false),
            C::PreviousGroup => self.focus_group_step(true),
            C::GroupLeft | C::GroupRight | C::GroupAbove | C::GroupBelow => {
                self.focus_adjacent_group(command)
            }
        }
        self.close_workbench_menu();
    }

    pub(crate) fn switch_go_editor(&mut self, reverse: bool, group_only: bool, recent: bool) {
        let group = self.editor_groups.focused();
        let current = self.editor_groups.active_view().id;
        let ordered = if group_only {
            self.editor_groups
                .view_ids_in_group(group)
                .unwrap_or_default()
        } else {
            self.editor_groups.ordered_view_ids()
        };
        if ordered.len() < 2 {
            return;
        }
        let order = if recent {
            let docs = self.recent_documents.clone();
            let mut ids = Vec::new();
            for doc in docs {
                for id in &ordered {
                    if self
                        .editor_groups
                        .view(*id)
                        .is_some_and(|v| v.document == doc)
                        && !ids.contains(id)
                    {
                        ids.push(*id);
                    }
                }
            }
            for id in &ordered {
                if !ids.contains(id) {
                    ids.push(*id);
                }
            }
            ids
        } else {
            ordered
        };
        let index = order.iter().position(|id| *id == current).unwrap_or(0);
        let next = if reverse {
            (index + order.len() - 1) % order.len()
        } else {
            (index + 1) % order.len()
        };
        self.editor_groups.focus_view(order[next]);
    }

    pub(crate) fn focus_group_number(&mut self, number: usize) {
        if let Some(id) = self
            .editor_groups
            .ordered_group_ids()
            .get(number.saturating_sub(1))
            .copied()
        {
            self.focus_editor_group(id);
        }
    }
    pub(crate) fn focus_group_step(&mut self, reverse: bool) {
        let ids = self.editor_groups.ordered_group_ids();
        if ids.len() < 2 {
            return;
        }
        let index = ids
            .iter()
            .position(|id| *id == self.editor_groups.focused())
            .unwrap_or(0);
        let next = if reverse {
            (index + ids.len() - 1) % ids.len()
        } else {
            (index + 1) % ids.len()
        };
        self.focus_editor_group(ids[next]);
    }
    pub(crate) fn focus_adjacent_group(&mut self, direction: GoSubcommand) {
        if let Some(id) = self.adjacent_group(direction) {
            self.focus_editor_group(id);
        }
    }
    pub(crate) fn adjacent_group(&self, direction: GoSubcommand) -> Option<u64> {
        let (w, h, lh) = self.gpu.as_ref().map_or((1200.0, 760.0, 20.0), |gpu| {
            let (w, h) = gpu.renderer.size();
            (w as f32, h as f32, gpu.renderer.line_height())
        });
        let rects = self.group_rectangles(w, h, lh);
        adjacent_group_id(&rects, self.editor_groups.focused(), direction)
    }
}

fn collect_workspace_symbols(
    workspace: &Workspace,
    opened: &std::collections::HashMap<PathBuf, String>,
) -> Vec<GoSymbolItem> {
    let mut results = Vec::new();
    for path in workspace.fuzzy_files("", usize::MAX, || false) {
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        let text = opened
            .get(&path)
            .or_else(|| {
                opened.iter().find_map(|(open_path, text)| {
                    (open_path
                        .canonicalize()
                        .unwrap_or_else(|_| open_path.clone())
                        == canonical_path)
                        .then_some(text)
                })
            })
            .cloned()
            .or_else(|| std::fs::read_to_string(&path).ok());
        let Some(text) = text else { continue };
        let Some(language) = graphite_syntax::Language::from_path(&path) else {
            continue;
        };
        results.extend(
            graphite_syntax::declaration_headers(language, &text)
                .into_iter()
                .map(|symbol| GoSymbolItem {
                    name: symbol.text.clone(),
                    scope: symbol.text,
                    path: path.clone(),
                    line: symbol.line,
                }),
        );
    }
    results
}

pub(crate) fn go_row_height(row: GoMenuRow) -> f32 {
    if matches!(row, GoMenuRow::Separator) {
        SEPARATOR_HEIGHT
    } else {
        ROW_HEIGHT
    }
}
pub(crate) fn go_subrow_height(row: GoSubmenuRow) -> f32 {
    if matches!(row, GoSubmenuRow::Separator) {
        SEPARATOR_HEIGHT
    } else {
        ROW_HEIGHT
    }
}
pub(crate) fn go_menu_height(rows: &[GoMenuRow]) -> f32 {
    rows.iter().copied().map(go_row_height).sum::<f32>() + 8.0
}
pub(crate) fn go_submenu_height(rows: &[GoSubmenuRow]) -> f32 {
    rows.iter().copied().map(go_subrow_height).sum::<f32>() + 8.0
}

pub(crate) fn parse_line_column(input: &str) -> Option<(usize, usize)> {
    let input = input.trim();
    let (line, column) = input
        .split_once(':')
        .map_or((input, "1"), |(line, column)| (line, column));
    let line = line.trim().parse::<usize>().ok()?;
    let column = column.trim().parse::<usize>().ok()?;
    (line > 0 && column > 0).then_some((line, column))
}

pub(crate) fn adjacent_group_id(
    rects: &[GroupRect],
    focused: u64,
    direction: GoSubcommand,
) -> Option<u64> {
    let source = rects.iter().find(|rect| rect.id == focused)?;
    let (cx, cy) = (
        source.x + source.width * 0.5,
        source.y + source.height * 0.5,
    );
    rects
        .iter()
        .filter(|rect| rect.id != focused)
        .filter_map(|rect| {
            let (x, y) = (rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
            let (major, minor) = match direction {
                GoSubcommand::GroupLeft if x < cx => (cx - x, (cy - y).abs()),
                GoSubcommand::GroupRight if x > cx => (x - cx, (cy - y).abs()),
                GoSubcommand::GroupAbove if y < cy => (cy - y, (cx - x).abs()),
                GoSubcommand::GroupBelow if y > cy => (y - cy, (cx - x).abs()),
                _ => return None,
            };
            Some((major + minor * 1.5, rect.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_rows_and_submenus_follow_the_reference_and_disable_missing_targets() {
        let temp = crate::test_support::TempDir::new();
        let app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let commands = app
            .go_menu_rows()
            .into_iter()
            .filter_map(|row| match row {
                GoMenuRow::Item { command, .. } => Some(command),
                GoMenuRow::Separator => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            commands,
            [
                GoMenuCommand::Back,
                GoMenuCommand::Forward,
                GoMenuCommand::LastEdit,
                GoMenuCommand::SwitchEditor,
                GoMenuCommand::SwitchGroup,
                GoMenuCommand::QuickOpen,
                GoMenuCommand::WorkspaceSymbols,
                GoMenuCommand::EditorSymbols,
                GoMenuCommand::Definition,
                GoMenuCommand::Declaration,
                GoMenuCommand::TypeDefinition,
                GoMenuCommand::Implementations,
                GoMenuCommand::References,
                GoMenuCommand::GoToLine,
                GoMenuCommand::Bracket,
                GoMenuCommand::NextProblem,
                GoMenuCommand::PreviousProblem,
                GoMenuCommand::NextChange,
                GoMenuCommand::PreviousChange,
            ]
        );
        assert!(!app.go_menu_rows().iter().any(|row| matches!(
            row,
            GoMenuRow::Item {
                command: GoMenuCommand::LastEdit,
                enabled: true,
                ..
            }
        )));
        assert_eq!(
            app.go_submenu_rows(GoSubmenu::SwitchGroup)
                .iter()
                .filter(|row| matches!(row, GoSubmenuRow::Item { .. }))
                .count(),
            11
        );
        assert!(app
            .go_submenu_rows(GoSubmenu::SwitchGroup)
            .iter()
            .any(|row| matches!(
                row,
                GoSubmenuRow::Item {
                    command: GoSubcommand::Group(3),
                    enabled: false,
                    ..
                }
            )));
        assert_eq!(
            app.go_submenu_rows(GoSubmenu::SwitchEditor)
                .iter()
                .filter(|row| matches!(row, GoSubmenuRow::Item { .. }))
                .count(),
            8
        );
    }

    #[test]
    fn editor_symbols_use_parser_declarations_and_workspace_search_spans_roots() {
        let temp = crate::test_support::TempDir::new();
        let first = temp.path().join("one");
        let second = temp.path().join("two");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        let one = first.join("a.rs");
        let two = second.join("b.py");
        std::fs::write(&one, "fn alpha() {}\nstruct Beta;").unwrap();
        std::fs::write(&two, "def gamma():\n    pass\n").unwrap();
        let mut app = App::new_with_options(first.clone(), None, true);
        app.open_file(one.clone());
        assert!(app
            .editor_symbol_items()
            .iter()
            .any(|item| item.name.contains("alpha")));
        let workspace = Workspace::from_roots([first, second]).unwrap();
        let opened =
            std::collections::HashMap::from([(one.clone(), "fn unsaved_symbol() {}".to_owned())]);
        let symbols = collect_workspace_symbols(&workspace, &opened);
        assert!(symbols
            .iter()
            .any(|item| item.name.contains("unsaved_symbol")));
        assert!(symbols.iter().any(|item| item.name.contains("gamma")));
        assert!(symbols
            .iter()
            .all(|item| item.path.file_name() == one.file_name()
                || item.path.file_name() == two.file_name()));
    }

    #[test]
    fn line_picker_moves_the_cursor_and_group_switching_uses_visual_order() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.editor_mut().insert("first\nsecond");
        app.go_picker = Some(GoPicker {
            kind: GoPickerKind::LineColumn,
            query: "2:3".into(),
            items: Vec::new(),
            selected: 0,
            loading: false,
            generation: 0,
            message: None,
        });
        app.accept_go_picker();
        assert_eq!(app.editor().cursor_line_col(), (1, 2));
        let first = app.editor_groups.focused();
        app.split_editor_group(crate::app::editor_groups::SplitDirection::Vertical);
        app.focus_group_number(1);
        assert_eq!(app.editor_groups.focused(), first);
        app.focus_group_number(2);
        assert_ne!(app.editor_groups.focused(), first);
    }

    #[test]
    fn editor_switching_uses_visual_or_recent_order_as_requested() {
        let temp = crate::test_support::TempDir::new();
        let paths = ["a.rs", "b.rs", "c.rs"].map(|name| {
            let path = temp.path().join(name);
            std::fs::write(&path, "fn main() {}\n").unwrap();
            path
        });
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        for path in &paths {
            app.open_file(path.clone());
        }

        app.switch_go_editor(false, false, true);
        assert_eq!(app.editor().path(), Some(paths[1].as_path()));
        app.open_file(paths[2].clone());
        app.switch_go_editor(false, false, false);
        assert_eq!(app.editor().path(), Some(paths[0].as_path()));
    }
    #[test]
    fn line_column_parser_is_one_based_and_validates_input() {
        assert_eq!(parse_line_column("12"), Some((12, 1)));
        assert_eq!(parse_line_column("12:5"), Some((12, 5)));
        assert_eq!(parse_line_column("0:1"), None);
        assert_eq!(parse_line_column("x"), None);
    }
    #[test]
    fn directional_group_focus_chooses_nearest_group_in_that_direction() {
        let rects = [
            GroupRect {
                id: 1,
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            GroupRect {
                id: 2,
                x: 110.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            GroupRect {
                id: 3,
                x: 0.0,
                y: 110.0,
                width: 100.0,
                height: 100.0,
            },
        ];
        assert_eq!(
            adjacent_group_id(&rects, 1, GoSubcommand::GroupRight),
            Some(2)
        );
        assert_eq!(
            adjacent_group_id(&rects, 1, GoSubcommand::GroupBelow),
            Some(3)
        );
        assert_eq!(adjacent_group_id(&rects, 2, GoSubcommand::GroupRight), None);
    }
}
