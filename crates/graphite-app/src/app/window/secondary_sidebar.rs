use super::activity_bar::PluginActivityEntry;
use super::secondary_tabs::{layout_tabs, tab_at, TabSlot};
use super::*;
use crate::app::chat::ChatSurface;
use crate::app::error_log::{self, ErrorSeverity};
use graphite_extension_host::code_oss::Session;
use graphite_plugins::manifest::Metadata;

#[derive(Clone, PartialEq, Eq)]
enum View {
    Chat,
    Outline,
    Extension(String),
}

impl App {
    pub(crate) fn secondary_plugin_entries(&self) -> Vec<PluginActivityEntry> {
        let mut context = self.context_keys.resolve(None);
        context.extend(self.vsix_context.clone());
        let matches = |when: Option<&str>| {
            when.is_none_or(|when| {
                crate::keybindings::Condition::parse(when)
                    .is_ok_and(|condition| condition.evaluate(&context))
            })
        };
        self.plugins
            .installed_plugins()
            .iter()
            .filter(|record| crate::app::plugins::vsix_extension_runnable(record))
            .flat_map(|record| {
                let Metadata::Vsix(vsix) = &record.metadata else {
                    return Vec::new();
                };
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
                            views,
                        })
                    })
                    .collect()
            })
            .collect()
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
        let mut views = vec![
            (View::Chat, self.t("secondary_sidebar.chat").into()),
            (View::Outline, self.t("scm.outline.title").into()),
        ];
        views.extend(
            self.secondary_plugin_entries()
                .into_iter()
                .map(|entry| (View::Extension(entry.key), entry.title)),
        );
        views
    }

    /// The tabs of the header strip that fit in the secondary side bar.
    fn secondary_tab_slots(&self, cw: f32) -> Vec<TabSlot> {
        let views = self.secondary_views();
        let active = self.selected_secondary_view();
        let labels: Vec<usize> = views
            .iter()
            .map(|(_, label)| label.chars().count())
            .collect();
        let active = active.and_then(|active| views.iter().position(|(view, _)| *view == active));
        layout_tabs(
            &labels,
            active,
            self.secondary_sidebar_width(),
            cw.max(1.0),
            PAD,
        )
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
        let views = self.secondary_views();

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
        for slot in self.secondary_tab_slots(cw) {
            let Some((view, label)) = views.get(slot.index) else {
                continue;
            };
            let active = Some(view) == selected.as_ref();
            let chars = ((slot.width - PAD * 2.0) / cw.max(1.0)).max(0.0) as usize;
            r.draw_text(
                &label.chars().take(chars).collect::<String>(),
                x + slot.x + PAD,
                top + lh * 0.25,
                if active { fg } else { dim },
            );
            if active {
                r.draw_rect(
                    x + slot.x,
                    body - 2.0,
                    slot.width,
                    2.0,
                    rgb(Theme::color(Theme::STATUS, 0x005fb8)),
                );
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
            let slots = self.secondary_tab_slots(cw);
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

        let cw = 10.0;
        let y = app.activity_bar_vertical_offset() + 2.0;
        let slots = app.secondary_tab_slots(cw);
        assert!(slots.len() >= 2, "Chat and Outline are always present");
        let outline = slots[1];
        let x = app.secondary_sidebar_x(w) + outline.x + 2.0;
        assert!(app.secondary_sidebar_click(x, y, w, h, lh, cw));
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
