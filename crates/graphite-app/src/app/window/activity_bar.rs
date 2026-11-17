//! Activity bar on the far left of the workbench.
//!
//! The bar owns a fixed-width column; the Explorer, Source Control and Plugins
//! panels start after it. Geometry is intentionally independent of the font
//! metrics so hit testing and drawing share the same slots.

use super::*;
use crate::config::{ActivityBarPosition, ActivityBarSize};

pub(crate) const ACTIVITY_BAR_ITEM_HEIGHT: f32 = 48.0;
const ACTIVITY_BAR_TOP: f32 = 8.0;
const ACTIVITY_BAR_ICON_SIZE: f32 = 24.0;
const MENU_ROW: f32 = 26.0;
const MENU_WIDTH: f32 = 286.0;
const MENU_PADDING: f32 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivityBarSubmenu {
    Position,
    Size,
}

#[derive(Clone, Debug)]
pub(crate) struct ActivityBarMenu {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub selected: usize,
    pub submenu: Option<ActivityBarSubmenu>,
    pub submenu_selected: usize,
    pub extension_items: Vec<PluginActivityEntry>,
    pub show_restore: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PluginActivityEntry {
    pub key: String,
    pub plugin_id: String,
    pub container_id: String,
    pub title: String,
    pub views: Vec<graphite_plugins::manifest::ActivityBarView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum VisibleActivityItem {
    Builtin(Activity),
    Plugin(PluginActivityEntry),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MenuRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl MenuRect {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActivityBarLayout {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    item_size: f32,
    compact: bool,
    position: ActivityBarPosition,
}

impl ActivityBarLayout {
    fn new(
        preferences: &config::ActivityBarPreferences,
        width: f32,
        height: f32,
        menu_bar_height: f32,
    ) -> Option<Self> {
        if preferences.position == ActivityBarPosition::Hidden {
            return None;
        }
        let compact = preferences.size == ActivityBarSize::Compact;
        let item_size = if compact {
            40.0
        } else {
            ACTIVITY_BAR_ITEM_HEIGHT
        };
        let horizontal = matches!(
            preferences.position,
            ActivityBarPosition::Top | ActivityBarPosition::Bottom
        );
        Some(Self {
            x: 0.0,
            y: if preferences.position == ActivityBarPosition::Bottom {
                height - item_size - 20.0
            } else {
                menu_bar_height
            },
            width: if horizontal {
                width
            } else if compact {
                36.0
            } else {
                ACTIVITY_BAR_WIDTH
            },
            height: if horizontal {
                item_size
            } else {
                height - menu_bar_height
            },
            item_size,
            compact,
            position: preferences.position,
        })
    }

    fn item_rect(self, index: usize) -> MenuRect {
        match self.position {
            ActivityBarPosition::Top | ActivityBarPosition::Bottom => MenuRect {
                x: 4.0 + index as f32 * self.item_size,
                y: self.y,
                width: self.item_size,
                height: self.item_size,
            },
            _ => MenuRect {
                x: 0.0,
                y: self.y + ACTIVITY_BAR_TOP + index as f32 * self.item_size,
                width: self.width,
                height: self.item_size,
            },
        }
    }

    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

pub(crate) fn menu_rows(extension_count: usize, show_restore: bool) -> Vec<f32> {
    let top = MENU_PADDING;
    let setting_start = 4 + extension_count;
    let mut rows = (0..setting_start)
        .map(|index| top + MENU_ROW * index as f32)
        .collect::<Vec<_>>();
    rows.extend((0..3).map(|index| top + MENU_ROW * (setting_start + index) as f32 + 10.0));
    if show_restore {
        rows.push(top + MENU_ROW * (setting_start + 3) as f32 + 10.0);
    }
    rows
}

fn menu_items_height(extension_count: usize, show_restore: bool) -> f32 {
    MENU_PADDING * 2.0 + MENU_ROW * (7 + extension_count + usize::from(show_restore)) as f32 + 10.0
}

fn main_menu_rect(menu: &ActivityBarMenu) -> MenuRect {
    MenuRect {
        x: menu.x,
        y: menu.y,
        width: menu.width,
        height: menu_items_height(menu.extension_items.len(), menu.show_restore),
    }
}

fn submenu_rect(
    menu: &ActivityBarMenu,
    window_width: f32,
    window_height: f32,
    row_top: f32,
    rows: usize,
) -> MenuRect {
    let x = if menu.x + menu.width + 170.0 <= window_width {
        menu.x + menu.width - 1.0
    } else {
        menu.x - 170.0 + 1.0
    };
    MenuRect {
        x: x.max(0.0),
        y: (menu.y + row_top).clamp(
            0.0,
            (window_height - (rows as f32 * MENU_ROW + MENU_PADDING * 2.0)).max(0.0),
        ),
        width: 170.0,
        height: rows as f32 * MENU_ROW + MENU_PADDING * 2.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Activity {
    Explorer,
    Search,
    SourceControl,
    Plugins,
}

impl Activity {
    pub(crate) const ALL: [Activity; 4] = [
        Activity::Explorer,
        Activity::Search,
        Activity::SourceControl,
        Activity::Plugins,
    ];

    pub(crate) fn glyph(self) -> char {
        match self {
            Activity::Explorer => '\u{eaf0}',      // cod-files
            Activity::Search => '\u{ea6d}',        // cod-search
            Activity::SourceControl => '\u{ea68}', // cod-source_control
            Activity::Plugins => '\u{eae6}',       // cod-extensions
        }
    }

    pub(crate) fn label_key(self) -> &'static str {
        match self {
            Activity::Explorer => "activity.explorer",
            Activity::Search => "activity.search",
            Activity::SourceControl => "activity.source_control",
            Activity::Plugins => "activity.plugins",
        }
    }

    fn preference_id(self) -> &'static str {
        match self {
            Activity::Explorer => "explorer",
            Activity::Search => "search",
            Activity::SourceControl => "source_control",
            Activity::Plugins => "plugins",
        }
    }
}

/// Top edge of the item at `index`, in physical pixels.
pub(crate) fn activity_top(index: usize) -> f32 {
    ACTIVITY_BAR_TOP + index as f32 * ACTIVITY_BAR_ITEM_HEIGHT
}

/// Returns the item under `y`, or `None` outside every slot.
pub(crate) fn activity_at(y: f32) -> Option<Activity> {
    if y < ACTIVITY_BAR_TOP {
        return None;
    }
    let index = ((y - ACTIVITY_BAR_TOP) / ACTIVITY_BAR_ITEM_HEIGHT) as usize;
    Activity::ALL.get(index).copied()
}

impl App {
    pub(crate) fn plugin_activity_entries(&self) -> Vec<PluginActivityEntry> {
        self.plugins
            .installed_plugins()
            .iter()
            .filter(|record| {
                record.enabled
                    && record.trusted_version.as_deref() == Some(record.metadata.version())
            })
            .flat_map(|record| {
                let plugin_id = record.metadata.id().to_owned();
                let containers = match &record.metadata {
                    graphite_plugins::manifest::Metadata::Native(manifest) => {
                        &manifest.activity_bar
                    }
                    graphite_plugins::manifest::Metadata::Vsix(vsix) => &vsix.activity_bar,
                };
                containers
                    .iter()
                    .map(move |container| PluginActivityEntry {
                        key: format!("plugin:{plugin_id}:{}", container.id),
                        plugin_id: plugin_id.clone(),
                        container_id: container.id.clone(),
                        title: container.title.clone(),
                        views: container.views.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn visible_activity_items(&self) -> Vec<VisibleActivityItem> {
        let mut items = Activity::ALL
            .into_iter()
            .filter(|activity| {
                !self
                    .activity_bar_preferences
                    .hidden_items
                    .contains(activity.preference_id())
            })
            .map(VisibleActivityItem::Builtin)
            .collect::<Vec<_>>();
        items.extend(
            self.plugin_activity_entries()
                .into_iter()
                .filter(|entry| {
                    !self
                        .activity_bar_preferences
                        .hidden_items
                        .contains(&entry.key)
                })
                .map(VisibleActivityItem::Plugin),
        );
        items
    }

    pub(crate) fn open_activity_bar_menu(
        &mut self,
        x: f32,
        y: f32,
        window_width: f32,
        window_height: f32,
    ) {
        self.open_activity_bar_menu_at(x, y, window_width, window_height);
    }

    pub(crate) fn open_activity_bar_menu_at(
        &mut self,
        x: f32,
        y: f32,
        window_width: f32,
        window_height: f32,
    ) {
        let extension_items = self.plugin_activity_entries();
        let show_restore = self.activity_bar_preferences.position == ActivityBarPosition::Hidden;
        let menu_height = menu_items_height(extension_items.len(), show_restore);
        let char_width = self
            .gpu
            .as_ref()
            .map_or(self.font_size * 0.6, |gpu| gpu.renderer.char_width());
        let longest_label = [
            self.t("activity.position"),
            self.t("activity.size"),
            self.t("activity.move_sidebar_right"),
            self.t("activity.position.default"),
            self.t("activity.position.bottom"),
            self.t("activity.position.hidden"),
            self.t("activity.position.top"),
            self.t("activity.size.compact"),
            self.t("activity.size.default"),
            self.t("activity.show"),
        ]
        .into_iter()
        .chain(extension_items.iter().map(|entry| entry.title.as_str()))
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0);
        let menu_width = MENU_WIDTH.max(longest_label as f32 * char_width + 58.0);
        let x = x.clamp(0.0, (window_width - menu_width).max(0.0));
        let y = y.clamp(0.0, (window_height - menu_height).max(0.0));
        self.activity_bar_menu = Some(ActivityBarMenu {
            x,
            y,
            width: menu_width,
            selected: 0,
            submenu: None,
            submenu_selected: 0,
            extension_items,
            show_restore,
        });
    }

    pub(crate) fn handle_activity_bar_menu_click(
        &mut self,
        x: f32,
        y: f32,
        window_width: f32,
    ) -> bool {
        let Some(menu) = self.activity_bar_menu.clone() else {
            return false;
        };
        if let Some(submenu) = menu.submenu {
            let submenu_row_count = match submenu {
                ActivityBarSubmenu::Position => 4,
                ActivityBarSubmenu::Size => 2,
            };
            let row_tops = menu_rows(menu.extension_items.len(), menu.show_restore);
            let base = 4 + menu.extension_items.len();
            let row_top = row_tops[base
                + match submenu {
                    ActivityBarSubmenu::Position => 0,
                    ActivityBarSubmenu::Size => 1,
                }];
            let window_height = self
                .gpu
                .as_ref()
                .map_or(f32::MAX, |gpu| gpu.renderer.size().1);
            let rect = submenu_rect(
                &menu,
                window_width,
                window_height,
                row_top,
                submenu_row_count,
            );
            if rect.contains(x, y)
                && y >= rect.y + MENU_PADDING
                && y < rect.y + MENU_PADDING + submenu_row_count as f32 * MENU_ROW
            {
                let index = ((y - rect.y - MENU_PADDING) / MENU_ROW) as usize;
                match submenu {
                    ActivityBarSubmenu::Position => self.set_activity_bar_position(match index {
                        0 => ActivityBarPosition::Default,
                        1 => ActivityBarPosition::Top,
                        2 => ActivityBarPosition::Bottom,
                        _ => ActivityBarPosition::Hidden,
                    }),
                    ActivityBarSubmenu::Size => self.set_activity_bar_size(if index == 0 {
                        ActivityBarSize::Default
                    } else {
                        ActivityBarSize::Compact
                    }),
                }
                self.activity_bar_menu = None;
                return true;
            }
        }
        if !main_menu_rect(&menu).contains(x, y) {
            self.activity_bar_menu = None;
            return true;
        }
        let local_y = y - menu.y;
        let rows = menu_rows(menu.extension_items.len(), menu.show_restore);
        let selected = rows.iter().enumerate().find_map(|(index, top)| {
            (local_y >= *top && local_y < *top + MENU_ROW).then_some(index)
        });
        let Some(selected) = selected else {
            return true;
        };
        if let Some(menu) = &mut self.activity_bar_menu {
            menu.selected = selected;
        }
        let extension_count = menu.extension_items.len();
        match selected {
            0..=3 => {
                self.toggle_activity_visibility(Activity::ALL[selected]);
                self.activity_bar_menu = None;
            }
            index if index < 4 + extension_count => {
                let entry = menu.extension_items[index - 4].clone();
                self.toggle_extension_activity_visibility(&entry);
                self.activity_bar_menu = None;
            }
            index if index == 4 + extension_count => {
                if let Some(menu) = &mut self.activity_bar_menu {
                    menu.submenu = Some(ActivityBarSubmenu::Position);
                }
            }
            index if index == 5 + extension_count => {
                if let Some(menu) = &mut self.activity_bar_menu {
                    menu.submenu = Some(ActivityBarSubmenu::Size);
                }
            }
            index if index == 6 + extension_count => {
                self.toggle_primary_sidebar_side();
                self.activity_bar_menu = None;
            }
            index if menu.show_restore && index == 7 + extension_count => {
                self.set_activity_bar_position(ActivityBarPosition::Default);
                self.activity_bar_menu = None;
                self.workbench_menu = None;
            }
            _ => {}
        }
        true
    }

    pub(crate) fn activity_bar_menu_pointer_moved(&mut self, x: f32, y: f32, window_width: f32) {
        let Some(menu) = self.activity_bar_menu.as_mut() else {
            return;
        };
        if !main_menu_rect(menu).contains(x, y) {
            return;
        }
        let local_y = y - menu.y;
        let selected = menu_rows(menu.extension_items.len(), menu.show_restore)
            .iter()
            .enumerate()
            .find_map(|(index, top)| {
                (local_y >= *top && local_y < *top + MENU_ROW).then_some(index)
            });
        let Some(selected) = selected else {
            return;
        };
        menu.selected = selected;
        let extension_count = menu.extension_items.len();
        menu.submenu = match selected {
            index if index == 4 + extension_count => Some(ActivityBarSubmenu::Position),
            index if index == 5 + extension_count => Some(ActivityBarSubmenu::Size),
            _ => None,
        };
        menu.submenu_selected = 0;
        let _ = window_width;
    }

    pub(crate) fn activity_bar_menu_key(&mut self, key: &winit::keyboard::Key) -> bool {
        let Some(menu_snapshot) = self.activity_bar_menu.as_ref() else {
            return false;
        };
        use winit::keyboard::{Key, NamedKey};
        if let Some(submenu) = menu_snapshot.submenu {
            let count = match submenu {
                ActivityBarSubmenu::Position => 4,
                ActivityBarSubmenu::Size => 2,
            };
            match key {
                Key::Named(NamedKey::Escape) => self.activity_bar_menu = None,
                Key::Named(NamedKey::ArrowLeft) => {
                    if let Some(menu) = &mut self.activity_bar_menu {
                        menu.submenu = None;
                    }
                }
                Key::Named(NamedKey::ArrowDown) => {
                    if let Some(menu) = &mut self.activity_bar_menu {
                        menu.submenu_selected = (menu.submenu_selected + 1) % count;
                    }
                }
                Key::Named(NamedKey::ArrowUp) => {
                    if let Some(menu) = &mut self.activity_bar_menu {
                        menu.submenu_selected = (menu.submenu_selected + count - 1) % count;
                    }
                }
                Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                    let selected = menu_snapshot.submenu_selected;
                    match submenu {
                        ActivityBarSubmenu::Position => {
                            self.set_activity_bar_position(match selected {
                                0 => ActivityBarPosition::Default,
                                1 => ActivityBarPosition::Top,
                                2 => ActivityBarPosition::Bottom,
                                _ => ActivityBarPosition::Hidden,
                            })
                        }
                        ActivityBarSubmenu::Size => self.set_activity_bar_size(if selected == 0 {
                            ActivityBarSize::Default
                        } else {
                            ActivityBarSize::Compact
                        }),
                    }
                    self.activity_bar_menu = None;
                }
                _ => return false,
            }
            return true;
        }
        match key {
            Key::Named(NamedKey::Escape) => self.activity_bar_menu = None,
            Key::Named(NamedKey::ArrowDown) => {
                if let Some(menu) = &mut self.activity_bar_menu {
                    menu.selected = (menu.selected + 1)
                        % (7 + menu.extension_items.len() + usize::from(menu.show_restore));
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if let Some(menu) = &mut self.activity_bar_menu {
                    let count = 7 + menu.extension_items.len() + usize::from(menu.show_restore);
                    menu.selected = (menu.selected + count - 1) % count;
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                if let Some(menu) = &mut self.activity_bar_menu {
                    let extension_count = menu.extension_items.len();
                    menu.submenu = match menu.selected {
                        index if index == 4 + extension_count => Some(ActivityBarSubmenu::Position),
                        index if index == 5 + extension_count => Some(ActivityBarSubmenu::Size),
                        _ => None,
                    };
                    menu.submenu_selected = 0;
                }
            }
            Key::Named(NamedKey::ArrowLeft) => self.activity_bar_menu = None,
            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                let selected = menu_snapshot.selected;
                let extension_count = menu_snapshot.extension_items.len();
                match selected {
                    0..=3 => {
                        self.toggle_activity_visibility(Activity::ALL[selected]);
                        self.activity_bar_menu = None;
                    }
                    index if index < 4 + extension_count => {
                        let entry = menu_snapshot.extension_items[index - 4].clone();
                        self.toggle_extension_activity_visibility(&entry);
                        self.activity_bar_menu = None;
                    }
                    index if index == 4 + extension_count || index == 5 + extension_count => {
                        if let Some(menu) = &mut self.activity_bar_menu {
                            menu.submenu = Some(if selected == 4 + extension_count {
                                ActivityBarSubmenu::Position
                            } else {
                                ActivityBarSubmenu::Size
                            });
                            menu.submenu_selected = 0;
                        }
                    }
                    index if index == 6 + extension_count => {
                        self.toggle_primary_sidebar_side();
                        self.activity_bar_menu = None;
                    }
                    index if menu_snapshot.show_restore && index == 7 + extension_count => {
                        self.set_activity_bar_position(ActivityBarPosition::Default);
                        self.activity_bar_menu = None;
                        self.workbench_menu = None;
                    }
                    _ => {}
                }
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn save_activity_bar_preferences(&mut self) {
        if self.profiles.is_some() {
            if let Err(error) = config::save_activity_bar_preferences(
                &self.profile_root,
                &self.activity_bar_preferences,
            ) {
                self.status = format!("{}: {error}", self.t("activity.preferences.save_error"));
            }
        }
    }

    pub(crate) fn set_activity_bar_position(&mut self, position: ActivityBarPosition) {
        self.activity_bar_preferences.position = position;
        self.save_activity_bar_preferences();
    }

    pub(crate) fn set_activity_bar_size(&mut self, size: ActivityBarSize) {
        self.activity_bar_preferences.size = size;
        self.save_activity_bar_preferences();
    }

    pub(crate) fn toggle_primary_sidebar_side(&mut self) {
        self.activity_bar_preferences.primary_sidebar_right =
            !self.activity_bar_preferences.primary_sidebar_right;
        self.save_activity_bar_preferences();
    }

    fn toggle_activity_visibility(&mut self, activity: Activity) {
        let id = activity.preference_id();
        if self.activity_bar_preferences.hidden_items.remove(id) {
            // Revealed.
        } else {
            let visible = Activity::ALL
                .iter()
                .filter(|item| {
                    !self
                        .activity_bar_preferences
                        .hidden_items
                        .contains(item.preference_id())
                })
                .count();
            if visible <= 1 {
                self.status = self.t("activity.visibility.last_item").to_owned();
                return;
            }
            self.activity_bar_preferences
                .hidden_items
                .insert(id.to_owned());
            if self.active_activity() == activity {
                self.activate_fallback_activity(activity);
            }
        }
        self.save_activity_bar_preferences();
    }

    fn toggle_extension_activity_visibility(&mut self, entry: &PluginActivityEntry) {
        if self
            .activity_bar_preferences
            .hidden_items
            .remove(&entry.key)
        {
            // Revealed.
        } else {
            self.activity_bar_preferences
                .hidden_items
                .insert(entry.key.clone());
            if self.active_plugin_activity.as_deref() == Some(&entry.key) {
                self.active_plugin_activity = None;
                self.activate_fallback_activity(Activity::Plugins);
            }
        }
        self.save_activity_bar_preferences();
    }

    pub(crate) fn activate_plugin_activity(&mut self, entry: PluginActivityEntry) {
        if !self
            .plugin_activity_entries()
            .iter()
            .any(|current| current.key == entry.key)
        {
            return;
        }
        self.close_plugins();
        self.close_project_search();
        self.scm_visible = false;
        self.active_plugin_activity = Some(entry.key.clone());
        let is_native = self.plugins.installed_plugins().iter().any(|record| {
            record.metadata.id() == entry.plugin_id
                && matches!(
                    record.metadata,
                    graphite_plugins::manifest::Metadata::Native(_)
                )
        });
        if is_native {
            for view in &entry.views {
                self.activate_plugins_for_event(&format!("onView:{}", view.id));
            }
            if !self.plugins.hosts.contains_key(&entry.plugin_id) {
                self.start_plugin(entry.plugin_id);
            } else {
                for view in &entry.views {
                    if view.kind == "tree" {
                        self.request_native_tree_children(&view.id, None);
                    }
                }
            }
        } else {
            for view in &entry.views {
                self.activate_plugins_for_event(&format!("onView:{}", view.id));
            }
            if self.plugins.vsix.is_none() && self.vsix_workspace_trusted() {
                let runtime_path = graphite_extension_host::code_oss::Session::bundled_runtime();
                if runtime_path.is_some() {
                    let _ = self.start_vsix_host(runtime_path);
                }
            }
            for view in &entry.views {
                if self.is_vsix_tree_view(&view.id) {
                    self.notify_vsix_tree_visible(&view.id, true);
                    self.request_vsix_tree_children(&view.id, None);
                }
            }
            self.status = format!("{} · {}", entry.title, entry.plugin_id);
        }
    }

    pub(crate) fn draw_plugin_activity_sidebar(
        &self,
        renderer: &mut Renderer,
        height: f32,
        line_height: f32,
        cell_width: f32,
    ) {
        let Some(key) = self.active_plugin_activity.as_deref() else {
            return;
        };
        let Some(entry) = self
            .plugin_activity_entries()
            .into_iter()
            .find(|entry| entry.key == key)
        else {
            return;
        };
        let width = self.sidebar_layout_width_px() - ACTIVITY_BAR_WIDTH;
        renderer.draw_rect(
            ACTIVITY_BAR_WIDTH,
            0.0,
            width,
            height,
            rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3)),
        );
        renderer.draw_text(
            self.t("activity.plugin_view"),
            ACTIVITY_BAR_WIDTH + PAD,
            line_height * 0.25,
            rgb(Theme::color(Theme::DIM, 0x616161)),
        );
        renderer.draw_text(
            &entry.title,
            ACTIVITY_BAR_WIDTH + PAD,
            line_height * 1.6,
            rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
        );
        renderer.draw_text(
            &entry.plugin_id,
            ACTIVITY_BAR_WIDTH + PAD,
            line_height * 2.8,
            rgb(Theme::color(Theme::DIM, 0x858585)),
        );
        let chars_per_row = ((width - PAD * 2.0) / cell_width.max(1.0)).max(1.0) as usize;
        let mut row = 0usize;
        for view in entry.views.iter().filter(|view| view.kind != "webview") {
            let y = line_height * (4.0 + row as f32);
            if y + line_height >= height {
                break;
            }
            let chars = ((width - PAD * 2.0) / cell_width.max(1.0)).max(1.0) as usize;
            let tree_rows = if view.kind == "tree" {
                self.plugin_tree_items(&view.id)
            } else {
                Vec::new()
            };
            let header = if tree_rows.is_empty() {
                format!("{}  ({})", view.title, view.kind)
            } else {
                view.title.clone()
            };
            renderer.draw_text(
                &header.chars().take(chars).collect::<String>(),
                ACTIVITY_BAR_WIDTH + PAD,
                y,
                rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
            );
            row += 1;
            let expanded = self
                .plugins
                .native_tree_views
                .get(&view.id)
                .map(|t| &t.expanded);
            for item in tree_rows {
                let item_y = line_height * (4.0 + row as f32);
                if item_y + line_height >= height {
                    break;
                }
                let glyph = match item.item.collapsible_state {
                    graphite_core_api::TreeCollapsibleState::None => "•",
                    _ if expanded.is_some_and(|set| set.contains(&item.item.id)) => "▾",
                    _ => "▸",
                };
                let indent = "  ".repeat(item.depth);
                let desc = item
                    .item
                    .description
                    .as_deref()
                    .map(|d| format!("  {d}"))
                    .unwrap_or_default();
                let line = format!("{indent}{glyph} {}{desc}", item.item.label);
                renderer.draw_text(
                    &line.chars().take(chars).collect::<String>(),
                    ACTIVITY_BAR_WIDTH + PAD,
                    item_y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
                row += 1;
            }
        }
        if let Some(lh) = self.gpu.as_ref().map(|gpu| gpu.renderer.line_height()) {
            for slot in self.webview_slots_for(&entry, height, lh) {
                let fallback = entry
                    .views
                    .iter()
                    .find(|view| view.id == slot.view_id)
                    .map(|view| view.title.as_str())
                    .unwrap_or_default();
                let header = self.webview_view_header(&slot.view_id, fallback);
                renderer.draw_text(
                    &header.chars().take(chars_per_row).collect::<String>(),
                    ACTIVITY_BAR_WIDTH + PAD,
                    slot.header_y,
                    rgb(Theme::color(Theme::TEXT, 0x1e1e1e)),
                );
            }
        }
        if entry.views.is_empty() {
            renderer.draw_text(
                self.t("activity.plugin_view.empty"),
                ACTIVITY_BAR_WIDTH + PAD,
                line_height * 4.0,
                rgb(Theme::color(Theme::DIM, 0x858585)),
            );
        }
    }

    /// Shows the next (or previous) side bar view, skipping the ones hidden from the activity
    /// bar, and makes the side bar visible.
    pub(crate) fn cycle_side_bar_view(&mut self, reverse: bool) {
        let visible: Vec<Activity> = Activity::ALL
            .into_iter()
            .filter(|item| {
                !self
                    .activity_bar_preferences
                    .hidden_items
                    .contains(item.preference_id())
            })
            .collect();
        if visible.is_empty() {
            return;
        }
        let current = self.active_activity();
        let next = match visible.iter().position(|item| *item == current) {
            Some(index) if reverse => visible[(index + visible.len() - 1) % visible.len()],
            Some(index) => visible[(index + 1) % visible.len()],
            None => visible[0],
        };
        if !self.view_preferences.primary_sidebar_visible {
            self.view_preferences.primary_sidebar_visible = true;
            self.save_view_preferences_for_command();
        }
        self.activate_activity(next);
    }

    fn activate_fallback_activity(&mut self, hidden: Activity) {
        if let Some(next) = Activity::ALL.into_iter().find(|item| {
            *item != hidden
                && !self
                    .activity_bar_preferences
                    .hidden_items
                    .contains(item.preference_id())
        }) {
            self.activate_activity(next);
        }
    }

    /// The view represented by the activity bar's current selection.
    pub(crate) fn active_activity(&self) -> Activity {
        if self.plugins.visible {
            Activity::Plugins
        } else if self.search_query.is_some() {
            Activity::Search
        } else if self.scm_visible {
            Activity::SourceControl
        } else {
            Activity::Explorer
        }
    }

    pub(crate) fn activate_activity(&mut self, activity: Activity) {
        self.active_plugin_activity = None;
        match activity {
            Activity::Explorer => {
                self.close_plugins();
                self.scm_visible = false;
                self.close_project_search();
            }
            Activity::Search => {
                self.close_plugins();
                if self.search_query.is_some() {
                    self.close_project_search();
                } else {
                    self.scm_visible = false;
                    self.open_project_search();
                }
            }
            Activity::SourceControl => {
                self.close_plugins();
                if self.scm_visible {
                    self.scm_visible = false;
                } else {
                    self.close_project_search();
                    self.scm_visible = true;
                    self.scm_focused = true;
                    self.refresh_scm_async();
                }
            }
            Activity::Plugins => {
                if !self.plugins.visible {
                    self.open_plugins();
                } else if self.plugins.focused {
                    self.close_plugins();
                } else {
                    self.focus_plugins();
                }
            }
        }
    }

    /// Handles a click inside the bar. Returns `false` when `y` misses an item.
    pub(crate) fn activity_bar_click(&mut self, y: f32) -> bool {
        let Some(activity) = activity_at(y).filter(|activity| {
            !self
                .activity_bar_preferences
                .hidden_items
                .contains(activity.preference_id())
        }) else {
            return false;
        };
        self.activate_activity(activity);
        true
    }

    pub(crate) fn activity_bar_click_at(&mut self, x: f32, y: f32) -> bool {
        let Some((width, height)) = self
            .gpu
            .as_ref()
            .map(|gpu| (gpu.renderer.size().0, gpu.renderer.size().1))
        else {
            return false;
        };
        let entries = self.visible_activity_items();
        let Some(layout) = ActivityBarLayout::new(
            &self.activity_bar_preferences,
            width,
            height,
            self.top_menu_height(),
        ) else {
            return false;
        };
        let gear = settings_gear_rect(
            layout,
            width,
            height,
            self.gpu
                .as_ref()
                .map_or(20.0, |gpu| gpu.renderer.line_height()),
        );
        let profile = profile_button_rect(layout, gear);
        if profile.contains(x, y) {
            if self.profile_menu.is_some() {
                self.profile_menu = None;
            } else {
                self.open_profile_menu(
                    (profile.x, profile.y, profile.width, profile.height),
                    width,
                    height,
                );
            }
            return true;
        }
        if gear.contains(x, y) {
            self.profile_menu = None;
            if self.settings_menu.is_some() {
                self.settings_menu = None;
            } else {
                self.open_settings_menu((gear.x, gear.y, gear.width, gear.height), width, height);
            }
            return true;
        }
        let Some((_, selected)) = entries.iter().enumerate().find(|(index, _)| {
            let rect = layout.item_rect(*index);
            rect.contains(x, y)
                && !gear.contains(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
        }) else {
            return false;
        };
        match selected {
            VisibleActivityItem::Builtin(activity) => self.activate_activity(*activity),
            VisibleActivityItem::Plugin(entry) => self.activate_plugin_activity(entry.clone()),
        }
        true
    }

    pub(crate) fn activity_bar_contains(&self, x: f32, y: f32) -> bool {
        self.gpu.as_ref().is_some_and(|gpu| {
            ActivityBarLayout::new(
                &self.activity_bar_preferences,
                gpu.renderer.size().0,
                gpu.renderer.size().1,
                self.top_menu_height(),
            )
            .is_some_and(|layout| layout.contains(x, y))
        })
    }

    pub(crate) fn activity_bar_right_click(&mut self, x: f32, y: f32) -> bool {
        let (width, height) = self
            .gpu
            .as_ref()
            .map(|gpu| (gpu.renderer.size().0, gpu.renderer.size().1))
            .unwrap_or((1200.0, 760.0));
        if !self.zen_mode
            && ActivityBarLayout::new(
                &self.activity_bar_preferences,
                width,
                height,
                self.top_menu_height(),
            )
            .is_some_and(|layout| layout.contains(x, y))
        {
            self.workbench_menu = None;
            self.settings_menu = None;
            self.profile_menu = None;
            self.open_activity_bar_menu(x, y, width, height);
            return true;
        }
        self.activity_bar_menu = None;
        false
    }

    pub(crate) fn draw_activity_bar(&self, renderer: &mut Renderer, height: f32) {
        let (window_width, _) = renderer.size();
        let Some(layout) = ActivityBarLayout::new(
            &self.activity_bar_preferences,
            window_width,
            height,
            self.top_menu_height(),
        ) else {
            self.draw_activity_bar_menu(renderer, height);
            return;
        };
        renderer.draw_rect(
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            rgb(Theme::color(Theme::ACTIVITY_BAR, 0xe8e8e8)),
        );
        let active = self.active_activity();
        let entries = self.visible_activity_items();
        let gear = settings_gear_rect(layout, window_width, height, renderer.line_height());
        let profile = profile_button_rect(layout, gear);
        for (index, entry) in entries.iter().enumerate() {
            let item = layout.item_rect(index);
            if item.y + item.height > height
                || item_overlaps(item, gear)
                || item_overlaps(item, profile)
            {
                break;
            }
            let is_active = match entry {
                VisibleActivityItem::Builtin(activity) => {
                    *activity == active && self.active_plugin_activity.is_none()
                }
                VisibleActivityItem::Plugin(plugin) => {
                    self.active_plugin_activity.as_deref() == Some(plugin.key.as_str())
                }
            };
            let hovered = item.contains(self.mouse.x as f32, self.mouse.y as f32);
            if hovered && !is_active {
                renderer.draw_rect(
                    item.x,
                    item.y,
                    item.width,
                    item.height,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
                );
            }
            if is_active {
                let (x, y, width, height) =
                    if layout.position == ActivityBarPosition::Default || layout.compact {
                        (item.x, item.y, 2.0, item.height)
                    } else if layout.position == ActivityBarPosition::Top {
                        (item.x, item.y + item.height - 2.0, item.width, 2.0)
                    } else {
                        (item.x, item.y, item.width, 2.0)
                    };
                renderer.draw_rect(
                    x,
                    y,
                    width,
                    height,
                    rgb(Theme::color(Theme::STATUS, 0x005a9e)),
                );
            }
            let color = if is_active {
                Theme::color(Theme::TEXT, 0x1e1e1e)
            } else {
                Theme::color(Theme::DIM, 0x616161)
            };
            match entry {
                VisibleActivityItem::Builtin(activity) => renderer.draw_icon(
                    activity.glyph(),
                    item.x
                        + (item.width
                            - if layout.compact {
                                20.0
                            } else {
                                ACTIVITY_BAR_ICON_SIZE
                            })
                            * 0.5,
                    item.y
                        + (item.height
                            - if layout.compact {
                                20.0
                            } else {
                                ACTIVITY_BAR_ICON_SIZE
                            })
                            * 0.5,
                    if layout.compact {
                        20.0
                    } else {
                        ACTIVITY_BAR_ICON_SIZE
                    },
                    rgb(color),
                ),
                VisibleActivityItem::Plugin(plugin) => {
                    let initial = plugin.title.chars().next().unwrap_or('?').to_string();
                    let tw = initial.chars().count() as f32 * renderer.char_width();
                    renderer.draw_text(
                        &initial,
                        item.x + (item.width - tw) * 0.5,
                        item.y + (item.height - renderer.line_height()) * 0.5,
                        rgb(color),
                    );
                }
            }
        }
        let gear_hovered = gear.contains(self.mouse.x as f32, self.mouse.y as f32);
        let gear_color = if gear_hovered {
            Theme::color(Theme::TEXT, 0xcccccc)
        } else {
            Theme::color(Theme::DIM, 0x858585)
        };
        let gear_size = 20.0;
        let gear_x = gear.x + (gear.width - gear_size) * 0.5;
        let gear_y = gear.y + (gear.height - gear_size) * 0.5;
        let gear_bg = if gear_hovered {
            Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)
        } else {
            Theme::color(Theme::ACTIVITY_BAR, 0xe8e8e8)
        };
        if gear_hovered {
            renderer.draw_rect(gear.x, gear.y, gear.width, gear.height, rgb(gear_bg));
        }
        draw_settings_gear(
            renderer,
            gear_x,
            gear_y,
            gear_size,
            rgb(gear_color),
            rgb(gear_bg),
        );
        let profile_hovered = profile.contains(self.mouse.x as f32, self.mouse.y as f32);
        if profile_hovered {
            renderer.draw_rect(
                profile.x,
                profile.y,
                profile.width,
                profile.height,
                rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
            );
        }
        draw_profile_silhouette(
            renderer,
            profile.x + (profile.width - gear_size) * 0.5,
            profile.y + (profile.height - gear_size) * 0.5,
            gear_size,
            rgb(if profile_hovered {
                Theme::color(Theme::TEXT, 0xcccccc)
            } else {
                Theme::color(Theme::DIM, 0x858585)
            }),
        );
        self.draw_activity_bar_menu(renderer, height);
        self.draw_settings_menu(renderer, window_width, height);
        self.draw_profile_menu(renderer, window_width, height);
    }

    pub(crate) fn draw_activity_bar_menu(&self, renderer: &mut Renderer, window_height: f32) {
        let Some(menu) = &self.activity_bar_menu else {
            return;
        };
        let rect = main_menu_rect(menu);
        let background = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let text = rgb(Theme::color(Theme::TEXT, 0xcccccc));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        renderer.draw_rect(rect.x, rect.y, rect.width, rect.height, border);
        renderer.draw_rect(
            rect.x + 1.0,
            rect.y + 1.0,
            rect.width - 2.0,
            rect.height - 2.0,
            background,
        );
        let rows = menu_rows(menu.extension_items.len(), menu.show_restore);
        for (index, activity) in Activity::ALL.iter().copied().enumerate() {
            let row_y = rect.y + rows[index];
            if menu.selected == index {
                renderer.draw_rect(
                    rect.x + 2.0,
                    row_y,
                    rect.width - 4.0,
                    MENU_ROW,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0x094771)),
                );
            }
            if !self
                .activity_bar_preferences
                .hidden_items
                .contains(activity.preference_id())
            {
                renderer.draw_text("✓", rect.x + 10.0, row_y + 3.0, text);
            }
            renderer.draw_text(
                self.t(activity.label_key()),
                rect.x + 34.0,
                row_y + 3.0,
                text,
            );
        }
        for (offset, entry) in menu.extension_items.iter().enumerate() {
            let index = 4 + offset;
            let row_y = rect.y + rows[index];
            if menu.selected == index {
                renderer.draw_rect(
                    rect.x + 2.0,
                    row_y,
                    rect.width - 4.0,
                    MENU_ROW,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0x094771)),
                );
            }
            if !self
                .activity_bar_preferences
                .hidden_items
                .contains(&entry.key)
            {
                renderer.draw_text("✓", rect.x + 10.0, row_y + 3.0, text);
            }
            renderer.draw_text(&entry.title, rect.x + 34.0, row_y + 3.0, text);
        }
        let settings_start = 4 + menu.extension_items.len();
        renderer.draw_rect(
            rect.x + 1.0,
            rect.y + rows[settings_start] - 5.0,
            rect.width - 2.0,
            1.0,
            border,
        );
        let labels = [
            self.t("activity.position"),
            self.t("activity.size"),
            self.t("activity.move_sidebar_right"),
        ];
        for offset in 0..3 {
            let index = settings_start + offset;
            let row_y = rect.y + rows[index];
            if menu.selected == index {
                renderer.draw_rect(
                    rect.x + 2.0,
                    row_y,
                    rect.width - 4.0,
                    MENU_ROW,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0x094771)),
                );
            }
            renderer.draw_text(&labels[offset], rect.x + 15.0, row_y + 3.0, text);
            if offset < 2 {
                renderer.draw_text("›", rect.x + rect.width - 18.0, row_y + 3.0, dim);
            } else if self.activity_bar_preferences.primary_sidebar_right {
                renderer.draw_text("✓", rect.x + 10.0, row_y + 3.0, text);
            }
        }
        if menu.show_restore {
            let index = settings_start + 3;
            let row_y = rect.y + rows[index];
            if menu.selected == index {
                renderer.draw_rect(
                    rect.x + 2.0,
                    row_y,
                    rect.width - 4.0,
                    MENU_ROW,
                    rgb(Theme::color(Theme::ACTIVE_ROW, 0x094771)),
                );
            }
            renderer.draw_text(self.t("activity.show"), rect.x + 15.0, row_y + 3.0, text);
        }
        if let Some(submenu) = menu.submenu {
            let (anchor_index, values, active) = match submenu {
                ActivityBarSubmenu::Position => (
                    settings_start,
                    [
                        self.t("activity.position.default"),
                        self.t("activity.position.top"),
                        self.t("activity.position.bottom"),
                        self.t("activity.position.hidden"),
                    ]
                    .to_vec(),
                    match self.activity_bar_preferences.position {
                        ActivityBarPosition::Default => 0,
                        ActivityBarPosition::Top => 1,
                        ActivityBarPosition::Bottom => 2,
                        ActivityBarPosition::Hidden => 3,
                    },
                ),
                ActivityBarSubmenu::Size => (
                    settings_start + 1,
                    vec![
                        self.t("activity.size.default"),
                        self.t("activity.size.compact"),
                    ],
                    usize::from(self.activity_bar_preferences.size == ActivityBarSize::Compact),
                ),
            };
            let sub = submenu_rect(
                menu,
                renderer.size().0,
                window_height,
                rows[anchor_index],
                values.len(),
            );
            renderer.draw_rect(sub.x, sub.y, sub.width, sub.height, border);
            renderer.draw_rect(
                sub.x + 1.0,
                sub.y + 1.0,
                sub.width - 2.0,
                sub.height - 2.0,
                background,
            );
            for (index, label) in values.iter().enumerate() {
                let row_y = sub.y + MENU_PADDING + index as f32 * MENU_ROW;
                if index == menu.submenu_selected {
                    renderer.draw_rect(
                        sub.x + 2.0,
                        row_y,
                        sub.width - 4.0,
                        MENU_ROW,
                        rgb(Theme::color(Theme::ACTIVE_ROW, 0x094771)),
                    );
                }
                if index == active {
                    renderer.draw_text("✓", sub.x + 10.0, row_y + 3.0, text);
                }
                renderer.draw_text(label, sub.x + 30.0, row_y + 3.0, text);
            }
        }
    }
}

fn settings_gear_rect(
    layout: ActivityBarLayout,
    width: f32,
    height: f32,
    status_height: f32,
) -> MenuRect {
    match layout.position {
        ActivityBarPosition::Top | ActivityBarPosition::Bottom => MenuRect {
            x: (width - layout.item_size - 4.0).max(0.0),
            y: layout.y,
            width: layout.item_size,
            height: layout.item_size,
        },
        _ => MenuRect {
            x: layout.x,
            y: (height - status_height - layout.item_size).max(layout.y),
            width: layout.width,
            height: layout.item_size,
        },
    }
}

fn profile_button_rect(layout: ActivityBarLayout, gear: MenuRect) -> MenuRect {
    match layout.position {
        ActivityBarPosition::Top | ActivityBarPosition::Bottom => MenuRect {
            x: (gear.x - layout.item_size).max(0.0),
            y: gear.y,
            width: layout.item_size,
            height: layout.item_size,
        },
        _ => MenuRect {
            x: gear.x,
            y: (gear.y - layout.item_size).max(layout.y),
            width: layout.width,
            height: layout.item_size,
        },
    }
}

pub(crate) fn activity_bar_profile_button_rect(
    preferences: &config::ActivityBarPreferences,
    width: f32,
    height: f32,
    menu_bar_height: f32,
    status_height: f32,
) -> Option<(f32, f32, f32, f32)> {
    let layout = ActivityBarLayout::new(preferences, width, height, menu_bar_height)?;
    let gear = settings_gear_rect(layout, width, height, status_height);
    let rect = profile_button_rect(layout, gear);
    Some((rect.x, rect.y, rect.width, rect.height))
}

pub(crate) fn activity_bar_settings_gear_rect(
    preferences: &config::ActivityBarPreferences,
    width: f32,
    height: f32,
    menu_bar_height: f32,
    status_height: f32,
) -> Option<(f32, f32, f32, f32)> {
    let layout = ActivityBarLayout::new(preferences, width, height, menu_bar_height)?;
    let rect = settings_gear_rect(layout, width, height, status_height);
    Some((rect.x, rect.y, rect.width, rect.height))
}

fn item_overlaps(a: MenuRect, b: MenuRect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

fn draw_settings_gear(
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    size: f32,
    color: graphite_render::Color,
    background: graphite_render::Color,
) {
    let scale = size / 20.0;
    let teeth = [
        (8.0, 0.0, 4.0, 5.0),
        (8.0, 15.0, 4.0, 5.0),
        (0.0, 8.0, 5.0, 4.0),
        (15.0, 8.0, 5.0, 4.0),
    ];
    for (dx, dy, width, height) in teeth {
        renderer.draw_rect(
            x + dx * scale,
            y + dy * scale,
            width * scale,
            height * scale,
            color,
        );
    }
    let disk_rows = [
        (6.0, 4.0, 8.0),
        (4.0, 6.0, 12.0),
        (4.0, 8.0, 12.0),
        (4.0, 10.0, 12.0),
        (6.0, 12.0, 8.0),
    ];
    for (dx, dy, width) in disk_rows {
        renderer.draw_rect(
            x + dx * scale,
            y + dy * scale,
            width * scale,
            2.0 * scale,
            color,
        );
    }
    renderer.draw_rect(
        x + 7.0 * scale,
        y + 7.0 * scale,
        6.0 * scale,
        6.0 * scale,
        background,
    );
}

fn draw_profile_silhouette(
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    size: f32,
    color: graphite_render::Color,
) {
    let scale = size / 20.0;
    for (dx, dy, width, height) in [
        (8.0, 2.0, 4.0, 2.0),
        (6.0, 4.0, 8.0, 2.0),
        (6.0, 6.0, 8.0, 4.0),
        (8.0, 10.0, 4.0, 2.0),
        (6.0, 12.0, 8.0, 2.0),
        (4.0, 14.0, 12.0, 2.0),
        (2.0, 16.0, 16.0, 2.0),
        (2.0, 18.0, 16.0, 2.0),
    ] {
        renderer.draw_rect(
            x + dx * scale,
            y + dy * scale,
            width * scale,
            height * scale,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_gear_is_pinned_to_each_visible_activity_bar_and_hidden_with_it() {
        let mut preferences = config::ActivityBarPreferences::default();
        let vertical =
            activity_bar_settings_gear_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).unwrap();
        assert_eq!(vertical.0, 0.0);
        assert_eq!(vertical.1 + vertical.3, 780.0);
        preferences.position = ActivityBarPosition::Top;
        let top = activity_bar_settings_gear_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).unwrap();
        assert!(top.0 + top.2 <= 1000.0);
        assert_eq!(top.1, 24.0);
        preferences.position = ActivityBarPosition::Bottom;
        let bottom =
            activity_bar_settings_gear_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).unwrap();
        assert_eq!(bottom.1, 732.0);
        preferences.position = ActivityBarPosition::Hidden;
        assert!(activity_bar_settings_gear_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).is_none());
    }

    #[test]
    fn profile_button_sits_before_gear_without_overlap_in_every_bar_position() {
        let mut preferences = config::ActivityBarPreferences::default();
        let gear =
            activity_bar_settings_gear_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).unwrap();
        let profile =
            activity_bar_profile_button_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).unwrap();
        assert_eq!(profile.1 + profile.3, gear.1);
        assert!(!item_overlaps(
            MenuRect {
                x: profile.0,
                y: profile.1,
                width: profile.2,
                height: profile.3
            },
            MenuRect {
                x: gear.0,
                y: gear.1,
                width: gear.2,
                height: gear.3
            },
        ));

        preferences.position = ActivityBarPosition::Top;
        let gear =
            activity_bar_settings_gear_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).unwrap();
        let profile =
            activity_bar_profile_button_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).unwrap();
        assert_eq!(profile.0 + profile.2, gear.0);
        assert_eq!(profile.1, gear.1);

        preferences.position = ActivityBarPosition::Bottom;
        let gear =
            activity_bar_settings_gear_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).unwrap();
        let profile =
            activity_bar_profile_button_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).unwrap();
        assert_eq!(profile.0 + profile.2, gear.0);
        assert_eq!(profile.1, gear.1);

        preferences.position = ActivityBarPosition::Hidden;
        assert!(
            activity_bar_profile_button_rect(&preferences, 1000.0, 800.0, 24.0, 20.0).is_none()
        );
    }

    #[test]
    fn hit_testing_maps_each_vertical_slot() {
        assert_eq!(activity_at(activity_top(0) + 1.0), Some(Activity::Explorer));
        assert_eq!(activity_at(activity_top(1) + 1.0), Some(Activity::Search));
        assert_eq!(
            activity_at(activity_top(2) + ACTIVITY_BAR_ITEM_HEIGHT - 1.0),
            Some(Activity::SourceControl)
        );
        assert_eq!(activity_at(activity_top(3) + 1.0), Some(Activity::Plugins));
        assert_eq!(activity_at(ACTIVITY_BAR_TOP - 1.0), None);
        assert_eq!(activity_at(activity_top(4)), None);
        assert_eq!(activity_at(-10.0), None);
    }

    #[test]
    fn active_activity_prefers_overlays_over_the_explorer() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        assert_eq!(app.active_activity(), Activity::Explorer);
        app.scm_visible = true;
        assert_eq!(app.active_activity(), Activity::SourceControl);
        app.search_query = Some(String::new());
        assert_eq!(app.active_activity(), Activity::Search);
        app.plugins.visible = true;
        assert_eq!(app.active_activity(), Activity::Plugins);
    }

    #[test]
    fn clicking_switches_views_and_toggles_search() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);

        assert!(app.activity_bar_click(activity_top(2) + 1.0));
        assert!(app.scm_visible);
        assert_eq!(app.active_activity(), Activity::SourceControl);

        assert!(app.activity_bar_click(activity_top(2) + 1.0));
        assert!(!app.scm_visible);
        assert_eq!(app.active_activity(), Activity::Explorer);
        assert!(app.activity_bar_click(activity_top(2) + 1.0));
        assert!(app.scm_visible);

        assert!(app.activity_bar_click(activity_top(1) + 1.0));
        assert!(!app.scm_visible);
        assert!(app.search_query.is_some());
        assert_eq!(app.active_activity(), Activity::Search);

        assert!(app.activity_bar_click(activity_top(1) + 1.0));
        assert!(app.search_query.is_none());
        assert_eq!(app.active_activity(), Activity::Explorer);

        assert!(app.activity_bar_click(activity_top(2) + 1.0));
        assert!(app.scm_visible);
        assert!(app.activity_bar_click(activity_top(0) + 1.0));
        assert!(!app.scm_visible);
        assert!(!app.activity_bar_click(activity_top(4)));
    }

    #[test]
    fn plugins_item_toggles_focus_and_other_views_close_it() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.use_plugin_store(temp.path().join("plugins"));

        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(app.plugins.visible);
        assert!(app.plugins.focused);
        assert_eq!(app.active_activity(), Activity::Plugins);

        app.plugins.focused = false;
        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(app.plugins.visible);
        assert!(app.plugins.focused);

        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(!app.plugins.visible);
        assert!(!app.plugins.focused);
        assert_eq!(app.active_activity(), Activity::Explorer);

        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(app.activity_bar_click(activity_top(2) + 1.0));
        assert!(!app.plugins.visible);
        assert!(app.scm_visible);

        assert!(app.activity_bar_click(activity_top(3) + 1.0));
        assert!(app.activity_bar_click(activity_top(1) + 1.0));
        assert!(!app.plugins.visible);
        assert!(app.search_query.is_some());
    }

    #[test]
    fn activity_bar_adds_to_the_existing_explorer_width() {
        assert_eq!(SIDEBAR_WIDTH - ACTIVITY_BAR_WIDTH, 260.0);
    }

    #[test]
    fn every_activity_glyph_exists_in_the_embedded_font() {
        for activity in Activity::ALL {
            assert!(
                graphite_render::icons::AVAILABLE_GLYPHS.contains(&activity.glyph()),
                "missing glyph U+{:04X} in the embedded icon font",
                activity.glyph() as u32
            );
        }
    }

    #[test]
    fn activity_bar_layout_modes_share_clickable_item_bounds() {
        let mut preferences = config::ActivityBarPreferences::default();
        let default = ActivityBarLayout::new(&preferences, 800.0, 600.0, 30.0).unwrap();
        assert_eq!(default.width, 48.0);
        assert!(default.item_rect(0).contains(20.0, 40.0));

        preferences.size = ActivityBarSize::Compact;
        let compact = ActivityBarLayout::new(&preferences, 800.0, 600.0, 30.0).unwrap();
        assert_eq!(compact.width, 36.0);
        assert_eq!(compact.item_size, 40.0);

        preferences.position = ActivityBarPosition::Top;
        let top = ActivityBarLayout::new(&preferences, 800.0, 600.0, 30.0).unwrap();
        assert!(top.item_rect(0).contains(10.0, 40.0));
        assert!(!top.item_rect(0).contains(50.0, 40.0));

        preferences.position = ActivityBarPosition::Bottom;
        let bottom = ActivityBarLayout::new(&preferences, 800.0, 600.0, 30.0).unwrap();
        assert!(bottom.item_rect(0).contains(10.0, 550.0));

        preferences.position = ActivityBarPosition::Hidden;
        assert!(ActivityBarLayout::new(&preferences, 800.0, 600.0, 30.0).is_none());
    }

    #[test]
    fn context_menu_toggles_visibility_and_changes_position_and_size() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.open_activity_bar_menu(0.0, 0.0, 800.0, 600.0);

        let rows = menu_rows(0, false);
        assert!(app.handle_activity_bar_menu_click(40.0, rows[1] + 1.0, 800.0));
        assert!(app.activity_bar_preferences.hidden_items.contains("search"));

        app.open_activity_bar_menu(0.0, 0.0, 800.0, 600.0);
        app.handle_activity_bar_menu_click(40.0, rows[4] + 1.0, 800.0);
        let submenu = app.activity_bar_menu.as_ref().unwrap();
        let rect = submenu_rect(submenu, 800.0, 600.0, rows[4], 4);
        app.handle_activity_bar_menu_click(
            rect.x + 40.0,
            rect.y + MENU_PADDING + MENU_ROW * 2.0 + 1.0,
            800.0,
        );
        assert_eq!(
            app.activity_bar_preferences.position,
            ActivityBarPosition::Bottom
        );

        app.open_activity_bar_menu(0.0, 0.0, 800.0, 600.0);
        app.handle_activity_bar_menu_click(40.0, rows[5] + 1.0, 800.0);
        let submenu = app.activity_bar_menu.as_ref().unwrap();
        let rect = submenu_rect(submenu, 800.0, 600.0, rows[5], 2);
        app.handle_activity_bar_menu_click(
            rect.x + 40.0,
            rect.y + MENU_PADDING + MENU_ROW + 1.0,
            800.0,
        );
        assert_eq!(app.activity_bar_preferences.size, ActivityBarSize::Compact);

        app.open_activity_bar_menu(0.0, 0.0, 800.0, 600.0);
        app.handle_activity_bar_menu_click(40.0, menu_rows(0, false)[6] + 11.0, 800.0);
        assert!(app.activity_bar_preferences.primary_sidebar_right);
    }

    #[test]
    fn hiding_last_builtin_activity_is_rejected_and_menu_keyboard_moves_selection() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        for activity in Activity::ALL.into_iter().skip(1) {
            app.activity_bar_preferences
                .hidden_items
                .insert(activity.preference_id().to_owned());
        }
        app.toggle_activity_visibility(Activity::Explorer);
        assert!(!app
            .activity_bar_preferences
            .hidden_items
            .contains("explorer"));
        assert!(app.activity_bar_menu.is_none());

        app.open_activity_bar_menu(0.0, 0.0, 800.0, 600.0);
        assert!(app.activity_bar_menu_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::ArrowDown
        )));
        assert_eq!(app.activity_bar_menu.as_ref().unwrap().selected, 1);
        assert!(app.activity_bar_menu_key(&winit::keyboard::Key::Named(
            winit::keyboard::NamedKey::Escape
        )));
        assert!(app.activity_bar_menu.is_none());
    }

    #[test]
    fn enabled_plugin_activity_entries_are_toggled_without_disabling_the_extension() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.plugins
            .installed_plugins_mut()
            .push(graphite_plugins::store::Installed {
                metadata: graphite_plugins::manifest::Metadata::Native(
                    graphite_plugins::manifest::Manifest {
                        id: "sample.plugin".into(),
                        version: "1.0.0".into(),
                        api_version: 1,
                        main: "main.js".into(),
                        description: "Test plugin".into(),
                        runtime: Default::default(),
                        permissions: vec![],
                        commands: vec![],
                        activity_bar: vec![graphite_plugins::manifest::ActivityBarContainer {
                            id: "sample.plugin.tools".into(),
                            title: "Tools".into(),
                            icon: None,
                            views: vec![graphite_plugins::manifest::ActivityBarView {
                                id: "sample.plugin.items".into(),
                                title: "Items".into(),
                                kind: "tree".into(),
                            }],
                        }],
                        activation_events: vec![],
                        keybindings: vec![],
                        menus: Default::default(),
                        themes: vec![],
                        snippets: vec![],
                        languages: vec![],
                    },
                ),
                enabled: true,
                trusted_version: Some("1.0.0".into()),
                directory: "sample.plugin@1.0.0".into(),
                dev_path: None,
            });
        let entries = app.plugin_activity_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Tools");

        app.open_activity_bar_menu(0.0, 0.0, 800.0, 600.0);
        let y = menu_rows(1, false)[4] + 1.0;
        app.handle_activity_bar_menu_click(40.0, y, 800.0);
        assert!(app
            .activity_bar_preferences
            .hidden_items
            .contains(&entries[0].key));
        assert!(app.plugins.installed_plugins()[0].enabled);

        app.toggle_extension_activity_visibility(&entries[0]);
        app.activate_plugin_activity(entries[0].clone());
        assert_eq!(
            app.active_plugin_activity.as_deref(),
            Some(entries[0].key.as_str())
        );
    }

    #[test]
    fn extension_activity_items_require_enabled_version_trust_and_are_removed_when_uninstalled() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let record = graphite_plugins::store::Installed {
            metadata: graphite_plugins::manifest::Metadata::Vsix(
                graphite_plugins::manifest::Vsix {
                    id: "sample.publisher.extension".into(),
                    version: "2.0.0".into(),
                    description: "VSIX contribution".into(),
                    engines: serde_json::json!({"vscode": "*"}),
                    main: None,
                    contributions: vec![],
                    commands: vec![],
                    activity_bar: vec![graphite_plugins::manifest::ActivityBarContainer {
                        id: "sample.tools".into(),
                        title: "Tools".into(),
                        icon: None,
                        views: vec![graphite_plugins::manifest::ActivityBarView {
                            id: "sample.tools.tree".into(),
                            title: "Tree".into(),
                            kind: "tree".into(),
                        }],
                    }],
                    language_comments: vec![],
                    snippets: vec![],
                    debuggers: vec![],
                    ..Default::default()
                },
            ),
            enabled: true,
            trusted_version: None,
            directory: "sample.publisher.extension@2.0.0".into(),
            dev_path: None,
        };
        app.plugins.installed_plugins_mut().push(record.clone());
        assert!(app.plugin_activity_entries().is_empty());

        app.plugins.installed_plugins_mut()[0].trusted_version = Some("2.0.0".into());
        assert_eq!(app.plugin_activity_entries().len(), 1);
        app.plugins.installed_plugins_mut()[0].enabled = false;
        assert!(app.plugin_activity_entries().is_empty());
        app.plugins.installed_plugins_mut().clear();
        assert!(app.plugin_activity_entries().is_empty());
    }
}
