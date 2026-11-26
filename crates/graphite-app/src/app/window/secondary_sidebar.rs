use super::activity_bar::PluginActivityEntry;
use super::secondary_tabs::{insertion_index_at, layout_icon_tabs, layout_tabs, tab_at, TabSlot};
use super::*;
use crate::app::chat::ChatSurface;
use crate::app::error_log::{self, ErrorSeverity};
use crate::config::SecondaryTabStyle;
use graphite_extension_host::code_oss::Session;
use graphite_plugins::manifest::Metadata;

#[derive(Clone, PartialEq, Eq)]
enum View {
    Chat,
    Outline,
    Extension(String),
}

impl View {
    fn preference_key(&self) -> &str {
        match self {
            Self::Chat => "chat",
            Self::Outline => "outline",
            Self::Extension(key) => key.as_str(),
        }
    }

    fn icon_glyph(&self) -> Option<char> {
        match self {
            Self::Chat => Some('\u{ea6b}'),    // cod-comment
            Self::Outline => Some('\u{ea84}'), // cod-list-tree
            Self::Extension(_) => None,
        }
    }
}

impl App {
    pub(crate) fn raw_secondary_plugin_entries(&self) -> Vec<PluginActivityEntry> {
        let mut context = self.context_keys.resolve(None);
        context.extend(self.vsix_context.clone());
        let matches = |when: Option<&str>| {
            when.is_none_or(|when| {
                crate::keybindings::Condition::parse(when)
                    .is_ok_and(|condition| condition.evaluate(&context))
            })
        };
        let store_root = self
            .plugins
            .store
            .as_ref()
            .map(|store| store.root.as_path());
        self.plugins
            .installed_plugins()
            .iter()
            .filter(|record| crate::app::plugins::vsix_extension_runnable(record))
            .flat_map(|record| {
                let Metadata::Vsix(vsix) = &record.metadata else {
                    return Vec::new();
                };
                let plugin_root =
                    store_root.map(|root| root.join(&record.directory).join("extension"));
                vsix.secondary_sidebar
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .filter(|container| matches(container.when.as_deref()))
                    .filter_map(|container| {
                        let views: Vec<_> = container
                            .views
                            .iter()
                            .filter(|view| matches(view.when.as_deref()))
                            .cloned()
                            .collect();
                        (!views.is_empty()).then(|| PluginActivityEntry {
                            key: format!("plugin:{}:{}", vsix.id, container.id),
                            plugin_id: vsix.id.clone(),
                            container_id: container.id.clone(),
                            title: container.title.clone(),
                            icon: container.icon.clone(),
                            plugin_root: plugin_root.clone(),
                            views,
                        })
                    })
                    .collect()
            })
            .collect()
    }

    pub(crate) fn secondary_plugin_entries(&self) -> Vec<PluginActivityEntry> {
        let mut entries: Vec<PluginActivityEntry> = self
            .raw_secondary_plugin_entries()
            .into_iter()
            .filter(|entry| {
                !self
                    .activity_bar_preferences
                    .moved_to_primary
                    .iter()
                    .any(|k| k == &entry.key)
            })
            .collect();
        for entry in self.raw_primary_plugin_entries() {
            if self
                .activity_bar_preferences
                .moved_to_secondary
                .iter()
                .any(|k| k == &entry.key)
                && !entries.iter().any(|existing| existing.key == entry.key)
            {
                entries.push(entry);
            }
        }
        entries
    }

    pub(crate) fn selected_secondary_plugin(&self) -> Option<PluginActivityEntry> {
        let key = self.secondary_plugin.as_deref()?;
        self.secondary_plugin_entries()
            .into_iter()
            .find(|entry| entry.key == key)
    }

    pub(crate) fn visible_secondary_plugin(&self) -> Option<PluginActivityEntry> {
        (self.secondary_sidebar_width() > 0.0)
            .then(|| self.selected_secondary_plugin())
            .flatten()
    }

    pub(crate) fn sync_secondary_tree_views(&mut self) {
        let views: std::collections::BTreeSet<_> = self
            .visible_secondary_plugin()
            .into_iter()
            .flat_map(|entry| entry.views)
            .filter(|view| view.kind == "tree" && self.is_vsix_tree_view(&view.id))
            .map(|view| view.id)
            .collect();
        let previous = std::mem::replace(&mut self.secondary_trees, views.clone());
        for view in previous.difference(&views) {
            self.notify_vsix_tree_visible(view, false);
        }
        for view in views.difference(&previous) {
            self.notify_vsix_tree_visible(view, true);
            self.request_vsix_tree_children(view, None);
        }
    }

    pub(crate) fn set_secondary_sidebar_visible(&mut self, visible: bool) {
        self.view_preferences.secondary_sidebar_visible = visible;
        if visible {
            self.activate_secondary_plugin();
        } else if self.chat.surface == ChatSurface::SideBar {
            self.chat.focused = false;
        }
    }

    pub(crate) fn open_secondary_extension_view(&mut self, view_id: &str) -> bool {
        let Some(entry) = self
            .secondary_plugin_entries()
            .into_iter()
            .find(|entry| entry.views.iter().any(|view| view.id == view_id))
        else {
            return false;
        };
        self.select_secondary_view(View::Extension(entry.key));
        true
    }

    pub(crate) fn activate_secondary_plugin(&mut self) {
        let Some(entry) = self.selected_secondary_plugin() else {
            return;
        };
        if self.secondary_sidebar_width() <= 0.0 || !self.vsix_workspace_trusted() {
            return;
        }
        if self.plugins.vsix.is_none() {
            let Some(path) = Session::bundled_runtime() else {
                return;
            };
            if let Err(error) = self.start_vsix_host(Some(path)) {
                error_log::record(
                    ErrorSeverity::Medium,
                    "plugins.secondary_sidebar",
                    "Could not start the VSIX host.",
                );
                self.status = error;
                return;
            }
        }
        for view in &entry.views {
            self.trigger_vsix_activation_event(&format!("onView:{}", view.id));
        }
    }

    fn selected_secondary_view(&self) -> Option<View> {
        if let Some(entry) = self.selected_secondary_plugin() {
            Some(View::Extension(entry.key))
        } else if self.chat.visible && self.chat.surface == ChatSurface::SideBar {
            Some(View::Chat)
        } else if self.outline_visible {
            Some(View::Outline)
        } else {
            None
        }
    }

    fn secondary_views(&self) -> Vec<(View, String)> {
        let mut views = vec![(View::Chat, self.t("secondary_sidebar.chat").into())];
        views.extend(
            self.secondary_plugin_entries()
                .into_iter()
                .map(|entry| (View::Extension(entry.key), entry.title)),
        );
        crate::app::bar_drag::sort_by_order(
            views,
            &self.activity_bar_preferences.secondary_order,
            |(view, _)| view.preference_key(),
        )
    }

    pub(crate) fn secondary_view_keys(&self) -> Vec<String> {
        self.secondary_views()
            .into_iter()
            .map(|(view, _)| view.preference_key().to_owned())
            .collect()
    }

    /// The tabs of the header strip that fit in the secondary side bar.
    fn secondary_tab_slots(&self, cw: f32, lh: f32) -> Vec<TabSlot> {
        let views = self.secondary_views();
        self.layout_secondary_views(&views, cw, lh)
    }

    fn previewed_secondary_views(&self) -> Vec<(View, String)> {
        crate::app::bar_drag::apply_bar_preview(
            self.secondary_views(),
            crate::app::bar_drag::BarKind::Secondary,
            self.bar_drag.as_ref(),
            |(view, _)| view.preference_key(),
            |key| {
                self.raw_primary_plugin_entries()
                    .into_iter()
                    .chain(self.raw_secondary_plugin_entries())
                    .find(|entry| entry.key == key)
                    .map(|entry| (View::Extension(entry.key), entry.title))
            },
        )
    }

    fn layout_secondary_views(&self, views: &[(View, String)], cw: f32, lh: f32) -> Vec<TabSlot> {
        let active = self.selected_secondary_view();
        let active = active.and_then(|active| views.iter().position(|(view, _)| *view == active));
        match self.secondary_tab_style() {
            SecondaryTabStyle::Text => {
                let labels: Vec<usize> = views
                    .iter()
                    .map(|(_, label)| label.chars().count())
                    .collect();
                layout_tabs(
                    &labels,
                    active,
                    self.secondary_sidebar_width(),
                    cw.max(1.0),
                    PAD,
                )
            }
            SecondaryTabStyle::Icons => {
                let tab_width = (lh * 1.7).max(26.0);
                layout_icon_tabs(
                    views.len(),
                    tab_width,
                    active,
                    self.secondary_sidebar_width(),
                )
            }
            SecondaryTabStyle::IconsAndText => {
                let icon_chars = ((lh * 1.1) / cw.max(1.0)).ceil() as usize + 1;
                let labels: Vec<usize> = views
                    .iter()
                    .map(|(_, label)| label.chars().count() + icon_chars)
                    .collect();
                layout_tabs(
                    &labels,
                    active,
                    self.secondary_sidebar_width(),
                    cw.max(1.0),
                    PAD,
                )
            }
        }
    }

    pub(crate) fn secondary_tab_press_target(
        &self,
        mx: f32,
        my: f32,
    ) -> Option<(String, bool, String, Option<char>)> {
        let gpu = self.gpu.as_ref()?;
        let (w, _h) = gpu.renderer.size();
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let left = self.secondary_sidebar_x(w);
        let width = self.secondary_sidebar_width();
        let top = self.activity_bar_vertical_offset();
        let body = self.secondary_sidebar_top(lh);
        if width <= 0.0 || mx < left || mx >= left + width || my < top || my >= body {
            return None;
        }
        let slots = self.secondary_tab_slots(cw, lh);
        let index = tab_at(&slots, mx - left)?;
        let (view, label) = self.secondary_views().into_iter().nth(index)?;
        let movable = matches!(view, View::Extension(_));
        let icon = view.icon_glyph();
        Some((view.preference_key().to_owned(), movable, label, icon))
    }

    pub(crate) fn secondary_tab_drop_index(&self, mx: f32, my: f32) -> Option<usize> {
        let gpu = self.gpu.as_ref()?;
        let (w, _h) = gpu.renderer.size();
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let left = self.secondary_sidebar_x(w);
        let width = self.secondary_sidebar_width();
        let top = self.activity_bar_vertical_offset();
        let body = self.secondary_sidebar_top(lh);
        if width <= 0.0 || mx < left || mx >= left + width || my < top || my >= body {
            return None;
        }
        let views = self.secondary_views();
        let slots = self.secondary_tab_slots(cw, lh);
        Some(insertion_index_at(&slots, mx - left, views.len()))
    }

    pub(crate) fn activate_secondary_view_by_key(&mut self, key: &str) {
        if let Some((view, _)) = self
            .secondary_views()
            .into_iter()
            .find(|(view, _)| view.preference_key() == key)
        {
            self.select_secondary_view(view);
        }
    }

    fn select_secondary_view(&mut self, view: View) {
        if self.chat.surface == ChatSurface::SideBar {
            self.chat.visible = false;
            self.chat.focused = false;
        }
        self.outline_visible = view == View::Outline;
        self.secondary_plugin = None;
        match view {
            View::Chat => self.open_chat_sidebar(),
            View::Outline => self.set_secondary_sidebar_visible(true),
            View::Extension(key) => {
                self.secondary_plugin = Some(key);
                self.set_secondary_sidebar_visible(true);
            }
        }
        self.save_view_preferences_for_command();
    }

    pub(crate) fn cycle_secondary_view(&mut self, reverse: bool) {
        let views = self.secondary_views();
        let current = self.selected_secondary_view();
        let index = current.and_then(|current| views.iter().position(|(view, _)| *view == current));
        let next = match index {
            Some(index) if reverse => (index + views.len() - 1) % views.len(),
            Some(index) => (index + 1) % views.len(),
            None if reverse => views.len() - 1,
            None => 0,
        };
        self.select_secondary_view(views[next].0.clone());
    }

    pub(crate) fn secondary_sidebar_x(&self, width: f32) -> f32 {
        if self.activity_bar_preferences.primary_sidebar_right {
            self.activity_bar_width_px()
        } else {
            width - self.secondary_sidebar_width()
        }
    }

    pub(crate) fn secondary_sidebar_top(&self, lh: f32) -> f32 {
        self.activity_bar_vertical_offset() + lh * 1.65
    }

    pub(crate) fn draw_secondary_sidebar(
        &self,
        r: &mut Renderer,
        w: f32,
        h: f32,
        lh: f32,
        cw: f32,
    ) {
        let width = self.secondary_sidebar_width();
        if width <= 0.0 {
            return;
        }
        let x = self.secondary_sidebar_x(w);
        let top = self.activity_bar_vertical_offset();
        let body = self.secondary_sidebar_top(lh);
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0xf3f3f3));
        let fg = rgb(Theme::color(Theme::TEXT, 0x1e1e1e));
        let dim = rgb(Theme::color(Theme::DIM, 0x616161));
        let max_chars = ((width - PAD * 2.0) / cw.max(1.0)).max(1.0) as usize;
        let draw_label = |r: &mut Renderer, label: &str, y: f32, color| {
            r.draw_text(
                &label.chars().take(max_chars).collect::<String>(),
                x + PAD,
                y,
                color,
            );
        };
        let selected = self.selected_secondary_view();
        let views = self.previewed_secondary_views();
        let dragged_key = self
            .bar_drag
            .as_ref()
            .filter(|d| {
                d.active
                    && d.target
                        .is_some_and(|t| t.bar == crate::app::bar_drag::BarKind::Secondary)
            })
            .map(|d| d.source_key.as_str());

        r.draw_rect(x, top, width, (h - lh - top).max(0.0), bg);
        // A thin border on the edge that faces the editor, like the other blocks have.
        let edge = if self.activity_bar_preferences.primary_sidebar_right {
            x + width - 1.0
        } else {
            x
        };
        r.draw_rect(
            edge,
            top,
            1.0,
            (h - lh - top).max(0.0),
            rgb(Theme::color(0x2b2b2b, 0xe5e5e5)),
        );
        r.draw_rect(
            x,
            top,
            width,
            body - top,
            rgb(Theme::color(Theme::ACTIVE_ROW, 0xd6ebff)),
        );
        let tab_style = self.secondary_tab_style();
        let slots = self.layout_secondary_views(&views, cw, lh);
        let plugin_entries = self.secondary_plugin_entries();
        let mut hovered_tooltip: Option<(f32, String)> = None;
        for slot in &slots {
            let Some((view, label)) = views.get(slot.index) else {
                continue;
            };
            let active = Some(view) == selected.as_ref();
            let tab_x = x + slot.x;
            let is_preview_target = dragged_key == Some(view.preference_key());
            if is_preview_target {
                r.draw_rect(
                    tab_x + 1.0,
                    top + 2.0,
                    (slot.width - 2.0).max(0.0),
                    (body - top - 4.0).max(0.0),
                    rgb(Theme::color(Theme::BG, 0xffffff)),
                );
            }
            let tab_hovered = self.mouse.x as f32 >= tab_x
                && (self.mouse.x as f32) < tab_x + slot.width
                && self.mouse.y as f32 >= top
                && (self.mouse.y as f32) < body;
            let color = if active { fg } else { dim };
            let icon_size = (lh * 0.85).clamp(14.0, 18.0);
            let resolved_plugin_icon = match view {
                View::Extension(key) => plugin_entries
                    .iter()
                    .find(|entry| &entry.key == key)
                    .and_then(|entry| {
                        self.plugins.activity_bar_icons.get_or_load(
                            &entry.plugin_id,
                            entry.plugin_root.as_deref(),
                            entry.icon.as_deref(),
                        )
                    }),
                View::Chat | View::Outline => None,
            };
            match tab_style {
                SecondaryTabStyle::Text => {
                    let chars = ((slot.width - PAD * 2.0) / cw.max(1.0)).max(0.0) as usize;
                    r.draw_text(
                        &label.chars().take(chars).collect::<String>(),
                        tab_x + PAD,
                        top + lh * 0.25,
                        color,
                    );
                }
                SecondaryTabStyle::Icons => {
                    if tab_hovered {
                        hovered_tooltip = Some((tab_x, label.clone()));
                    }
                    if let Some(glyph) = view.icon_glyph() {
                        r.draw_icon(
                            glyph,
                            tab_x + (slot.width - icon_size) * 0.5,
                            top + (body - top - icon_size) * 0.5,
                            icon_size,
                            color,
                        );
                    } else {
                        super::activity_bar_icons::draw_plugin_activity_icon(
                            r,
                            resolved_plugin_icon.as_ref(),
                            label,
                            tab_x,
                            top,
                            slot.width,
                            body - top,
                            icon_size,
                            color,
                        );
                    }
                }
                SecondaryTabStyle::IconsAndText => {
                    let icon_x = tab_x + PAD;
                    let icon_y = top + (body - top - icon_size) * 0.5;
                    if let Some(glyph) = view.icon_glyph() {
                        r.draw_icon(glyph, icon_x, icon_y, icon_size, color);
                    } else {
                        super::activity_bar_icons::draw_plugin_activity_icon(
                            r,
                            resolved_plugin_icon.as_ref(),
                            label,
                            icon_x,
                            top,
                            icon_size,
                            body - top,
                            icon_size,
                            color,
                        );
                    }
                    let text_x = icon_x + icon_size + cw * 0.6;
                    let chars =
                        ((tab_x + slot.width - PAD - text_x) / cw.max(1.0)).max(0.0) as usize;
                    r.draw_text(
                        &label.chars().take(chars).collect::<String>(),
                        text_x,
                        top + lh * 0.25,
                        color,
                    );
                }
            }
            if active {
                r.draw_rect(
                    tab_x,
                    body - 2.0,
                    slot.width,
                    2.0,
                    rgb(Theme::color(Theme::STATUS, 0x005fb8)),
                );
            }
        }
        if let Some(drag) = &self.bar_drag {
            if drag.active {
                if let Some(target) = drag.target {
                    if target.bar == crate::app::bar_drag::BarKind::Secondary {
                        let indicator_x = slots
                            .iter()
                            .find(|slot| slot.index == target.index)
                            .map(|slot| x + slot.x)
                            .or_else(|| slots.last().map(|slot| x + slot.x + slot.width))
                            .unwrap_or(x);
                        r.draw_rect(
                            (indicator_x - 1.0).max(x),
                            top + 3.0,
                            2.0,
                            (body - top - 6.0).max(4.0),
                            rgb(Theme::color(Theme::STATUS, 0x007acc)),
                        );
                    }
                }
            }
        }
        if let Some((tip_x, tip_text)) = hovered_tooltip {
            if self.bar_drag.as_ref().is_none_or(|drag| !drag.active) {
                let tip_w = (tip_text.chars().count() as f32 * cw + PAD * 2.0).min(width);
                let tip_x = tip_x.clamp(x, (x + width - tip_w).max(x));
                let tip_y = body + 2.0;
                r.draw_rect(
                    tip_x,
                    tip_y,
                    tip_w,
                    lh * 1.2,
                    rgb(Theme::color(Theme::SIDEBAR, 0x252526)),
                );
                r.draw_text(&tip_text, tip_x + PAD, tip_y + lh * 0.1, fg);
            }
        }
        if let Some(entry) = self.selected_secondary_plugin() {
            let message = if !self.vsix_workspace_trusted() {
                Some(self.t("secondary_sidebar.trust"))
            } else if self.plugins.vsix.is_none() {
                Some(if Session::bundled_runtime().is_some() {
                    self.t("secondary_sidebar.host_unavailable")
                } else {
                    self.t("secondary_sidebar.runtime_missing")
                })
            } else {
                None
            };
            if let Some(message) = message {
                for (index, line) in wrap_sidebar_message(message, max_chars)
                    .into_iter()
                    .enumerate()
                {
                    draw_label(r, &line, body + (index as f32 + 0.4) * lh, dim);
                }
            } else {
                r.push_translation(x - ACTIVITY_BAR_WIDTH, body);
                self.draw_plugin_container(
                    r,
                    &entry,
                    h - body,
                    h - (body - top),
                    width,
                    lh,
                    cw,
                    entry.only_webviews(),
                );
                r.pop_translation();
            }
        } else if selected.is_none() {
            draw_label(r, self.t("secondary_sidebar.empty"), body + lh * 0.4, dim);
        }
    }

    pub(crate) fn secondary_sidebar_click(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        lh: f32,
        cw: f32,
    ) -> bool {
        let left = self.secondary_sidebar_x(w);
        let width = self.secondary_sidebar_width();
        let top = self.activity_bar_vertical_offset();
        let body = self.secondary_sidebar_top(lh);
        if width <= 0.0 || x < left || x >= left + width || y < top || y >= h - lh {
            return false;
        }
        if y < body {
            let slots = self.secondary_tab_slots(cw, lh);
            if let Some((view, _)) = tab_at(&slots, x - left)
                .and_then(|index| self.secondary_views().into_iter().nth(index))
            {
                self.select_secondary_view(view);
            }
            return true;
        }
        if let Some(entry) = self.selected_secondary_plugin() {
            if self.vsix_workspace_trusted() {
                self.plugin_tree_click(&entry, y - body, lh);
            }
            true
        } else {
            self.selected_secondary_view().is_none()
        }
    }
}

fn wrap_sidebar_message(message: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in message.split_whitespace() {
        let line = lines.last_mut().unwrap();
        if line.is_empty() {
            line.push_str(word);
        } else if line.chars().count() + word.chars().count() < width {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(word.to_owned());
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_opens_empty_secondary_sidebar_and_reserves_the_opposite_side() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let (w, h, lh) = (1200.0, 800.0, 20.0);
        let before = app.group_rectangles(w, h, lh)[0];
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        let after = app.group_rectangles(w, h, lh)[0];
        assert!(app.selected_secondary_view().is_none());
        assert_eq!(before.width - after.width, app.secondary_sidebar_width());
        assert_eq!(after.x + after.width, app.secondary_sidebar_x(w));

        app.activity_bar_preferences.primary_sidebar_right = true;
        let mirrored = app.group_rectangles(w, h, lh)[0];
        assert_eq!(
            mirrored.x,
            app.secondary_sidebar_x(w) + app.secondary_sidebar_width()
        );

        assert_eq!(app.secondary_tab_slots(10.0, lh).len(), 1);
        app.run_ui_command("workbench.action.toggleOutline");
        assert!(app.selected_secondary_view() == Some(View::Outline));
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        assert_eq!(app.secondary_sidebar_width(), 0.0);
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        assert!(app.selected_secondary_view() == Some(View::Outline));
    }

    #[test]
    fn dragging_the_secondary_sidebar_edge_resizes_within_limits_on_both_sides() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.sidebar_window_width_px = 1200.0;
        let w = 1200.0;
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        assert_eq!(app.secondary_sidebar_width(), 320.0);
        let y = app.activity_bar_vertical_offset() + 4.0;
        assert!(app.is_over_secondary_resize_handle(w - 320.0, y));
        assert!(!app.is_over_secondary_resize_handle(w - 320.0 - 40.0, y));

        app.resize_secondary_sidebar_from_pointer(w - 500.0);
        assert_eq!(app.secondary_sidebar_width(), 500.0);
        app.resize_secondary_sidebar_from_pointer(w - 20.0);
        assert_eq!(
            app.secondary_sidebar_width(),
            config::MIN_SECONDARY_SIDEBAR_WIDTH
        );
        // As wide as the window allows: the primary side bar narrows to its minimum and the
        // editor keeps its own.
        app.resize_secondary_sidebar_from_pointer(10.0);
        assert_eq!(
            app.secondary_sidebar_width(),
            app.secondary_sidebar_max_width()
        );
        assert_eq!(app.secondary_sidebar_width(), 652.0);

        app.activity_bar_preferences.primary_sidebar_right = true;
        let left = app.secondary_sidebar_x(w);
        app.resize_secondary_sidebar_from_pointer(left + 400.0);
        assert_eq!(app.secondary_sidebar_width(), 400.0);
        assert!(app.is_over_secondary_resize_handle(left + 400.0, y));
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        assert_eq!(app.secondary_sidebar_width(), 0.0);
        assert!(!app.is_over_secondary_resize_handle(left + 400.0, y));
    }

    /// Width of the editor column for the current sizes.
    fn editor_column_width(app: &App, w: f32) -> f32 {
        let (left, right) = app.editor_column(w);
        right - left
    }

    #[test]
    fn the_side_bar_being_shaped_takes_the_space_and_its_neighbour_gives_way() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let w = 1400.0;
        app.sidebar_window_width_px = w;
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        let activity = app.activity_bar_width_px();
        let min_editor = crate::app::layout::MIN_EDITOR_WIDTH;

        // Shaping the secondary side bar: the primary one gives way, down to its minimum.
        app.resize_secondary_sidebar_from_pointer(w - 700.0);
        assert_eq!(app.secondary_sidebar_width(), 700.0);
        assert!(editor_column_width(&app, w) >= min_editor);
        app.resize_secondary_sidebar_from_pointer(w - 1000.0);
        assert!(app.secondary_sidebar_width() < 1000.0);
        let primary = app.sidebar_width_px() - activity;
        assert!(primary >= config::MIN_SIDEBAR_WIDTH - 0.01, "{primary}");
        assert!((editor_column_width(&app, w) - min_editor).abs() < 0.01);

        // Shaping the primary one: it gets what it asks for and the secondary one gives way.
        app.resize_sidebar_from_pointer(activity + 700.0);
        assert!((app.sidebar_width_px() - activity - 700.0).abs() < 0.01);
        assert!(app.secondary_sidebar_width() >= config::MIN_SECONDARY_SIDEBAR_WIDTH);
        assert!(app.secondary_sidebar_width() < 700.0);
        assert!(editor_column_width(&app, w) >= min_editor - 0.01);

        // Back to the secondary one: it gets its width again and the primary one yields.
        app.resize_secondary_sidebar_from_pointer(w - 500.0);
        assert_eq!(app.secondary_sidebar_width(), 500.0);
        assert!(editor_column_width(&app, w) >= min_editor);
    }

    #[test]
    fn the_blocks_never_overlap_while_either_side_bar_is_dragged_across_its_range() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let w = 1000.0;
        app.sidebar_window_width_px = w;
        app.run_ui_command("graphite.layout.toggleSecondarySidebar");
        let min_editor = crate::app::layout::MIN_EDITOR_WIDTH;
        for pointer in (0..=1000).step_by(50) {
            for secondary in [true, false] {
                if secondary {
                    app.resize_secondary_sidebar_from_pointer(pointer as f32);
                } else {
                    app.resize_sidebar_from_pointer(pointer as f32);
                }
                let (left, right) = app.editor_column(w);
                assert!(left <= right && right <= w, "pointer {pointer}");
                assert!(
                    right - left >= min_editor - 0.01,
                    "pointer {pointer} secondary {secondary}: {}",
                    right - left
                );
                assert!(app.secondary_sidebar_x(w) >= right - 0.01);
            }
        }
    }
}
