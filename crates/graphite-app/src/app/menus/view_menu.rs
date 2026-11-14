use super::*;
use winit::keyboard::{Key, NamedKey};

pub(crate) const VIEW_ROW_HEIGHT: f32 = 27.0;
const VIEW_MENU_WIDTH: f32 = 336.0;
const VIEW_MENU_PADDING: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewPage {
    Root,
    Appearance,
    EditorLayout,
    ActivityPosition,
    ActivitySize,
    PanelPosition,
    PanelAlignment,
    TabBar,
    EditorActions,
    Whitespace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewAction {
    Palette,
    OpenView,
    Explorer,
    Search,
    SourceControl,
    Run,
    Extensions,
    Chat,
    PluginActivity,
    Problems,
    Output,
    DebugConsole,
    Terminal,
    WordWrap,
    FullScreen,
    Zen,
    Centered,
    MenuBar,
    PrimarySidebar,
    SecondarySidebar,
    StatusBar,
    Panel,
    MoveSidebar,
    ActivityDefault,
    ActivityTop,
    ActivityBottom,
    ActivityHidden,
    ActivitySizeDefault,
    ActivitySizeCompact,
    PanelBottom,
    PanelRight,
    PanelAlignLeft,
    PanelAlignCenter,
    PanelAlignRight,
    TabsDefault,
    TabsSingle,
    TabsNone,
    EditorActionsRight,
    EditorActionsLeft,
    Minimap,
    Breadcrumbs,
    StickyScroll,
    WhitespaceNone,
    WhitespaceBoundary,
    WhitespaceSelection,
    WhitespaceAll,
    ControlCharacters,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    SplitUp,
    SplitDown,
    SplitLeft,
    SplitRight,
    Single,
    TwoColumns,
    ThreeColumns,
    TwoRows,
    ThreeRows,
    Grid,
    TwoRowsRight,
    TwoColumnsBottom,
    FlipLayout,
    MoveEditorNewWindow,
    CopyEditorNewWindow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ViewRow {
    label: &'static str,
    shortcut: &'static str,
    action: Option<ViewAction>,
    submenu: Option<ViewPage>,
    separator: bool,
    enabled: bool,
    checked: bool,
    extension_index: Option<usize>,
}

impl ViewRow {
    fn item(label: &'static str, action: ViewAction, shortcut: &'static str) -> Self {
        Self {
            label,
            shortcut,
            action: Some(action),
            submenu: None,
            separator: false,
            enabled: true,
            checked: false,
            extension_index: None,
        }
    }
    fn submenu(label: &'static str, page: ViewPage) -> Self {
        Self {
            label,
            shortcut: "",
            action: None,
            submenu: Some(page),
            separator: false,
            enabled: true,
            checked: false,
            extension_index: None,
        }
    }
    fn disabled(label: &'static str) -> Self {
        Self {
            label,
            shortcut: "",
            action: None,
            submenu: None,
            separator: false,
            enabled: false,
            checked: false,
            extension_index: None,
        }
    }
    fn separator() -> Self {
        Self {
            label: "",
            shortcut: "",
            action: None,
            submenu: None,
            separator: true,
            enabled: false,
            checked: false,
            extension_index: None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ViewMenuState {
    pub x: f32,
    pub y: f32,
    pub page: ViewPage,
    pub selected: usize,
    scroll: usize,
    parent: Vec<ViewPage>,
}

impl ViewMenuState {
    fn rows(&self, app: &App) -> Vec<ViewRow> {
        use ViewAction as A;
        use ViewPage as P;
        let mut rows = match self.page {
            P::Root => vec![
                ViewRow::item("view.command_palette", A::Palette, "Ctrl+Shift+P"),
                ViewRow::item("view.open_view", A::OpenView, ""),
                ViewRow::separator(),
                ViewRow::submenu("view.appearance", P::Appearance),
                ViewRow::submenu("view.editor_layout", P::EditorLayout),
                ViewRow::separator(),
                ViewRow::item("activity.explorer", A::Explorer, "Ctrl+Shift+E"),
                ViewRow::item("activity.search", A::Search, "Ctrl+Shift+F"),
                ViewRow::item("activity.source_control", A::SourceControl, "Ctrl+Shift+G"),
                ViewRow::item("view.run", A::Run, "Ctrl+Shift+D"),
                ViewRow::item("activity.plugins", A::Extensions, "Ctrl+Shift+X"),
                ViewRow::separator(),
                ViewRow::item("view.chat", A::Chat, "Ctrl+Alt+I"),
                ViewRow::disabled("view.browser"),
                ViewRow::separator(),
                ViewRow::item("view.problems", A::Problems, "Ctrl+Shift+M"),
                ViewRow::item("view.output", A::Output, "Ctrl+Shift+U"),
                ViewRow::item("view.debug_console", A::DebugConsole, "Ctrl+Shift+Y"),
                ViewRow::item("view.terminal", A::Terminal, "Ctrl+`"),
                ViewRow::separator(),
                ViewRow::item("view.word_wrap", A::WordWrap, "Alt+Z"),
            ],
            P::Appearance => vec![
                ViewRow::item("view.full_screen", A::FullScreen, "F11"),
                ViewRow::item("view.zen_mode", A::Zen, "Ctrl+K Z"),
                ViewRow::item("view.centered_layout", A::Centered, ""),
                ViewRow::separator(),
                ViewRow::item("view.menu_bar", A::MenuBar, ""),
                ViewRow::item("view.primary_sidebar", A::PrimarySidebar, "Ctrl+B"),
                ViewRow::item("view.secondary_sidebar", A::SecondarySidebar, "Ctrl+Alt+B"),
                ViewRow::item("view.status_bar", A::StatusBar, ""),
                ViewRow::item("view.panel", A::Panel, "Ctrl+J"),
                ViewRow::separator(),
                ViewRow::item("activity.move_sidebar_right", A::MoveSidebar, ""),
                ViewRow::submenu("activity.position", P::ActivityPosition),
                ViewRow::submenu("activity.size", P::ActivitySize),
                ViewRow::submenu("view.panel_position", P::PanelPosition),
                ViewRow::submenu("view.panel_alignment", P::PanelAlignment),
                ViewRow::submenu("view.tab_bar", P::TabBar),
                ViewRow::submenu("view.editor_actions", P::EditorActions),
                ViewRow::separator(),
                ViewRow::item("view.minimap", A::Minimap, ""),
                ViewRow::item("view.breadcrumbs", A::Breadcrumbs, ""),
                ViewRow::item("view.sticky_scroll", A::StickyScroll, ""),
                ViewRow::submenu("view.render_whitespace", P::Whitespace),
                ViewRow::item("view.control_characters", A::ControlCharacters, ""),
                ViewRow::separator(),
                ViewRow::item("view.zoom_in", A::ZoomIn, "Ctrl++"),
                ViewRow::item("view.zoom_out", A::ZoomOut, "Ctrl+-"),
                ViewRow::item("view.reset_zoom", A::ResetZoom, "Ctrl+Numpad0"),
            ],
            P::EditorLayout => vec![
                ViewRow::item("view.split_up", A::SplitUp, "Ctrl+K Ctrl+↑"),
                ViewRow::item("view.split_down", A::SplitDown, ""),
                ViewRow::item("view.split_left", A::SplitLeft, ""),
                ViewRow::item("view.split_right", A::SplitRight, ""),
                ViewRow::separator(),
                ViewRow::item("view.move_editor_window", A::MoveEditorNewWindow, ""),
                ViewRow::item(
                    "view.copy_editor_window",
                    A::CopyEditorNewWindow,
                    "Ctrl+K O",
                ),
                ViewRow::separator(),
                ViewRow::item("view.layout.single", A::Single, ""),
                ViewRow::item("view.layout.two_columns", A::TwoColumns, ""),
                ViewRow::item("view.layout.three_columns", A::ThreeColumns, ""),
                ViewRow::item("view.layout.two_rows", A::TwoRows, ""),
                ViewRow::item("view.layout.three_rows", A::ThreeRows, ""),
                ViewRow::item("view.layout.grid", A::Grid, ""),
                ViewRow::item("view.layout.two_rows_right", A::TwoRowsRight, ""),
                ViewRow::item("view.layout.two_columns_bottom", A::TwoColumnsBottom, ""),
                ViewRow::separator(),
                ViewRow::item("view.layout.flip", A::FlipLayout, "Shift+Alt+0"),
            ],
            P::ActivityPosition => vec![
                ViewRow::item("activity.position.default", A::ActivityDefault, ""),
                ViewRow::item("activity.position.top", A::ActivityTop, ""),
                ViewRow::item("activity.position.bottom", A::ActivityBottom, ""),
                ViewRow::item("activity.position.hidden", A::ActivityHidden, ""),
            ],
            P::ActivitySize => vec![
                ViewRow::item("activity.size.default", A::ActivitySizeDefault, ""),
                ViewRow::item("activity.size.compact", A::ActivitySizeCompact, ""),
            ],
            P::PanelPosition => vec![
                ViewRow::item("view.position.bottom", A::PanelBottom, ""),
                ViewRow::item("view.position.right", A::PanelRight, ""),
            ],
            P::PanelAlignment => vec![
                ViewRow::item("view.align.left", A::PanelAlignLeft, ""),
                ViewRow::item("view.align.center", A::PanelAlignCenter, ""),
                ViewRow::item("view.align.right", A::PanelAlignRight, ""),
            ],
            P::TabBar => vec![
                ViewRow::item("view.tabbar.default", A::TabsDefault, ""),
                ViewRow::item("view.tabbar.single", A::TabsSingle, ""),
                ViewRow::item("view.tabbar.hidden", A::TabsNone, ""),
            ],
            P::EditorActions => vec![
                ViewRow::item("view.actions.right", A::EditorActionsRight, ""),
                ViewRow::item("view.actions.left", A::EditorActionsLeft, ""),
            ],
            P::Whitespace => vec![
                ViewRow::item("view.whitespace.none", A::WhitespaceNone, ""),
                ViewRow::item("view.whitespace.boundary", A::WhitespaceBoundary, ""),
                ViewRow::item("view.whitespace.selection", A::WhitespaceSelection, ""),
                ViewRow::item("view.whitespace.all", A::WhitespaceAll, ""),
            ],
        };
        if self.page == P::Root {
            let extension_rows = app
                .plugin_activity_entries()
                .into_iter()
                .filter(|entry| {
                    !app.activity_bar_preferences
                        .hidden_items
                        .contains(&entry.key)
                })
                .enumerate()
                .map(|(index, _)| ViewRow {
                    label: "activity.plugin_view",
                    shortcut: "",
                    action: Some(A::PluginActivity),
                    submenu: None,
                    separator: false,
                    enabled: true,
                    checked: app
                        .plugin_activity_entries()
                        .into_iter()
                        .filter(|entry| {
                            !app.activity_bar_preferences
                                .hidden_items
                                .contains(&entry.key)
                        })
                        .nth(index)
                        .is_some_and(|entry| {
                            app.active_plugin_activity.as_deref() == Some(&entry.key)
                        }),
                    extension_index: Some(index),
                })
                .collect::<Vec<_>>();
            rows.splice(11..11, extension_rows);
        }
        for row in &mut rows {
            let Some(action) = row.action else { continue };
            row.checked = match action {
                A::WordWrap => app.word_wrap,
                A::FullScreen => app
                    .gpu
                    .as_ref()
                    .is_some_and(|g| g.window.fullscreen().is_some()),
                A::Zen => app.zen_mode,
                A::Centered => app.centered_editor,
                A::MenuBar => app.view_preferences.menu_bar_visible,
                A::PrimarySidebar => app.view_preferences.primary_sidebar_visible,
                A::SecondarySidebar => app.view_preferences.secondary_sidebar_visible,
                A::StatusBar => app.view_preferences.status_bar_visible,
                A::Panel => app.view_preferences.panel_visible,
                A::ActivityDefault => {
                    app.activity_bar_preferences.position == config::ActivityBarPosition::Default
                }
                A::ActivityTop => {
                    app.activity_bar_preferences.position == config::ActivityBarPosition::Top
                }
                A::ActivityBottom => {
                    app.activity_bar_preferences.position == config::ActivityBarPosition::Bottom
                }
                A::ActivityHidden => {
                    app.activity_bar_preferences.position == config::ActivityBarPosition::Hidden
                }
                A::ActivitySizeDefault => {
                    app.activity_bar_preferences.size == config::ActivityBarSize::Default
                }
                A::ActivitySizeCompact => {
                    app.activity_bar_preferences.size == config::ActivityBarSize::Compact
                }
                A::PanelBottom => {
                    app.view_preferences.panel_position == config::PanelPosition::Bottom
                }
                A::PanelRight => {
                    app.view_preferences.panel_position == config::PanelPosition::Right
                }
                A::PanelAlignLeft => {
                    !app.view_preferences.panel_align_center
                        && !app.view_preferences.panel_align_right
                }
                A::PanelAlignCenter => app.view_preferences.panel_align_center,
                A::PanelAlignRight => app.view_preferences.panel_align_right,
                A::TabsDefault => app.view_preferences.tabs_visible,
                A::TabsSingle => app.view_preferences.tabs_visible,
                A::TabsNone => !app.view_preferences.tabs_visible,
                A::EditorActionsRight => app.view_preferences.editor_actions_right,
                A::EditorActionsLeft => !app.view_preferences.editor_actions_right,
                A::Minimap => app.view_preferences.minimap_visible,
                A::Breadcrumbs => app.view_preferences.breadcrumbs_visible,
                A::StickyScroll => app.view_preferences.sticky_scroll,
                A::ControlCharacters => app.view_preferences.control_characters,
                A::WhitespaceNone => {
                    app.view_preferences.whitespace == config::WhitespaceRendering::None
                }
                A::WhitespaceBoundary => {
                    app.view_preferences.whitespace == config::WhitespaceRendering::Boundary
                }
                A::WhitespaceSelection => {
                    app.view_preferences.whitespace == config::WhitespaceRendering::Selection
                }
                A::WhitespaceAll => {
                    app.view_preferences.whitespace == config::WhitespaceRendering::All
                }
                A::PluginActivity => row.checked,
                _ => false,
            };
        }
        rows
    }

    fn selectable_rows(&self, app: &App) -> Vec<usize> {
        self.rows(app)
            .iter()
            .enumerate()
            .filter_map(|(i, row)| (!row.separator).then_some(i))
            .collect()
    }
}

impl App {
    pub(crate) fn visible_extension_activities(&self) -> Vec<activity_bar::PluginActivityEntry> {
        self.plugin_activity_entries()
            .into_iter()
            .filter(|entry| {
                !self
                    .activity_bar_preferences
                    .hidden_items
                    .contains(&entry.key)
            })
            .collect()
    }

    pub(crate) fn open_view_menu(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.view_menu = Some(ViewMenuState {
            x: x.clamp(0.0, (width - VIEW_MENU_WIDTH).max(0.0)),
            y,
            page: ViewPage::Root,
            selected: 0,
            scroll: 0,
            parent: Vec::new(),
        });
        self.activity_bar_menu = None;
        self.adjust_view_menu_position(height);
    }

    fn adjust_view_menu_position(&mut self, height: f32) {
        if let Some(snapshot) = &self.view_menu {
            let count = snapshot.rows(self).len();
            let popup_height = count as f32 * VIEW_ROW_HEIGHT + VIEW_MENU_PADDING * 2.0;
            let menu = self.view_menu.as_mut().unwrap();
            menu.y = menu.y.min((height - popup_height.min(height)).max(0.0));
        }
    }

    pub(crate) fn close_view_menu(&mut self) {
        self.view_menu = None;
    }

    pub(crate) fn view_menu_accessible_rows(&self) -> Vec<(String, bool, bool, bool)> {
        self.view_menu
            .as_ref()
            .map(|menu| {
                menu.rows(self)
                    .into_iter()
                    .map(|row| {
                        let label = row
                            .extension_index
                            .and_then(|index| {
                                self.visible_extension_activities()
                                    .get(index)
                                    .map(|entry| entry.title.clone())
                            })
                            .unwrap_or_else(|| self.t(row.label).to_owned());
                        (label, row.enabled, row.checked, row.separator)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn activate_view_menu_accessibility_item(&mut self, index: usize) {
        let Some(menu) = self.view_menu.clone() else {
            return;
        };
        let Some(row) = menu.rows(self).get(index).copied() else {
            return;
        };
        if !row.enabled || row.separator {
            return;
        }
        if let Some(page) = row.submenu {
            let state = self.view_menu.as_mut().unwrap();
            state.parent.push(state.page);
            state.page = page;
            state.selected = 0;
        } else if let Some(action) = row.action {
            self.execute_view_action(action, row.extension_index);
            self.close_view_menu();
        }
    }

    pub(crate) fn view_menu_pointer_moved(&mut self, x: f32, y: f32) {
        let Some(snapshot) = self.view_menu.clone() else {
            return;
        };
        let rows = snapshot.rows(self);
        if x < snapshot.x || x >= snapshot.x + VIEW_MENU_WIDTH {
            return;
        }
        let index = ((y - snapshot.y - VIEW_MENU_PADDING) / VIEW_ROW_HEIGHT) as isize
            + snapshot.scroll as isize;
        if index >= snapshot.scroll as isize
            && (index as usize) < rows.len()
            && !rows[index as usize].separator
        {
            self.view_menu.as_mut().unwrap().selected = index as usize;
        }
    }

    pub(crate) fn view_menu_click(&mut self, x: f32, y: f32) -> bool {
        let Some(snapshot) = self.view_menu.clone() else {
            return false;
        };
        let rows = snapshot.rows(self);
        let width = self
            .gpu
            .as_ref()
            .map_or(1200.0, |g| g.renderer.size().0 as f32);
        let window_height = self
            .gpu
            .as_ref()
            .map_or(760.0, |gpu| gpu.renderer.size().1 as f32);
        let visible_rows = ((window_height - snapshot.y - VIEW_MENU_PADDING * 2.0)
            / VIEW_ROW_HEIGHT)
            .floor()
            .max(1.0) as usize;
        if x < snapshot.x
            || x >= snapshot.x + VIEW_MENU_WIDTH
            || y < snapshot.y
            || y >= snapshot.y
                + visible_rows.min(rows.len().saturating_sub(snapshot.scroll)) as f32
                    * VIEW_ROW_HEIGHT
                + VIEW_MENU_PADDING * 2.0
        {
            self.close_view_menu();
            return true;
        }
        let index = snapshot.scroll
            + ((y - snapshot.y - VIEW_MENU_PADDING) / VIEW_ROW_HEIGHT).max(0.0) as usize;
        let Some(row) = rows.get(index).copied() else {
            return true;
        };
        if !row.enabled || row.separator {
            return true;
        }
        if let Some(page) = row.submenu {
            if let Some(menu) = &mut self.view_menu {
                menu.parent.push(menu.page);
                menu.page = page;
                menu.selected = 0;
                menu.scroll = 0;
            }
        } else if let Some(action) = row.action {
            self.execute_view_action(action, row.extension_index);
            self.close_view_menu();
        }
        let _ = width;
        true
    }

    pub(crate) fn view_menu_key(&mut self, key: &Key) -> bool {
        let Some(snapshot) = self.view_menu.clone() else {
            return false;
        };
        let indices = snapshot.selectable_rows(self);
        match key {
            Key::Named(NamedKey::Escape) => {
                if let Some(parent) = self.view_menu.as_mut().and_then(|m| m.parent.pop()) {
                    let menu = self.view_menu.as_mut().unwrap();
                    menu.page = parent;
                    menu.selected = 0;
                } else {
                    self.close_view_menu();
                }
            }
            Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::ArrowUp) => {
                let current = indices
                    .iter()
                    .position(|i| *i == snapshot.selected)
                    .unwrap_or(0);
                let next = if matches!(key, Key::Named(NamedKey::ArrowDown)) {
                    (current + 1) % indices.len().max(1)
                } else {
                    (current + indices.len().saturating_sub(1)) % indices.len().max(1)
                };
                self.view_menu.as_mut().unwrap().selected = indices.get(next).copied().unwrap_or(0);
                self.view_menu_ensure_selected_visible();
            }
            Key::Named(NamedKey::ArrowRight)
            | Key::Named(NamedKey::Enter)
            | Key::Named(NamedKey::Space) => {
                let row = snapshot.rows(self).get(snapshot.selected).copied();
                if let Some(row) = row {
                    if let Some(page) = row.submenu {
                        let menu = self.view_menu.as_mut().unwrap();
                        menu.parent.push(menu.page);
                        menu.page = page;
                        menu.selected = 0;
                        menu.scroll = 0;
                    } else if row.enabled {
                        if let Some(action) = row.action {
                            self.execute_view_action(action, row.extension_index);
                            self.close_view_menu();
                        }
                    }
                }
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if let Some(parent) = self.view_menu.as_mut().and_then(|m| m.parent.pop()) {
                    let menu = self.view_menu.as_mut().unwrap();
                    menu.page = parent;
                    menu.selected = 0;
                } else {
                    self.close_view_menu();
                }
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn draw_view_menu(&self, renderer: &mut Renderer) {
        let Some(menu) = &self.view_menu else { return };
        let rows = menu.rows(self);
        let visible_extensions = self.visible_extension_activities();
        let width = VIEW_MENU_WIDTH.min(renderer.size().0 as f32);
        let height = (rows.len() as f32 * VIEW_ROW_HEIGHT + VIEW_MENU_PADDING * 2.0)
            .min(renderer.size().1 as f32 - menu.y);
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        renderer.draw_rect(menu.x, menu.y, width, height, border);
        renderer.draw_rect(
            menu.x + 1.0,
            menu.y + 1.0,
            width - 2.0,
            height - 2.0,
            background,
        );
        renderer.set_clip(Some([menu.x, menu.y, menu.x + width, menu.y + height]));
        let visible_rows = ((height - VIEW_MENU_PADDING * 2.0) / VIEW_ROW_HEIGHT).ceil() as usize;
        for (index, row) in rows.iter().enumerate().skip(menu.scroll).take(visible_rows) {
            let y = menu.y + VIEW_MENU_PADDING + (index - menu.scroll) as f32 * VIEW_ROW_HEIGHT;
            if row.separator {
                renderer.draw_rect(
                    menu.x + 1.0,
                    y + VIEW_ROW_HEIGHT * 0.5,
                    width - 2.0,
                    1.0,
                    border,
                );
                continue;
            }
            if menu.selected == index {
                renderer.draw_rect(
                    menu.x + 2.0,
                    y,
                    width - 4.0,
                    VIEW_ROW_HEIGHT,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0x3f3f46)),
                );
            }
            let color = rgb(Theme::color(
                if row.enabled { Theme::TEXT } else { Theme::DIM },
                if row.enabled { 0xd4d4d4 } else { 0x777777 },
            ));
            if row.checked {
                renderer.draw_text("✓", menu.x + 8.0, y + 4.0, color);
            }
            let label = row
                .extension_index
                .and_then(|index| {
                    visible_extensions
                        .get(index)
                        .map(|entry| entry.title.as_str())
                })
                .unwrap_or_else(|| self.t(row.label));
            renderer.draw_text(label, menu.x + 28.0, y + 4.0, color);
            if !row.shortcut.is_empty() {
                renderer.draw_text(
                    row.shortcut,
                    menu.x + width
                        - row.shortcut.chars().count() as f32 * renderer.char_width()
                        - 12.0,
                    y + 4.0,
                    color,
                );
            }
            if row.submenu.is_some() {
                renderer.draw_text("›", menu.x + width - 16.0, y + 4.0, color);
            }
        }
        renderer.set_clip(None);
    }

    fn view_menu_ensure_selected_visible(&mut self) {
        let height = self
            .gpu
            .as_ref()
            .map_or(760.0, |gpu| gpu.renderer.size().1 as f32);
        let Some(snapshot) = self.view_menu.clone() else {
            return;
        };
        let visible = ((height - snapshot.y - VIEW_MENU_PADDING * 2.0) / VIEW_ROW_HEIGHT)
            .floor()
            .max(1.0) as usize;
        if let Some(menu) = &mut self.view_menu {
            if menu.selected < menu.scroll {
                menu.scroll = menu.selected;
            } else if menu.selected >= menu.scroll + visible {
                menu.scroll = menu.selected + 1 - visible;
            }
        }
    }

    fn save_view_preferences(&mut self) {
        if let Err(error) =
            config::save_view_layout_preferences(&self.profile_root, &self.view_preferences)
        {
            self.status = error;
        }
    }

    pub(crate) fn save_view_preferences_for_command(&mut self) {
        self.save_view_preferences();
    }

    pub(crate) fn toggle_full_screen(&mut self) {
        self.execute_view_action(ViewAction::FullScreen, None);
    }

    pub(crate) fn toggle_sticky_scroll(&mut self) {
        self.execute_view_action(ViewAction::StickyScroll, None);
    }

    /// Zooms the editor text in (1), out (-1) or back to 100% (0).
    pub(crate) fn run_font_zoom(&mut self, step: i8) {
        let action = match step {
            1.. => ViewAction::ZoomIn,
            0 => ViewAction::ResetZoom,
            _ => ViewAction::ZoomOut,
        };
        self.execute_view_action(action, None);
    }

    fn execute_view_action(&mut self, action: ViewAction, extension_index: Option<usize>) {
        use ViewAction as A;
        match action {
            A::Palette => self.open_command_palette(),
            A::OpenView => self.open_command_palette(),
            A::Explorer => self.activate_activity(activity_bar::Activity::Explorer),
            A::Search => self.activate_activity(activity_bar::Activity::Search),
            A::SourceControl => self.activate_activity(activity_bar::Activity::SourceControl),
            A::Run => {
                self.debug_panel_visible = !self.debug_panel_visible;
            }
            A::Extensions => self.activate_activity(activity_bar::Activity::Plugins),
            A::Chat => self.toggle_chat_sidebar(),
            A::PluginActivity => {
                if let Some(entry) = extension_index
                    .and_then(|index| self.visible_extension_activities().get(index).cloned())
                {
                    self.activate_plugin_activity(entry);
                }
            }
            A::Problems => self.run_ui_command("workbench.actions.view.problems"),
            A::Output => {
                self.task_output.clear();
                self.terminal_visible = true;
                self.terminal_problems = false;
            }
            A::DebugConsole => {
                self.debug_panel_visible = true;
            }
            A::Terminal => self.toggle_terminal(),
            A::WordWrap => {
                self.word_wrap = !self.word_wrap;
            }
            A::FullScreen => {
                if let Some(gpu) = &self.gpu {
                    let fullscreen = gpu.window.fullscreen().is_none().then(|| {
                        winit::window::Fullscreen::Borderless(gpu.window.current_monitor())
                    });
                    gpu.window.set_fullscreen(fullscreen);
                }
            }
            A::Zen => self.toggle_zen_mode(),
            A::Centered => self.toggle_centered_editor(),
            A::MenuBar => {
                self.view_preferences.menu_bar_visible = !self.view_preferences.menu_bar_visible
            }
            A::PrimarySidebar => {
                self.view_preferences.primary_sidebar_visible =
                    !self.view_preferences.primary_sidebar_visible
            }
            A::SecondarySidebar => {
                self.view_preferences.secondary_sidebar_visible =
                    !self.view_preferences.secondary_sidebar_visible
            }
            A::StatusBar => {
                self.view_preferences.status_bar_visible = !self.view_preferences.status_bar_visible
            }
            A::Panel => self.view_preferences.panel_visible = !self.view_preferences.panel_visible,
            A::MoveSidebar => self.toggle_primary_sidebar_side(),
            A::ActivityDefault => {
                self.set_activity_bar_position(config::ActivityBarPosition::Default)
            }
            A::ActivityTop => self.set_activity_bar_position(config::ActivityBarPosition::Top),
            A::ActivityBottom => {
                self.set_activity_bar_position(config::ActivityBarPosition::Bottom)
            }
            A::ActivityHidden => {
                self.set_activity_bar_position(config::ActivityBarPosition::Hidden)
            }
            A::ActivitySizeDefault => self.set_activity_bar_size(config::ActivityBarSize::Default),
            A::ActivitySizeCompact => self.set_activity_bar_size(config::ActivityBarSize::Compact),
            A::PanelBottom => self.view_preferences.panel_position = config::PanelPosition::Bottom,
            A::PanelRight => self.view_preferences.panel_position = config::PanelPosition::Right,
            A::PanelAlignLeft => {
                self.view_preferences.panel_align_center = false;
                self.view_preferences.panel_align_right = false;
            }
            A::PanelAlignCenter => {
                self.view_preferences.panel_align_center = true;
                self.view_preferences.panel_align_right = false;
            }
            A::PanelAlignRight => {
                self.view_preferences.panel_align_center = false;
                self.view_preferences.panel_align_right = true;
            }
            A::TabsDefault | A::TabsSingle => self.view_preferences.tabs_visible = true,
            A::TabsNone => self.view_preferences.tabs_visible = false,
            A::EditorActionsRight => self.view_preferences.editor_actions_right = true,
            A::EditorActionsLeft => self.view_preferences.editor_actions_right = false,
            A::Minimap => {
                self.view_preferences.minimap_visible = !self.view_preferences.minimap_visible
            }
            A::Breadcrumbs => {
                self.view_preferences.breadcrumbs_visible =
                    !self.view_preferences.breadcrumbs_visible
            }
            A::StickyScroll => {
                self.view_preferences.sticky_scroll = !self.view_preferences.sticky_scroll
            }
            A::WhitespaceNone => {
                self.view_preferences.whitespace = config::WhitespaceRendering::None
            }
            A::WhitespaceBoundary => {
                self.view_preferences.whitespace = config::WhitespaceRendering::Boundary
            }
            A::WhitespaceSelection => {
                self.view_preferences.whitespace = config::WhitespaceRendering::Selection
            }
            A::WhitespaceAll => self.view_preferences.whitespace = config::WhitespaceRendering::All,
            A::ControlCharacters => {
                self.view_preferences.control_characters = !self.view_preferences.control_characters
            }
            A::ZoomIn => {
                self.view_preferences.zoom_percent = self
                    .view_preferences
                    .zoom_percent
                    .saturating_add(10)
                    .min(200)
            }
            A::ZoomOut => {
                self.view_preferences.zoom_percent = self
                    .view_preferences
                    .zoom_percent
                    .saturating_sub(10)
                    .max(50)
            }
            A::ResetZoom => self.view_preferences.zoom_percent = 100,
            A::SplitUp => {
                self.split_editor_group_before(super::editor_groups::SplitDirection::Horizontal);
            }
            A::SplitDown => {
                self.editor_groups
                    .split(super::editor_groups::SplitDirection::Horizontal);
            }
            A::SplitLeft => {
                self.split_editor_group_before(super::editor_groups::SplitDirection::Vertical);
            }
            A::SplitRight => {
                self.editor_groups
                    .split(super::editor_groups::SplitDirection::Vertical);
            }
            A::TwoColumns => self.editor_groups.arrange_layout_grid(2, 1),
            A::ThreeColumns => self.editor_groups.arrange_layout_grid(3, 1),
            A::TwoRows => self.editor_groups.arrange_layout_grid(1, 2),
            A::ThreeRows => self.editor_groups.arrange_layout_grid(1, 3),
            A::Grid => self.editor_groups.arrange_layout_grid(2, 2),
            A::TwoRowsRight => self.editor_groups.arrange_two_rows_right(),
            A::TwoColumnsBottom => self.editor_groups.arrange_two_columns_bottom(),
            A::Single => self.editor_groups.collapse_to_single_group(),
            A::MoveEditorNewWindow => self.open_editor_in_new_window(true),
            A::CopyEditorNewWindow => self.open_editor_in_new_window(false),
            A::FlipLayout => self.editor_groups.flip_layout(),
        }
        if matches!(action, A::ZoomIn | A::ZoomOut | A::ResetZoom) {
            if let Some(gpu) = &mut self.gpu {
                gpu.renderer.set_font_size(
                    self.font_size
                        * gpu.window.scale_factor() as f32
                        * f32::from(self.view_preferences.zoom_percent)
                        / 100.0,
                );
            }
        }
        if !matches!(
            action,
            A::Palette
                | A::OpenView
                | A::Explorer
                | A::Search
                | A::SourceControl
                | A::Run
                | A::Extensions
                | A::Problems
                | A::Output
                | A::DebugConsole
                | A::Terminal
                | A::ActivityDefault
                | A::ActivityTop
                | A::ActivityBottom
                | A::ActivityHidden
                | A::ActivitySizeDefault
                | A::ActivitySizeCompact
                | A::MoveSidebar
                | A::SplitUp
                | A::SplitDown
                | A::SplitLeft
                | A::SplitRight
                | A::TwoRows
                | A::TwoColumns
                | A::TwoRowsRight
                | A::TwoColumnsBottom
                | A::ThreeRows
                | A::ThreeColumns
                | A::Grid
                | A::Single
                | A::FlipLayout
                | A::MoveEditorNewWindow
                | A::CopyEditorNewWindow
        ) {
            self.save_view_preferences();
        }
        if matches!(
            action,
            A::PanelBottom
                | A::PanelRight
                | A::PanelAlignLeft
                | A::PanelAlignCenter
                | A::PanelAlignRight
        ) {
            self.save_view_preferences();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_toggle_is_saved_to_the_active_profile() {
        let profile = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(profile.path().to_path_buf(), None, true);
        app.profile_root = profile.path().to_path_buf();
        app.view_menu = Some(ViewMenuState {
            x: 0.0,
            y: 0.0,
            page: ViewPage::Appearance,
            selected: 18,
            scroll: 0,
            parent: vec![ViewPage::Root],
        });
        let rows = app.view_menu_accessible_rows();
        assert_eq!(rows[18].0, app.t("view.minimap"));
        assert!(rows[18].1 && !rows[18].2);
        app.activate_view_menu_accessibility_item(18);
        assert!(app.view_preferences.minimap_visible);
        assert!(app.view_menu.is_none());
        assert!(config::load_view_layout_preferences(profile.path()).minimap_visible);
    }

    #[test]
    fn root_menu_disables_unimplemented_views_and_scrolls_long_appearance_page() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_view_menu(10.0, 10.0, 900.0, 430.0);
        let root = app.view_menu_accessible_rows();
        assert_eq!(root[12].0, app.t("view.chat"));
        assert!(root[12].1);
        assert_eq!(root[13].0, app.t("view.browser"));
        assert!(!root[13].1);
        app.view_menu.as_mut().unwrap().page = ViewPage::Appearance;
        app.view_menu.as_mut().unwrap().parent = vec![ViewPage::Root];
        app.view_menu.as_mut().unwrap().selected = 25;
        app.view_menu.as_mut().unwrap().y = 700.0;
        app.view_menu_ensure_selected_visible();
        assert!(app.view_menu.as_ref().unwrap().scroll > 0);
        assert!(app.view_menu_key(&Key::Named(NamedKey::Escape)));
        assert!(app.view_menu.is_some());
        assert!(app.view_menu_key(&Key::Named(NamedKey::Escape)));
        assert!(app.view_menu.is_none());
    }
}
