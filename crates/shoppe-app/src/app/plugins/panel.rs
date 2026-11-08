use super::*;
impl App {
    pub(crate) fn plugin_commit_text(&mut self, text: &str) -> bool {
        if !self.plugins.visible {
            return false;
        }
        if self.plugins.editing {
            self.plugins.query.push_str(text);
        }
        true
    }
    pub(crate) fn scroll_plugins(&mut self, lines: i32) {
        if self.plugins.worker.is_some()
            || self.plugins.editing
            || self.plugins.preview.is_some()
            || self.plugins.inspecting.is_some()
            || self.plugins.trust.is_some()
        {
            return;
        }
        let count = if self.plugins.github {
            self.plugins
                .assets
                .as_ref()
                .map(|a| a.len())
                .unwrap_or(self.plugins.repositories.len())
        } else {
            self.plugins.installed.len()
        };
        self.plugins.index = (self.plugins.index as i64 + lines as i64)
            .clamp(0, count.saturating_sub(1) as i64) as usize;
    }
    pub(crate) fn plugin_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if !self.plugins.visible {
            return false;
        }
        if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
            if self.plugins.preview.take().is_some() {
                self.cleanup_plugin_download();
            } else if self.plugins.trust.take().is_some() {
            } else if self.plugins.inspecting.take().is_some() {
            } else if self.plugins.editing {
                self.plugins.editing = false;
            } else if self.plugins.assets.take().is_some() {
                self.plugins.index = 0;
            } else {
                self.plugins.visible = false;
            }
            return true;
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
                self.plugins.github = false;
                self.plugins.assets = None;
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
            if self.modifiers.control_key()
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
                                shoppe_plugins::package::inspect(&path)?,
                            ))
                        });
                    } else {
                        self.plugins.github = true;
                        self.plugins.page = 1;
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
        let count = if self.plugins.github {
            self.plugins
                .assets
                .as_ref()
                .map(|a| a.len())
                .unwrap_or(self.plugins.repositories.len())
        } else {
            self.plugins.installed.len()
        };
        match event.logical_key {
            Key::Named(NamedKey::ArrowDown) => {
                self.plugins.index = (self.plugins.index + 1).min(count.saturating_sub(1))
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.plugins.index = self.plugins.index.saturating_sub(1)
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
    fn cleanup_plugin_download(&mut self) {
        if let Some(path) = self.plugins.temporary.take() {
            let _ = std::fs::remove_file(path);
        }
    }
    pub(crate) fn plugin_click(&mut self) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let lh = gpu.renderer.line_height();
        let cw = gpu.renderer.char_width();
        let x = self.mouse.x as f32;
        let y = self.mouse.y as f32;
        if self.plugins.worker.is_some() {
            return;
        }
        if self.plugins.preview.is_some()
            || self.plugins.trust.is_some()
            || self.plugins.inspecting.is_some()
        {
            return;
        }
        if y < lh * 2.0 {
            if x < cw * 16.0 {
                self.plugin_action('i');
            } else if x < cw * 30.0 {
                self.plugin_action('g');
            } else {
                self.plugins.visible = false;
            }
        } else if y < lh * 3.5 {
            if self.plugins.preview.is_some() || self.plugins.trust.is_some() {
                return;
            }
            let action = if self.plugins.github {
                if x < cw * 14.0 {
                    '/'
                } else if x < cw * 30.0 {
                    'p'
                } else {
                    'n'
                }
            } else {
                match (x / (cw * 14.0)) as usize {
                    0 => 'l',
                    1 => 'e',
                    2 => 'd',
                    3 => 'r',
                    4 => 'x',
                    _ => 'c',
                }
            };
            self.plugin_action(action);
        } else if self.plugins.preview.is_none()
            && self.plugins.trust.is_none()
            && !self.plugins.editing
        {
            let row = (y / lh) as usize;
            if row >= 5 {
                let offset = self.plugins.index.saturating_sub(5);
                self.plugins.index = offset + (row - 5) / 3;
                if self.plugins.github {
                    self.plugin_action('\n');
                }
            }
        }
    }
}
