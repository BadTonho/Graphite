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
    mode_button: MenuItemRectLike,
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
        let gap = 4.0 * scale;
        let button_right = (controls.minimize.x - gap).max(0.0);
        let button_width = (6.0 * char_width + 16.0 * scale).min(button_right);
        let mode_button = MenuItemRectLike {
            x: button_right - button_width,
            y: 0.0,
            width: button_width,
            height,
        };
        let layout_controls = super::titlebar_layout_controls::layout_control_rects(
            mode_button.x - gap,
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
            mode_button,
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
            || layout.mode_button.contains(x, y)
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

mod drawing;
mod input;
mod panels;
#[cfg(test)]
mod tests;

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
