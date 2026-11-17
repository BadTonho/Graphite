use super::*;

const ROW_HEIGHT: f32 = 27.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const PADDING: f32 = 7.0;
const MIN_WIDTH: f32 = 290.0;
const SUBMENU_WIDTH: f32 = 270.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsMenuCommand {
    CommandPalette,
    Profiles,
    Settings,
    Extensions,
    KeyboardShortcuts,
    Snippets,
    Tasks,
    Themes,
    LayoutDensity,
    Updated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsSubmenu {
    Profiles,
    Themes,
    Density,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SettingsSubcommand {
    Profile(String),
    CreateProfile,
    DarkTheme,
    LightTheme,
    ColorTheme(String),
    FileIconTheme,
    ProductIconTheme,
    DensityDefault,
    DensityCompact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsMenuRow {
    Separator,
    Item {
        command: SettingsMenuCommand,
        label: &'static str,
        shortcut: &'static str,
        enabled: bool,
        submenu: Option<SettingsSubmenu>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SettingsSubmenuRow {
    Separator,
    Item {
        command: SettingsSubcommand,
        label: String,
        shortcut: String,
        enabled: bool,
        checked: bool,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct SettingsMenu {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub selected: usize,
    pub submenu: Option<SettingsSubmenu>,
    pub submenu_selected: usize,
    pub profiles: Vec<String>,
}

fn row_height(row: SettingsMenuRow) -> f32 {
    if matches!(row, SettingsMenuRow::Separator) {
        SEPARATOR_HEIGHT
    } else {
        ROW_HEIGHT
    }
}

fn subrow_height(row: &SettingsSubmenuRow) -> f32 {
    if matches!(row, SettingsSubmenuRow::Separator) {
        SEPARATOR_HEIGHT
    } else {
        ROW_HEIGHT
    }
}

fn menu_rect(menu: &SettingsMenu) -> (f32, f32, f32, f32) {
    (menu.x, menu.y, menu.width, settings_menu_dimensions().1)
}

impl App {
    pub(crate) fn draw_settings_menu(
        &self,
        renderer: &mut Renderer,
        window_width: f32,
        window_height: f32,
    ) {
        let Some(menu) = &self.settings_menu else {
            return;
        };
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let (x, y, width, height) = menu_rect(menu);
        renderer.draw_rect(x, y, width, height, border);
        renderer.draw_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0, bg);
        let mut top = y + PADDING;
        for (index, row) in self.settings_menu_rows().into_iter().enumerate() {
            match row {
                SettingsMenuRow::Separator => {
                    renderer.draw_rect(x + 1.0, top + 4.0, width - 2.0, 1.0, border);
                    top += SEPARATOR_HEIGHT;
                }
                SettingsMenuRow::Item {
                    label,
                    shortcut,
                    enabled,
                    submenu,
                    ..
                } => {
                    if index == menu.selected {
                        renderer.draw_rect(
                            x + 2.0,
                            top,
                            width - 4.0,
                            ROW_HEIGHT,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    renderer.draw_text(
                        &self.t(label),
                        x + 14.0,
                        top + 5.0,
                        if enabled { text } else { dim },
                    );
                    if !shortcut.is_empty() {
                        let sw = shortcut.chars().count() as f32 * renderer.char_width();
                        renderer.draw_text(shortcut, x + width - sw - 25.0, top + 5.0, dim);
                    }
                    if submenu.is_some() {
                        renderer.draw_text("›", x + width - 19.0, top + 5.0, dim);
                    }
                    top += ROW_HEIGHT;
                }
            }
        }
        if let Some((sx, sy, sw, sh)) =
            self.settings_submenu_rect(menu, window_width, window_height)
        {
            renderer.draw_rect(sx, sy, sw, sh, border);
            renderer.draw_rect(sx + 1.0, sy + 1.0, sw - 2.0, sh - 2.0, bg);
            if let Some(submenu) = menu.submenu {
                let mut top = sy + PADDING;
                let mut item_index = 0;
                for row in self.settings_submenu_rows(submenu, &menu.profiles) {
                    match row {
                        SettingsSubmenuRow::Separator => {
                            renderer.draw_rect(sx + 1.0, top + 4.0, sw - 2.0, 1.0, border);
                            top += SEPARATOR_HEIGHT;
                        }
                        SettingsSubmenuRow::Item {
                            label,
                            shortcut,
                            enabled,
                            checked,
                            ..
                        } => {
                            if item_index == menu.submenu_selected {
                                renderer.draw_rect(
                                    sx + 2.0,
                                    top,
                                    sw - 4.0,
                                    ROW_HEIGHT,
                                    rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                                );
                            }
                            if checked {
                                renderer.draw_text("✓", sx + 10.0, top + 5.0, text);
                            }
                            renderer.draw_text(
                                &label,
                                sx + 28.0,
                                top + 5.0,
                                if enabled { text } else { dim },
                            );
                            if !shortcut.is_empty() {
                                let swidth =
                                    shortcut.chars().count() as f32 * renderer.char_width();
                                renderer.draw_text(
                                    &shortcut,
                                    sx + sw - swidth - 18.0,
                                    top + 5.0,
                                    dim,
                                );
                            }
                            item_index += 1;
                            top += ROW_HEIGHT;
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn settings_menu_rows(&self) -> Vec<SettingsMenuRow> {
        use SettingsMenuCommand as C;
        use SettingsMenuRow::{Item as I, Separator as S};
        vec![
            I {
                command: C::CommandPalette,
                label: "settings_menu.command_palette",
                shortcut: "Ctrl+Shift+P",
                enabled: true,
                submenu: None,
            },
            S,
            I {
                command: C::Profiles,
                label: "settings_menu.profiles",
                shortcut: "",
                enabled: true,
                submenu: Some(SettingsSubmenu::Profiles),
            },
            I {
                command: C::Settings,
                label: "settings_menu.settings",
                shortcut: "Ctrl+,",
                enabled: true,
                submenu: None,
            },
            I {
                command: C::Extensions,
                label: "settings_menu.extensions",
                shortcut: "Ctrl+Shift+X",
                enabled: true,
                submenu: None,
            },
            I {
                command: C::KeyboardShortcuts,
                label: "settings_menu.keyboard_shortcuts",
                shortcut: "Ctrl+K Ctrl+S",
                enabled: true,
                submenu: None,
            },
            I {
                command: C::Snippets,
                label: "settings_menu.snippets",
                shortcut: "",
                enabled: true,
                submenu: None,
            },
            I {
                command: C::Tasks,
                label: "settings_menu.tasks",
                shortcut: "",
                enabled: true,
                submenu: None,
            },
            I {
                command: C::Themes,
                label: "settings_menu.themes",
                shortcut: "",
                enabled: true,
                submenu: Some(SettingsSubmenu::Themes),
            },
            I {
                command: C::LayoutDensity,
                label: "settings_menu.layout_density",
                shortcut: "",
                enabled: true,
                submenu: Some(SettingsSubmenu::Density),
            },
            S,
            I {
                command: C::Updated,
                label: "settings_menu.updated",
                shortcut: "",
                enabled: false,
                submenu: None,
            },
        ]
    }

    pub(crate) fn settings_submenu_rows(
        &self,
        submenu: SettingsSubmenu,
        profiles: &[String],
    ) -> Vec<SettingsSubmenuRow> {
        use SettingsSubcommand as C;
        use SettingsSubmenuRow::{Item as I, Separator as S};
        match submenu {
            SettingsSubmenu::Profiles => {
                let current_name = self.profiles.as_ref().and_then(|manager| {
                    self.user_profiles().ok()?.into_iter().find(|name| {
                        manager.path(name).ok().as_deref() == Some(self.profile_root.as_path())
                    })
                });
                let mut rows = profiles
                    .iter()
                    .cloned()
                    .map(|name| I {
                        checked: current_name.as_deref() == Some(name.as_str()),
                        command: C::Profile(name.clone()),
                        label: name,
                        shortcut: String::new(),
                        enabled: true,
                    })
                    .collect::<Vec<_>>();
                if !rows.is_empty() {
                    rows.push(S);
                }
                rows.push(I {
                    command: C::CreateProfile,
                    label: self.t("settings_menu.create_profile").into(),
                    shortcut: String::new(),
                    enabled: true,
                    checked: false,
                });
                rows
            }
            SettingsSubmenu::Themes => {
                let dark = crate::app::theme::DARK_THEME.load(std::sync::atomic::Ordering::Relaxed);
                let selected = config::selected_color_theme(
                    &self.profile_root,
                    self.tree.root(),
                    &self.translator,
                );
                let installed = config::list_color_themes(&self.profile_root);
                let mut rows = vec![
                    I {
                        command: C::DarkTheme,
                        label: self.t("settings_menu.theme_dark").into(),
                        shortcut: String::new(),
                        enabled: true,
                        checked: selected.is_none() && dark,
                    },
                    I {
                        command: C::LightTheme,
                        label: self.t("settings_menu.theme_light").into(),
                        shortcut: String::new(),
                        enabled: true,
                        checked: selected.is_none() && !dark,
                    },
                ];
                if !installed.is_empty() {
                    rows.push(S);
                    rows.extend(installed.into_iter().map(|theme| I {
                        checked: selected.as_deref() == Some(theme.name.as_str()),
                        command: C::ColorTheme(theme.name.clone()),
                        label: theme.name,
                        shortcut: String::new(),
                        enabled: true,
                    }));
                }
                rows.push(S);
                rows.push(I {
                    command: C::FileIconTheme,
                    label: self.t("settings_menu.file_icon_theme").into(),
                    shortcut: String::new(),
                    enabled: true,
                    checked: false,
                });
                rows.push(I {
                    command: C::ProductIconTheme,
                    label: self.t("settings_menu.product_icon_theme").into(),
                    shortcut: String::new(),
                    enabled: false,
                    checked: false,
                });
                rows
            }
            SettingsSubmenu::Density => vec![
                I {
                    command: C::DensityDefault,
                    label: self.t("settings_menu.density_default").into(),
                    shortcut: String::new(),
                    enabled: true,
                    checked: self.activity_bar_preferences.size == config::ActivityBarSize::Default,
                },
                I {
                    command: C::DensityCompact,
                    label: self.t("settings_menu.density_compact").into(),
                    shortcut: String::new(),
                    enabled: true,
                    checked: self.activity_bar_preferences.size == config::ActivityBarSize::Compact,
                },
            ],
        }
    }

    pub(crate) fn open_settings_menu(
        &mut self,
        button: (f32, f32, f32, f32),
        width: f32,
        height: f32,
    ) {
        self.activity_bar_menu = None;
        self.workbench_menu = None;
        self.profile_menu = None;
        let rows = self.settings_menu_rows();
        let menu_height = PADDING * 2.0 + rows.iter().copied().map(row_height).sum::<f32>();
        let char_width = self
            .gpu
            .as_ref()
            .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width());
        let width_needed = rows
            .iter()
            .filter_map(|row| match row {
                SettingsMenuRow::Item {
                    label, shortcut, ..
                } => Some(
                    38.0 + self.t(label).chars().count() as f32 * char_width
                        + shortcut.chars().count() as f32 * char_width
                        + if shortcut.is_empty() { 24.0 } else { 42.0 },
                ),
                SettingsMenuRow::Separator => None,
            })
            .fold(MIN_WIDTH, f32::max);
        let status_height = self
            .gpu
            .as_ref()
            .map_or(20.0, |gpu| gpu.renderer.line_height());
        let width_needed = width_needed.min(width);
        let (x, y) = super::popup_placement::beside_button(
            button,
            (width_needed, menu_height),
            (width, height),
            status_height,
        );
        self.settings_menu = Some(SettingsMenu {
            x,
            y,
            width: width_needed,
            selected: 0,
            submenu: None,
            submenu_selected: 0,
            profiles: self.user_profiles().unwrap_or_default(),
        });
    }

    pub(crate) fn settings_menu_hit(&self, x: f32, y: f32) -> bool {
        self.settings_menu.as_ref().is_some_and(|menu| {
            let (mx, my, width, height) = menu_rect(menu);
            x >= mx && x < mx + width && y >= my && y < my + height
        })
    }

    pub(crate) fn settings_menu_contains(&self, x: f32, y: f32, width: f32, height: f32) -> bool {
        if self.settings_menu_hit(x, y) {
            return true;
        }
        self.settings_menu
            .as_ref()
            .and_then(|menu| self.settings_submenu_rect(menu, width, height))
            .is_some_and(|(sx, sy, sw, sh)| x >= sx && x < sx + sw && y >= sy && y < sy + sh)
    }

    fn settings_submenu_rect(
        &self,
        menu: &SettingsMenu,
        window_width: f32,
        window_height: f32,
    ) -> Option<(f32, f32, f32, f32)> {
        let submenu = menu.submenu?;
        let index = self
            .settings_menu_rows()
            .iter()
            .filter_map(|row| match row {
                SettingsMenuRow::Item {
                    submenu: Some(value),
                    ..
                } => Some(*value),
                _ => None,
            })
            .position(|value| value == submenu)?;
        let rows = self.settings_menu_rows();
        let root_index = rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| match row {
                SettingsMenuRow::Item {
                    submenu: Some(_), ..
                } => Some(index),
                _ => None,
            })
            .nth(index)?;
        let root_y = menu.y
            + PADDING
            + rows
                .iter()
                .take(root_index)
                .copied()
                .map(row_height)
                .sum::<f32>();
        let subrows = self.settings_submenu_rows(submenu, &menu.profiles);
        let sub_height = PADDING * 2.0 + subrows.iter().map(subrow_height).sum::<f32>();
        let char_width = self
            .gpu
            .as_ref()
            .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width());
        let submenu_width = subrows
            .iter()
            .filter_map(|row| match row {
                SettingsSubmenuRow::Item {
                    label, shortcut, ..
                } => Some(
                    48.0 + label.chars().count() as f32 * char_width
                        + shortcut.chars().count() as f32 * char_width,
                ),
                SettingsSubmenuRow::Separator => None,
            })
            .fold(SUBMENU_WIDTH, f32::max)
            .min(window_width);
        let x = if menu.x + menu.width + submenu_width <= window_width {
            menu.x + menu.width - 1.0
        } else {
            (menu.x - submenu_width + 1.0).max(0.0)
        };
        Some((
            x,
            root_y.clamp(0.0, (window_height - sub_height).max(0.0)),
            submenu_width,
            sub_height,
        ))
    }

    pub(crate) fn settings_menu_pointer_moved(&mut self, x: f32, y: f32, width: f32, height: f32) {
        let Some(menu) = self.settings_menu.clone() else {
            return;
        };
        if let Some((sx, sy, sw, sh)) = self.settings_submenu_rect(&menu, width, height) {
            if x >= sx && x < sx + sw && y >= sy + PADDING && y < sy + sh - PADDING {
                let rows = self.settings_submenu_rows(menu.submenu.unwrap(), &menu.profiles);
                let mut top = sy + PADDING;
                for (index, row) in rows.into_iter().enumerate() {
                    let row_h = subrow_height(&row);
                    if y >= top && y < top + row_h {
                        if let Some(current) = self.settings_menu.as_mut() {
                            current.submenu_selected = index;
                        }
                        return;
                    }
                    top += row_h;
                }
                return;
            }
        }
        if !self.settings_menu_hit(x, y) {
            return;
        }
        let (mx, my, _, _) = menu_rect(&menu);
        let local_y = y - my - PADDING;
        let mut top = 0.0;
        let mut index = None;
        for (i, row) in self.settings_menu_rows().into_iter().enumerate() {
            let h = row_height(row);
            if local_y >= top && local_y < top + h && !matches!(row, SettingsMenuRow::Separator) {
                index = Some(i);
                break;
            }
            top += h;
        }
        if let Some(index) = index {
            let submenu = match self.settings_menu_rows()[index] {
                SettingsMenuRow::Item { submenu, .. } => submenu,
                SettingsMenuRow::Separator => None,
            };
            if let Some(current) = self.settings_menu.as_mut() {
                current.selected = index;
                current.submenu = submenu;
                current.submenu_selected = 0;
            }
        }
        let _ = mx;
    }

    pub(crate) fn settings_menu_click(&mut self, x: f32, y: f32, width: f32, height: f32) -> bool {
        let Some(menu) = self.settings_menu.clone() else {
            return false;
        };
        if let Some((sx, sy, sw, sh)) = self.settings_submenu_rect(&menu, width, height) {
            if x >= sx && x < sx + sw && y >= sy + PADDING && y < sy + sh - PADDING {
                let submenu = menu.submenu.unwrap();
                let rows = self.settings_submenu_rows(submenu, &menu.profiles);
                let mut top = sy + PADDING;
                for row in rows {
                    let h = subrow_height(&row);
                    if y >= top && y < top + h {
                        if let SettingsSubmenuRow::Item {
                            command, enabled, ..
                        } = row
                        {
                            if enabled {
                                self.execute_settings_subcommand(command);
                            }
                        }
                        return true;
                    }
                    top += h;
                }
                return true;
            }
        }
        if !self.settings_menu_hit(x, y) {
            self.settings_menu = None;
            return true;
        }
        let (_, my, _, _) = menu_rect(&menu);
        let local_y = y - my - PADDING;
        let mut top = 0.0;
        for (index, row) in self.settings_menu_rows().into_iter().enumerate() {
            let h = row_height(row);
            if local_y >= top && local_y < top + h {
                if let SettingsMenuRow::Item {
                    command,
                    enabled,
                    submenu,
                    ..
                } = row
                {
                    if !enabled {
                        return true;
                    }
                    if let Some(menu) = self.settings_menu.as_mut() {
                        menu.selected = index;
                        menu.submenu = submenu;
                        menu.submenu_selected = 0;
                    }
                    if submenu.is_none() {
                        self.execute_settings_command(command);
                    }
                }
                return true;
            }
            top += h;
        }
        true
    }

    pub(crate) fn settings_menu_key(&mut self, key: &winit::keyboard::Key) -> bool {
        use winit::keyboard::{Key, NamedKey};
        let Some(snapshot) = self.settings_menu.clone() else {
            return false;
        };
        if let Some(submenu) = snapshot.submenu {
            let rows = self.settings_submenu_rows(submenu, &snapshot.profiles);
            let indices = rows
                .iter()
                .enumerate()
                .filter_map(|(index, row)| {
                    matches!(row, SettingsSubmenuRow::Item { .. }).then_some(index)
                })
                .collect::<Vec<_>>();
            let position = indices
                .iter()
                .position(|index| *index == snapshot.submenu_selected)
                .unwrap_or(0);
            match key {
                Key::Named(NamedKey::Escape) => self.settings_menu = None,
                Key::Named(NamedKey::ArrowLeft) => {
                    if let Some(menu) = self.settings_menu.as_mut() {
                        menu.submenu = None;
                    }
                }
                Key::Named(NamedKey::ArrowDown) => {
                    if let Some(menu) = self.settings_menu.as_mut() {
                        menu.submenu_selected = indices[(position + 1) % indices.len().max(1)];
                    }
                }
                Key::Named(NamedKey::ArrowUp) => {
                    if let Some(menu) = self.settings_menu.as_mut() {
                        menu.submenu_selected =
                            indices[(position + indices.len().max(1) - 1) % indices.len().max(1)];
                    }
                }
                Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                    if let Some(row) = rows.get(snapshot.submenu_selected).cloned() {
                        if let SettingsSubmenuRow::Item {
                            command, enabled, ..
                        } = row
                        {
                            if enabled {
                                self.execute_settings_subcommand(command);
                            }
                        }
                    }
                }
                _ => return false,
            }
            return true;
        }
        let rows = self.settings_menu_rows();
        let indices = rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| matches!(row, SettingsMenuRow::Item { .. }).then_some(index))
            .collect::<Vec<_>>();
        let position = indices
            .iter()
            .position(|index| *index == snapshot.selected)
            .unwrap_or(0);
        match key {
            Key::Named(NamedKey::Escape) => self.settings_menu = None,
            Key::Named(NamedKey::ArrowDown) => {
                if let Some(menu) = self.settings_menu.as_mut() {
                    menu.selected = indices[(position + 1) % indices.len().max(1)];
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if let Some(menu) = self.settings_menu.as_mut() {
                    menu.selected =
                        indices[(position + indices.len().max(1) - 1) % indices.len().max(1)];
                }
            }
            Key::Named(NamedKey::ArrowRight)
            | Key::Named(NamedKey::Enter)
            | Key::Named(NamedKey::Space) => {
                let row = rows.get(snapshot.selected).copied();
                if let Some(SettingsMenuRow::Item {
                    command,
                    enabled,
                    submenu,
                    ..
                }) = row
                {
                    if !enabled {
                        return true;
                    }
                    if let Some(submenu) = submenu {
                        if let Some(menu) = self.settings_menu.as_mut() {
                            menu.submenu = Some(submenu);
                            menu.submenu_selected = 0;
                        }
                    } else {
                        self.execute_settings_command(command);
                    }
                }
            }
            Key::Named(NamedKey::ArrowLeft) => self.settings_menu = None,
            _ => return false,
        }
        true
    }

    pub(crate) fn activate_settings_menu_accessibility_item(&mut self, index: usize) {
        let row = self.settings_menu_rows().into_iter().nth(index);
        if let Some(SettingsMenuRow::Item {
            command,
            enabled,
            submenu,
            ..
        }) = row
        {
            if enabled {
                if let Some(submenu) = submenu {
                    if let Some(menu) = self.settings_menu.as_mut() {
                        menu.selected = index;
                        menu.submenu = Some(submenu);
                        menu.submenu_selected = 0;
                    }
                } else {
                    self.execute_settings_command(command);
                }
            }
        }
    }

    pub(crate) fn activate_settings_submenu_accessibility_item(&mut self, index: usize) {
        let Some(menu) = self.settings_menu.clone() else {
            return;
        };
        if let Some(row) = menu.submenu.and_then(|submenu| {
            self.settings_submenu_rows(submenu, &menu.profiles)
                .into_iter()
                .nth(index)
        }) {
            if let SettingsSubmenuRow::Item {
                command, enabled, ..
            } = row
            {
                if enabled {
                    self.execute_settings_subcommand(command);
                }
            }
        }
    }

    fn execute_settings_command(&mut self, command: SettingsMenuCommand) {
        self.settings_menu = None;
        match command {
            SettingsMenuCommand::CommandPalette => self.open_command_palette(),
            SettingsMenuCommand::Profiles
            | SettingsMenuCommand::Themes
            | SettingsMenuCommand::LayoutDensity
            | SettingsMenuCommand::Updated => {}
            SettingsMenuCommand::Settings => {
                self.open_settings_editor(crate::app::settings_editor::SettingsScope::User)
            }
            SettingsMenuCommand::Extensions => self.open_plugins(),
            SettingsMenuCommand::KeyboardShortcuts => self.open_preference_file("keybindings.json"),
            SettingsMenuCommand::Snippets => self.open_snippet_configuration(),
            SettingsMenuCommand::Tasks => self.open_task_configuration(),
        }
    }

    pub(crate) fn execute_settings_subcommand(&mut self, command: SettingsSubcommand) {
        self.settings_menu = None;
        match command {
            SettingsSubcommand::Profile(name) => {
                if let Err(error) = self.switch_user_profile(&name) {
                    self.status = error;
                }
            }
            SettingsSubcommand::CreateProfile => {
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
            }
            SettingsSubcommand::DarkTheme => self.select_builtin_theme(true),
            SettingsSubcommand::LightTheme => self.select_builtin_theme(false),
            SettingsSubcommand::ColorTheme(name) => self.select_color_theme(&name),
            SettingsSubcommand::FileIconTheme => self.open_icon_theme_picker(),
            SettingsSubcommand::ProductIconTheme => {}
            SettingsSubcommand::DensityDefault => {
                self.set_activity_bar_size(config::ActivityBarSize::Default)
            }
            SettingsSubcommand::DensityCompact => {
                self.set_activity_bar_size(config::ActivityBarSize::Compact)
            }
        }
    }
}

pub(crate) fn settings_menu_dimensions() -> (f32, f32, f32) {
    let rows = [
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::CommandPalette,
            label: "",
            shortcut: "",
            enabled: true,
            submenu: None,
        },
        SettingsMenuRow::Separator,
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::Profiles,
            label: "",
            shortcut: "",
            enabled: true,
            submenu: None,
        },
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::Settings,
            label: "",
            shortcut: "",
            enabled: true,
            submenu: None,
        },
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::Extensions,
            label: "",
            shortcut: "",
            enabled: true,
            submenu: None,
        },
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::KeyboardShortcuts,
            label: "",
            shortcut: "",
            enabled: true,
            submenu: None,
        },
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::Snippets,
            label: "",
            shortcut: "",
            enabled: true,
            submenu: None,
        },
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::Tasks,
            label: "",
            shortcut: "",
            enabled: true,
            submenu: None,
        },
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::Themes,
            label: "",
            shortcut: "",
            enabled: true,
            submenu: None,
        },
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::LayoutDensity,
            label: "",
            shortcut: "",
            enabled: true,
            submenu: None,
        },
        SettingsMenuRow::Separator,
        SettingsMenuRow::Item {
            command: SettingsMenuCommand::Updated,
            label: "",
            shortcut: "",
            enabled: false,
            submenu: None,
        },
    ];
    (
        MIN_WIDTH,
        PADDING * 2.0 + rows.iter().copied().map(row_height).sum::<f32>(),
        ROW_HEIGHT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_rows_match_local_settings_actions_and_keep_update_state() {
        let temp = crate::test_support::TempDir::new();
        let app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let rows = app.settings_menu_rows();
        let labels = rows
            .iter()
            .filter_map(|row| match row {
                SettingsMenuRow::Item { label, .. } => Some(*label),
                SettingsMenuRow::Separator => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            [
                "settings_menu.command_palette",
                "settings_menu.profiles",
                "settings_menu.settings",
                "settings_menu.extensions",
                "settings_menu.keyboard_shortcuts",
                "settings_menu.snippets",
                "settings_menu.tasks",
                "settings_menu.themes",
                "settings_menu.layout_density",
                "settings_menu.updated",
            ]
        );
        assert!(matches!(
            rows.last(),
            Some(SettingsMenuRow::Item {
                command: SettingsMenuCommand::Updated,
                enabled: false,
                ..
            })
        ));
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(row, SettingsMenuRow::Separator))
                .count(),
            2
        );
    }

    #[test]
    fn compact_density_shrinks_the_activity_bar_buttons_and_is_profile_persisted() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profiles = Some(crate::app::phase5::Profiles::open(temp.path()).unwrap());
        app.profile_root = temp.path().join("profiles").join("Default");
        assert_eq!(
            app.activity_bar_preferences.size,
            config::ActivityBarSize::Default
        );
        app.execute_settings_subcommand(SettingsSubcommand::DensityCompact);
        assert_eq!(
            app.activity_bar_preferences.size,
            config::ActivityBarSize::Compact
        );
        let loaded = config::load_activity_bar_preferences(&app.profile_root);
        assert_eq!(loaded.size, config::ActivityBarSize::Compact);
        app.execute_settings_subcommand(SettingsSubcommand::DensityDefault);
        assert_eq!(
            app.activity_bar_preferences.size,
            config::ActivityBarSize::Default
        );
    }

    #[test]
    fn density_checks_follow_the_activity_bar_size_however_it_was_changed() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profile_root = temp.path().join("profile");
        let checked = |app: &App| {
            app.settings_submenu_rows(SettingsSubmenu::Density, &[])
                .into_iter()
                .filter_map(|row| match row {
                    SettingsSubmenuRow::Item {
                        command,
                        checked: true,
                        ..
                    } => Some(command),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(checked(&app), [SettingsSubcommand::DensityDefault]);
        // The View menu changes the same setting.
        app.set_activity_bar_size(config::ActivityBarSize::Compact);
        assert_eq!(checked(&app), [SettingsSubcommand::DensityCompact]);
    }

    #[test]
    fn menu_dimensions_fit_rows_and_compact_width_has_room_for_labels() {
        let (width, height, row_height) = settings_menu_dimensions();
        assert!(width >= MIN_WIDTH);
        assert_eq!(
            height,
            PADDING * 2.0 + ROW_HEIGHT * 10.0 + SEPARATOR_HEIGHT * 2.0
        );
        assert!(row_height >= 24.0);
    }

    #[test]
    fn pointer_opens_density_submenu_and_selects_compact() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profiles = Some(crate::app::phase5::Profiles::open(temp.path()).unwrap());
        app.profile_root = temp.path().join("profiles").join("Default");
        app.open_settings_menu((0.0, 740.0, 48.0, 40.0), 1000.0, 800.0);
        let menu = app.settings_menu.as_ref().unwrap().clone();
        let density_y = menu.y + PADDING + ROW_HEIGHT + SEPARATOR_HEIGHT + ROW_HEIGHT * 7.0 + 3.0;
        app.settings_menu_click(menu.x + 20.0, density_y, 1000.0, 800.0);
        assert_eq!(
            app.settings_menu.as_ref().unwrap().submenu,
            Some(SettingsSubmenu::Density)
        );
        let menu = app.settings_menu.as_ref().unwrap().clone();
        let (_, submenu_y, _, _) = app.settings_submenu_rect(&menu, 1000.0, 800.0).unwrap();
        app.settings_menu_click(
            menu.x + menu.width + 20.0,
            submenu_y + PADDING + ROW_HEIGHT + 3.0,
            1000.0,
            800.0,
        );
        assert_eq!(
            app.activity_bar_preferences.size,
            config::ActivityBarSize::Compact
        );
        assert!(app.settings_menu.is_none());
    }

    #[test]
    fn theme_submenu_lists_graphite_simple_and_selecting_it_applies_and_persists() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profile_root = temp.path().join("profile");
        config::install_bundled_themes(&app.profile_root);
        let simple = SettingsSubcommand::ColorTheme("Graphite Simple".into());
        let rows = app.settings_submenu_rows(SettingsSubmenu::Themes, &[]);
        assert!(rows.iter().any(|row| matches!(
            row,
            SettingsSubmenuRow::Item { command, checked: false, .. } if *command == simple
        )));
        app.execute_settings_subcommand(simple.clone());
        let rows = app.settings_submenu_rows(SettingsSubmenu::Themes, &[]);
        let checked: Vec<_> = rows
            .iter()
            .filter_map(|row| match row {
                SettingsSubmenuRow::Item {
                    command,
                    checked: true,
                    ..
                } => Some(command.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(checked, [simple]);
        assert!(
            std::fs::read_to_string(app.profile_root.join("settings.json"))
                .unwrap()
                .contains("Graphite Simple")
        );
        app.execute_settings_subcommand(SettingsSubcommand::DarkTheme);
        assert!(
            !std::fs::read_to_string(app.profile_root.join("settings.json"))
                .unwrap()
                .contains("Graphite Simple")
        );
        let rows = app.settings_submenu_rows(SettingsSubmenu::Themes, &[]);
        assert!(matches!(
            rows[0],
            SettingsSubmenuRow::Item {
                command: SettingsSubcommand::DarkTheme,
                checked: true,
                ..
            }
        ));
    }

    #[test]
    fn selecting_an_unknown_color_theme_reports_it_and_keeps_settings() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profile_root = temp.path().join("profile");
        app.execute_settings_subcommand(SettingsSubcommand::ColorTheme("Nope".into()));
        assert!(app.status.contains("Nope"), "{}", app.status);
        assert!(!app.profile_root.join("settings.json").exists());
    }

    #[test]
    fn theme_submenu_offers_file_icon_themes_but_not_product_icons() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.profile_root = temp.path().join("profile");
        let rows = app.settings_submenu_rows(SettingsSubmenu::Themes, &[]);
        assert!(matches!(
            rows[3],
            SettingsSubmenuRow::Item {
                enabled: true,
                command: SettingsSubcommand::FileIconTheme,
                ..
            }
        ));
        assert!(matches!(
            rows[4],
            SettingsSubmenuRow::Item {
                enabled: false,
                command: SettingsSubcommand::ProductIconTheme,
                ..
            }
        ));
    }
}
