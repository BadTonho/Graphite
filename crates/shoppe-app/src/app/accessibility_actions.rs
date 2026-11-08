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
            let before = self.begin_editor_event();
            self.process_accessibility_action(request);
            self.end_editor_event(before);
        }
    }

    fn process_accessibility_action(&mut self, request: ActionRequest) {
        let id = request.target_node.0;
        if id == 3 && request.action == Action::Click {
            self.toggle_terminal();
            return;
        }
        if id == 4 && request.action == Action::Click {
            self.terminal_visible = true;
            self.terminal_problems = true;
            self.terminal_focused = false;
            self.resize_terminal();
            return;
        }
        if let Some((group, document_id)) = super::accessibility::tab_target(request.target_node) {
            if !matches!(request.action, Action::Click | Action::Focus) {
                return;
            }
            let Some((views, _)) = self.editor_groups.views(group) else {
                return;
            };
            if !views.iter().any(|view| view.document == document_id) {
                return;
            }
            let Some(document) = self.document_index(document_id) else {
                return;
            };
            self.focus_editor_group(group);
            self.activate_editor_document(document);
            return;
        }
        if let Some(group) = super::accessibility::editor_target(request.target_node) {
            if !self.focus_editor_group(group) {
                return;
            }
            self.terminal_focused = false;
            match (request.action, request.data) {
                (Action::Focus, _) => self.status = "Editor focused".into(),
                (Action::SetValue, Some(ActionData::Value(value))) => {
                    let b = &mut self.tabs[self.active_tab].buffer;
                    if b.is_read_only() {
                        return;
                    }
                    b.set_selection(0, b.len_chars());
                    b.insert(&value);
                }
                (Action::ReplaceSelectedText, Some(ActionData::Value(value))) => {
                    self.tabs[self.active_tab].buffer.insert(&value)
                }
                (Action::SetTextSelection, Some(ActionData::SetTextSelection(selection))) => {
                    let b = &mut self.tabs[self.active_tab].buffer;
                    let Some((anchor_group, anchor_line)) =
                        super::accessibility::line_target(selection.anchor.node)
                    else {
                        return;
                    };
                    let Some((focus_group, focus_line)) =
                        super::accessibility::line_target(selection.focus.node)
                    else {
                        return;
                    };
                    if anchor_group != group
                        || focus_group != group
                        || anchor_line >= b.len_lines()
                        || focus_line >= b.len_lines()
                    {
                        return;
                    }
                    let anchor = accessibility_position_to_char(
                        b,
                        anchor_line,
                        selection.anchor.character_index,
                    );
                    let focus = accessibility_position_to_char(
                        b,
                        focus_line,
                        selection.focus.character_index,
                    );
                    b.set_selection(anchor, focus);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn accessible_panel_text(&self) -> Option<String> {
        if !self.terminal_visible {
            return None;
        }
        if self.terminal_problems {
            let diagnostics = self.diagnostic_entries();
            return Some(
                diagnostics
                    .iter()
                    .map(|(document, diagnostic)| {
                        let name = self.tabs[*document]
                            .buffer
                            .path()
                            .and_then(Path::file_name)
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "Untitled".into());
                        format!(
                            "{name}, line {}: {}",
                            diagnostic.range.start.line + 1,
                            diagnostic.message
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        self.terminals.get(self.focused_terminal).map(|terminal| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::accessibility::{editor_node, line_node, tab_node};
    use crate::app::editor_groups::SplitDirection;

    fn request(action: Action, target: NodeId, data: Option<ActionData>) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: accesskit::TreeId::ROOT,
            target_node: target,
            data,
        }
    }

    #[test]
    fn accessible_groups_expose_focus_selections_and_unicode_text_runs() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("é🙂\r\nend");
        app.tabs[0].buffer.set_selection(1, 2);
        let right = app.split_editor_group(SplitDirection::Vertical);
        app.tabs[0].buffer.set_selection(5, 6);
        let tree = app.grouped_accessibility_tree(1200.0, 800.0, 20.0);
        assert_eq!(tree.focus, editor_node(right));
        let left_editor = &tree
            .nodes
            .iter()
            .find(|(id, _)| *id == editor_node(0))
            .unwrap()
            .1;
        assert_eq!(
            left_editor.text_selection().unwrap().anchor.character_index,
            1
        );
        assert_eq!(
            left_editor.text_selection().unwrap().focus.character_index,
            2
        );
        let run = &tree
            .nodes
            .iter()
            .find(|(id, _)| *id == line_node(right, 0))
            .unwrap()
            .1;
        assert_eq!(run.value(), Some("é🙂\r\n"));
        assert_eq!(run.character_lengths(), &[2, 4, 2]);
        app.process_accessibility_action(request(Action::Focus, editor_node(0), None));
        assert_eq!(app.editor_groups.focused(), 0);
        assert_eq!(app.tabs[0].buffer.selection().unwrap().head, 2);
    }

    #[test]
    fn accessible_actions_focus_target_group_and_block_readonly_changes() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("é🙂\ntext");
        let other = app.split_editor_group(SplitDirection::Horizontal);
        let document_id = app.tabs[0].id;
        app.process_accessibility_action(request(Action::Click, tab_node(0, document_id), None));
        assert_eq!(app.editor_groups.focused(), 0);
        let selection = accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: line_node(other, 0),
                character_index: 1,
            },
            focus: accesskit::TextPosition {
                node: line_node(other, 1),
                character_index: 2,
            },
        };
        app.process_accessibility_action(request(
            Action::SetTextSelection,
            editor_node(other),
            Some(ActionData::SetTextSelection(selection)),
        ));
        assert_eq!(app.editor_groups.focused(), other);
        assert_eq!(
            app.tabs[0].buffer.selection().unwrap(),
            shoppe_text::Selection { anchor: 1, head: 5 }
        );
        app.tabs[0].buffer.set_read_only(true);
        app.process_accessibility_action(request(
            Action::SetValue,
            editor_node(other),
            Some(ActionData::Value("changed".into())),
        ));
        assert_eq!(app.tabs[0].buffer.text(), "é🙂\ntext");
        assert_eq!(
            app.tabs[0].buffer.selection().unwrap(),
            shoppe_text::Selection { anchor: 1, head: 5 }
        );
        let tree = app.grouped_accessibility_tree(1000., 800., 20.);
        let editor = &tree
            .nodes
            .iter()
            .find(|(id, _)| *id == editor_node(other))
            .unwrap()
            .1;
        assert!(editor.is_read_only());
        assert!(!editor.supports_action(Action::SetValue));
    }

    #[test]
    fn accessible_edit_in_other_group_updates_positions_in_previous_group() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.tabs[0].buffer.insert("abcd");
        app.tabs[0].buffer.set_selection(3, 3);
        let other = app.split_editor_group(SplitDirection::Vertical);
        app.tabs[0].buffer.set_selection(1, 1);
        assert!(app.focus_editor_group(0));
        let before = app.begin_editor_event();
        app.process_accessibility_action(request(
            Action::ReplaceSelectedText,
            editor_node(other),
            Some(ActionData::Value("é".into())),
        ));
        app.end_editor_event(before);
        assert!(app.focus_editor_group(0));
        assert_eq!(app.tabs[0].buffer.text(), "aébcd");
        assert_eq!(app.tabs[0].buffer.cursor().char_idx, 4);
    }
}
