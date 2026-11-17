use super::*;

const ROW_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const PADDING: f32 = 6.0;
const MIN_PROFILE_WIDTH: f32 = 210.0;
const MIN_ACTIONS_WIDTH: f32 = 340.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalToolbarMenuKind {
    ProfilePicker,
    MoreActions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TerminalToolbarMenuRow {
    Profile {
        index: usize,
        name: String,
        active: bool,
    },
    Action {
        label_key: &'static str,
        shortcut: &'static str,
        command: &'static str,
        enabled: bool,
    },
    Unavailable,
    Separator,
}

impl TerminalToolbarMenuRow {
    fn is_selectable(&self) -> bool {
        match self {
            Self::Profile { .. } => true,
            Self::Action { enabled, .. } => *enabled,
            Self::Unavailable | Self::Separator => false,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TerminalToolbarMenu {
    pub kind: TerminalToolbarMenuKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub selected: usize,
    pub rows: Vec<TerminalToolbarMenuRow>,
}

fn row_height(row: &TerminalToolbarMenuRow) -> f32 {
    if matches!(row, TerminalToolbarMenuRow::Separator) {
        SEPARATOR_HEIGHT
    } else {
        ROW_HEIGHT
    }
}

fn menu_height(rows: &[TerminalToolbarMenuRow]) -> f32 {
    PADDING * 2.0 + rows.iter().map(row_height).sum::<f32>()
}

fn menu_rect(menu: &TerminalToolbarMenu) -> (f32, f32, f32, f32) {
    (menu.x, menu.y, menu.width, menu_height(&menu.rows))
}

/// Places a dropdown anchored to `button` (`x, y, width, height`), opening upward when there is
/// not enough room below `button` above the status bar.
fn place_dropdown(
    button: (f32, f32, f32, f32),
    size: (f32, f32),
    window: (f32, f32),
    bottom_inset: f32,
) -> (f32, f32) {
    let (bx, by, bw, bh) = button;
    let (width, height) = size;
    let (window_width, window_height) = window;
    let max_x = (window_width - width - 4.0).max(4.0);
    let x = (bx + bw - width).clamp(4.0, max_x);
    let max_bottom = (window_height - bottom_inset - 4.0).max(0.0);
    let below_y = by + bh + 2.0;
    let y = if below_y + height <= max_bottom {
        below_y
    } else {
        (by - height - 2.0).max(4.0)
    };
    (x, y)
}

impl App {
    #[cfg(test)]
    pub(crate) fn toggle_terminal_profile_menu(
        &mut self,
        button: (f32, f32, f32, f32),
        window_width: f32,
        window_height: f32,
    ) {
        if self
            .terminal_toolbar_menu
            .as_ref()
            .is_some_and(|m| m.kind == TerminalToolbarMenuKind::ProfilePicker)
        {
            self.terminal_toolbar_menu = None;
            return;
        }
        self.open_terminal_profile_menu(button, window_width, window_height);
    }

    #[cfg(test)]
    pub(crate) fn toggle_terminal_more_actions_menu(
        &mut self,
        button: (f32, f32, f32, f32),
        window_width: f32,
        window_height: f32,
    ) {
        if self
            .terminal_toolbar_menu
            .as_ref()
            .is_some_and(|m| m.kind == TerminalToolbarMenuKind::MoreActions)
        {
            self.terminal_toolbar_menu = None;
            return;
        }
        self.open_terminal_more_actions_menu(button, window_width, window_height);
    }

    pub(crate) fn open_terminal_profile_menu(
        &mut self,
        button: (f32, f32, f32, f32),
        window_width: f32,
        window_height: f32,
    ) {
        let mut rows: Vec<TerminalToolbarMenuRow> = self
            .terminal_profiles
            .iter()
            .enumerate()
            .map(|(index, profile)| TerminalToolbarMenuRow::Profile {
                index,
                name: profile.name.clone(),
                active: index == self.selected_terminal_profile,
            })
            .collect();
        if rows.is_empty() {
            rows.push(TerminalToolbarMenuRow::Unavailable);
        }
        let selected = rows
            .iter()
            .position(|row| matches!(row, TerminalToolbarMenuRow::Profile { active: true, .. }))
            .unwrap_or(0);
        self.open_terminal_toolbar_menu(
            TerminalToolbarMenuKind::ProfilePicker,
            rows,
            selected,
            MIN_PROFILE_WIDTH,
            button,
            window_width,
            window_height,
        );
    }

    pub(crate) fn open_terminal_more_actions_menu(
        &mut self,
        button: (f32, f32, f32, f32),
        window_width: f32,
        window_height: f32,
    ) {
        let has_terminal = !self.terminals.is_empty();
        let has_active_file = self.documents[self.active_document()]
            .buffer
            .path()
            .is_some();
        let editor = self.editor();
        let has_selection = editor.selections().iter().any(|s| s.anchor != s.head);

        let action = |label_key, shortcut, command, enabled| TerminalToolbarMenuRow::Action {
            label_key,
            shortcut,
            command,
            enabled,
        };
        let rows = vec![
            action(
                "terminal.menu.scroll_previous_command",
                "Ctrl+UpArrow",
                "graphite.terminal.previousCommand",
                has_terminal,
            ),
            action(
                "terminal.menu.scroll_next_command",
                "Ctrl+DownArrow",
                "graphite.terminal.nextCommand",
                has_terminal,
            ),
            action(
                "terminal.menu.clear_terminal",
                "",
                "graphite.terminal.clear",
                has_terminal,
            ),
            action(
                "terminal.menu.run_active_file",
                "",
                "graphite.terminal.runActiveFile",
                has_active_file,
            ),
            action(
                "terminal.menu.run_selected_text",
                "",
                "graphite.terminal.runSelectedText",
                has_selection,
            ),
            TerminalToolbarMenuRow::Separator,
            action(
                "terminal.menu.new_terminal",
                "Ctrl+Shift+'",
                "graphite.terminal.new",
                true,
            ),
            action(
                "terminal.menu.split_terminal",
                "Ctrl+Shift+5",
                "graphite.terminal.split",
                has_terminal,
            ),
            action(
                "terminal.menu.kill_terminal",
                "",
                "graphite.terminal.close",
                has_terminal,
            ),
        ];
        let selected = rows
            .iter()
            .position(TerminalToolbarMenuRow::is_selectable)
            .unwrap_or(0);
        self.open_terminal_toolbar_menu(
            TerminalToolbarMenuKind::MoreActions,
            rows,
            selected,
            MIN_ACTIONS_WIDTH,
            button,
            window_width,
            window_height,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn open_terminal_toolbar_menu(
        &mut self,
        kind: TerminalToolbarMenuKind,
        rows: Vec<TerminalToolbarMenuRow>,
        selected: usize,
        min_width: f32,
        button: (f32, f32, f32, f32),
        window_width: f32,
        window_height: f32,
    ) {
        self.profile_menu = None;
        self.settings_menu = None;
        self.activity_bar_menu = None;
        self.workbench_menu = None;

        let char_width = self
            .gpu
            .as_ref()
            .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width());
        let width = rows
            .iter()
            .map(|row| match row {
                TerminalToolbarMenuRow::Profile { name, .. } => {
                    48.0 + name.chars().count() as f32 * char_width
                }
                TerminalToolbarMenuRow::Action {
                    label_key,
                    shortcut,
                    ..
                } => {
                    let label_len = self.t(label_key).chars().count();
                    let shortcut_len = shortcut.chars().count();
                    let gap = if shortcut.is_empty() { 0.0 } else { 28.0 };
                    36.0 + (label_len + shortcut_len) as f32 * char_width + gap
                }
                TerminalToolbarMenuRow::Unavailable => {
                    32.0 + self.t("terminal.no_profiles").chars().count() as f32 * char_width
                }
                TerminalToolbarMenuRow::Separator => 0.0,
            })
            .fold(min_width, f32::max)
            .min((window_width - 8.0).max(min_width));

        let height = menu_height(&rows);
        let status_height = self
            .gpu
            .as_ref()
            .map_or(20.0, |gpu| gpu.renderer.line_height());
        let (x, y) = place_dropdown(
            button,
            (width, height),
            (window_width, window_height),
            status_height,
        );
        self.terminal_toolbar_menu = Some(TerminalToolbarMenu {
            kind,
            x,
            y,
            width,
            selected,
            rows,
        });
    }

    pub(crate) fn terminal_toolbar_menu_contains(&self, x: f32, y: f32) -> bool {
        self.terminal_toolbar_menu.as_ref().is_some_and(|menu| {
            let (mx, my, mw, mh) = menu_rect(menu);
            x >= mx && x < mx + mw && y >= my && y < my + mh
        })
    }

    pub(crate) fn terminal_toolbar_menu_pointer_moved(&mut self, x: f32, y: f32) {
        let Some(menu) = self.terminal_toolbar_menu.as_mut() else {
            return;
        };
        let (mx, my, mw, _) = menu_rect(menu);
        if x < mx || x >= mx + mw {
            return;
        }
        let mut top = my + PADDING;
        for (index, row) in menu.rows.iter().enumerate() {
            let row_h = row_height(row);
            if y >= top && y < top + row_h {
                if row.is_selectable() {
                    menu.selected = index;
                }
                return;
            }
            top += row_h;
        }
    }

    pub(crate) fn terminal_toolbar_menu_click(&mut self, x: f32, y: f32) -> bool {
        let Some(menu) = self.terminal_toolbar_menu.clone() else {
            return false;
        };
        if !self.terminal_toolbar_menu_contains(x, y) {
            self.terminal_toolbar_menu = None;
            return false;
        }
        let mut top = menu.y + PADDING;
        for row in &menu.rows {
            let row_h = row_height(row);
            if y >= top && y < top + row_h {
                if row.is_selectable() {
                    self.execute_terminal_toolbar_menu_row(row.clone());
                }
                return true;
            }
            top += row_h;
        }
        true
    }

    pub(crate) fn terminal_toolbar_menu_key(&mut self, key: &winit::keyboard::Key) -> bool {
        use winit::keyboard::{Key, NamedKey};
        let Some(snapshot) = self.terminal_toolbar_menu.clone() else {
            return false;
        };
        let step = |from: usize, delta: isize| -> usize {
            let len = snapshot.rows.len();
            if len == 0 {
                return 0;
            }
            let mut idx = from as isize;
            for _ in 0..len {
                idx = (idx + delta).rem_euclid(len as isize);
                if snapshot.rows[idx as usize].is_selectable() {
                    return idx as usize;
                }
            }
            from
        };
        match key {
            Key::Named(NamedKey::Escape) => {
                self.terminal_toolbar_menu = None;
            }
            Key::Named(NamedKey::ArrowDown) => {
                if let Some(menu) = self.terminal_toolbar_menu.as_mut() {
                    menu.selected = step(menu.selected, 1);
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if let Some(menu) = self.terminal_toolbar_menu.as_mut() {
                    menu.selected = step(menu.selected, -1);
                }
            }
            Key::Named(NamedKey::Enter) => {
                if let Some(row) = snapshot.rows.get(snapshot.selected) {
                    if row.is_selectable() {
                        self.execute_terminal_toolbar_menu_row(row.clone());
                    }
                }
            }
            _ => {}
        }
        true
    }

    fn execute_terminal_toolbar_menu_row(&mut self, row: TerminalToolbarMenuRow) {
        self.terminal_toolbar_menu = None;
        match row {
            TerminalToolbarMenuRow::Profile { index, .. } => {
                if index < self.terminal_profiles.len() {
                    self.selected_terminal_profile = index;
                    self.terminal_problems = false;
                    self.terminal_focused = true;
                    self.create_terminal_tab();
                }
            }
            TerminalToolbarMenuRow::Action {
                command,
                enabled: true,
                ..
            } => {
                self.run_ui_command(command);
            }
            TerminalToolbarMenuRow::Action { enabled: false, .. }
            | TerminalToolbarMenuRow::Unavailable
            | TerminalToolbarMenuRow::Separator => {}
        }
    }

    pub(crate) fn draw_terminal_toolbar_menu(&self, renderer: &mut Renderer) {
        let Some(menu) = &self.terminal_toolbar_menu else {
            return;
        };
        let (x, y, width, height) = menu_rect(menu);
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let char_width = renderer.char_width().max(4.0);

        renderer.draw_rect(x, y, width, height, border);
        renderer.draw_rect(x + 1.0, y + 1.0, width - 2.0, height - 2.0, bg);

        let mut top = y + PADDING;
        for (index, row) in menu.rows.iter().enumerate() {
            let row_h = row_height(row);
            match row {
                TerminalToolbarMenuRow::Separator => {
                    renderer.draw_rect(x + 1.0, top + 4.0, width - 2.0, 1.0, border);
                }
                TerminalToolbarMenuRow::Profile { name, active, .. } => {
                    if menu.selected == index {
                        renderer.draw_rect(
                            x + 2.0,
                            top,
                            width - 4.0,
                            row_h,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    if *active {
                        renderer.draw_text("✓", x + 10.0, top + 4.0, text);
                    }
                    renderer.draw_text(name, x + 30.0, top + 4.0, text);
                }
                TerminalToolbarMenuRow::Action {
                    label_key,
                    shortcut,
                    enabled,
                    ..
                } => {
                    if *enabled && menu.selected == index {
                        renderer.draw_rect(
                            x + 2.0,
                            top,
                            width - 4.0,
                            row_h,
                            rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d)),
                        );
                    }
                    let fg = if *enabled { text } else { dim };
                    renderer.draw_text(self.t(label_key), x + 14.0, top + 4.0, fg);
                    if !shortcut.is_empty() {
                        let sx = x + width - 14.0 - shortcut.chars().count() as f32 * char_width;
                        renderer.draw_text(shortcut, sx, top + 4.0, dim);
                    }
                }
                TerminalToolbarMenuRow::Unavailable => {
                    renderer.draw_text(self.t("terminal.no_profiles"), x + 14.0, top + 4.0, dim);
                }
            }
            top += row_h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_app() -> (crate::test_support::TempDir, App) {
        let temp = crate::test_support::TempDir::new();
        let app = App::new_with_options(temp.path().to_path_buf(), None, true);
        (temp, app)
    }

    #[test]
    fn dropdown_opens_upward_when_near_bottom_and_downward_when_near_top() {
        let (_, up_y) = place_dropdown(
            (800.0, 620.0, 22.0, 22.0),
            (240.0, 200.0),
            (1000.0, 700.0),
            22.0,
        );
        assert!(up_y + 200.0 <= 620.0);

        let (_, down_y) = place_dropdown(
            (800.0, 40.0, 22.0, 22.0),
            (240.0, 200.0),
            (1000.0, 700.0),
            22.0,
        );
        assert_eq!(down_y, 64.0);
    }

    #[test]
    fn profile_picker_lists_profiles_and_selects_chosen_profile() {
        let (_temp, mut app) = temp_app();
        app.terminal_profiles = vec![
            crate::config::TerminalProfile {
                name: "PowerShell".into(),
                path: "powershell.exe".into(),
                args: Vec::new(),
            },
            crate::config::TerminalProfile {
                name: "Command Prompt".into(),
                path: "cmd.exe".into(),
                args: Vec::new(),
            },
        ];
        app.selected_terminal_profile = 0;

        app.toggle_terminal_profile_menu((800.0, 620.0, 16.0, 22.0), 1000.0, 700.0);
        let menu = app.terminal_toolbar_menu.as_ref().unwrap();
        assert_eq!(menu.kind, TerminalToolbarMenuKind::ProfilePicker);
        assert_eq!(menu.rows.len(), 2);
        assert!(matches!(
            &menu.rows[0],
            TerminalToolbarMenuRow::Profile { name, active: true, .. } if name == "PowerShell"
        ));
        assert!(matches!(
            &menu.rows[1],
            TerminalToolbarMenuRow::Profile { name, active: false, .. } if name == "Command Prompt"
        ));

        // Clicking the same button again toggles it closed.
        app.toggle_terminal_profile_menu((800.0, 620.0, 16.0, 22.0), 1000.0, 700.0);
        assert!(app.terminal_toolbar_menu.is_none());
    }

    #[test]
    fn more_actions_menu_includes_scroll_clear_run_and_terminal_controls() {
        let (_temp, mut app) = temp_app();
        app.toggle_terminal_more_actions_menu((920.0, 620.0, 22.0, 22.0), 1000.0, 700.0);
        let menu = app.terminal_toolbar_menu.as_ref().unwrap();
        assert_eq!(menu.kind, TerminalToolbarMenuKind::MoreActions);
        let commands: Vec<&str> = menu
            .rows
            .iter()
            .filter_map(|row| match row {
                TerminalToolbarMenuRow::Action { command, .. } => Some(*command),
                _ => None,
            })
            .collect();
        assert_eq!(
            commands,
            vec![
                "graphite.terminal.previousCommand",
                "graphite.terminal.nextCommand",
                "graphite.terminal.clear",
                "graphite.terminal.runActiveFile",
                "graphite.terminal.runSelectedText",
                "graphite.terminal.new",
                "graphite.terminal.split",
                "graphite.terminal.close",
            ]
        );
    }

    #[test]
    fn keyboard_escape_closes_terminal_toolbar_menu() {
        let (_temp, mut app) = temp_app();
        app.toggle_terminal_more_actions_menu((920.0, 620.0, 22.0, 22.0), 1000.0, 700.0);
        assert!(app.terminal_toolbar_menu.is_some());
        assert!(app.terminal_toolbar_menu_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Escape
        )));
        assert!(app.terminal_toolbar_menu.is_none());
    }
}
