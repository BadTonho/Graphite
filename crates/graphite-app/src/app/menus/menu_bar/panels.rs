//! Drawing the panel of each top-level menu: File, Edit, Selection, Go, Run, Terminal and
//! Help.

use super::*;

impl App {
    pub(super) fn draw_run_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
        use super::run_menu::RunMenuRow;
        let (window_width, window_height) = renderer.size();
        let char_width = renderer.char_width();
        let width = self.run_menu_width(char_width, window_width);
        let x = layout
            .menu_anchor(WorkbenchMenu::Run)
            .clamp(0.0, (window_width - width).max(0.0));
        let y = layout.height;
        let rows = self.run_menu_rows();
        let height = rows
            .iter()
            .map(|row| {
                if matches!(row, RunMenuRow::Separator) {
                    9.0
                } else {
                    28.0
                }
            })
            .sum::<f32>()
            + 8.0;
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let hover = rgb(Theme::color(Theme::ACTIVE_ROW, 0x3f3f46));
        renderer.draw_rect(x, y, width, height, border);
        renderer.draw_rect(
            x + 1.0,
            y + 1.0,
            (width - 2.0).max(0.0),
            (height - 2.0).max(0.0),
            bg,
        );
        let mut top = y + 4.0;
        let mut item = 0;
        for row in &rows {
            match row {
                RunMenuRow::Separator => {
                    renderer.draw_rect(x + 8.0, top + 4.0, (width - 16.0).max(0.0), 1.0, border);
                    top += 9.0;
                }
                RunMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    submenu,
                    ..
                } => {
                    if item == self.run_menu_selection {
                        renderer.draw_rect(x + 2.0, top, (width - 4.0).max(0.0), 28.0, hover);
                    }
                    let shortcut_width = char_width * shortcut.chars().count() as f32;
                    let reserved_right =
                        14.0 + if shortcut.is_empty() {
                            0.0
                        } else {
                            shortcut_width + 12.0
                        } + if *submenu { 18.0 } else { 0.0 };
                    let visible = Self::help_menu_visible_label(
                        self.t(label),
                        char_width,
                        (width - 30.0 - reserved_right).max(0.0),
                    );
                    renderer.draw_text(
                        &visible,
                        x + 30.0,
                        top + 6.0,
                        if *enabled { text } else { dim },
                    );
                    if !shortcut.is_empty() && shortcut_width + 14.0 <= width - 30.0 {
                        renderer.draw_text(
                            shortcut,
                            x + width - shortcut_width - 14.0 - if *submenu { 18.0 } else { 0.0 },
                            top + 6.0,
                            dim,
                        );
                    }
                    if *submenu {
                        renderer.draw_text("›", x + width - 20.0, top + 6.0, dim);
                    }
                    top += 28.0;
                    item += 1;
                }
            }
        }
        if self.run_breakpoint_submenu {
            let submenu_width = self.run_breakpoint_menu_width(char_width, window_width);
            let submenu_x =
                self.run_breakpoint_submenu_position(x, width, window_width, submenu_width);
            let submenu_rows = self.run_breakpoint_rows();
            let submenu_height = submenu_rows
                .iter()
                .map(|row| {
                    if matches!(row, RunMenuRow::Separator) {
                        9.0
                    } else {
                        28.0
                    }
                })
                .sum::<f32>()
                + 8.0;
            let submenu_y = self.run_breakpoint_submenu_top(y, window_height);
            renderer.draw_rect(submenu_x, submenu_y, submenu_width, submenu_height, border);
            renderer.draw_rect(
                submenu_x + 1.0,
                submenu_y + 1.0,
                (submenu_width - 2.0).max(0.0),
                (submenu_height - 2.0).max(0.0),
                bg,
            );
            let mut top = submenu_y + 4.0;
            let mut item = 0;
            for row in submenu_rows {
                match row {
                    RunMenuRow::Separator => {
                        renderer.draw_rect(
                            submenu_x + 8.0,
                            top + 4.0,
                            (submenu_width - 16.0).max(0.0),
                            1.0,
                            border,
                        );
                        top += 9.0;
                    }
                    RunMenuRow::Item {
                        label,
                        shortcut,
                        enabled,
                        ..
                    } => {
                        if item == self.run_breakpoint_selection {
                            renderer.draw_rect(
                                submenu_x + 2.0,
                                top,
                                (submenu_width - 4.0).max(0.0),
                                28.0,
                                hover,
                            );
                        }
                        let shortcut_width = char_width * shortcut.chars().count() as f32;
                        let visible = Self::help_menu_visible_label(
                            self.t(label),
                            char_width,
                            (submenu_width
                                - 30.0
                                - 14.0
                                - if shortcut.is_empty() {
                                    0.0
                                } else {
                                    shortcut_width + 12.0
                                })
                            .max(0.0),
                        );
                        renderer.draw_text(
                            &visible,
                            submenu_x + 30.0,
                            top + 6.0,
                            if enabled { text } else { dim },
                        );
                        if !shortcut.is_empty() && shortcut_width + 14.0 <= submenu_width - 30.0 {
                            renderer.draw_text(
                                shortcut,
                                submenu_x + submenu_width - shortcut_width - 14.0,
                                top + 6.0,
                                dim,
                            );
                        }
                        top += 28.0;
                        item += 1;
                    }
                }
            }
        }
    }

    pub(crate) fn draw_rename_prompt(&self, renderer: &mut Renderer) {
        let Some(prompt) = &self.rename_prompt else {
            return;
        };
        let (width, height) = renderer.size();
        let cw = renderer.char_width();
        let panel_width = (width * 0.72)
            .clamp(360.0, 700.0)
            .min((width - 24.0).max(0.0));
        let panel_height = 120.0;
        let x = ((width - panel_width) * 0.5).max(12.0);
        let y = ((height - panel_height) * 0.5).max(12.0);
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        renderer.draw_rect(x, y, panel_width, panel_height, border);
        renderer.draw_rect(
            x + 1.0,
            y + 1.0,
            (panel_width - 2.0).max(0.0),
            panel_height - 2.0,
            bg,
        );
        renderer.draw_text(self.t("lsp.rename.prompt"), x + 16.0, y + 18.0, text);
        let max_chars = ((panel_width - 32.0) / cw.max(1.0)).floor() as usize;
        let input = Self::help_menu_visible_label(
            &format!("{}▏", prompt.value),
            cw,
            (max_chars as f32 * cw).max(0.0),
        );
        renderer.draw_rect(x + 12.0, y + 48.0, panel_width - 24.0, 28.0, border);
        renderer.draw_text(&input, x + 18.0, y + 54.0, text);
        renderer.draw_text(self.t("lsp.rename.hint"), x + 16.0, y + 90.0, dim);
    }

    pub(super) fn draw_run_breakpoint_prompt(&self, renderer: &mut Renderer) {
        let Some(prompt) = &self.run_breakpoint_prompt else {
            return;
        };
        let (width, height) = renderer.size();
        let cw = renderer.char_width();
        let panel_width = (width * 0.72)
            .clamp(360.0, 700.0)
            .min((width - 24.0).max(0.0));
        let panel_height = 120.0;
        let x = ((width - panel_width) * 0.5).max(12.0);
        let y = ((height - panel_height) * 0.5).max(12.0);
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        renderer.draw_rect(x, y, panel_width, panel_height, border);
        renderer.draw_rect(
            x + 1.0,
            y + 1.0,
            (panel_width - 2.0).max(0.0),
            panel_height - 2.0,
            bg,
        );
        let label = match prompt.kind {
            super::run_menu::BreakpointPromptKind::Conditional => "run.prompt.conditional",
            super::run_menu::BreakpointPromptKind::Edit => "run.prompt.edit",
            super::run_menu::BreakpointPromptKind::Logpoint => "run.prompt.logpoint",
        };
        renderer.draw_text(self.t(label), x + 16.0, y + 18.0, text);
        let max_chars = ((panel_width - 32.0) / cw.max(1.0)).floor() as usize;
        let input = Self::help_menu_visible_label(
            &format!("{}▏", prompt.value),
            cw,
            (max_chars as f32 * cw).max(0.0),
        );
        renderer.draw_rect(x + 12.0, y + 48.0, panel_width - 24.0, 28.0, border);
        renderer.draw_text(&input, x + 18.0, y + 54.0, text);
        renderer.draw_text(self.t("run.prompt.hint"), x + 16.0, y + 90.0, dim);
    }

    pub(super) fn draw_help_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
        use super::help_menu::HelpRow;
        let (window_width, _) = renderer.size();
        let char_width = renderer.char_width();
        let width = self.help_menu_width(char_width, window_width);
        let x = layout
            .menu_anchor(WorkbenchMenu::Help)
            .clamp(0.0, (window_width - width).max(0.0));
        let y = layout.height;
        let rows = self.help_menu_rows();
        let height = rows
            .iter()
            .map(|row| {
                if matches!(row, HelpRow::Separator) {
                    9.0
                } else {
                    28.0
                }
            })
            .sum::<f32>()
            + 8.0;
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let hover = rgb(Theme::color(Theme::ACTIVE_ROW, 0x3f3f46));
        renderer.draw_rect(x, y, width, height, border);
        renderer.draw_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0, bg);
        let mut top = y + 4.0;
        let mut item = 0;
        for row in rows {
            match row {
                HelpRow::Separator => {
                    renderer.draw_rect(x + 8.0, top + 4.0, width - 16.0, 1.0, border);
                    top += 9.0
                }
                HelpRow::Item {
                    label,
                    shortcut,
                    enabled,
                    ..
                } => {
                    if item == self.help_menu_selection {
                        renderer.draw_rect(x + 2.0, top, width - 4.0, 28.0, hover);
                    }
                    let shortcut_width = char_width * shortcut.chars().count() as f32;
                    let label_width = (width
                        - 30.0
                        - 14.0
                        - if shortcut.is_empty() {
                            0.0
                        } else {
                            shortcut_width + 14.0
                        })
                    .max(0.0);
                    let visible_label =
                        Self::help_menu_visible_label(self.t(label), char_width, label_width);
                    renderer.draw_text(
                        &visible_label,
                        x + 30.0,
                        top + 6.0,
                        if enabled { text } else { dim },
                    );
                    if !shortcut.is_empty() && shortcut_width + 14.0 <= width - 30.0 {
                        renderer.draw_text(
                            shortcut,
                            x + width - shortcut_width - 14.0,
                            top + 6.0,
                            dim,
                        );
                    }
                    top += 28.0;
                    item += 1;
                }
            }
        }
    }

    pub(super) fn draw_file_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
        use super::file_menu::FileMenuRow;
        let (width, window_height) = renderer.size();
        let popup_width = super::file_menu::FILE_MENU_WIDTH.min(width);
        let popup_x = layout
            .menu_anchor(WorkbenchMenu::File)
            .clamp(0.0, width - popup_width);
        let popup_y = layout.height;
        let rows = self.file_menu_rows(None);
        let main_selected_command = match self.file_menu_submenu {
            Some(super::file_menu::FileMenuSubmenu::NewWindowProfile) => {
                Some(super::file_menu::FileMenuCommand::NewWindowProfile)
            }
            Some(super::file_menu::FileMenuSubmenu::OpenRecent) => {
                Some(super::file_menu::FileMenuCommand::OpenRecent)
            }
            Some(_) => Some(super::file_menu::FileMenuCommand::Preferences),
            None => self.file_menu_selected().map(|(command, _, _)| command),
        };
        let main_selected_index = rows
            .iter()
            .filter_map(|row| match row {
                FileMenuRow::Item { command, .. } => Some(*command),
                FileMenuRow::Separator => None,
            })
            .position(|command| Some(command) == main_selected_command);
        let popup_height = rows
            .iter()
            .map(|row| Self::file_menu_row_height(*row))
            .sum::<f32>()
            + 8.0;
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let foreground = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let selected = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        renderer.draw_rect(popup_x, popup_y, popup_width, popup_height, border);
        renderer.draw_rect(
            popup_x + 1.0,
            popup_y + 1.0,
            popup_width - 2.0,
            popup_height - 2.0,
            background,
        );
        let mut y = popup_y + 4.0;
        let mut item_index = 0;
        for row in rows.iter().copied() {
            match row {
                FileMenuRow::Separator => {
                    renderer.draw_rect(popup_x + 8.0, y + 4.0, popup_width - 16.0, 1.0, border);
                    y += Self::file_menu_row_height(row);
                }
                FileMenuRow::Item {
                    command,
                    label,
                    shortcut,
                    submenu,
                    enabled,
                    checked,
                } => {
                    if Some(item_index) == main_selected_index {
                        renderer.draw_rect(popup_x + 2.0, y, popup_width - 4.0, 26.0, selected);
                    }
                    if checked {
                        renderer.draw_text("✓", popup_x + 9.0, y + 5.0, foreground);
                    }
                    let label = fit_file_menu_label(
                        self.file_menu_label(label, command),
                        ((popup_width
                            - 50.0
                            - if shortcut.is_empty() {
                                12.0
                            } else {
                                shortcut.chars().count() as f32 * renderer.char_width() + 42.0
                            })
                            / renderer.char_width())
                        .floor()
                        .max(1.0) as usize,
                    );
                    renderer.draw_text(
                        &label,
                        popup_x + 30.0,
                        y + 5.0,
                        if enabled { foreground } else { dim },
                    );
                    if !shortcut.is_empty() {
                        let x = popup_x + popup_width
                            - 12.0
                            - shortcut.chars().count() as f32 * renderer.char_width();
                        renderer.draw_text(shortcut, x, y + 5.0, dim);
                    }
                    if submenu.is_some() {
                        renderer.draw_text("›", popup_x + popup_width - 18.0, y + 4.0, dim);
                    }
                    item_index += 1;
                    y += 26.0;
                }
            }
        }
        if let Some(open) = self.file_menu_submenu {
            let (submenu_x, nested_x, submenu_width) =
                Self::file_panel_xs(popup_x, popup_width, width);
            let nested_row = self.file_menu_nested.and_then(|nested| {
                self.file_menu_rows(Some(open))
                    .into_iter()
                    .filter_map(|row| match row {
                        FileMenuRow::Item { submenu, .. } => Some(submenu),
                        FileMenuRow::Separator => None,
                    })
                    .position(|submenu| submenu == Some(nested))
            });
            self.draw_file_panel(
                renderer,
                open,
                submenu_x,
                self.file_submenu_top(window_height),
                submenu_width,
                Some(nested_row.unwrap_or(self.file_menu_selection)),
            );
            if let Some(nested) = self.file_menu_nested {
                self.draw_file_nested_panel(
                    renderer,
                    nested,
                    nested_x,
                    self.file_nested_top(window_height),
                    submenu_width,
                    Some(self.file_menu_nested_selection),
                );
            }
        }
    }

    /// Draws one File submenu panel with its top-left corner at `(x, top)`.
    pub(super) fn draw_file_panel(
        &self,
        renderer: &mut Renderer,
        open: super::file_menu::FileMenuSubmenu,
        x: f32,
        top: f32,
        width: f32,
        selected_index: Option<usize>,
    ) {
        use super::file_menu::FileMenuRow;
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let foreground = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let selected = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        let submenu_rows = self.file_menu_rows(Some(open));
        let submenu_height = submenu_rows
            .iter()
            .map(|row| Self::file_menu_row_height(*row))
            .sum::<f32>()
            + 8.0;
        renderer.draw_rect(x, top, width, submenu_height, border);
        renderer.draw_rect(
            x + 1.0,
            top + 1.0,
            width - 2.0,
            submenu_height - 2.0,
            background,
        );
        let mut y = top + 4.0;
        let mut submenu_item_index = 0;
        for row in submenu_rows.iter().copied() {
            match row {
                FileMenuRow::Separator => {
                    renderer.draw_rect(x + 8.0, y + 4.0, width - 16.0, 1.0, border);
                    y += Self::file_menu_row_height(row);
                }
                FileMenuRow::Item {
                    command,
                    label,
                    shortcut,
                    submenu,
                    enabled,
                    checked,
                } => {
                    if Some(submenu_item_index) == selected_index {
                        renderer.draw_rect(x + 2.0, y, width - 4.0, 26.0, selected);
                    }
                    if checked {
                        renderer.draw_text("✓", x + 9.0, y + 5.0, foreground);
                    }
                    let label = fit_file_menu_label(
                        self.file_menu_label(label, command),
                        ((width
                            - 50.0
                            - if shortcut.is_empty() {
                                12.0
                            } else {
                                shortcut.chars().count() as f32 * renderer.char_width() + 42.0
                            })
                            / renderer.char_width())
                        .floor()
                        .max(1.0) as usize,
                    );
                    renderer.draw_text(
                        &label,
                        x + 30.0,
                        y + 5.0,
                        if enabled { foreground } else { dim },
                    );
                    if !shortcut.is_empty() {
                        renderer.draw_text(
                            shortcut,
                            x + width
                                - 12.0
                                - shortcut.chars().count() as f32 * renderer.char_width(),
                            y + 5.0,
                            dim,
                        );
                    }
                    if submenu.is_some() {
                        renderer.draw_text("›", x + width - 18.0, y + 4.0, dim);
                    }
                    submenu_item_index += 1;
                    y += 26.0;
                }
            }
        }
    }

    /// Draws the nested Profiles or Themes panel, which shows the same rows as the settings
    /// gear menu, with its top-left corner at `(x, top)`.
    pub(super) fn draw_file_nested_panel(
        &self,
        renderer: &mut Renderer,
        open: super::file_menu::FileMenuSubmenu,
        x: f32,
        top: f32,
        width: f32,
        selected_index: Option<usize>,
    ) {
        use super::settings_menu::SettingsSubmenuRow;
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let foreground = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let highlight = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        let height = self.file_panel_height(open);
        renderer.draw_rect(x, top, width, height, border);
        renderer.draw_rect(x + 1.0, top + 1.0, width - 2.0, height - 2.0, background);
        let mut y = top + 4.0;
        let mut index = 0;
        for row in self.file_nested_rows(open) {
            match row {
                SettingsSubmenuRow::Separator => {
                    renderer.draw_rect(x + 8.0, y + 4.0, width - 16.0, 1.0, border);
                    y += 9.0;
                }
                SettingsSubmenuRow::Item {
                    label,
                    enabled,
                    checked,
                    ..
                } => {
                    if Some(index) == selected_index {
                        renderer.draw_rect(x + 2.0, y, width - 4.0, 26.0, highlight);
                    }
                    if checked {
                        renderer.draw_text("✓", x + 9.0, y + 5.0, foreground);
                    }
                    let max_chars = ((width - 50.0 - 12.0) / renderer.char_width())
                        .floor()
                        .max(1.0) as usize;
                    renderer.draw_text(
                        &fit_file_menu_label(self.t(&label).to_owned(), max_chars),
                        x + 30.0,
                        y + 5.0,
                        if enabled { foreground } else { dim },
                    );
                    index += 1;
                    y += 26.0;
                }
            }
        }
    }

    pub(super) fn draw_terminal_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
        use super::terminal_menu::TerminalMenuRow;
        let (width, _) = renderer.size();
        let popup_width = super::terminal_menu::TERMINAL_MENU_WIDTH.min(width);
        let popup_x = layout
            .menu_anchor(WorkbenchMenu::Terminal)
            .clamp(0.0, width - popup_width);
        let popup_y = layout.height;
        let rows = self.terminal_menu_rows();
        let popup_height = rows
            .iter()
            .map(|row| Self::terminal_menu_row_height(*row))
            .sum::<f32>()
            + 8.0;
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let foreground = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let selected = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        renderer.draw_rect(popup_x, popup_y, popup_width, popup_height, border);
        renderer.draw_rect(
            popup_x + 1.0,
            popup_y + 1.0,
            popup_width - 2.0,
            popup_height - 2.0,
            background,
        );
        let mut y = popup_y + 4.0;
        let mut item_index = 0;
        for row in rows {
            match row {
                TerminalMenuRow::Separator => {
                    renderer.draw_rect(popup_x + 8.0, y + 4.0, popup_width - 16.0, 1.0, border);
                    y += Self::terminal_menu_row_height(row);
                }
                TerminalMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    ..
                } => {
                    if item_index == self.terminal_menu_selection {
                        renderer.draw_rect(popup_x + 2.0, y, popup_width - 4.0, 26.0, selected);
                    }
                    renderer.draw_text(
                        self.t(label),
                        popup_x + 30.0,
                        y + 5.0,
                        if enabled { foreground } else { dim },
                    );
                    if !shortcut.is_empty() {
                        renderer.draw_text(
                            shortcut,
                            popup_x + popup_width
                                - 12.0
                                - shortcut.chars().count() as f32 * renderer.char_width(),
                            y + 5.0,
                            dim,
                        );
                    }
                    item_index += 1;
                    y += Self::terminal_menu_row_height(row);
                }
            }
        }
    }

    pub(super) fn draw_edit_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
        use super::edit_menu::EditMenuRow;
        let (width, _) = renderer.size();
        let popup_width = super::edit_menu::EDIT_MENU_WIDTH.min(width);
        let popup_x = layout
            .menu_anchor(WorkbenchMenu::Edit)
            .clamp(0.0, width - popup_width);
        let popup_y = layout.height;
        let rows = self.edit_menu_rows();
        let popup_height = rows
            .iter()
            .map(|row| Self::edit_menu_row_height(*row))
            .sum::<f32>()
            + 8.0;
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let foreground = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let selected = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        renderer.draw_rect(popup_x, popup_y, popup_width, popup_height, border);
        renderer.draw_rect(
            popup_x + 1.0,
            popup_y + 1.0,
            popup_width - 2.0,
            popup_height - 2.0,
            background,
        );
        let mut y = popup_y + 4.0;
        let mut item_index = 0;
        for row in rows {
            match row {
                EditMenuRow::Separator => {
                    renderer.draw_rect(popup_x + 8.0, y + 4.0, popup_width - 16.0, 1.0, border);
                    y += Self::edit_menu_row_height(row);
                }
                EditMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    ..
                } => {
                    if item_index == self.edit_menu_selection {
                        renderer.draw_rect(popup_x + 2.0, y, popup_width - 4.0, 26.0, selected);
                    }
                    let label = self.edit_menu_label(label);
                    renderer.draw_text(
                        &label,
                        popup_x + 30.0,
                        y + 5.0,
                        if enabled { foreground } else { dim },
                    );
                    renderer.draw_text(
                        shortcut,
                        popup_x + popup_width
                            - 12.0
                            - shortcut.chars().count() as f32 * renderer.char_width(),
                        y + 5.0,
                        dim,
                    );
                    item_index += 1;
                    y += Self::edit_menu_row_height(row);
                }
            }
        }
    }

    pub(super) fn draw_selection_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
        use super::selection_menu::SelectionMenuRow;
        let (width, _) = renderer.size();
        let char_width = renderer.char_width();
        let popup_width = self.selection_menu_width(char_width, width);
        let popup_x = layout
            .menu_anchor(WorkbenchMenu::Selection)
            .clamp(0.0, (width - popup_width).max(0.0));
        let popup_y = layout.height;
        let rows = self.selection_menu_rows();
        let popup_height = super::selection_menu::selection_menu_height(&rows);
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let foreground = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let selected = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        renderer.draw_rect(popup_x, popup_y, popup_width, popup_height, border);
        renderer.draw_rect(
            popup_x + 1.0,
            popup_y + 1.0,
            (popup_width - 2.0).max(0.0),
            (popup_height - 2.0).max(0.0),
            background,
        );
        let mut y = popup_y + 4.0;
        let mut item_index = 0;
        for row in rows {
            match row {
                SelectionMenuRow::Separator => {
                    renderer.draw_rect(popup_x + 8.0, y + 4.0, popup_width - 16.0, 1.0, border);
                    y += super::selection_menu::selection_menu_row_height(row);
                }
                SelectionMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    checked,
                    ..
                } => {
                    if item_index == self.selection_menu_selection {
                        renderer.draw_rect(popup_x + 2.0, y, popup_width - 4.0, 28.0, selected);
                    }
                    if checked {
                        renderer.draw_text("✓", popup_x + 10.0, y + 6.0, foreground);
                    }
                    let shortcut_width = shortcut.chars().count() as f32 * char_width;
                    let label_width = (popup_width - 44.0 - shortcut_width - 20.0).max(0.0);
                    let text =
                        Self::help_menu_visible_label(self.t(label), char_width, label_width);
                    renderer.draw_text(
                        &text,
                        popup_x + 30.0,
                        y + 6.0,
                        if enabled { foreground } else { dim },
                    );
                    if !shortcut.is_empty() {
                        renderer.draw_text(
                            shortcut,
                            popup_x + popup_width - 12.0 - shortcut_width,
                            y + 6.0,
                            dim,
                        );
                    }
                    item_index += 1;
                    y += super::selection_menu::selection_menu_row_height(row);
                }
            }
        }
    }

    pub(super) fn draw_go_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
        use super::go_menu::{GoMenuRow as R, GoSubmenuRow as SR};
        let (width, _) = renderer.size();
        let cw = renderer.char_width();
        let lh = renderer.line_height();
        let popup_width = self.go_menu_width(cw, width);
        let popup_x = layout
            .menu_anchor(WorkbenchMenu::Go)
            .clamp(0.0, (width - popup_width).max(0.0));
        let popup_y = layout.height;
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let selected = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        renderer.draw_rect(
            popup_x,
            popup_y,
            popup_width,
            super::go_menu::go_menu_height(&self.go_menu_rows()),
            border,
        );
        renderer.draw_rect(
            popup_x + 1.0,
            popup_y + 1.0,
            popup_width - 2.0,
            super::go_menu::go_menu_height(&self.go_menu_rows()) - 2.0,
            bg,
        );
        let mut y = popup_y + 4.0;
        let mut index = 0;
        for row in self.go_menu_rows() {
            match row {
                R::Separator => {
                    renderer.draw_rect(popup_x + 8.0, y + 4.0, popup_width - 16.0, 1.0, border);
                    y += super::go_menu::go_row_height(row);
                }
                R::Item {
                    label,
                    shortcut,
                    enabled,
                    submenu,
                    ..
                } => {
                    if index == self.go_menu_selection
                        || submenu.is_some_and(|s| Some(s) == self.go_submenu)
                    {
                        renderer.draw_rect(popup_x + 2.0, y, popup_width - 4.0, 28.0, selected);
                    }
                    let shortcut_width = shortcut.chars().count() as f32 * cw;
                    let label_width = (popup_width - 48.0 - shortcut_width - 20.0).max(0.0);
                    let visible = Self::help_menu_visible_label(self.t(label), cw, label_width);
                    renderer.draw_text(
                        &visible,
                        popup_x + 14.0,
                        y + 5.0,
                        if enabled { text } else { dim },
                    );
                    if !shortcut.is_empty() {
                        renderer.draw_text(
                            shortcut,
                            popup_x + popup_width - 18.0 - shortcut_width,
                            y + 5.0,
                            dim,
                        );
                    }
                    if submenu.is_some() {
                        renderer.draw_text("›", popup_x + popup_width - 13.0, y + 5.0, dim);
                    }
                    index += 1;
                    y += super::go_menu::go_row_height(row);
                }
            }
        }
        if let Some(submenu) = self.go_submenu {
            let sub_width = self.go_submenu_width(submenu, cw, width);
            let sub_x = if popup_x + popup_width + sub_width <= width {
                popup_x + popup_width
            } else {
                (popup_x - sub_width).max(0.0)
            };
            let sub_y = self.go_submenu_anchor_y(popup_y, popup_x, popup_width);
            let rows = self.go_submenu_rows(submenu);
            let sub_height = super::go_menu::go_submenu_height(&rows);
            renderer.draw_rect(sub_x, sub_y, sub_width, sub_height, border);
            renderer.draw_rect(
                sub_x + 1.0,
                sub_y + 1.0,
                sub_width - 2.0,
                sub_height - 2.0,
                bg,
            );
            let mut sy = sub_y + 4.0;
            let mut si = 0;
            for row in rows {
                match row {
                    SR::Separator => {
                        renderer.draw_rect(sub_x + 8.0, sy + 4.0, sub_width - 16.0, 1.0, border);
                        sy += super::go_menu::go_subrow_height(row);
                    }
                    SR::Item {
                        label,
                        shortcut,
                        enabled,
                        ..
                    } => {
                        if si == self.go_submenu_selection {
                            renderer.draw_rect(sub_x + 2.0, sy, sub_width - 4.0, 28.0, selected);
                        }
                        let shortcut_width = shortcut.chars().count() as f32 * cw;
                        let label_width = (sub_width - 30.0 - shortcut_width - 22.0).max(0.0);
                        let visible = Self::help_menu_visible_label(self.t(label), cw, label_width);
                        renderer.draw_text(
                            &visible,
                            sub_x + 14.0,
                            sy + 5.0,
                            if enabled { text } else { dim },
                        );
                        if !shortcut.is_empty() {
                            renderer.draw_text(
                                shortcut,
                                sub_x + sub_width - 18.0 - shortcut_width,
                                sy + 5.0,
                                dim,
                            );
                        }
                        si += 1;
                        sy += super::go_menu::go_subrow_height(row);
                    }
                }
            }
        }
        let _ = lh;
    }

    pub(crate) fn file_menu_label(
        &self,
        key: &'static str,
        command: super::file_menu::FileMenuCommand,
    ) -> String {
        use super::file_menu::FileMenuCommand as C;
        match command {
            C::SwitchProfile(index) => self
                .user_profiles()
                .ok()
                .and_then(|profiles| profiles.get(index).cloned())
                .unwrap_or_default(),
            C::RecentPath(index) => self
                .recent_workspaces
                .get(index)
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            _ => self.t(key).to_owned(),
        }
    }
}
