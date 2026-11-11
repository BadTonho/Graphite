use super::layout::{PluginsLayout, PluginsTab};
use super::*;
impl App {
    /// Whether the sidebar should receive text and navigation keys.
    ///
    /// Focus is explicit because the editor stays visible next to the Plugins
    /// sidebar. Transient overlays always win over the sidebar.
    pub(crate) fn plugin_owns_keyboard(&self) -> bool {
        self.plugins.visible
            && self.plugins.focused
            && !self.zen_mode
            && !self.scm_visible
            && !self.terminal_focused
            && self.quick_open.is_none()
            && self.local_search.is_none()
            && self.search_query.is_none()
            && self.snippet_query.is_none()
            && self.completion_items.is_empty()
            && !self.quick_fix_visible
            && !self.scm_commit_prompt
            && !self.branch_picker
            && self.diff_view.is_none()
            && self.conflict_view.is_none()
            && !self.close_confirm
            && !self.close_window_confirm
    }

    pub(super) fn plugin_row_count(&self) -> usize {
        match self.plugins.tab {
            PluginsTab::Installed => self.plugins.installed.len(),
            PluginsTab::OpenVsx => self.plugins.open_vsx.len(),
            PluginsTab::Graphite => self
                .plugins
                .graphite_assets
                .as_ref()
                .map_or(self.plugins.graphite.len(), |(_, assets)| assets.len()),
        }
    }

    pub(super) fn plugin_visible_rows(&self) -> usize {
        self.gpu
            .as_ref()
            .map(|gpu| {
                let (_, height) = gpu.renderer.size();
                PluginsLayout::new(
                    height,
                    self.sidebar_density_line_height(gpu.renderer.line_height()),
                    self.sidebar_layout_width_px(),
                )
                .visible_rows()
            })
            .unwrap_or(6)
    }

    fn ensure_plugin_visible(&mut self) {
        let rows = self.plugin_visible_rows();
        if self.plugins.index < self.plugins.scroll {
            self.plugins.scroll = self.plugins.index;
        } else if self.plugins.index >= self.plugins.scroll + rows {
            self.plugins.scroll = self.plugins.index + 1 - rows;
        }
        let max = self.plugin_row_count().saturating_sub(rows);
        self.plugins.scroll = self.plugins.scroll.min(max);
    }

    pub(crate) fn plugin_commit_text(&mut self, text: &str) -> bool {
        if !self.plugin_owns_keyboard() {
            return false;
        }
        if self.plugins.editing {
            self.plugins.query.push_str(text);
        }
        true
    }

    pub(crate) fn scroll_plugins(&mut self, lines: i32) {
        if !self.plugins.visible
            || self.plugins.preview.is_some()
            || self.plugins.inspecting.is_some()
            || self.plugins.trust.is_some()
        {
            return;
        }
        let count = self.plugin_row_count();
        let rows = self.plugin_visible_rows();
        let max = count.saturating_sub(rows);
        self.plugins.scroll =
            (self.plugins.scroll as i64 + lines as i64).clamp(0, max as i64) as usize;
    }

    pub(crate) fn plugin_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if !self.plugin_owns_keyboard() {
            return false;
        }
        if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
            if self.plugins.preview.take().is_some() {
                self.cleanup_plugin_download();
            } else if self.plugins.graphite_assets.take().is_some() {
                self.cleanup_plugin_download();
                self.plugins.index = 0;
                self.plugins.scroll = 0;
            } else if self.plugins.trust.take().is_some() {
            } else if self.plugins.inspecting.take().is_some() {
            } else if self.plugins.editing {
                self.plugins.editing = false;
            } else {
                self.close_plugins();
            }
            return true;
        }
        let ctrl = self.modifiers.control_key();
        let alt = self.modifiers.alt_key();
        if self.plugins.editing
            && ctrl
            && event
                .logical_key
                .to_text()
                .is_some_and(|s| s.eq_ignore_ascii_case("v"))
        {
            use super::super::clipboard::ClipboardAccess;
            if let Ok(text) = super::super::clipboard::SystemClipboard.get_text() {
                self.plugins
                    .query
                    .extend(text.chars().filter(|c| !c.is_control()));
            }
            return true;
        }
        if ctrl || alt {
            return false;
        }
        if self.plugins.worker.is_some() {
            return true;
        }
        if self.plugins.inspecting.is_some() {
            return true;
        }
        if let Some(id) = self.plugins.trust.clone() {
            if event
                .logical_key
                .to_text()
                .is_some_and(|s| s.eq_ignore_ascii_case("y"))
            {
                self.plugins.trust = None;
                self.enable_plugin(id);
            }
            return true;
        }
        if let Some((path, metadata)) = self.plugins.preview.clone() {
            if event
                .logical_key
                .to_text()
                .is_some_and(|s| s.eq_ignore_ascii_case("y"))
            {
                self.stop_plugin(metadata.id());
                let temporary = self.plugins.temporary.take();
                self.plugins.preview = None;
                self.plugins.tab = PluginsTab::Installed;
                self.plugins.index = 0;
                self.plugins.scroll = 0;
                self.plugin_job(move |store| {
                    let result = store
                        .install(&path)
                        .and_then(|_| store.list())
                        .map(Outcome::Refreshed);
                    if let Some(path) = temporary {
                        let _ = std::fs::remove_file(path);
                    }
                    result
                });
            }
            return true;
        }
        if self.plugins.editing {
            match &event.logical_key {
                Key::Named(NamedKey::Backspace) => {
                    self.plugins.query.pop();
                }
                Key::Named(NamedKey::Enter) => {
                    self.plugins.editing = false;
                    if self.plugins.path_input {
                        let path = PathBuf::from(self.plugins.query.trim().trim_matches('"'));
                        self.plugin_job(move |_| {
                            Ok(Outcome::Preview(
                                path.clone(),
                                graphite_plugins::package::inspect(&path)?,
                            ))
                        });
                    } else {
                        if self.plugins.tab == PluginsTab::Installed {
                            self.plugins.tab = PluginsTab::OpenVsx;
                        }
                        self.search_plugins();
                    }
                }
                _ => {
                    if let Some(text) = event
                        .text
                        .as_ref()
                        .filter(|s| !s.chars().any(char::is_control))
                    {
                        self.plugins.query.push_str(text);
                    }
                }
            }
            return true;
        }
        let count = self.plugin_row_count();
        match event.logical_key {
            Key::Named(NamedKey::ArrowDown) => {
                self.plugins.index = (self.plugins.index + 1).min(count.saturating_sub(1));
                self.ensure_plugin_visible();
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.plugins.index = self.plugins.index.saturating_sub(1);
                self.ensure_plugin_visible();
            }
            Key::Named(NamedKey::Enter) => self.plugin_action('\n'),
            _ => {
                if let Some(text) = event.logical_key.to_text() {
                    if let Some(c) = text.to_lowercase().chars().next() {
                        self.plugin_action(c);
                    }
                }
            }
        }
        true
    }

    /// Leaves detail/confirmation state without hiding the sidebar.
    pub(super) fn leave_plugin_details(&mut self) {
        if self.plugins.preview.take().is_some() {
            self.cleanup_plugin_download();
        }
        self.plugins.trust = None;
        self.plugins.inspecting = None;
        self.plugins.editing = false;
        if self.plugins.graphite_assets.take().is_some() {
            self.cleanup_plugin_download();
        }
    }

    pub(super) fn cleanup_plugin_download(&mut self) {
        if let Some(path) = self.plugins.temporary.take() {
            let _ = std::fs::remove_file(path);
        }
    }

    pub(crate) fn plugin_click(&mut self) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let lh = self.sidebar_density_line_height(gpu.renderer.line_height());
        let (_, height) = gpu.renderer.size();
        let layout = PluginsLayout::new(height, lh, self.sidebar_layout_width_px());
        let window_width = gpu.renderer.size().0;
        let x = self.sidebar_local_x(self.mouse.x as f32, window_width);
        let y = self.sidebar_local_y(self.mouse.y as f32);
        if x < layout.sidebar_x || x >= layout.sidebar_x + layout.width || y >= height - lh {
            return;
        }
        self.focus_plugins();
        if self.plugins.worker.is_some() {
            return;
        }
        if let Some(tab) = layout.tab_at(x, y) {
            self.leave_plugin_details();
            self.plugin_action(match tab {
                PluginsTab::Installed => 'i',
                PluginsTab::OpenVsx => 'o',
                PluginsTab::Graphite => 'g',
            });
            return;
        }
        if layout.search_contains(x, y) {
            if !self.plugins.editing {
                self.leave_plugin_details();
                self.plugin_action('/');
            }
            return;
        }
        if self.plugins.preview.is_some()
            || self.plugins.trust.is_some()
            || self.plugins.inspecting.is_some()
        {
            return;
        }
        if self.plugins.path_input {
            self.plugins.query.clear();
            self.plugins.path_input = false;
        }
        self.plugins.editing = false;
        let count = self.plugin_row_count();
        let scroll = self
            .plugins
            .scroll
            .min(count.saturating_sub(layout.visible_rows()));
        if let Some(index) = layout.card_index(x, y, scroll, count) {
            self.plugins.index = index;
            self.plugin_action('\n');
        }
    }
}
