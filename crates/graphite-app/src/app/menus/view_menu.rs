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

/// Marks a panel that has no highlighted row (for example a flyout opened with the pointer).
const NO_ROW: usize = usize::MAX;

/// A submenu opened to the side of the row that owns it.
#[derive(Clone, Debug)]
struct Flyout {
    page: ViewPage,
    from_row: usize,
    selected: usize,
    scroll: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ViewMenuState {
    pub x: f32,
    pub y: f32,
    pub page: ViewPage,
    pub selected: usize,
    scroll: usize,
    /// Submenus opened from the root panel, outermost first. `flyouts[n]` is panel `n + 1`.
    flyouts: Vec<Flyout>,
}

impl ViewMenuState {
    fn rows_for(page: ViewPage, app: &App) -> Vec<ViewRow> {
        use ViewAction as A;
        use ViewPage as P;
        let mut rows = match page {
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
        if page == P::Root {
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
                A::WordWrap => app.word_wrap(),
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

    fn depth(&self) -> usize {
        self.flyouts.len()
    }

    fn page_at(&self, panel: usize) -> ViewPage {
        match panel {
            0 => self.page,
            _ => self.flyouts[panel - 1].page,
        }
    }

    fn selected_at(&self, panel: usize) -> usize {
        match panel {
            0 => self.selected,
            _ => self.flyouts[panel - 1].selected,
        }
    }

    fn scroll_at(&self, panel: usize) -> usize {
        match panel {
            0 => self.scroll,
            _ => self.flyouts[panel - 1].scroll,
        }
    }

    fn set_selected(&mut self, panel: usize, row: usize) {
        match panel {
            0 => self.selected = row,
            _ => self.flyouts[panel - 1].selected = row,
        }
    }

    fn set_scroll(&mut self, panel: usize, scroll: usize) {
        match panel {
            0 => self.scroll = scroll,
            _ => self.flyouts[panel - 1].scroll = scroll,
        }
    }

    /// The highlighted row of the innermost open panel.
    pub(crate) fn active_selected(&self) -> usize {
        self.selected_at(self.depth())
    }

    fn rows_at(&self, app: &App, panel: usize) -> Vec<ViewRow> {
        Self::rows_for(self.page_at(panel), app)
    }

    fn first_selectable(app: &App, page: ViewPage) -> usize {
        Self::rows_for(page, app)
            .iter()
            .position(|row| !row.separator)
            .unwrap_or(0)
    }

    /// Screen rectangle `(x, y, width, height)` of a panel. A flyout starts level with the row
    /// that opened it, sits to the right of its parent (or to the left when there is no room)
    /// and is kept inside the window.
    fn rect_at(&self, app: &App, panel: usize, size: (f32, f32)) -> (f32, f32, f32, f32) {
        let (window_width, window_height) = size;
        let width = VIEW_MENU_WIDTH.min(window_width);
        let full =
            self.rows_at(app, panel).len() as f32 * VIEW_ROW_HEIGHT + VIEW_MENU_PADDING * 2.0;
        if panel == 0 {
            return (
                self.x,
                self.y,
                width,
                full.min((window_height - self.y).max(0.0)),
            );
        }
        let (parent_x, parent_y, parent_width, _) = self.rect_at(app, panel - 1, size);
        let from_row = self.flyouts[panel - 1].from_row as f32;
        let row_top = parent_y
            + VIEW_MENU_PADDING
            + (from_row - self.scroll_at(panel - 1) as f32) * VIEW_ROW_HEIGHT;
        let x = if parent_x + parent_width + width <= window_width {
            parent_x + parent_width
        } else {
            (parent_x - width).max(0.0)
        };
        let height = full.min(window_height);
        let y = (row_top - VIEW_MENU_PADDING)
            .min(window_height - height)
            .max(0.0);
        (x, y, width, height)
    }

    /// The innermost panel under the point and the row there, if any.
    fn hit(&self, app: &App, x: f32, y: f32, size: (f32, f32)) -> Option<(usize, Option<usize>)> {
        for panel in (0..=self.depth()).rev() {
            let (left, top, width, height) = self.rect_at(app, panel, size);
            if x >= left && x < left + width && y >= top && y < top + height {
                let offset = ((y - top - VIEW_MENU_PADDING) / VIEW_ROW_HEIGHT).floor();
                let row = (offset >= 0.0)
                    .then(|| self.scroll_at(panel) + offset as usize)
                    .filter(|row| *row < self.rows_at(app, panel).len());
                return Some((panel, row));
            }
        }
        None
    }

    /// Opens `page` as the submenu of `row` in `panel`, closing whatever was open beside it.
    fn open_flyout(&mut self, panel: usize, row: usize, page: ViewPage, selected: usize) {
        let same = self
            .flyouts
            .get(panel)
            .is_some_and(|open| open.from_row == row && open.page == page);
        if same {
            self.flyouts.truncate(panel + 1);
            if selected != NO_ROW {
                self.flyouts[panel].selected = selected;
            }
        } else {
            self.flyouts.truncate(panel);
            self.flyouts.push(Flyout {
                page,
                from_row: row,
                selected,
                scroll: 0,
            });
        }
    }

    fn ensure_visible(&mut self, app: &App, panel: usize, size: (f32, f32)) {
        let (_, _, _, height) = self.rect_at(app, panel, size);
        let visible = ((height - VIEW_MENU_PADDING * 2.0) / VIEW_ROW_HEIGHT)
            .floor()
            .max(1.0) as usize;
        let selected = self.selected_at(panel);
        if selected == NO_ROW {
            return;
        }
        let scroll = self.scroll_at(panel);
        if selected < scroll {
            self.set_scroll(panel, selected);
        } else if selected >= scroll + visible {
            self.set_scroll(panel, selected + 1 - visible);
        }
    }
}

/// What a click or key press on the View menu asks the application to do.
enum ViewStep {
    Keep,
    Close,
    Run(ViewAction, Option<usize>),
    Unhandled,
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
            flyouts: Vec::new(),
        });
        self.activity_bar_menu = None;
        self.adjust_view_menu_position(height);
    }

    fn adjust_view_menu_position(&mut self, height: f32) {
        if let Some(snapshot) = &self.view_menu {
            let count = snapshot.rows_at(self, 0).len();
            let popup_height = count as f32 * VIEW_ROW_HEIGHT + VIEW_MENU_PADDING * 2.0;
            let menu = self.view_menu.as_mut().unwrap();
            menu.y = menu.y.min((height - popup_height.min(height)).max(0.0));
        }
    }

    pub(crate) fn close_view_menu(&mut self) {
        self.view_menu = None;
    }

    fn view_window_size(&self) -> (f32, f32) {
        self.gpu.as_ref().map_or((1200.0, 760.0), |gpu| {
            let (width, height) = gpu.renderer.size();
            (width, height)
        })
    }

    /// Runs `f` on the open View menu with read access to the rest of the application.
    fn with_view_menu<R>(&mut self, f: impl FnOnce(&mut ViewMenuState, &App) -> R) -> Option<R> {
        let mut menu = self.view_menu.take()?;
        let result = f(&mut menu, self);
        self.view_menu = Some(menu);
        Some(result)
    }

    fn finish_view_step(&mut self, step: Option<ViewStep>) -> bool {
        match step {
            Some(ViewStep::Close) => self.close_view_menu(),
            Some(ViewStep::Run(action, extension_index)) => {
                self.execute_view_action(action, extension_index);
                self.close_view_menu();
            }
            Some(ViewStep::Keep) | None => {}
            Some(ViewStep::Unhandled) => return false,
        }
        true
    }

    pub(crate) fn view_menu_accessible_rows(&self) -> Vec<(String, bool, bool, bool)> {
        self.view_menu
            .as_ref()
            .map(|menu| {
                menu.rows_at(self, menu.depth())
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
        let step = self.with_view_menu(|menu, app| {
            let panel = menu.depth();
            let Some(row) = menu.rows_at(app, panel).get(index).copied() else {
                return ViewStep::Keep;
            };
            if !row.enabled || row.separator {
                return ViewStep::Keep;
            }
            menu.set_selected(panel, index);
            if let Some(page) = row.submenu {
                let first = ViewMenuState::first_selectable(app, page);
                menu.open_flyout(panel, index, page, first);
                ViewStep::Keep
            } else if let Some(action) = row.action {
                ViewStep::Run(action, row.extension_index)
            } else {
                ViewStep::Keep
            }
        });
        self.finish_view_step(step);
    }

    /// Highlights the row under the pointer and opens its submenu beside it.
    pub(crate) fn view_menu_pointer_moved(&mut self, x: f32, y: f32) {
        let size = self.view_window_size();
        self.with_view_menu(|menu, app| {
            let Some((panel, Some(index))) = menu.hit(app, x, y, size) else {
                return;
            };
            let row = menu.rows_at(app, panel)[index];
            if row.separator {
                return;
            }
            menu.set_selected(panel, index);
            match row.submenu {
                Some(page) if row.enabled => menu.open_flyout(panel, index, page, NO_ROW),
                _ => menu.flyouts.truncate(panel),
            }
        });
    }

    pub(crate) fn view_menu_click(&mut self, x: f32, y: f32) -> bool {
        let size = self.view_window_size();
        let step = self.with_view_menu(|menu, app| {
            let Some((panel, index)) = menu.hit(app, x, y, size) else {
                return ViewStep::Close;
            };
            let Some(index) = index else {
                return ViewStep::Keep;
            };
            let row = menu.rows_at(app, panel)[index];
            if !row.enabled || row.separator {
                return ViewStep::Keep;
            }
            menu.set_selected(panel, index);
            if let Some(page) = row.submenu {
                let first = ViewMenuState::first_selectable(app, page);
                menu.open_flyout(panel, index, page, first);
                ViewStep::Keep
            } else if let Some(action) = row.action {
                ViewStep::Run(action, row.extension_index)
            } else {
                ViewStep::Keep
            }
        });
        self.finish_view_step(step);
        true
    }

    pub(crate) fn view_menu_key(&mut self, key: &Key) -> bool {
        let size = self.view_window_size();
        let step = self.with_view_menu(|menu, app| {
            let panel = menu.depth();
            let rows = menu.rows_at(app, panel);
            match key {
                Key::Named(NamedKey::Escape) | Key::Named(NamedKey::ArrowLeft) => {
                    if panel == 0 {
                        return ViewStep::Close;
                    }
                    menu.flyouts.pop();
                }
                Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::ArrowUp) => {
                    let indices: Vec<usize> = rows
                        .iter()
                        .enumerate()
                        .filter_map(|(index, row)| (!row.separator).then_some(index))
                        .collect();
                    if indices.is_empty() {
                        return ViewStep::Keep;
                    }
                    let down = matches!(key, Key::Named(NamedKey::ArrowDown));
                    let current = indices
                        .iter()
                        .position(|index| *index == menu.selected_at(panel));
                    let next = match (current, down) {
                        (Some(at), true) => (at + 1) % indices.len(),
                        (Some(at), false) => (at + indices.len() - 1) % indices.len(),
                        (None, true) => 0,
                        (None, false) => indices.len() - 1,
                    };
                    menu.set_selected(panel, indices[next]);
                    menu.ensure_visible(app, panel, size);
                }
                Key::Named(NamedKey::ArrowRight)
                | Key::Named(NamedKey::Enter)
                | Key::Named(NamedKey::Space) => {
                    let index = menu.selected_at(panel);
                    let Some(row) = rows.get(index).copied().filter(|row| row.enabled) else {
                        return ViewStep::Keep;
                    };
                    if let Some(page) = row.submenu {
                        let first = ViewMenuState::first_selectable(app, page);
                        menu.open_flyout(panel, index, page, first);
                    } else if let Some(action) = row.action {
                        return ViewStep::Run(action, row.extension_index);
                    }
                }
                _ => return ViewStep::Unhandled,
            }
            ViewStep::Keep
        });
        match step {
            Some(step) => self.finish_view_step(Some(step)),
            None => false,
        }
    }

    pub(crate) fn draw_view_menu(&self, renderer: &mut Renderer) {
        let Some(menu) = &self.view_menu else { return };
        let visible_extensions = self.visible_extension_activities();
        let (window_width, window_height) = renderer.size();
        let size = (window_width, window_height);
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        for panel in 0..=menu.depth() {
            let rows = menu.rows_at(self, panel);
            let (left, top, width, height) = menu.rect_at(self, panel, size);
            renderer.draw_rect(left, top, width, height, border);
            renderer.draw_rect(left + 1.0, top + 1.0, width - 2.0, height - 2.0, background);
            renderer.set_clip(Some([left, top, left + width, top + height]));
            let visible_rows =
                ((height - VIEW_MENU_PADDING * 2.0) / VIEW_ROW_HEIGHT).ceil() as usize;
            let scroll = menu.scroll_at(panel);
            let selected = menu.selected_at(panel);
            for (index, row) in rows.iter().enumerate().skip(scroll).take(visible_rows) {
                let y = top + VIEW_MENU_PADDING + (index - scroll) as f32 * VIEW_ROW_HEIGHT;
                if row.separator {
                    renderer.draw_rect(
                        left + 1.0,
                        y + VIEW_ROW_HEIGHT * 0.5,
                        width - 2.0,
                        1.0,
                        border,
                    );
                    continue;
                }
                if selected == index {
                    renderer.draw_rect(
                        left + 2.0,
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
                    renderer.draw_text("✓", left + 8.0, y + 4.0, color);
                }
                let label = row
                    .extension_index
                    .and_then(|index| {
                        visible_extensions
                            .get(index)
                            .map(|entry| entry.title.as_str())
                    })
                    .unwrap_or_else(|| self.t(row.label));
                renderer.draw_text(label, left + 28.0, y + 4.0, color);
                if !row.shortcut.is_empty() {
                    renderer.draw_text(
                        row.shortcut,
                        left + width
                            - row.shortcut.chars().count() as f32 * renderer.char_width()
                            - 12.0,
                        y + 4.0,
                        color,
                    );
                }
                if row.submenu.is_some() {
                    renderer.draw_text("›", left + width - 16.0, y + 4.0, color);
                }
            }
            renderer.set_clip(None);
        }
    }

    /// Center of a row of an open panel, for tests that drive the pointer.
    #[cfg(test)]
    pub(crate) fn view_menu_row_point(&self, panel: usize, row: usize) -> Option<(f32, f32)> {
        let menu = self.view_menu.as_ref()?;
        let (left, top, width, _) = menu.rect_at(self, panel, self.view_window_size());
        let scroll = menu.scroll_at(panel);
        Some((
            left + width / 2.0,
            top + VIEW_MENU_PADDING
                + (row - scroll) as f32 * VIEW_ROW_HEIGHT
                + VIEW_ROW_HEIGHT / 2.0,
        ))
    }

    /// The page shown by the innermost open panel.
    #[cfg(test)]
    pub(crate) fn view_menu_open_page(&self) -> Option<ViewPage> {
        self.view_menu
            .as_ref()
            .map(|menu| menu.page_at(menu.depth()))
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
            A::WordWrap => self.toggle_word_wrap(),
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
                self.set_secondary_sidebar_visible(!self.view_preferences.secondary_sidebar_visible)
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

    fn open_menu(app: &mut App) {
        app.open_view_menu(10.0, 40.0, 1200.0, 760.0);
    }

    fn state_with_appearance(selected: usize) -> ViewMenuState {
        ViewMenuState {
            x: 0.0,
            y: 0.0,
            page: ViewPage::Root,
            selected: 3,
            scroll: 0,
            flyouts: vec![Flyout {
                page: ViewPage::Appearance,
                from_row: 3,
                selected,
                scroll: 0,
            }],
        }
    }

    #[test]
    fn appearance_toggle_is_saved_to_the_active_profile() {
        let profile = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(profile.path().to_path_buf(), None, true);
        app.profile_root = profile.path().to_path_buf();
        app.view_menu = Some(state_with_appearance(18));
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
        app.view_menu.as_mut().unwrap().selected = 3;
        assert!(app.view_menu_key(&Key::Named(NamedKey::ArrowRight)));
        assert_eq!(app.view_menu_open_page(), Some(ViewPage::Appearance));
        app.view_menu.as_mut().unwrap().flyouts[0].selected = 25;
        // A window too short for the whole Appearance page scrolls to keep the row visible.
        app.with_view_menu(|menu, app| menu.ensure_visible(app, 1, (900.0, 300.0)));
        assert!(app.view_menu.as_ref().unwrap().flyouts[0].scroll > 0);
        assert!(app.view_menu_key(&Key::Named(NamedKey::Escape)));
        assert_eq!(app.view_menu_open_page(), Some(ViewPage::Root));
        assert!(app.view_menu_key(&Key::Named(NamedKey::Escape)));
        assert!(app.view_menu.is_none());
    }

    #[test]
    fn hovering_a_submenu_row_opens_it_beside_that_row() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        open_menu(&mut app);
        let (x, y) = app.view_menu_row_point(0, 3).unwrap();
        app.view_menu_pointer_moved(x, y);
        assert_eq!(app.view_menu_open_page(), Some(ViewPage::Appearance));
        let size = app.view_window_size();
        let menu = app.view_menu.as_ref().unwrap();
        let (root_x, _, root_width, _) = menu.rect_at(&app, 0, size);
        let (flyout_x, flyout_y, _, flyout_height) = menu.rect_at(&app, 1, size);
        assert_eq!(flyout_x, root_x + root_width);
        assert!(flyout_y >= 0.0 && flyout_y + flyout_height <= size.1);
        // The submenu is level with the hovered row, not with the top of the menu.
        let (_, hovered_row_y) = app.view_menu_row_point(0, 3).unwrap();
        assert!(flyout_y <= hovered_row_y && flyout_y + flyout_height > hovered_row_y);
    }

    #[test]
    fn hovering_inside_a_flyout_keeps_it_and_nested_submenus_cascade() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        open_menu(&mut app);
        let (x, y) = app.view_menu_row_point(0, 3).unwrap();
        app.view_menu_pointer_moved(x, y);
        let (x, y) = app.view_menu_row_point(1, 11).unwrap();
        app.view_menu_pointer_moved(x, y);
        assert_eq!(app.view_menu.as_ref().unwrap().depth(), 2);
        assert_eq!(app.view_menu_open_page(), Some(ViewPage::ActivityPosition));
        // Moving over a plain row of the first flyout closes the nested one only.
        let (x, y) = app.view_menu_row_point(1, 0).unwrap();
        app.view_menu_pointer_moved(x, y);
        assert_eq!(app.view_menu.as_ref().unwrap().depth(), 1);
        // A plain root row closes every flyout.
        let (x, y) = app.view_menu_row_point(0, 0).unwrap();
        app.view_menu_pointer_moved(x, y);
        assert_eq!(app.view_menu.as_ref().unwrap().depth(), 0);
    }

    #[test]
    fn a_flyout_opens_to_the_left_when_there_is_no_room_on_the_right() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_view_menu(900.0, 40.0, 1200.0, 760.0);
        let (x, y) = app.view_menu_row_point(0, 3).unwrap();
        app.view_menu_pointer_moved(x, y);
        let size = app.view_window_size();
        let menu = app.view_menu.as_ref().unwrap();
        let (root_x, _, _, _) = menu.rect_at(&app, 0, size);
        let (flyout_x, _, flyout_width, _) = menu.rect_at(&app, 1, size);
        assert_eq!(flyout_x + flyout_width, root_x);
    }

    #[test]
    fn clicking_outside_every_panel_closes_the_menu_and_an_action_closes_it_too() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        open_menu(&mut app);
        assert!(app.view_menu_click(1100.0, 700.0));
        assert!(app.view_menu.is_none());
        open_menu(&mut app);
        let (x, y) = app.view_menu_row_point(0, 3).unwrap();
        assert!(app.view_menu_click(x, y));
        assert_eq!(app.view_menu_open_page(), Some(ViewPage::Appearance));
        let before = app.view_preferences.minimap_visible;
        let (x, y) = app.view_menu_row_point(1, 18).unwrap();
        app.view_menu_click(x, y);
        assert_eq!(app.view_preferences.minimap_visible, !before);
        assert!(app.view_menu.is_none());
    }
}
