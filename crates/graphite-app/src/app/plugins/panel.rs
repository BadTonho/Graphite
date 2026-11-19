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
            && self.merge_editor.is_none()
            && self.multi_diff.is_none()
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

    pub(super) fn plugin_section_collapsed(&self) -> bool {
        self.plugins.section_collapsed[self.plugins.tab.index()]
    }

    pub(super) fn plugin_details_open(&self) -> bool {
        self.plugins.preview.is_some()
            || self.plugins.trust.is_some()
            || self.plugins.inspecting.is_some()
            || self.plugins.running_scroll.is_some()
            || self.plugins.bisect.is_some()
    }

    /// Geometry of the sidebar for the list that is open and the current state.
    pub(super) fn plugin_layout(&self, height: f32, line_height: f32) -> PluginsLayout {
        PluginsLayout::new(
            height,
            line_height,
            self.sidebar_layout_width_px(),
            self.plugins.tab.index(),
            self.plugin_section_collapsed(),
            self.plugin_details_open(),
        )
    }

    pub(super) fn plugin_visible_rows(&self) -> usize {
        self.gpu
            .as_ref()
            .map(|gpu| {
                let (_, height) = gpu.renderer.size();
                self.plugin_layout(height, gpu.renderer.line_height())
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
        if self.scroll_plugin_menu(lines) {
            return;
        }
        if self.plugins.running_scroll.is_some() {
            self.scroll_running_plugins(lines);
            return;
        }
        if !self.plugins.visible
            || self.plugins.bisect.is_some()
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
        if !self.modifiers.control_key()
            && !self.modifiers.alt_key()
            && self.plugin_toolbar_key(&event.logical_key, self.modifiers.shift_key())
        {
            return true;
        }
        if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
            if self.plugins.preview.is_some() {
                self.cancel_plugin_install();
            } else if self.plugins.graphite_assets.take().is_some() {
                self.cleanup_plugin_download();
                self.plugins.index = 0;
                self.plugins.scroll = 0;
            } else if self.plugins.trust.take().is_some()
                || self.plugins.inspecting.take().is_some()
            {
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
        if self.plugins.trust.is_some() {
            if event
                .logical_key
                .to_text()
                .is_some_and(|s| s.eq_ignore_ascii_case("y"))
            {
                self.confirm_plugin_trust();
            }
            return true;
        }
        if self.plugins.preview.is_some() {
            if matches!(event.logical_key, Key::Named(NamedKey::Enter))
                || event
                    .logical_key
                    .to_text()
                    .is_some_and(|s| s.eq_ignore_ascii_case("y"))
            {
                self.confirm_plugin_install();
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
                        self.inspect_local_plugin(path);
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
        self.dismiss_plugin_menu();
        self.plugins.running_scroll = None;
        if self.plugins.preview.is_some() {
            self.cancel_plugin_install();
        }
        self.plugins.trust = None;
        self.plugins.inspecting = None;
        self.plugins.editing = false;
        if self.plugins.graphite_assets.take().is_some() {
            self.cleanup_plugin_download();
        }
    }

    pub(super) fn cleanup_plugin_download(&mut self) {
        if let Some(download) = self.plugins.temporary.take() {
            close_plugin_download(download);
        }
    }

    pub(crate) fn plugin_click(&mut self) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let lh = gpu.renderer.line_height();
        let (_, height) = gpu.renderer.size();
        let layout = self.plugin_layout(height, lh);
        let window_width = gpu.renderer.size().0;
        let x = self.sidebar_local_x(self.mouse.x as f32, window_width);
        let y = self.sidebar_local_y(self.mouse.y as f32);
        if x < layout.sidebar_x || x >= layout.sidebar_x + layout.width || y >= height - lh {
            return;
        }
        self.focus_plugins();
        if self.plugin_toolbar_click(&layout, lh, height, x, y) {
            return;
        }
        if self.plugins.worker.is_some() {
            return;
        }
        if let Some(index) = layout.bar_at(x, y) {
            self.leave_plugin_details();
            if index == self.plugins.tab.index() {
                self.plugins.section_collapsed[index] = !self.plugins.section_collapsed[index];
                self.plugins.scroll = 0;
            } else {
                self.plugin_action(PluginsTab::from_index(index).action_key());
                self.plugins.section_collapsed[index] = false;
            }
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
        if self.plugin_section_collapsed() {
            return;
        }
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

pub(super) fn close_plugin_download(download: tempfile::TempDir) {
    if let Err(error) = download.close() {
        crate::app::error_log::record(
            crate::app::error_log::ErrorSeverity::Low,
            "plugins.download.cleanup",
            error.to_string(),
        );
    }
}
