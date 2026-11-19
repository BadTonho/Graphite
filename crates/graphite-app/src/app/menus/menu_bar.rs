use super::*;
use winit::keyboard::{Key, NamedKey};

/// A rectangle as `(x, y, width, height)`.
type RectTuple = (f32, f32, f32, f32);

const MENU_START_X: f32 = 38.0;
const MENU_HORIZONTAL_PADDING: f32 = 8.0;
const EMPTY_MENU_WIDTH: f32 = 220.0;
const EMPTY_MENU_HEIGHT: f32 = 40.0;
const TITLEBAR_ICON_LEFT: f32 = 9.0;
const TITLEBAR_ICON_SIZE: f32 = 20.0;
const TITLEBAR_CONTROL_WIDTH: f32 = 46.0;
const TITLEBAR_RESIZE_EDGE: f32 = 6.0;
const OVERFLOW_ROW_HEIGHT: f32 = 28.0;
const COMMAND_CENTER_WIDTH: f32 = 420.0;
const COMMAND_CENTER_MIN_WIDTH: f32 = 64.0;
const COMMAND_CENTER_BUTTON_WIDTH: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchMenu {
    File,
    Edit,
    Selection,
    View,
    Go,
    Run,
    Terminal,
    Help,
    More,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowControl {
    Minimize,
    MaximizeRestore,
    Close,
}

impl WorkbenchMenu {
    pub(crate) const ALL: [Self; 8] = [
        Self::File,
        Self::Edit,
        Self::Selection,
        Self::View,
        Self::Go,
        Self::Run,
        Self::Terminal,
        Self::Help,
    ];

    pub(crate) fn label_key(self) -> &'static str {
        match self {
            Self::File => "menubar.file",
            Self::Edit => "menubar.edit",
            Self::Selection => "menubar.selection",
            Self::View => "menubar.view",
            Self::Go => "menubar.go",
            Self::Run => "menubar.run",
            Self::Terminal => "menubar.terminal",
            Self::Help => "menubar.help",
            Self::More => "menubar.more",
        }
    }

    fn label_len(self) -> usize {
        match self {
            Self::File => 7,
            Self::Edit => 6,
            Self::Selection => 9,
            Self::View => 5,
            Self::Go => 2,
            Self::Run => 8,
            Self::Terminal => 8,
            Self::Help => 5,
            Self::More => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MenuItemRect {
    menu: WorkbenchMenu,
    x: f32,
    width: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TitlebarControlRects {
    minimize: MenuItemRectLike,
    maximize: MenuItemRectLike,
    close: MenuItemRectLike,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CommandCenterRects {
    back: MenuItemRectLike,
    forward: MenuItemRectLike,
    field: MenuItemRectLike,
}

impl CommandCenterRects {
    fn at(self, x: f32, y: f32) -> Option<CommandCenterAction> {
        if self.back.contains(x, y) {
            Some(CommandCenterAction::Back)
        } else if self.forward.contains(x, y) {
            Some(CommandCenterAction::Forward)
        } else if self.field.contains(x, y) {
            Some(CommandCenterAction::Search)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandCenterAction {
    Back,
    Forward,
    Search,
}

impl TitlebarControlRects {
    fn at(self, x: f32, y: f32) -> Option<WindowControl> {
        if self.minimize.contains(x, y) {
            Some(WindowControl::Minimize)
        } else if self.maximize.contains(x, y) {
            Some(WindowControl::MaximizeRestore)
        } else if self.close.contains(x, y) {
            Some(WindowControl::Close)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MenuItemRectLike {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl MenuItemRectLike {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TitlebarLayout {
    icon: MenuItemRectLike,
    controls: TitlebarControlRects,
    pub(crate) layout_controls: [(
        super::titlebar_layout_controls::LayoutControl,
        super::titlebar_layout_controls::Rect,
    ); 4],
    pub(crate) update_button: Option<MenuItemRectLike>,
    command_center: CommandCenterRects,
    menus: Vec<MenuItemRect>,
    overflowed: Vec<WorkbenchMenu>,
    height: f32,
}

impl TitlebarLayout {
    fn new(
        width: f32,
        height: f32,
        char_width: f32,
        scale: f32,
        localized_widths: [f32; 8],
        update_button_width: f32,
    ) -> Self {
        let scale = scale.max(0.5);
        let control_width = TITLEBAR_CONTROL_WIDTH * scale;
        let icon = MenuItemRectLike {
            x: TITLEBAR_ICON_LEFT * scale,
            y: (height - TITLEBAR_ICON_SIZE * scale) * 0.5,
            width: TITLEBAR_ICON_SIZE * scale,
            height: TITLEBAR_ICON_SIZE * scale,
        };
        let controls = TitlebarControlRects {
            minimize: MenuItemRectLike {
                x: width - control_width * 3.0,
                y: 0.0,
                width: control_width,
                height,
            },
            maximize: MenuItemRectLike {
                x: width - control_width * 2.0,
                y: 0.0,
                width: control_width,
                height,
            },
            close: MenuItemRectLike {
                x: width - control_width,
                y: 0.0,
                width: control_width,
                height,
            },
        };
        let layout_controls = super::titlebar_layout_controls::layout_control_rects(
            controls.minimize.x,
            height,
            scale,
        );
        let update_button = if update_button_width > 0.0 {
            let btn_h = (22.0 * scale).min(height);
            let btn_x = (layout_controls[0].1 .0 - update_button_width - 6.0 * scale).max(0.0);
            Some(MenuItemRectLike {
                x: btn_x,
                y: (height - btn_h) * 0.5,
                width: update_button_width,
                height: btn_h,
            })
        } else {
            None
        };
        let right = update_button.map_or(layout_controls[0].1 .0, |btn| btn.x) - 5.0 * scale;
        let center_x = width * 0.5;
        let max_field_width = ((right - center_x - 4.0 * scale) * 2.0).max(0.0);
        let preferred_field_width = (COMMAND_CENTER_WIDTH * scale)
            .min(width * 0.4)
            .min(max_field_width);
        let minimum_field_width = COMMAND_CENTER_MIN_WIDTH * scale.min(1.0);
        let field_width = if max_field_width >= minimum_field_width {
            preferred_field_width.max(minimum_field_width)
        } else {
            max_field_width
        };
        let field = MenuItemRectLike {
            x: center_x - field_width * 0.5,
            y: (height - 26.0 * scale) * 0.5,
            width: field_width,
            height: (26.0 * scale).min(height),
        };
        let button_width = COMMAND_CENTER_BUTTON_WIDTH * scale;
        let button_height = field.height;
        let left_space = (field.x - (icon.x + icon.width) - 10.0 * scale).max(0.0);
        let button_width = button_width.min((left_space * 0.5).max(0.0));
        let command_center = CommandCenterRects {
            back: MenuItemRectLike {
                x: field.x - button_width * 2.0,
                y: field.y,
                width: button_width,
                height: button_height,
            },
            forward: MenuItemRectLike {
                x: field.x - button_width,
                y: field.y,
                width: button_width,
                height: button_height,
            },
            field,
        };
        let mut menu_rects = Vec::new();
        let widths = localized_widths.to_vec();
        let available = (command_center.back.x - 5.0 * scale - MENU_START_X * scale).max(0.0);
        let total: f32 = widths.iter().sum();
        let overflow_width =
            (4.0 * char_width + MENU_HORIZONTAL_PADDING * 2.0 * scale).max(38.0 * scale);
        let visible_count = if total <= available {
            WorkbenchMenu::ALL.len()
        } else {
            let mut count = WorkbenchMenu::ALL.len().saturating_sub(1);
            while count > 0 && widths[..count].iter().sum::<f32>() + overflow_width > available {
                count -= 1;
            }
            count
        };
        let mut x = MENU_START_X * scale;
        for (index, menu) in WorkbenchMenu::ALL
            .iter()
            .copied()
            .take(visible_count)
            .enumerate()
        {
            let rect = MenuItemRect {
                menu,
                x,
                width: widths[index],
            };
            x += rect.width;
            menu_rects.push(rect);
        }
        let overflowed = WorkbenchMenu::ALL[visible_count..].to_vec();
        if !overflowed.is_empty() {
            let menu_right = command_center.back.x - 5.0 * scale;
            let more = MenuItemRect {
                menu: WorkbenchMenu::More,
                x,
                width: overflow_width.min((menu_right - x).max(0.0)),
            };
            if more.width > 0.0 {
                menu_rects.push(more);
            }
        }
        Self {
            icon,
            controls,
            layout_controls,
            update_button,
            command_center,
            menus: menu_rects,
            overflowed,
            height,
        }
    }

    fn more_rect(&self) -> Option<MenuItemRect> {
        self.menus
            .iter()
            .copied()
            .find(|item| item.menu == WorkbenchMenu::More)
    }

    fn menu_anchor(&self, menu: WorkbenchMenu) -> f32 {
        self.menus
            .iter()
            .find(|item| item.menu == menu)
            .map(|item| item.x)
            .or_else(|| self.more_rect().map(|item| item.x))
            .unwrap_or(MENU_START_X)
    }

    fn overflow_popup(&self, char_width: f32, window_width: f32) -> MenuItemRectLike {
        let desired_width = self
            .overflowed
            .iter()
            .map(|menu| menu.label_len() as f32 * char_width + 46.0)
            .fold(160.0, f32::max);
        let popup_width = desired_width.min((window_width - 4.0).max(80.0));
        let x = self
            .more_rect()
            .map_or(MENU_START_X, |item| item.x)
            .clamp(0.0, (window_width - popup_width).max(0.0));
        MenuItemRectLike {
            x,
            y: self.height,
            width: popup_width,
            height: self.overflowed.len() as f32 * OVERFLOW_ROW_HEIGHT + 8.0,
        }
    }
}

fn resize_direction_at(
    width: f32,
    height: f32,
    edge: f32,
    x: f32,
    y: f32,
    maximized: bool,
) -> Option<winit::window::ResizeDirection> {
    if maximized {
        return None;
    }
    let north = y < edge;
    let south = y >= height - edge;
    let west = x < edge;
    let east = x >= width - edge;
    use winit::window::ResizeDirection::*;
    match (north, south, west, east) {
        (true, _, true, _) => Some(NorthWest),
        (true, _, _, true) => Some(NorthEast),
        (_, true, true, _) => Some(SouthWest),
        (_, true, _, true) => Some(SouthEast),
        (true, _, _, _) => Some(North),
        (_, true, _, _) => Some(South),
        (_, _, true, _) => Some(West),
        (_, _, _, true) => Some(East),
        _ => None,
    }
}

fn resize_direction_for_titlebar(
    layout: &TitlebarLayout,
    width: f32,
    window_height: f32,
    edge: f32,
    x: f32,
    y: f32,
    maximized: bool,
) -> Option<winit::window::ResizeDirection> {
    let direction = resize_direction_at(width, window_height, edge, x, y, maximized)?;
    let on_top_edge = y < edge;
    let on_corner = x < edge || x >= width - edge;
    if on_top_edge
        && !on_corner
        && (layout.icon.contains(x, y)
            || layout.command_center.back.contains(x, y)
            || layout.command_center.forward.contains(x, y)
            || layout.command_center.field.contains(x, y)
            || layout
                .menus
                .iter()
                .any(|item| item.contains(x, y, layout.height))
            || layout.controls.at(x, y).is_some())
    {
        None
    } else {
        Some(direction)
    }
}

impl MenuItemRect {
    fn contains(self, x: f32, y: f32, height: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= 0.0 && y < height
    }
}

impl App {
    pub(crate) fn top_menu_height(&self) -> f32 {
        if self.zen_mode {
            return 0.0;
        }
        // Match the tab strip's 1.5 line height, including display scaling.
        self.font_size * self.sidebar_scale_factor * 2.1
    }

    pub(crate) fn command_center_rects(
        &self,
        width: f32,
        char_width: f32,
    ) -> (RectTuple, RectTuple, RectTuple) {
        let layout = self.titlebar_layout(width, char_width);
        let rect = |rect: MenuItemRectLike| (rect.x, rect.y, rect.width, rect.height);
        (
            rect(layout.command_center.back),
            rect(layout.command_center.forward),
            rect(layout.command_center.field),
        )
    }

    pub(crate) fn titlebar_layout(&self, width: f32, char_width: f32) -> TitlebarLayout {
        let scale = self.sidebar_scale_factor.max(0.5);
        let localized_widths = WorkbenchMenu::ALL.map(|menu| {
            self.t(menu.label_key()).chars().count() as f32 * char_width
                + MENU_HORIZONTAL_PADDING * 2.0 * scale
        });
        let update_button_width = self.titlebar_update_button().map_or(0.0, |btn| {
            btn.label.chars().count() as f32 * char_width + 16.0 * scale
        });
        TitlebarLayout::new(
            width,
            self.top_menu_height(),
            char_width,
            scale,
            localized_widths,
            update_button_width,
        )
    }

    pub(crate) fn titlebar_accessible_menus(
        &self,
        width: f32,
        char_width: f32,
    ) -> Vec<WorkbenchMenu> {
        if !self.view_preferences.menu_bar_visible {
            return Vec::new();
        }
        self.titlebar_layout(width, char_width)
            .menus
            .into_iter()
            .map(|item| item.menu)
            .collect()
    }

    pub(crate) fn view_menu_open(&mut self, x: f32, width: f32, height: f32) {
        let char_width = self
            .gpu
            .as_ref()
            .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width());
        let anchor = self
            .titlebar_layout(width, char_width)
            .menu_anchor(WorkbenchMenu::View);
        self.view_menu_open_at(if anchor > 0.0 { anchor } else { x }, width, height);
    }

    fn view_menu_open_at(&mut self, x: f32, width: f32, height: f32) {
        self.open_view_menu(x, self.top_menu_height(), width, height);
    }

    pub(crate) fn close_workbench_menu(&mut self) {
        self.workbench_menu = None;
        self.workbench_menu_overflow = false;
        self.activity_bar_menu = None;
        self.settings_menu = None;
        self.profile_menu = None;
        self.view_menu = None;
        self.run_breakpoint_submenu = false;
        self.go_submenu = None;
    }

    pub(crate) fn dismiss_workbench_menu(&mut self) {
        self.close_workbench_menu();
    }

    fn activate_menu(&mut self, menu: WorkbenchMenu, anchor_x: f32, width: f32, height: f32) {
        if self.command_center.is_some() {
            self.close_command_center();
        }
        self.workbench_menu = Some(menu);
        self.workbench_menu_overflow = false;
        self.activity_bar_menu = None;
        self.settings_menu = None;
        self.profile_menu = None;
        self.view_menu = None;
        if menu == WorkbenchMenu::File {
            self.file_menu_submenu = None;
            self.file_menu_nested = None;
            self.file_menu_selection = 0;
        }
        if menu == WorkbenchMenu::Terminal {
            self.terminal_menu_selection = 0;
        }
        if menu == WorkbenchMenu::Selection {
            self.selection_menu_selection = 0;
        }
        if menu == WorkbenchMenu::Go {
            self.go_menu_selection = 0;
            self.go_submenu = None;
        }
        if menu == WorkbenchMenu::Run {
            self.open_run_menu();
        }
        if menu == WorkbenchMenu::Help {
            self.help_menu_selection = 0;
        }
        if menu == WorkbenchMenu::View {
            self.view_menu_open_at(anchor_x, width, height);
        }
    }

    pub(crate) fn titlebar_control_click(&mut self, x: f32, y: f32) -> bool {
        let Some(width) = self.gpu.as_ref().map(|gpu| gpu.renderer.size().0) else {
            return false;
        };
        let layout = self.titlebar_layout(width, self.font_size * 0.6);
        if layout.update_button.is_some_and(|rect| rect.contains(x, y)) {
            self.click_titlebar_update_button();
            return true;
        }
        if let Some(control) =
            super::titlebar_layout_controls::layout_control_at(&layout.layout_controls, x, y)
        {
            let anchor = layout
                .layout_controls
                .iter()
                .find(|(item, _)| *item == control)
                .map_or(x, |(_, rect)| rect.0);
            self.run_layout_control(control, anchor);
            return true;
        }
        let target = layout.controls.at(x, y);
        let Some(target) = target else { return false };
        self.titlebar_control_action(target);
        true
    }

    pub(crate) fn titlebar_control_action(&mut self, control: WindowControl) {
        match control {
            WindowControl::Minimize => {
                if let Some(gpu) = &self.gpu {
                    gpu.window.set_minimized(true);
                }
            }
            WindowControl::MaximizeRestore => {
                if let Some(gpu) = &self.gpu {
                    gpu.window.set_maximized(!gpu.window.is_maximized());
                }
            }
            WindowControl::Close => self.run_ui_command("workbench.action.quit"),
        }
    }

    pub(crate) fn titlebar_resize_direction(
        &self,
        x: f32,
        y: f32,
    ) -> Option<winit::window::ResizeDirection> {
        let gpu = self.gpu.as_ref()?;
        if gpu.window.is_maximized() || !gpu.window.is_resizable() {
            return None;
        }
        let (width, height) = gpu.renderer.size();
        let edge = TITLEBAR_RESIZE_EDGE * self.sidebar_scale_factor;
        let layout = self.titlebar_layout(width, gpu.renderer.char_width());
        resize_direction_for_titlebar(
            &layout,
            width,
            height,
            edge,
            x,
            y,
            gpu.window.is_maximized(),
        )
    }

    pub(crate) fn titlebar_handle_press(&mut self, x: f32, y: f32) -> bool {
        if let Some(direction) = self.titlebar_resize_direction(x, y) {
            if let Some(gpu) = &self.gpu {
                let _ = gpu.window.drag_resize_window(direction);
            }
            self.titlebar_pointer_down = None;
            return true;
        }
        let Some((width, char_width, window)) = self.gpu.as_ref().map(|gpu| {
            let (width, _) = gpu.renderer.size();
            (width, gpu.renderer.char_width(), gpu.window.clone())
        }) else {
            return false;
        };
        let layout = self.titlebar_layout(width, char_width);
        if y >= layout.height {
            return false;
        }
        if self.titlebar_control_click(x, y) {
            return true;
        }
        if layout.icon.contains(x, y) {
            return true;
        }
        if self.view_preferences.menu_bar_visible
            && layout
                .menus
                .iter()
                .any(|item| item.contains(x, y, layout.height))
        {
            let _ = self.workbench_menu_click(x, y);
            return true;
        }

        match layout.command_center.at(x, y) {
            Some(CommandCenterAction::Back) => {
                if self.can_navigate_history(-1) {
                    self.run_ui_command("workbench.action.navigateBack");
                }
                return true;
            }
            Some(CommandCenterAction::Forward) => {
                if self.can_navigate_history(1) {
                    self.run_ui_command("workbench.action.navigateForward");
                }
                return true;
            }
            Some(CommandCenterAction::Search) => {
                self.open_command_center();
                return true;
            }
            None => {}
        }

        let now = std::time::Instant::now();
        let double_click = self
            .last_titlebar_click
            .is_some_and(|last| now.duration_since(last) <= std::time::Duration::from_millis(450));
        if double_click {
            window.set_maximized(!window.is_maximized());
            self.last_titlebar_click = None;
            self.titlebar_pointer_down = None;
        } else {
            self.titlebar_pointer_down = Some((x, y));
            self.titlebar_drag_started = false;
        }
        true
    }

    pub(crate) fn titlebar_pointer_moved(&mut self, x: f32, y: f32) -> bool {
        let Some((start_x, start_y)) = self.titlebar_pointer_down else {
            return false;
        };
        if !self.titlebar_drag_started
            && ((x - start_x).powi(2) + (y - start_y).powi(2)).sqrt() >= 4.0
        {
            if let Some(gpu) = &self.gpu {
                let _ = gpu.window.drag_window();
            }
            self.titlebar_drag_started = true;
            self.last_titlebar_click = None;
        }
        true
    }

    pub(crate) fn titlebar_pointer_released(&mut self) -> bool {
        if self.titlebar_pointer_down.take().is_none() {
            return false;
        }
        if !self.titlebar_drag_started {
            self.last_titlebar_click = Some(std::time::Instant::now());
        }
        self.titlebar_drag_started = false;
        true
    }

    pub(crate) fn titlebar_cursor(&self, x: f32, y: f32) -> Option<winit::window::CursorIcon> {
        if let Some(direction) = self.titlebar_resize_direction(x, y) {
            return Some(direction.into());
        }
        let gpu = self.gpu.as_ref()?;
        let layout = self.titlebar_layout(gpu.renderer.size().0, gpu.renderer.char_width());
        if layout.command_center.field.contains(x, y) {
            return Some(winit::window::CursorIcon::Text);
        }
        if layout.command_center.back.contains(x, y) || layout.command_center.forward.contains(x, y)
        {
            return Some(winit::window::CursorIcon::Pointer);
        }
        (y < layout.height).then_some(winit::window::CursorIcon::Default)
    }

    pub(crate) fn can_navigate_history(&self, direction: isize) -> bool {
        let Some(index) = self.navigation_history_index else {
            return false;
        };
        if direction < 0 {
            (0..index).rev().any(|candidate| {
                self.documents
                    .resolve(self.navigation_history[candidate].0)
                    .is_some()
            })
        } else {
            ((index + 1)..self.navigation_history.len()).any(|candidate| {
                self.documents
                    .resolve(self.navigation_history[candidate].0)
                    .is_some()
            })
        }
    }

    pub(crate) fn workbench_menu_click(&mut self, x: f32, y: f32) -> bool {
        let Some((width, height, char_width)) = self.gpu.as_ref().map(|gpu| {
            let (width, height) = gpu.renderer.size();
            (width, height, gpu.renderer.char_width())
        }) else {
            return false;
        };
        let layout = self.titlebar_layout(width, char_width);
        if y < layout.height {
            let selected = self
                .view_preferences
                .menu_bar_visible
                .then(|| {
                    layout
                        .menus
                        .iter()
                        .copied()
                        .find(|item| item.contains(x, y, layout.height))
                        .map(|item| item.menu)
                })
                .flatten();
            match selected {
                Some(WorkbenchMenu::More) => {
                    if self.workbench_menu == Some(WorkbenchMenu::More)
                        && self.workbench_menu_overflow
                    {
                        self.close_workbench_menu();
                    } else {
                        self.workbench_menu = Some(WorkbenchMenu::More);
                        self.workbench_menu_overflow = true;
                        self.workbench_overflow_selection = 0;
                        self.activity_bar_menu = None;
                        self.settings_menu = None;
                        self.profile_menu = None;
                    }
                }
                Some(selected) => {
                    if self.workbench_menu == Some(selected) && !self.workbench_menu_overflow {
                        self.close_workbench_menu();
                    } else {
                        self.workbench_menu = Some(selected);
                        self.workbench_menu_overflow = false;
                        self.activity_bar_menu = None;
                        self.settings_menu = None;
                        self.profile_menu = None;
                        if selected == WorkbenchMenu::File {
                            self.file_menu_submenu = None;
                            self.file_menu_nested = None;
                            self.file_menu_selection = 0;
                        } else if selected == WorkbenchMenu::Edit {
                            self.edit_menu_selection = 0;
                        } else if selected == WorkbenchMenu::Selection {
                            self.selection_menu_selection = 0;
                        } else if selected == WorkbenchMenu::Terminal {
                            self.terminal_menu_selection = 0;
                        } else if selected == WorkbenchMenu::Run {
                            self.open_run_menu();
                        } else if selected == WorkbenchMenu::Help {
                            self.help_menu_selection = 0;
                        } else if selected == WorkbenchMenu::View {
                            self.view_menu_open_at(layout.menu_anchor(selected), width, height);
                        }
                    }
                }
                None => self.close_workbench_menu(),
            }
            return true;
        }

        let Some(active) = self.workbench_menu else {
            return false;
        };
        if active == WorkbenchMenu::More && self.workbench_menu_overflow {
            let popup = layout.overflow_popup(char_width, width);
            let index = ((y - popup.y - 4.0) / OVERFLOW_ROW_HEIGHT) as usize;
            if popup.contains(x, y) && index < layout.overflowed.len() && y >= popup.y + 4.0 {
                let selected = layout.overflowed[index];
                self.workbench_menu = Some(selected);
                self.workbench_menu_overflow = false;
                if selected == WorkbenchMenu::Terminal {
                    self.terminal_menu_selection = 0;
                }
                if selected == WorkbenchMenu::Selection {
                    self.selection_menu_selection = 0;
                }
                if selected == WorkbenchMenu::Run {
                    self.open_run_menu();
                }
                if selected == WorkbenchMenu::View {
                    self.view_menu_open_at(popup.x, width, height);
                }
                return true;
            }
            self.close_workbench_menu();
        } else if active == WorkbenchMenu::Selection {
            let popup_width = self.selection_menu_width(char_width, width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Selection)
                .clamp(0.0, (width - popup_width).max(0.0));
            let popup_height =
                super::selection_menu::selection_menu_height(&self.selection_menu_rows());
            if x >= popup_x
                && x < popup_x + popup_width
                && y >= layout.height
                && y < layout.height + popup_height
            {
                if let Some((command, true)) =
                    self.selection_menu_command_at(x, y, popup_x, layout.height, popup_width)
                {
                    self.execute_selection_menu_command(command);
                    self.close_workbench_menu();
                }
            } else {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::Edit {
            let popup_width = super::edit_menu::EDIT_MENU_WIDTH.min(width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Edit)
                .clamp(0.0, (width - popup_width).max(0.0));
            if let Some((command, enabled)) =
                self.edit_menu_command_at(x, y, popup_x, layout.height)
            {
                if enabled {
                    self.execute_edit_menu_command(command);
                    self.close_workbench_menu();
                }
            } else if x < popup_x || x >= popup_x + popup_width || y < layout.height {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::File {
            let popup_width = super::file_menu::FILE_MENU_WIDTH.min(width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::File)
                .clamp(0.0, (width - popup_width).max(0.0));
            let (submenu_x, nested_x, submenu_width) =
                Self::file_panel_xs(popup_x, popup_width, width);
            if let Some((command, enabled)) = self.file_nested_command_at(x, y, nested_x, height) {
                if enabled {
                    self.execute_settings_subcommand(command);
                    self.close_workbench_menu();
                }
                return true;
            }
            if let Some((command, submenu, enabled)) =
                self.file_menu_command_at(x, y, popup_x, submenu_x, nested_x, height)
            {
                if !enabled {
                    return true;
                }
                if let Some(submenu) = submenu {
                    self.open_file_submenu(submenu);
                } else {
                    self.execute_file_menu_command(command);
                    self.close_workbench_menu();
                }
            } else {
                let inside_panel = |app: &Self, open: Option<_>, left: f32, top: f32| {
                    let panel_height = open.map_or(0.0, |open| app.file_panel_height(open));
                    open.is_some()
                        && x >= left
                        && x < left + submenu_width
                        && y >= top
                        && y < top + panel_height.max(250.0)
                };
                let in_submenu = inside_panel(
                    self,
                    self.file_menu_submenu,
                    submenu_x,
                    self.file_submenu_top(height),
                ) || inside_panel(
                    self,
                    self.file_menu_nested,
                    nested_x,
                    self.file_nested_top(height),
                );
                if !in_submenu {
                    self.close_workbench_menu();
                }
            }
        } else if active == WorkbenchMenu::Go {
            let popup_width = self.go_menu_width(char_width, width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Go)
                .clamp(0.0, (width - popup_width).max(0.0));
            if let Some(submenu) = self.go_submenu {
                let sub_width = self.go_submenu_width(submenu, char_width, width);
                let sub_x = if popup_x + popup_width + sub_width <= width {
                    popup_x + popup_width
                } else {
                    (popup_x - sub_width).max(0.0)
                };
                let sub_y = self.go_submenu_anchor_y(layout.height, popup_x, popup_width);
                if let Some((command, true)) =
                    self.go_submenu_command_at(submenu, x, y, sub_x, sub_y, sub_width)
                {
                    self.go_submenu_execute(command);
                } else if x >= popup_x && x < popup_x + popup_width {
                    if let Some((command, enabled, child)) =
                        self.go_menu_command_at(x, y, popup_x, layout.height, popup_width)
                    {
                        if let Some(child) = child {
                            self.go_submenu = Some(child);
                            self.go_submenu_selection = 0;
                        } else if enabled {
                            self.go_menu_execute(command);
                            self.close_workbench_menu();
                        }
                    }
                } else if x < sub_x || x >= sub_x + sub_width || y < layout.height {
                    self.close_workbench_menu();
                }
            } else if let Some((command, enabled, submenu)) =
                self.go_menu_command_at(x, y, popup_x, layout.height, popup_width)
            {
                if let Some(submenu) = submenu {
                    self.go_submenu = Some(submenu);
                    self.go_submenu_selection = 0;
                } else if enabled {
                    self.go_menu_execute(command);
                    self.close_workbench_menu();
                }
            } else if x < popup_x || x >= popup_x + popup_width || y < layout.height {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::Terminal {
            let popup_width = super::terminal_menu::TERMINAL_MENU_WIDTH.min(width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Terminal)
                .clamp(0.0, (width - popup_width).max(0.0));
            if let Some((command, enabled)) =
                self.terminal_menu_command_at(x, y, popup_x, layout.height)
            {
                if enabled {
                    self.execute_terminal_menu_command(command);
                    self.close_workbench_menu();
                }
            } else if x < popup_x || x >= popup_x + popup_width || y < layout.height {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::Run {
            let popup_width = self.run_menu_width(char_width, width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Run)
                .clamp(0.0, (width - popup_width).max(0.0));
            let submenu_width = self.run_breakpoint_menu_width(char_width, width);
            let submenu_x =
                self.run_breakpoint_submenu_position(popup_x, popup_width, width, submenu_width);
            let submenu_height = self
                .run_breakpoint_rows()
                .iter()
                .map(|row| {
                    if matches!(row, super::run_menu::RunMenuRow::Separator) {
                        9.0
                    } else {
                        28.0
                    }
                })
                .sum::<f32>()
                + 8.0;
            let submenu_top = self.run_breakpoint_submenu_top(layout.height, height);
            let in_submenu = self.run_breakpoint_submenu
                && x >= submenu_x
                && x < submenu_x + submenu_width
                && y >= submenu_top
                && y < submenu_top + submenu_height;
            if in_submenu {
                if let Some((command, enabled)) =
                    self.run_breakpoint_command_at(x, y, submenu_x, submenu_top, submenu_width)
                {
                    if enabled {
                        self.execute_run_menu_command(command);
                        self.close_workbench_menu();
                    } else {
                        self.show_run_menu_unavailable_reason(command, true);
                    }
                }
            } else if let Some((command, enabled, submenu)) =
                self.run_menu_command_at(x, y, popup_x, layout.height, popup_width)
            {
                if submenu {
                    self.run_breakpoint_submenu = true;
                    self.run_breakpoint_selection = 0;
                } else if enabled {
                    self.execute_run_menu_command(command);
                    self.close_workbench_menu();
                } else {
                    self.show_run_menu_unavailable_reason(command, false);
                }
            } else {
                let main_height = self
                    .run_menu_rows()
                    .iter()
                    .map(|row| {
                        if matches!(row, super::run_menu::RunMenuRow::Separator) {
                            9.0
                        } else {
                            28.0
                        }
                    })
                    .sum::<f32>()
                    + 8.0;
                let in_main = x >= popup_x
                    && x < popup_x + popup_width
                    && y >= layout.height
                    && y < layout.height + main_height;
                if !in_main {
                    self.close_workbench_menu();
                }
            }
        } else if active == WorkbenchMenu::Help {
            let popup_width = self.help_menu_width(char_width, width);
            let popup_x = layout
                .menu_anchor(WorkbenchMenu::Help)
                .clamp(0.0, (width - popup_width).max(0.0));
            if let Some((command, enabled)) =
                self.help_menu_command_at(x, y, popup_x, layout.height, popup_width)
            {
                if enabled {
                    self.execute_help_command(command);
                    self.close_workbench_menu();
                }
            } else if x < popup_x || x >= popup_x + popup_width || y < layout.height {
                self.close_workbench_menu();
            }
        } else if active == WorkbenchMenu::View {
            let _ = self.view_menu_click(x, y);
            if self.view_menu.is_none() {
                self.close_workbench_menu();
            }
        } else {
            self.close_workbench_menu();
        }
        true
    }

    pub(crate) fn workbench_menu_pointer_moved(&mut self, x: f32, y: f32) {
        let Some((char_width, width, height)) = self.gpu.as_ref().map(|gpu| {
            (
                gpu.renderer.char_width(),
                gpu.renderer.size().0,
                gpu.renderer.size().1,
            )
        }) else {
            return;
        };
        let layout = self.titlebar_layout(width, char_width);
        let Some(active) = self.workbench_menu else {
            return;
        };
        if active == WorkbenchMenu::More && self.workbench_menu_overflow {
            let popup = layout.overflow_popup(char_width, width);
            if popup.contains(x, y) && y >= popup.y + 4.0 {
                self.workbench_overflow_selection = (((y - popup.y - 4.0) / OVERFLOW_ROW_HEIGHT)
                    as usize)
                    .min(layout.overflowed.len().saturating_sub(1));
            }
            return;
        }
        if y >= layout.height {
            if active == WorkbenchMenu::Go {
                let popup_width = self.go_menu_width(char_width, width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Go)
                    .clamp(0.0, (width - popup_width).max(0.0));
                let in_main = x >= popup_x && x < popup_x + popup_width;
                if let Some(submenu) = self.go_submenu {
                    let sub_width = self.go_submenu_width(submenu, char_width, width);
                    let sub_x = if popup_x + popup_width + sub_width <= width {
                        popup_x + popup_width
                    } else {
                        (popup_x - sub_width).max(0.0)
                    };
                    let sub_y = self.go_submenu_anchor_y(layout.height, popup_x, popup_width);
                    if x >= sub_x && x < sub_x + sub_width && y >= sub_y {
                        self.go_submenu_pointer_selection(submenu, y, sub_y);
                    } else if in_main {
                        self.go_menu_pointer_selection(y, layout.height);
                        if let Some((_, _, hovered)) =
                            self.go_menu_command_at(x, y, popup_x, layout.height, popup_width)
                        {
                            if hovered != self.go_submenu {
                                self.go_submenu = hovered;
                                self.go_submenu_selection = 0;
                            }
                        }
                    }
                } else if in_main {
                    self.go_menu_pointer_selection(y, layout.height);
                    if let Some((_, _, Some(submenu))) =
                        self.go_menu_command_at(x, y, popup_x, layout.height, popup_width)
                    {
                        self.go_submenu = Some(submenu);
                        self.go_submenu_selection = 0;
                    }
                }
            } else if active == WorkbenchMenu::Selection {
                let popup_width = self.selection_menu_width(char_width, width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Selection)
                    .clamp(0.0, (width - popup_width).max(0.0));
                if x >= popup_x && x < popup_x + popup_width {
                    self.selection_menu_pointer_selection(y, layout.height);
                }
            } else if active == WorkbenchMenu::Edit {
                let popup_width = super::edit_menu::EDIT_MENU_WIDTH.min(width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Edit)
                    .clamp(0.0, (width - popup_width).max(0.0));
                if x >= popup_x && x < popup_x + popup_width {
                    self.edit_menu_pointer_selection(y, layout.height);
                }
            } else if active == WorkbenchMenu::File {
                let popup_width = super::file_menu::FILE_MENU_WIDTH.min(width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::File)
                    .clamp(0.0, (width - popup_width).max(0.0));
                let (submenu_x, nested_x, submenu_width) =
                    Self::file_panel_xs(popup_x, popup_width, width);
                let in_nested = self.file_menu_nested.is_some()
                    && x >= nested_x
                    && x < nested_x + submenu_width;
                let in_submenu = !in_nested
                    && self.file_menu_submenu.is_some()
                    && x >= submenu_x
                    && x < submenu_x + submenu_width;
                let hovered = self
                    .file_menu_command_at(x, y, popup_x, submenu_x, nested_x, height)
                    .and_then(|(_, submenu, enabled)| enabled.then_some(submenu).flatten());
                if in_nested {
                    self.file_menu_pointer_selection(y, self.file_menu_nested, height);
                } else if in_submenu {
                    self.file_menu_pointer_selection(y, self.file_menu_submenu, height);
                    // Hovering Profiles or Themes opens them beside their row; any other row
                    // closes the nested submenu.
                    match hovered {
                        Some(child) if self.file_menu_nested != Some(child) => {
                            self.open_file_submenu(child)
                        }
                        Some(_) => {}
                        None => self.file_menu_nested = None,
                    }
                } else if x >= popup_x && x < popup_x + popup_width {
                    if hovered.is_none() || hovered != self.file_menu_submenu {
                        self.file_menu_pointer_selection(y, None, height);
                    }
                    if hovered != self.file_menu_submenu {
                        self.file_menu_submenu = hovered;
                        self.file_menu_nested = None;
                        if hovered.is_some() {
                            self.file_menu_selection = 0;
                        }
                    }
                }
            } else if active == WorkbenchMenu::Terminal {
                let popup_width = super::terminal_menu::TERMINAL_MENU_WIDTH.min(width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Terminal)
                    .clamp(0.0, (width - popup_width).max(0.0));
                if x >= popup_x && x < popup_x + popup_width {
                    self.terminal_menu_pointer_selection(y, layout.height);
                }
            } else if active == WorkbenchMenu::Run {
                let popup_width = self.run_menu_width(char_width, width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Run)
                    .clamp(0.0, (width - popup_width).max(0.0));
                let submenu_width = self.run_breakpoint_menu_width(char_width, width);
                let submenu_x = self.run_breakpoint_submenu_position(
                    popup_x,
                    popup_width,
                    width,
                    submenu_width,
                );
                let submenu_height = self
                    .run_breakpoint_rows()
                    .iter()
                    .map(|row| {
                        if matches!(row, super::run_menu::RunMenuRow::Separator) {
                            9.0
                        } else {
                            28.0
                        }
                    })
                    .sum::<f32>()
                    + 8.0;
                let submenu_top = self.run_breakpoint_submenu_top(layout.height, height);
                if self.run_breakpoint_submenu
                    && x >= submenu_x
                    && x < submenu_x + submenu_width
                    && y < submenu_top + submenu_height
                {
                    self.run_menu_pointer_selection(y, submenu_top, true);
                } else if x >= popup_x && x < popup_x + popup_width {
                    self.run_menu_pointer_selection(y, layout.height, false);
                }
            } else if active == WorkbenchMenu::Help {
                let popup_width = self.help_menu_width(char_width, width);
                let popup_x = layout
                    .menu_anchor(WorkbenchMenu::Help)
                    .clamp(0.0, (width - popup_width).max(0.0));
                let mut top = layout.height + 4.0;
                let mut item = 0;
                for row in self.help_menu_rows() {
                    match row {
                        super::help_menu::HelpRow::Separator => top += 9.0,
                        super::help_menu::HelpRow::Item { .. } => {
                            if x >= popup_x
                                && x < popup_x + popup_width
                                && y >= top
                                && y < top + 28.0
                            {
                                self.help_menu_selection = item;
                            }
                            top += 28.0;
                            item += 1;
                        }
                    }
                }
            } else if active == WorkbenchMenu::View {
                self.view_menu_pointer_moved(x, y);
            }
            return;
        }
        if let Some(item) = layout
            .menus
            .into_iter()
            .find(|item| item.contains(x, y, layout.height))
        {
            if item.menu == WorkbenchMenu::More {
                if !self.workbench_menu_overflow {
                    self.workbench_menu = Some(WorkbenchMenu::More);
                    self.workbench_menu_overflow = true;
                    self.workbench_overflow_selection = 0;
                    self.activity_bar_menu = None;
                    self.settings_menu = None;
                    self.profile_menu = None;
                }
            } else if item.menu != active || self.workbench_menu_overflow {
                self.workbench_menu = Some(item.menu);
                self.workbench_menu_overflow = false;
                self.activity_bar_menu = None;
                self.settings_menu = None;
                self.profile_menu = None;
                if item.menu == WorkbenchMenu::Run {
                    self.open_run_menu();
                }
                if item.menu == WorkbenchMenu::Terminal {
                    self.terminal_menu_selection = 0;
                }
                if item.menu == WorkbenchMenu::Selection {
                    self.selection_menu_selection = 0;
                }
                if item.menu == WorkbenchMenu::Go {
                    self.go_menu_selection = 0;
                    self.go_submenu = None;
                }
                if item.menu == WorkbenchMenu::View {
                    self.view_menu_open_at(item.x, width, height);
                }
            }
        }
    }

    pub(crate) fn workbench_menu_key(&mut self, key: &Key) -> bool {
        if let Some(active) = self.workbench_menu {
            if active == WorkbenchMenu::View && self.view_menu.is_some() {
                return self.view_menu_key(key);
            }
            if active == WorkbenchMenu::File {
                match key {
                    Key::Named(NamedKey::Escape) => {
                        if !self.close_file_submenu() {
                            self.close_workbench_menu();
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => self.file_menu_move_selection(1),
                    Key::Named(NamedKey::ArrowUp) => self.file_menu_move_selection(-1),
                    Key::Named(NamedKey::ArrowRight) => {
                        if let Some((_, Some(submenu), _)) = self.file_menu_selected() {
                            self.open_file_submenu(submenu);
                        } else if self.file_menu_submenu.is_none() {
                            self.workbench_menu = Some(WorkbenchMenu::Edit);
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if !self.close_file_submenu() {
                            self.workbench_menu = Some(WorkbenchMenu::Help);
                        }
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space)
                        if self.file_menu_nested.is_some() =>
                    {
                        if let Some((command, true)) = self.file_nested_selected() {
                            self.execute_settings_subcommand(command);
                            self.close_workbench_menu();
                        }
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        if let Some((command, submenu, enabled)) = self.file_menu_selected() {
                            if enabled {
                                if let Some(submenu) = submenu {
                                    self.open_file_submenu(submenu);
                                } else {
                                    self.execute_file_menu_command(command);
                                    self.close_workbench_menu();
                                }
                            }
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            // Left/Right fall through to the shared titlebar cycling below.
            if active == WorkbenchMenu::Terminal
                && !matches!(
                    key,
                    Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight)
                )
            {
                match key {
                    Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                    Key::Named(NamedKey::ArrowDown) => self.terminal_menu_move_selection(1),
                    Key::Named(NamedKey::ArrowUp) => self.terminal_menu_move_selection(-1),
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        if let Some((command, true)) = self.terminal_menu_selected() {
                            self.execute_terminal_menu_command(command);
                            self.close_workbench_menu();
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            if active == WorkbenchMenu::Run
                && (self.run_breakpoint_submenu
                    || !matches!(
                        key,
                        Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight)
                    )
                    || (matches!(key, Key::Named(NamedKey::ArrowRight))
                        && self
                            .run_menu_selected(false)
                            .is_some_and(|(_, enabled, submenu)| enabled && submenu)))
            {
                if self.run_breakpoint_submenu {
                    match key {
                        Key::Named(NamedKey::Escape) | Key::Named(NamedKey::ArrowLeft) => {
                            self.run_breakpoint_submenu = false
                        }
                        Key::Named(NamedKey::ArrowDown) => self.run_menu_move_selection(1, true),
                        Key::Named(NamedKey::ArrowUp) => self.run_menu_move_selection(-1, true),
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                            if let Some((command, enabled, _)) = self.run_menu_selected(true) {
                                if enabled {
                                    self.execute_run_menu_command(command);
                                    self.close_workbench_menu();
                                } else {
                                    self.show_run_menu_unavailable_reason(command, true);
                                }
                            }
                        }
                        _ => return false,
                    }
                } else {
                    match key {
                        Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                        Key::Named(NamedKey::ArrowDown) => self.run_menu_move_selection(1, false),
                        Key::Named(NamedKey::ArrowUp) => self.run_menu_move_selection(-1, false),
                        Key::Named(NamedKey::ArrowRight) => {
                            if self
                                .run_menu_selected(false)
                                .is_some_and(|(_, enabled, submenu)| enabled && submenu)
                            {
                                self.run_breakpoint_submenu = true;
                                self.run_breakpoint_selection = 0;
                            } else {
                                return false;
                            }
                        }
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                            if let Some((command, enabled, submenu)) = self.run_menu_selected(false)
                            {
                                if enabled && submenu {
                                    self.run_breakpoint_submenu = true;
                                    self.run_breakpoint_selection = 0;
                                } else if enabled {
                                    self.execute_run_menu_command(command);
                                    self.close_workbench_menu();
                                } else {
                                    self.show_run_menu_unavailable_reason(command, false);
                                }
                            }
                        }
                        _ => return false,
                    }
                }
                return true;
            }
            if active == WorkbenchMenu::Help
                && !matches!(
                    key,
                    Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight)
                )
            {
                let count = self
                    .help_menu_rows()
                    .iter()
                    .filter(|row| matches!(row, super::help_menu::HelpRow::Item { .. }))
                    .count();
                match key {
                    Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                    Key::Named(NamedKey::ArrowDown) => {
                        self.help_menu_selection =
                            (self.help_menu_selection + 1).min(count.saturating_sub(1))
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.help_menu_selection = self.help_menu_selection.saturating_sub(1)
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        self.activate_help_menu_accessibility_item(self.help_menu_selection)
                    }
                    _ => return false,
                }
                return true;
            }
            if active == WorkbenchMenu::Selection {
                match key {
                    Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                    Key::Named(NamedKey::ArrowDown) => self.selection_menu_move_selection(1),
                    Key::Named(NamedKey::ArrowUp) => self.selection_menu_move_selection(-1),
                    Key::Named(NamedKey::ArrowRight) => {
                        self.workbench_menu = Some(WorkbenchMenu::View)
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.workbench_menu = Some(WorkbenchMenu::Edit)
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        if let Some((command, true, _)) = self
                            .selection_menu_items()
                            .get(self.selection_menu_selection)
                            .copied()
                        {
                            self.execute_selection_menu_command(command);
                            self.close_workbench_menu();
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            if active == WorkbenchMenu::Go {
                if let Some(submenu) = self.go_submenu {
                    match key {
                        Key::Named(NamedKey::Escape) | Key::Named(NamedKey::ArrowLeft) => {
                            self.go_submenu = None;
                            self.go_submenu_selection = 0;
                        }
                        Key::Named(NamedKey::ArrowDown) => self.go_submenu_move_selection(1),
                        Key::Named(NamedKey::ArrowUp) => self.go_submenu_move_selection(-1),
                        Key::Named(NamedKey::ArrowRight) => {
                            self.workbench_menu = Some(WorkbenchMenu::Run)
                        }
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                            if let Some((command, true)) = self.go_submenu_selected() {
                                self.go_submenu_execute(command);
                            }
                        }
                        _ => return false,
                    }
                    let _ = submenu;
                } else {
                    match key {
                        Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                        Key::Named(NamedKey::ArrowDown) => self.go_menu_move_selection(1),
                        Key::Named(NamedKey::ArrowUp) => self.go_menu_move_selection(-1),
                        Key::Named(NamedKey::ArrowRight) => {
                            if let Some((_, true, Some(submenu))) = self.go_menu_selected() {
                                self.go_submenu = Some(submenu);
                                self.go_submenu_selection = 0;
                            } else {
                                self.workbench_menu = Some(WorkbenchMenu::Run);
                            }
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.workbench_menu = Some(WorkbenchMenu::Selection)
                        }
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                            if let Some((command, true, submenu)) = self.go_menu_selected() {
                                if let Some(submenu) = submenu {
                                    self.go_submenu = Some(submenu);
                                    self.go_submenu_selection = 0;
                                } else {
                                    self.go_menu_execute(command);
                                    self.close_workbench_menu();
                                }
                            }
                        }
                        _ => return false,
                    }
                }
                return true;
            }
            if active == WorkbenchMenu::Edit {
                match key {
                    Key::Named(NamedKey::Escape) => self.close_workbench_menu(),
                    Key::Named(NamedKey::ArrowDown) => self.edit_menu_move_selection(1),
                    Key::Named(NamedKey::ArrowUp) => self.edit_menu_move_selection(-1),
                    Key::Named(NamedKey::ArrowRight) => {
                        self.workbench_menu = Some(WorkbenchMenu::Selection)
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.workbench_menu = Some(WorkbenchMenu::File)
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        if let Some((command, true)) = self.edit_menu_selected() {
                            self.execute_edit_menu_command(command);
                            self.close_workbench_menu();
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            if active == WorkbenchMenu::More && self.workbench_menu_overflow {
                let layout = self.gpu.as_ref().map(|gpu| {
                    self.titlebar_layout(gpu.renderer.size().0, gpu.renderer.char_width())
                });
                let entries = layout
                    .as_ref()
                    .map_or(&[][..], |layout| layout.overflowed.as_slice());
                if entries.is_empty() {
                    self.close_workbench_menu();
                    return true;
                }
                match key {
                    Key::Named(NamedKey::Escape) | Key::Named(NamedKey::ArrowLeft) => {
                        self.close_workbench_menu();
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.workbench_overflow_selection =
                            (self.workbench_overflow_selection + 1) % entries.len();
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.workbench_overflow_selection =
                            (self.workbench_overflow_selection + entries.len() - 1) % entries.len();
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        let selected =
                            entries[self.workbench_overflow_selection.min(entries.len() - 1)];
                        let (width, height) = self.gpu.as_ref().map_or((1200.0, 760.0), |gpu| {
                            let (width, height) = gpu.renderer.size();
                            (width, height)
                        });
                        let char_width = self
                            .gpu
                            .as_ref()
                            .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width());
                        let anchor = self
                            .titlebar_layout(width, char_width)
                            .menu_anchor(selected);
                        self.activate_menu(selected, anchor, width, height);
                    }
                    _ => return false,
                }
                return true;
            }
            match key {
                Key::Named(NamedKey::Escape) => {
                    self.close_workbench_menu();
                }
                Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight) => {
                    let entries = self.gpu.as_ref().map_or_else(
                        || WorkbenchMenu::ALL.to_vec(),
                        |gpu| {
                            self.titlebar_layout(gpu.renderer.size().0, gpu.renderer.char_width())
                                .menus
                                .iter()
                                .map(|item| item.menu)
                                .collect()
                        },
                    );
                    let index = entries.iter().position(|menu| *menu == active).unwrap_or(0);
                    let next = if matches!(key, Key::Named(NamedKey::ArrowLeft)) {
                        (index + entries.len() - 1) % entries.len()
                    } else {
                        (index + 1) % entries.len()
                    };
                    self.workbench_menu = Some(entries[next]);
                    self.workbench_menu_overflow = false;
                    self.activity_bar_menu = None;
                    self.settings_menu = None;
                    self.profile_menu = None;
                    let selected = entries[next];
                    if selected == WorkbenchMenu::Selection {
                        self.selection_menu_selection = 0;
                    }
                    if selected == WorkbenchMenu::Terminal {
                        self.terminal_menu_selection = 0;
                    }
                    if selected == WorkbenchMenu::View {
                        let (width, height, char_width) = self.gpu.as_ref().map_or(
                            (1200.0, 760.0, self.font_size * 0.6),
                            |gpu| {
                                let (width, height) = gpu.renderer.size();
                                (width, height, gpu.renderer.char_width())
                            },
                        );
                        let anchor = self
                            .titlebar_layout(width, char_width)
                            .menu_anchor(selected);
                        self.activate_menu(selected, anchor, width, height);
                    }
                }
                Key::Named(NamedKey::Enter) | Key::Named(NamedKey::ArrowDown) => {
                    if active == WorkbenchMenu::More {
                        self.workbench_menu_overflow = true;
                        self.workbench_overflow_selection = 0;
                    } else if active == WorkbenchMenu::View {
                        let (width, height, char_width) = self.gpu.as_ref().map_or(
                            (1200.0, 760.0, self.font_size * 0.6),
                            |gpu| {
                                let (width, height) = gpu.renderer.size();
                                (width, height, gpu.renderer.char_width())
                            },
                        );
                        let anchor = self.titlebar_layout(width, char_width).menu_anchor(active);
                        self.activate_menu(active, anchor, width, height);
                    } else if active == WorkbenchMenu::File {
                        self.file_menu_selection = 0;
                    } else if active == WorkbenchMenu::Edit {
                        self.edit_menu_selection = 0;
                    }
                }
                _ => return false,
            }
            return true;
        }
        if matches!(key, Key::Named(NamedKey::F10)) {
            self.workbench_menu = Some(WorkbenchMenu::File);
            self.workbench_menu_overflow = false;
            self.file_menu_submenu = None;
            self.file_menu_nested = None;
            self.file_menu_selection = 0;
            return true;
        }
        false
    }

    pub(crate) fn draw_workbench_menu(&self, renderer: &mut Renderer) {
        if self.zen_mode {
            return;
        }
        let (width, _) = renderer.size();
        let line_height = renderer.line_height();
        let char_width = renderer.char_width();
        let layout = self.titlebar_layout(width, char_width);
        let menu_height = layout.height;
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let hovered = rgb(Theme::color(Theme::ACTIVE_ROW, 0x3f3f46));
        renderer.draw_rect(0.0, 0.0, width, menu_height, background);
        renderer.draw_rect(0.0, menu_height - 1.0, width, 1.0, border);
        draw_brand_icon(renderer, layout.icon);

        let active_is_overflowed = self.workbench_menu.is_some_and(|menu| {
            menu != WorkbenchMenu::More
                && layout.overflowed.contains(&menu)
                && !self.workbench_menu_overflow
        });
        for item in layout
            .menus
            .iter()
            .filter(|_| self.view_preferences.menu_bar_visible)
        {
            let selected = self.workbench_menu == Some(item.menu)
                || (item.menu == WorkbenchMenu::More && active_is_overflowed);
            let item_hovered = item.contains(self.mouse.x as f32, self.mouse.y as f32, menu_height);
            if selected || item_hovered {
                renderer.draw_rect(
                    item.x,
                    1.0,
                    item.width,
                    menu_height - 2.0,
                    if item_hovered {
                        hovered
                    } else {
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d))
                    },
                );
            }
            renderer.draw_text(
                self.t(item.menu.label_key()),
                item.x + MENU_HORIZONTAL_PADDING * self.sidebar_scale_factor,
                (menu_height - line_height) * 0.5,
                text,
            );
        }

        if self.workbench_menu == Some(WorkbenchMenu::More) && self.workbench_menu_overflow {
            let popup = layout.overflow_popup(char_width, width);
            renderer.draw_rect(popup.x, popup.y, popup.width, popup.height, border);
            renderer.draw_rect(
                popup.x + 1.0,
                popup.y + 1.0,
                popup.width - 2.0,
                popup.height - 2.0,
                background,
            );
            for (index, menu) in layout.overflowed.iter().copied().enumerate() {
                let y = popup.y + 4.0 + index as f32 * OVERFLOW_ROW_HEIGHT;
                if index == self.workbench_overflow_selection {
                    renderer.draw_rect(
                        popup.x + 2.0,
                        y,
                        popup.width - 4.0,
                        OVERFLOW_ROW_HEIGHT,
                        hovered,
                    );
                }
                renderer.draw_text(self.t(menu.label_key()), popup.x + 14.0, y + 5.0, text);
            }
        }

        if let Some(active) = self.workbench_menu {
            if active == WorkbenchMenu::File {
                self.draw_file_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Edit {
                self.draw_edit_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Selection {
                self.draw_selection_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Go {
                self.draw_go_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Terminal {
                self.draw_terminal_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Run {
                self.draw_run_menu(renderer, &layout);
            } else if active == WorkbenchMenu::Help {
                self.draw_help_menu(renderer, &layout);
            } else if active != WorkbenchMenu::View
                && active != WorkbenchMenu::Go
                && active != WorkbenchMenu::More
                && !self.workbench_menu_overflow
            {
                let popup_width = (self.t("menubar.empty").chars().count() as f32 * char_width
                    + MENU_HORIZONTAL_PADDING * 2.0)
                    .max(EMPTY_MENU_WIDTH)
                    .min((width - 2.0).max(80.0));
                let popup_x = layout
                    .menu_anchor(active)
                    .clamp(0.0, (width - popup_width).max(0.0));
                renderer.draw_rect(popup_x, menu_height, popup_width, EMPTY_MENU_HEIGHT, border);
                renderer.draw_rect(
                    popup_x + 1.0,
                    menu_height + 1.0,
                    popup_width - 2.0,
                    EMPTY_MENU_HEIGHT - 2.0,
                    background,
                );
                renderer.draw_text(
                    self.t("menubar.empty"),
                    popup_x + MENU_HORIZONTAL_PADDING,
                    menu_height + (EMPTY_MENU_HEIGHT - line_height) * 0.5,
                    dim,
                );
            }
        }
        if self.workbench_menu == Some(WorkbenchMenu::View) {
            self.draw_view_menu(renderer);
        }

        self.draw_run_breakpoint_prompt(renderer);
        self.draw_rename_prompt(renderer);
        self.draw_branch_name_prompt(renderer);
        self.draw_explorer_prompt(renderer);
        self.draw_explorer_delete_confirm(renderer);
        self.draw_scm_confirm(renderer);
        self.draw_scm_publish_modal(renderer);
        self.draw_github_dialog(renderer);
        self.draw_command_center_controls(renderer, &layout);
        self.draw_titlebar_controls(renderer, layout.controls);
        self.draw_layout_controls(renderer, &layout.layout_controls, layout.height);
        self.draw_titlebar_update_button(renderer, &layout);
    }

    pub(crate) fn titlebar_update_button(&self) -> Option<TitlebarUpdateButton> {
        use crate::app::updates::state::UpdateStatus;
        match &self.updates.status {
            UpdateStatus::Available(release) => Some(TitlebarUpdateButton {
                label: self.tf("titlebar.update.available", &[("tag", &release.tag_name)]),
                tooltip: self.tf(
                    "titlebar.update.tip_available",
                    &[("tag", &release.tag_name)],
                ),
                bg_hex: 0x007acc,
                hover_hex: 0x1a8ad4,
                action: TitlebarUpdateAction::Download,
            }),
            UpdateStatus::Downloading { .. } => Some(TitlebarUpdateButton {
                label: self.t("titlebar.update.downloading").to_owned(),
                tooltip: self.t("titlebar.update.downloading").to_owned(),
                bg_hex: 0x3a3d41,
                hover_hex: 0x3a3d41,
                action: TitlebarUpdateAction::None,
            }),
            UpdateStatus::ReadyToRestart { release, .. } => Some(TitlebarUpdateButton {
                label: self.tf("titlebar.update.restart", &[("tag", &release.tag_name)]),
                tooltip: self.tf("titlebar.update.tip_restart", &[("tag", &release.tag_name)]),
                bg_hex: 0x2f855a,
                hover_hex: 0x38a169,
                action: TitlebarUpdateAction::Restart,
            }),
            _ => None,
        }
    }

    pub(crate) fn click_titlebar_update_button(&mut self) {
        let Some(button) = self.titlebar_update_button() else {
            return;
        };
        match button.action {
            TitlebarUpdateAction::Download => self.download_update(),
            TitlebarUpdateAction::Restart => self.restart_to_update(),
            TitlebarUpdateAction::None => {}
        }
    }

    fn draw_titlebar_update_button(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
        let Some(rect) = layout.update_button else {
            return;
        };
        let Some(button) = self.titlebar_update_button() else {
            return;
        };
        let (mx, my) = (self.mouse.x as f32, self.mouse.y as f32);
        let hovered = rect.contains(mx, my);
        let bg = rgb(if hovered {
            button.hover_hex
        } else {
            button.bg_hex
        });
        renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, bg);
        let cw = renderer.char_width().max(4.0);
        let lh = renderer.line_height();
        let text_w = button.label.chars().count() as f32 * cw;
        let text_x = rect.x + ((rect.width - text_w) * 0.5).max(4.0);
        let text_y = rect.y + ((rect.height - lh) * 0.5).max(0.0);
        renderer.draw_text(&button.label, text_x, text_y, rgb(0xffffff));

        if hovered {
            let win_w = renderer.size().0;
            let tip_w = button.tooltip.chars().count() as f32 * cw + 16.0;
            let tip_h = lh + 10.0;
            let tip_x = rect.x.clamp(4.0, (win_w - tip_w - 4.0).max(4.0));
            let tip_y = layout.height + 4.0;
            renderer.draw_rect(
                tip_x,
                tip_y,
                tip_w,
                tip_h,
                rgb(Theme::color(Theme::DIM, 0x454545)),
            );
            renderer.draw_rect(
                tip_x + 1.0,
                tip_y + 1.0,
                tip_w - 2.0,
                tip_h - 2.0,
                rgb(Theme::color(Theme::SIDEBAR, 0x252526)),
            );
            renderer.draw_text(
                &button.tooltip,
                tip_x + 8.0,
                tip_y + 5.0,
                rgb(Theme::color(Theme::TEXT, 0xcccccc)),
            );
        }
    }

    fn draw_command_center_controls(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
        let center = layout.command_center;
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let hover = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        for (rect, enabled, glyph) in [
            (center.back, self.can_navigate_history(-1), "←"),
            (center.forward, self.can_navigate_history(1), "→"),
        ] {
            if rect.contains(self.mouse.x as f32, self.mouse.y as f32) {
                renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, hover);
            }
            let color = if enabled { text } else { dim };
            renderer.draw_text(
                glyph,
                rect.x + (rect.width - renderer.char_width()) * 0.5,
                rect.y + (rect.height - renderer.line_height()) * 0.5,
                color,
            );
        }
        let active = self.command_center.is_some();
        let focused = center
            .field
            .contains(self.mouse.x as f32, self.mouse.y as f32);
        let border = if active {
            rgb(0x007acc)
        } else if focused {
            rgb(Theme::color(Theme::DIM, 0x777777))
        } else {
            rgb(Theme::color(Theme::DIM, 0x454545))
        };
        renderer.draw_rect(
            center.field.x,
            center.field.y,
            center.field.width,
            center.field.height,
            border,
        );
        renderer.draw_rect(
            center.field.x + 1.0,
            center.field.y + 1.0,
            (center.field.width - 2.0).max(0.0),
            (center.field.height - 2.0).max(0.0),
            rgb(Theme::color(Theme::SIDEBAR, 0x252526)),
        );
        let value = self
            .command_center
            .as_ref()
            .map(|center| center.query.as_str())
            .filter(|query| !query.is_empty())
            .unwrap_or_else(|| self.t("command_center.placeholder"));
        let value_width = (center.field.width - 18.0).max(0.0);
        let visible = value
            .chars()
            .rev()
            .take((value_width / renderer.char_width().max(1.0)) as usize)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        renderer.draw_text(
            &visible,
            center.field.x + 9.0,
            center.field.y + (center.field.height - renderer.line_height()) * 0.5,
            if self.command_center.is_some() || value != self.t("command_center.placeholder") {
                text
            } else {
                dim
            },
        );
    }

    fn draw_run_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
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

    fn draw_run_breakpoint_prompt(&self, renderer: &mut Renderer) {
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

    fn draw_help_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
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

    fn draw_file_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
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
    fn draw_file_panel(
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
    fn draw_file_nested_panel(
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

    fn draw_terminal_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
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

    fn draw_edit_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
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

    fn draw_selection_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
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

    fn draw_go_menu(&self, renderer: &mut Renderer, layout: &TitlebarLayout) {
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

    fn draw_titlebar_controls(&self, renderer: &mut Renderer, controls: TitlebarControlRects) {
        let (x, y) = (self.mouse.x as f32, self.mouse.y as f32);
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let hovered = rgb(Theme::color(Theme::ACTIVE_ROW, 0x3f3f46));
        let close_hovered = rgb(0xc42b1c);
        let foreground = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        for (rect, close) in [
            (controls.minimize, false),
            (controls.maximize, false),
            (controls.close, true),
        ] {
            let is_hovered = rect.contains(x, y);
            if is_hovered {
                renderer.draw_rect(
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    if close { close_hovered } else { hovered },
                );
            } else {
                renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, background);
            }
        }
        let cy = controls.minimize.height * 0.5;
        let min_cx = controls.minimize.x + controls.minimize.width * 0.5;
        renderer.draw_rect(min_cx - 5.5, cy + 4.0, 11.0, 1.2, foreground);
        let max_cx = controls.maximize.x + controls.maximize.width * 0.5;
        if self
            .gpu
            .as_ref()
            .is_some_and(|gpu| gpu.window.is_maximized())
        {
            renderer.draw_rect(max_cx - 4.0, cy - 4.0, 8.0, 7.0, foreground);
            renderer.draw_rect(max_cx - 1.0, cy - 1.0, 7.0, 7.0, background);
            renderer.draw_rect(max_cx - 1.0, cy - 1.0, 7.0, 1.0, foreground);
            renderer.draw_rect(max_cx + 5.0, cy - 1.0, 1.0, 7.0, foreground);
            renderer.draw_rect(max_cx - 1.0, cy + 5.0, 7.0, 1.0, foreground);
        } else {
            draw_outline_rect(renderer, max_cx - 5.0, cy - 5.0, 10.0, 10.0, foreground);
        }
        let close = controls.close;
        renderer.draw_text(
            "×",
            close.x + close.width * 0.5 - char_width_center(renderer, '×'),
            cy - renderer.line_height() * 0.5,
            foreground,
        );
    }
}

fn draw_outline_rect(
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color,
) {
    renderer.draw_rect(x, y, width, 1.0, color);
    renderer.draw_rect(x, y + height - 1.0, width, 1.0, color);
    renderer.draw_rect(x, y, 1.0, height, color);
    renderer.draw_rect(x + width - 1.0, y, 1.0, height, color);
}

fn char_width_center(renderer: &Renderer, character: char) -> f32 {
    let _ = character;
    renderer.char_width() * 0.5
}

fn fit_file_menu_label(label: String, max_chars: usize) -> String {
    let chars = label.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        label
    } else if max_chars <= 1 {
        "…".into()
    } else {
        chars[..max_chars - 1].iter().collect::<String>() + "…"
    }
}

fn draw_brand_icon(renderer: &mut Renderer, rect: MenuItemRectLike) {
    const ICON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/graphite-icon.rgba"));
    const SOURCE_SIZE: usize = 256;
    const GRID: usize = 16;
    let cell = rect.width / GRID as f32;
    for gy in 0..GRID {
        for gx in 0..GRID {
            let sx = gx * SOURCE_SIZE / GRID + SOURCE_SIZE / (GRID * 2);
            let sy = gy * SOURCE_SIZE / GRID + SOURCE_SIZE / (GRID * 2);
            let offset = (sy * SOURCE_SIZE + sx) * 4;
            let alpha = ICON[offset + 3] as f32 / 255.0;
            if alpha <= 0.05 {
                continue;
            }
            let color = [
                ICON[offset] as f32 / 255.0,
                ICON[offset + 1] as f32 / 255.0,
                ICON[offset + 2] as f32 / 255.0,
                alpha,
            ];
            renderer.draw_rect(
                rect.x + gx as f32 * cell,
                rect.y + gy as f32 * cell,
                cell + 0.05,
                cell + 0.05,
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(width: f32, scale: f32) -> TitlebarLayout {
        TitlebarLayout::new(
            width,
            32.0 * scale,
            8.0 * scale,
            scale,
            [72.0, 64.0, 96.0, 56.0, 32.0, 64.0, 72.0, 56.0].map(|width| width * scale),
            0.0,
        )
    }

    #[test]
    fn titlebar_geometry_keeps_icon_menus_and_controls_separate_at_common_scales() {
        for (width, scale) in [(1280.0, 1.0), (1920.0, 1.5), (900.0, 1.25)] {
            let layout = layout(width, scale);
            assert!(layout.icon.x + layout.icon.width <= layout.menus[0].x);
            for pair in layout.menus.windows(2) {
                assert!(pair[0].x + pair[0].width <= pair[1].x);
            }
            let last = layout.menus.last().unwrap();
            assert!(last.x + last.width <= layout.command_center.back.x);
            assert!(
                layout.command_center.back.x + layout.command_center.back.width
                    <= layout.command_center.forward.x
            );
            assert!(
                layout.command_center.forward.x + layout.command_center.forward.width
                    <= layout.command_center.field.x
            );
            assert_eq!(
                layout.command_center.field.x + layout.command_center.field.width * 0.5,
                width * 0.5
            );
            assert!(
                layout.command_center.field.x + layout.command_center.field.width
                    <= layout.controls.minimize.x
            );
            assert!(
                layout.controls.minimize.x + layout.controls.minimize.width
                    <= layout.controls.maximize.x
            );
            assert!(
                layout.controls.maximize.x + layout.controls.maximize.width
                    <= layout.controls.close.x
            );
            assert!(layout.controls.close.x + layout.controls.close.width <= width);
        }
    }

    #[test]
    fn narrow_titlebar_moves_remaining_menus_into_a_bounded_overflow_menu() {
        let layout_600 = layout(600.0, 1.0);
        let more = layout_600
            .more_rect()
            .expect("narrow layout should expose More");
        assert!(!layout_600.overflowed.is_empty());
        assert_eq!(
            layout_600.overflowed.len() + layout_600.menus.len() - 1,
            WorkbenchMenu::ALL.len()
        );
        assert!(more.x + more.width <= layout_600.command_center.back.x);
        let popup = layout_600.overflow_popup(8.0, 600.0);
        assert!(popup.x >= 0.0);
        assert!(popup.x + popup.width <= 600.0);
        for width in [320.0, 280.0] {
            let layout = layout(width, 1.0);
            if let Some(more) = layout.more_rect() {
                assert!(more.x + more.width <= layout.command_center.back.x);
            }
            assert!(
                layout.command_center.back.x + layout.command_center.back.width
                    <= layout.command_center.forward.x
            );
            assert!(
                layout.command_center.forward.x + layout.command_center.forward.width
                    <= layout.command_center.field.x
            );
            assert!(
                layout.command_center.field.x + layout.command_center.field.width
                    <= layout.controls.minimize.x
            );
        }
    }

    #[test]
    fn titlebar_control_hit_regions_are_distinct_and_resize_edges_are_shared() {
        let layout = layout(1000.0, 1.0);
        let controls = layout.controls;
        assert_eq!(
            controls.at(controls.minimize.x + 2.0, 10.0),
            Some(WindowControl::Minimize)
        );
        assert_eq!(
            controls.at(controls.maximize.x + 2.0, 10.0),
            Some(WindowControl::MaximizeRestore)
        );
        assert_eq!(
            controls.at(controls.close.x + 2.0, 10.0),
            Some(WindowControl::Close)
        );
        assert_eq!(
            controls.at(controls.close.x, 10.0),
            Some(WindowControl::Close)
        );
        assert_eq!(
            resize_direction_at(1000.0, 700.0, 6.0, 3.0, 3.0, false),
            Some(winit::window::ResizeDirection::NorthWest)
        );
        assert_eq!(
            resize_direction_at(1000.0, 700.0, 6.0, 3.0, 3.0, true),
            None
        );
        assert_eq!(
            resize_direction_for_titlebar(&layout, 1000.0, 700.0, 6.0, 45.0, 2.0, false),
            None,
            "the top border must not block menu clicks"
        );
        assert_eq!(
            resize_direction_for_titlebar(&layout, 1000.0, 700.0, 6.0, 999.0, 2.0, false),
            Some(winit::window::ResizeDirection::NorthEast),
            "the window corner remains resizable"
        );
    }

    #[test]
    fn menu_bar_contains_all_eight_top_level_menus_and_height_tracks_font_scale() {
        assert_eq!(WorkbenchMenu::ALL.len(), 8);
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.sidebar_scale_factor = 2.0;
        assert_eq!(app.top_menu_height(), app.font_size * 4.2);
    }

    #[test]
    fn keyboard_opens_and_switches_top_level_menus() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        assert!(app.workbench_menu_key(&Key::Named(NamedKey::F10)));
        assert_eq!(app.workbench_menu, Some(WorkbenchMenu::File));
        assert!(app.workbench_menu_key(&Key::Named(NamedKey::ArrowRight)));
        assert_eq!(app.workbench_menu, Some(WorkbenchMenu::Edit));
        assert!(app.workbench_menu_key(&Key::Named(NamedKey::Escape)));
        assert!(app.workbench_menu.is_none());
    }

    #[test]
    fn view_menu_is_separate_from_the_activity_context_menu_and_has_appearance_controls() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.activity_bar_preferences.position = crate::config::ActivityBarPosition::Hidden;
        app.workbench_menu = Some(WorkbenchMenu::View);
        app.view_menu_open(0.0, 800.0, 600.0);
        assert!(app.activity_bar_menu.is_none());
        assert_eq!(
            app.view_menu_open_page(),
            Some(super::super::view_menu::ViewPage::Root)
        );
        let (x, y) = app.view_menu_row_point(0, 3).unwrap();
        app.view_menu_click(x, y);
        assert_eq!(
            app.view_menu_open_page(),
            Some(super::super::view_menu::ViewPage::Appearance)
        );
        let (x, y) = app.view_menu_row_point(1, 11).unwrap();
        app.view_menu_click(x, y);
        assert_eq!(
            app.view_menu_open_page(),
            Some(super::super::view_menu::ViewPage::ActivityPosition)
        );
        let (x, y) = app.view_menu_row_point(2, 0).unwrap();
        app.view_menu_click(x, y);
        assert_eq!(
            app.activity_bar_preferences.position,
            crate::config::ActivityBarPosition::Default
        );
    }

    #[test]
    fn titlebar_update_button_appears_only_when_update_exists_and_stays_left_of_layout_controls() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let idle = app.titlebar_layout(1280.0, 8.0);
        assert!(idle.update_button.is_none());

        app.updates.status = crate::app::updates::state::UpdateStatus::Available(
            crate::app::updates::state::ReleaseInfo {
                tag_name: "v0.2.0".into(),
                version: "0.2.0".into(),
                prerelease: false,
                notes: String::new(),
                html_url: "https://github.com/BadTonho/Graphite/releases/tag/v0.2.0".into(),
                asset: None,
            },
        );
        let with_update = app.titlebar_layout(1280.0, 8.0);
        let btn = with_update.update_button.expect("update button visible");
        assert!(btn.x + btn.width <= with_update.layout_controls[0].1 .0);
        assert!(
            with_update.command_center.field.x + with_update.command_center.field.width <= btn.x
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TitlebarUpdateAction {
    Download,
    Restart,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TitlebarUpdateButton {
    pub label: String,
    pub tooltip: String,
    pub bg_hex: u32,
    pub hover_hex: u32,
    pub action: TitlebarUpdateAction,
}
