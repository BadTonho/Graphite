use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StatusSegment {
    Branch,
    Encoding,
    LineEnding,
    Indentation,
    Screencast,
}

impl App {
    pub(crate) fn status_bar_prefix(&self) -> String {
        let tab = &self.tabs[self.active_tab];
        let dirty = if tab.buffer.is_dirty() { " ●" } else { "" };
        let read_only = if tab.buffer.is_read_only() {
            " 🔒 Read-only"
        } else {
            ""
        };
        let (line, column) = tab.buffer.cursor_line_col();
        format!(
            " {}{}{}   Ln {}, Col {}",
            self.status,
            dirty,
            read_only,
            line + 1,
            column + 1
        )
    }

    pub(crate) fn status_bar_segments(&self) -> Vec<(StatusSegment, String)> {
        let tab = &self.tabs[self.active_tab];
        vec![
            (
                StatusSegment::Branch,
                self.scm_snapshot
                    .branch
                    .as_deref()
                    .unwrap_or("Git: —")
                    .to_owned(),
            ),
            (StatusSegment::Encoding, tab.encoding.label().to_owned()),
            (
                StatusSegment::LineEnding,
                tab.line_ending.label().to_owned(),
            ),
            (
                StatusSegment::Indentation,
                if tab.indentation.insert_spaces {
                    format!("Spaces: {}", tab.indentation.tab_size)
                } else {
                    format!("Tabs: {}", tab.indentation.tab_size)
                },
            ),
            (
                StatusSegment::Screencast,
                if self.screencast_mode {
                    "Screencast: On"
                } else {
                    "Screencast: Off"
                }
                .to_owned(),
            ),
        ]
    }

    pub(crate) fn status_segment_at(&self, x: f32, char_width: f32) -> Option<StatusSegment> {
        status_segment_at(
            self.status_bar_prefix().chars().count(),
            &self.status_bar_segments(),
            x,
            char_width,
        )
    }

    pub(crate) fn toggle_zen_mode(&mut self) {
        self.zen_mode = !self.zen_mode;
        self.terminal_visible = false;
        self.scm_visible = false;
        if let Some(gpu) = &self.gpu {
            let fullscreen = self
                .zen_mode
                .then(|| winit::window::Fullscreen::Borderless(gpu.window.current_monitor()));
            gpu.window.set_fullscreen(fullscreen);
        }
        self.status = if self.zen_mode {
            "Zen Mode"
        } else {
            "Zen Mode closed"
        }
        .into();
    }

    pub(crate) fn toggle_centered_editor(&mut self) {
        self.centered_editor = !self.centered_editor;
        self.status = if self.centered_editor {
            "Centered editor enabled"
        } else {
            "Centered editor disabled"
        }
        .into();
    }

    pub(crate) fn toggle_outline(&mut self) {
        self.outline_visible = !self.outline_visible;
        self.status = if self.outline_visible {
            "Outline visible"
        } else {
            "Outline hidden"
        }
        .into();
    }

    pub(crate) fn toggle_process_explorer(&mut self) {
        self.process_panel_visible = !self.process_panel_visible;
        if !self.process_panel_visible {
            self.process_cancel.store(true, Ordering::Release);
            return;
        }
        self.process_scroll = 0;
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        self.process_cancel.store(false, Ordering::Release);
        let cancel = self.process_cancel.clone();
        let root_pid = std::process::id();
        std::thread::Builder::new()
            .name("shoppe-process-monitor".into())
            .spawn(move || {
                while !cancel.load(Ordering::Acquire) {
                    let metrics = crate::process_explorer::collect_process_tree(root_pid);
                    if proxy
                        .send_event(UserEvent::ProcessSnapshot(metrics))
                        .is_err()
                    {
                        break;
                    }
                    for _ in 0..10 {
                        if cancel.load(Ordering::Acquire) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            })
            .ok();
    }

    pub(crate) fn toggle_screencast_mode(&mut self) {
        self.screencast_mode = !self.screencast_mode;
        if !self.screencast_mode {
            self.screencast_cancel.store(true, Ordering::Release);
        } else if let Some(proxy) = self.event_proxy.clone() {
            let cancel = self.screencast_cancel.clone();
            cancel.store(false, Ordering::Release);
            std::thread::Builder::new()
                .name("shoppe-screencast-animation".into())
                .spawn(move || {
                    while !cancel.load(Ordering::Acquire) {
                        if proxy.send_event(UserEvent::ScreencastTick).is_err() {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(40));
                    }
                })
                .ok();
        }
        self.status = if self.screencast_mode {
            "Screencast Mode enabled"
        } else {
            "Screencast Mode disabled"
        }
        .into();
    }

    pub(crate) fn note_screencast_shortcut(&mut self, event: &winit::event::KeyEvent) {
        if !self.screencast_mode {
            return;
        }
        let Some(shortcut) = format_screencast_shortcut(self.modifiers, &event.logical_key) else {
            return;
        };
        self.screencast_shortcut = Some((shortcut, std::time::Instant::now()));
    }
}

fn format_screencast_shortcut(modifiers: ModifiersState, key: &Key) -> Option<String> {
    let mut parts = Vec::new();
    if modifiers.control_key() {
        parts.push("Ctrl");
    }
    if modifiers.shift_key() {
        parts.push("Shift");
    }
    if modifiers.alt_key() {
        parts.push("Alt");
    }
    if modifiers.super_key() {
        parts.push("Super");
    }
    let key = match key {
        Key::Character(value) => value.to_uppercase(),
        Key::Named(named) => format!("{named:?}"),
        _ => return None,
    };
    parts.push(&key);
    Some(parts.join("+"))
}

fn status_segment_at(
    prefix_chars: usize,
    segments: &[(StatusSegment, String)],
    x: f32,
    char_width: f32,
) -> Option<StatusSegment> {
    let mut left = prefix_chars as f32 * char_width;
    for (segment, label) in segments {
        left += 3.0 * char_width;
        let right = left + label.chars().count() as f32 * char_width;
        if (left..right).contains(&x) {
            return Some(*segment);
        }
        left = right;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_segment_hit_testing_matches_rendered_segment_positions() {
        let segments = vec![
            (StatusSegment::Branch, "main".to_owned()),
            (StatusSegment::Encoding, "UTF-8".to_owned()),
        ];
        assert_eq!(
            status_segment_at(4, &segments, 7.0, 1.0),
            Some(StatusSegment::Branch)
        );
        assert_eq!(
            status_segment_at(4, &segments, 18.0, 1.0),
            Some(StatusSegment::Encoding)
        );
        assert_eq!(status_segment_at(4, &segments, 12.0, 1.0), None);
    }

    #[test]
    fn screencast_shortcut_formats_modifier_chords() {
        let modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
        let key = Key::Character("p".into());
        assert_eq!(
            format_screencast_shortcut(modifiers, &key).as_deref(),
            Some("Ctrl+Shift+P")
        );
    }
}
