use super::*;

impl App {
    pub(crate) fn process_accessibility_actions(&mut self) {
        let actions = self
            .gpu
            .as_ref()
            .and_then(|g| {
                g.accessibility_actions
                    .lock()
                    .ok()
                    .map(|mut q| std::mem::take(&mut *q))
            })
            .unwrap_or_default();
        for request in actions {
            let id = request.target_node.0;
            if id == 3 && request.action == Action::Click {
                self.toggle_terminal();
                continue;
            }
            if id == 4 && request.action == Action::Click {
                self.terminal_visible = true;
                self.terminal_problems = true;
                self.terminal_focused = false;
                self.resize_terminal();
                continue;
            }
            if id >= 10 && id < 10 + self.tabs.len() as u64 && request.action == Action::Click {
                self.active_tab = (id - 10) as usize;
                continue;
            }
            if id == 2 {
                match (request.action, request.data) {
                    (Action::Focus, _) => self.status = "Editor focused".into(),
                    (Action::SetValue, Some(ActionData::Value(value))) => {
                        let b = &mut self.tabs[self.active_tab].buffer;
                        b.set_selection(0, b.len_chars());
                        b.insert(&value);
                    }
                    (Action::ReplaceSelectedText, Some(ActionData::Value(value))) => {
                        self.tabs[self.active_tab].buffer.insert(&value)
                    }
                    (Action::SetTextSelection, Some(ActionData::SetTextSelection(selection))) => {
                        let b = &mut self.tabs[self.active_tab].buffer;
                        let anchor = accessibility_position_to_char(
                            b,
                            selection.anchor.node,
                            selection.anchor.character_index,
                        );
                        let focus = accessibility_position_to_char(
                            b,
                            selection.focus.node,
                            selection.focus.character_index,
                        );
                        b.set_selection(anchor, focus);
                    }
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn accessible_panel_text(&self) -> Option<String> {
        if !self.terminal_visible {
            return None;
        }
        if self.terminal_problems {
            let diagnostics = &self.tabs[self.active_tab].diagnostics;
            return Some(
                diagnostics
                    .iter()
                    .map(|d| format!("Line {}: {}", d.range.start.line + 1, d.message))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        self.terminal.as_ref().map(|terminal| {
            terminal
                .snapshot()
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| cell.character)
                        .collect::<String>()
                        .trim_end()
                        .to_owned()
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
    }
}
