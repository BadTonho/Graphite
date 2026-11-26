//! Right-click context menu for plugin cards in the Plugins sidebar.

use super::layout::PluginsTab;
use super::*;
use crate::app::explorer_context_menu::place_context_menu;
use winit::keyboard::{Key, NamedKey};

const ROW_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const PADDING: f32 = 6.0;
const MIN_MENU_WIDTH: f32 = 250.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PluginContextTarget {
    Installed(String),
    OpenVsx(usize),
    Graphite(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PluginContextAction {
    Enable,
    Disable,
    Restart,
    Update,
    Uninstall,
    RunCommand,
    Details,
    Inspect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PluginContextMenuRow {
    Action {
        label_key: &'static str,
        shortcut: &'static str,
        action: PluginContextAction,
        enabled: bool,
    },
    Separator,
}

impl PluginContextMenuRow {
    fn is_selectable(&self) -> bool {
        matches!(self, Self::Action { enabled: true, .. })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PluginContextMenu {
    pub target: PluginContextTarget,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub selected: usize,
    pub rows: Vec<PluginContextMenuRow>,
}

fn row_height(row: &PluginContextMenuRow) -> f32 {
    if matches!(row, PluginContextMenuRow::Separator) {
        SEPARATOR_HEIGHT
    } else {
        ROW_HEIGHT
    }
}

fn menu_height(rows: &[PluginContextMenuRow]) -> f32 {
    PADDING * 2.0 + rows.iter().map(row_height).sum::<f32>()
}

fn menu_rect(menu: &PluginContextMenu) -> (f32, f32, f32, f32) {
    (menu.x, menu.y, menu.width, menu_height(&menu.rows))
}

fn first_selectable(rows: &[PluginContextMenuRow]) -> usize {
    rows.iter()
        .position(PluginContextMenuRow::is_selectable)
        .unwrap_or(0)
}

fn move_selection(rows: &[PluginContextMenuRow], current: usize, delta: isize) -> usize {
    if rows.is_empty() {
        return 0;
    }
    let len = rows.len() as isize;
    let mut index = current as isize;
    for _ in 0..rows.len() {
        index = (index + delta).rem_euclid(len);
        if rows[index as usize].is_selectable() {
            return index as usize;
        }
    }
    current
}

fn row_at_y(menu: &PluginContextMenu, my: f32) -> Option<usize> {
    let mut top = menu.y + PADDING;
    for (index, row) in menu.rows.iter().enumerate() {
        let height = row_height(row);
        if my >= top && my < top + height {
            return row.is_selectable().then_some(index);
        }
        top += height;
    }
    None
}

impl App {
    pub(crate) fn plugin_right_click(&mut self, mx: f32, my: f32) -> bool {
        let Some(gpu) = &self.gpu else {
            return false;
        };
        if !self.plugins.visible
            || self.zen_mode
            || self.plugins.worker.is_some()
            || self.plugins.bisect.is_some()
            || self.plugin_details_open()
            || self.plugin_section_collapsed()
        {
            return false;
        }
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width().max(6.0);
        let (window_width, window_height) = gpu.renderer.size();
        if !self.primary_sidebar_contains(mx, my, window_width, window_height, lh) {
            return false;
        }
        let layout = self.plugin_layout(window_height, lh);
        let x = self.sidebar_local_x(mx, window_width);
        let y = self.sidebar_local_y(my);
        let count = self.plugin_row_count();
        let scroll = self
            .plugins
            .scroll
            .min(count.saturating_sub(layout.visible_rows()));
        let Some(index) = layout.card_index(x, y, scroll, count) else {
            return false;
        };
        self.plugins.index = index;
        self.focus_plugins();
        self.plugins.editing = false;
        self.dismiss_plugin_menu();
        self.open_plugin_context_menu(index, (mx, my), (window_width, window_height), lh, cw)
    }

    pub(super) fn open_plugin_context_menu(
        &mut self,
        index: usize,
        anchor: (f32, f32),
        window: (f32, f32),
        lh: f32,
        cw: f32,
    ) -> bool {
        let (target, rows) = match self.plugins.tab {
            PluginsTab::Installed => {
                let Some(record) = self.plugins.installed.get(index) else {
                    return false;
                };
                let id = record.metadata.id().to_owned();
                let runnable =
                    !matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.main.is_none());
                let is_vsix = matches!(record.metadata, Metadata::Vsix(_));
                let has_command = self
                    .plugins
                    .hosts
                    .get(&id)
                    .is_some_and(|host| !host.commands.is_empty());
                let mut rows = Vec::new();
                if record.enabled {
                    rows.push(PluginContextMenuRow::Action {
                        label_key: "plugins.action.disable",
                        shortcut: "D",
                        action: PluginContextAction::Disable,
                        enabled: true,
                    });
                    if runnable {
                        rows.push(PluginContextMenuRow::Action {
                            label_key: "plugins.action.restart",
                            shortcut: "R",
                            action: PluginContextAction::Restart,
                            enabled: true,
                        });
                    }
                    if has_command {
                        rows.push(PluginContextMenuRow::Action {
                            label_key: "plugins.action.run_command",
                            shortcut: "C",
                            action: PluginContextAction::RunCommand,
                            enabled: true,
                        });
                    }
                } else {
                    let label_key =
                        if record.trusted_version.as_deref() != Some(record.metadata.version()) {
                            "plugins.action.trust_and_enable"
                        } else {
                            "plugins.action.enable"
                        };
                    rows.push(PluginContextMenuRow::Action {
                        label_key,
                        shortcut: "E",
                        action: PluginContextAction::Enable,
                        enabled: runnable,
                    });
                }
                rows.push(PluginContextMenuRow::Separator);
                if is_vsix {
                    rows.push(PluginContextMenuRow::Action {
                        label_key: "plugins.action.update",
                        shortcut: "U",
                        action: PluginContextAction::Update,
                        enabled: true,
                    });
                }
                rows.push(PluginContextMenuRow::Action {
                    label_key: "plugins.action.uninstall",
                    shortcut: "X",
                    action: PluginContextAction::Uninstall,
                    enabled: true,
                });
                rows.push(PluginContextMenuRow::Separator);
                rows.push(PluginContextMenuRow::Action {
                    label_key: "plugins.action.details",
                    shortcut: "Enter",
                    action: PluginContextAction::Details,
                    enabled: true,
                });
                (PluginContextTarget::Installed(id), rows)
            }
            PluginsTab::OpenVsx => {
                if index >= self.plugins.open_vsx.len() {
                    return false;
                }
                (
                    PluginContextTarget::OpenVsx(index),
                    vec![PluginContextMenuRow::Action {
                        label_key: "plugins.action.inspect",
                        shortcut: "Enter",
                        action: PluginContextAction::Inspect,
                        enabled: true,
                    }],
                )
            }
            PluginsTab::Graphite => {
                if index >= self.plugins.graphite.len() {
                    return false;
                }
                (
                    PluginContextTarget::Graphite(index),
                    vec![PluginContextMenuRow::Action {
                        label_key: "plugins.action.details",
                        shortcut: "Enter",
                        action: PluginContextAction::Details,
                        enabled: true,
                    }],
                )
            }
        };

        let width = rows
            .iter()
            .filter_map(|row| match row {
                PluginContextMenuRow::Action {
                    label_key,
                    shortcut,
                    ..
                } => Some(
                    (self.t(label_key).chars().count() + shortcut.chars().count() + 6) as f32 * cw
                        + 28.0,
                ),
                PluginContextMenuRow::Separator => None,
            })
            .fold(MIN_MENU_WIDTH, f32::max);
        let height = menu_height(&rows);
        let bottom_inset = if self.view_preferences.status_bar_visible && !self.zen_mode {
            lh
        } else {
            0.0
        };
        let (x, y) = place_context_menu(anchor, (width, height), window, bottom_inset);
        let selected = first_selectable(&rows);
        self.plugins.context_menu = Some(PluginContextMenu {
            target,
            x,
            y,
            width,
            selected,
            rows,
        });
        true
    }

    pub(crate) fn plugin_context_menu_contains(&self, mx: f32, my: f32) -> bool {
        self.plugins.context_menu.as_ref().is_some_and(|menu| {
            let (x, y, width, height) = menu_rect(menu);
            mx >= x && mx < x + width && my >= y && my < y + height
        })
    }

    pub(crate) fn plugin_context_menu_pointer_moved(&mut self, mx: f32, my: f32) {
        let Some(menu) = self.plugins.context_menu.as_mut() else {
            return;
        };
        let (x, y, width, height) = menu_rect(menu);
        if mx >= x && mx < x + width && my >= y && my < y + height {
            if let Some(row) = row_at_y(menu, my) {
                menu.selected = row;
            }
        }
    }

    pub(crate) fn plugin_context_menu_click(&mut self, mx: f32, my: f32) {
        let Some(menu) = self.plugins.context_menu.clone() else {
            return;
        };
        let (x, y, width, height) = menu_rect(&menu);
        if mx < x || mx >= x + width || my < y || my >= y + height {
            self.plugins.context_menu = None;
            return;
        }
        let Some(row_index) = row_at_y(&menu, my) else {
            return;
        };
        let Some(PluginContextMenuRow::Action {
            action,
            enabled: true,
            ..
        }) = menu.rows.get(row_index)
        else {
            return;
        };
        self.plugins.context_menu = None;
        self.execute_plugin_context_action(&menu.target, *action);
    }

    pub(crate) fn plugin_context_menu_key(&mut self, key: &Key) -> bool {
        let Some(menu) = self.plugins.context_menu.as_mut() else {
            return false;
        };
        match key {
            Key::Named(NamedKey::Escape) => {
                self.plugins.context_menu = None;
            }
            Key::Named(NamedKey::ArrowDown) => {
                menu.selected = move_selection(&menu.rows, menu.selected, 1);
            }
            Key::Named(NamedKey::ArrowUp) => {
                menu.selected = move_selection(&menu.rows, menu.selected, -1);
            }
            Key::Named(NamedKey::Enter) => {
                let target = menu.target.clone();
                let action = menu.rows.get(menu.selected).and_then(|row| match row {
                    PluginContextMenuRow::Action {
                        action,
                        enabled: true,
                        ..
                    } => Some(*action),
                    _ => None,
                });
                self.plugins.context_menu = None;
                if let Some(action) = action {
                    self.execute_plugin_context_action(&target, action);
                }
            }
            _ => {}
        }
        true
    }

    fn execute_plugin_context_action(
        &mut self,
        target: &PluginContextTarget,
        action: PluginContextAction,
    ) {
        match target {
            PluginContextTarget::Installed(id) => {
                let key = match action {
                    PluginContextAction::Enable => 'e',
                    PluginContextAction::Disable => 'd',
                    PluginContextAction::Restart => 'r',
                    PluginContextAction::Update => 'u',
                    PluginContextAction::Uninstall => 'x',
                    PluginContextAction::RunCommand => 'c',
                    PluginContextAction::Details => '\n',
                    PluginContextAction::Inspect => return,
                };
                self.installed_plugin_action_confirmed(id, key);
            }
            PluginContextTarget::OpenVsx(index) | PluginContextTarget::Graphite(index) => {
                self.plugins.index = *index;
                self.plugin_action('\n');
            }
        }
    }

    pub(crate) fn draw_plugin_context_menu(&self, renderer: &mut Renderer) {
        let Some(menu) = &self.plugins.context_menu else {
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
                PluginContextMenuRow::Separator => {
                    renderer.draw_rect(x + 1.0, top + 4.0, width - 2.0, 1.0, border);
                }
                PluginContextMenuRow::Action {
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
            }
            top += row_h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphite_plugins::manifest::{Metadata, Vsix};

    fn sample_vsix(id: &str, enabled: bool, trusted_version: Option<&str>) -> Installed {
        Installed {
            directory: id.to_owned(),
            enabled,
            trusted_version: trusted_version.map(str::to_owned),
            dev_path: None,
            metadata: Metadata::Vsix(Vsix {
                id: id.to_owned(),
                version: "1.0.0".to_owned(),
                main: Some("./extension.js".to_owned()),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn installed_disabled_plugin_context_menu_offers_trust_and_enable() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().into(), None, true);
        app.plugins.store = Some(graphite_plugins::store::Store::new(
            temp.path().join("plugins"),
        ));
        app.plugins.tab = PluginsTab::Installed;
        app.plugins.installed = vec![sample_vsix("google.google-antigravity", false, None)];

        assert!(app.open_plugin_context_menu(0, (40.0, 120.0), (1200.0, 760.0), 18.0, 8.0));
        let menu = app.plugins.context_menu.as_ref().expect("menu opened");
        let actions: Vec<PluginContextAction> = menu
            .rows
            .iter()
            .filter_map(|row| match row {
                PluginContextMenuRow::Action { action, .. } => Some(*action),
                PluginContextMenuRow::Separator => None,
            })
            .collect();
        assert_eq!(
            actions,
            vec![
                PluginContextAction::Enable,
                PluginContextAction::Update,
                PluginContextAction::Uninstall,
                PluginContextAction::Details,
            ]
        );
    }

    #[test]
    fn installed_enabled_plugin_context_menu_offers_disable_restart_and_uninstall() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().into(), None, true);
        app.plugins.store = Some(graphite_plugins::store::Store::new(
            temp.path().join("plugins"),
        ));
        app.plugins.tab = PluginsTab::Installed;
        app.plugins.installed = vec![sample_vsix(
            "google.google-antigravity",
            true,
            Some("1.0.0"),
        )];

        assert!(app.open_plugin_context_menu(0, (40.0, 120.0), (1200.0, 760.0), 18.0, 8.0));
        let menu = app.plugins.context_menu.as_ref().expect("menu opened");
        let actions: Vec<PluginContextAction> = menu
            .rows
            .iter()
            .filter_map(|row| match row {
                PluginContextMenuRow::Action { action, .. } => Some(*action),
                PluginContextMenuRow::Separator => None,
            })
            .collect();
        assert_eq!(
            actions,
            vec![
                PluginContextAction::Disable,
                PluginContextAction::Restart,
                PluginContextAction::Update,
                PluginContextAction::Uninstall,
                PluginContextAction::Details,
            ]
        );
    }

    #[test]
    fn clicking_enable_in_context_menu_enables_directly_without_reprompting_trust() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().into(), None, true);
        app.plugins.store = Some(graphite_plugins::store::Store::new(
            temp.path().join("plugins"),
        ));
        app.plugins.tab = PluginsTab::Installed;
        app.plugins.installed = vec![sample_vsix("google.google-antigravity", false, None)];

        assert!(app.open_plugin_context_menu(0, (40.0, 120.0), (1200.0, 760.0), 18.0, 8.0));
        assert!(app.plugin_context_menu_key(&Key::Named(NamedKey::Enter)));
        assert!(app.plugins.context_menu.is_none());
        assert!(app.plugins.trust.is_none());
        assert!(app.plugins.worker.is_some());
    }
}
