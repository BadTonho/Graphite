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
            self.process_accessibility_action(request);
        }
    }

    fn process_accessibility_action(&mut self, request: ActionRequest) {
        let id = request.target_node.0;
        if id == 3 && request.action == Action::Click {
            self.run_ui_command("workbench.action.terminal.toggleTerminal");
            return;
        }
        if id == 4 && request.action == Action::Click {
            self.run_ui_command("workbench.actions.view.problems");
            return;
        }
        if let Some((group, document_id)) = super::accessibility::tab_target(request.target_node) {
            if !matches!(request.action, Action::Click | Action::Focus) {
                return;
            }
            let target = self.editor_groups.views(group).and_then(|(views, _)| {
                views
                    .iter()
                    .find(|view| view.document == document_id)
                    .map(|view| view.id)
            });
            if let Some(view) = target {
                self.run_view_command_args("graphite.view.focus", view, serde_json::Value::Null);
            }
            return;
        }
        if let Some(group) = super::accessibility::editor_target(request.target_node) {
            let Some((views, active)) = self.editor_groups.views(group) else {
                return;
            };
            let view = views[active].id;
            match (request.action, request.data) {
                (Action::Focus, _) => {
                    self.run_view_command_args(
                        "graphite.view.focus",
                        view,
                        serde_json::Value::Null,
                    );
                    self.terminal_focused = false;
                }
                (Action::SetValue, Some(ActionData::Value(value))) => self.run_view_command_args(
                    "graphite.editor.setValue",
                    view,
                    serde_json::json!({"text":value}),
                ),
                (Action::ReplaceSelectedText, Some(ActionData::Value(value))) => {
                    self.run_view_command_args("type", view, serde_json::json!({"text":value}))
                }
                (Action::SetTextSelection, Some(ActionData::SetTextSelection(selection))) => {
                    let buffer = &self.documents[views[active].document].buffer;
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
                        || anchor_line >= buffer.len_lines()
                        || focus_line >= buffer.len_lines()
                    {
                        return;
                    }
                    let anchor = accessibility_position_to_char(
                        buffer,
                        anchor_line,
                        selection.anchor.character_index,
                    );
                    let head = accessibility_position_to_char(
                        buffer,
                        focus_line,
                        selection.focus.character_index,
                    );
                    self.run_view_command_args(
                        "graphite.editor.setSelection",
                        view,
                        serde_json::json!({"anchor":anchor,"head":head}),
                    );
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
                        let name = self.documents[*document]
                            .buffer
                            .path()
                            .and_then(Path::file_name)
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| self.t("common.untitled").into());
                        self.tf(
                            "accessibility.diagnostic",
                            &[
                                ("name", &name),
                                ("line", &(diagnostic.range.start.line + 1)),
                                ("message", &diagnostic.message),
                            ],
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
        app.test_editor_mut(0).insert("é🙂\r\nend");
        app.test_editor_mut(0).set_selection(1, 2);
        let right = app.split_editor_group(SplitDirection::Vertical);
        app.test_editor_mut(0).set_selection(5, 6);
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
        assert_eq!(app.test_editor(0).selection().unwrap().head, 2);
    }

    #[test]
    fn accessible_actions_focus_target_group_and_block_readonly_changes() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.test_editor_mut(0).insert("é🙂\ntext");
        let other = app.split_editor_group(SplitDirection::Horizontal);
        let document_id = app.active_document();
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
        assert_eq!(app.editor_groups.focused(), 0);
        app.focus_editor_group(other);
        assert_eq!(
            app.test_editor(0).selection().unwrap(),
            graphite_text::Selection { anchor: 1, head: 5 }
        );
        app.documents
            .get_mut(app.document_at(0))
            .unwrap()
            .buffer
            .set_read_only(true);
        app.process_accessibility_action(request(
            Action::SetValue,
            editor_node(other),
            Some(ActionData::Value("changed".into())),
        ));
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "é🙂\ntext");
        assert_eq!(
            app.test_editor(0).selection().unwrap(),
            graphite_text::Selection { anchor: 1, head: 5 }
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
        app.test_editor_mut(0).insert("abcd");
        app.test_editor_mut(0).set_selection(3, 3);
        let other = app.split_editor_group(SplitDirection::Vertical);
        app.test_editor_mut(0).set_selection(1, 1);
        assert!(app.focus_editor_group(0));

        app.process_accessibility_action(request(
            Action::ReplaceSelectedText,
            editor_node(other),
            Some(ActionData::Value("é".into())),
        ));

        assert!(app.focus_editor_group(0));
        assert_eq!(app.documents[app.document_at(0)].buffer.text(), "aébcd");
        assert_eq!(app.test_editor(0).cursor().char_idx, 4);
    }

    #[test]
    fn accessible_multicursor_edit_preserves_focus_scroll_and_peer_ime() {
        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().into(), None, true);
        app.editor_mut().insert("abc\ndef\n");
        app.editor_mut().set_selection(6, 6);
        app.ime_preedit("日".into());
        app.editor_groups.active_view_mut().scroll_line = 1;
        let right = app.split_editor_group(SplitDirection::Vertical);
        app.editor_mut().set_selections(vec![
            graphite_text::Selection { anchor: 0, head: 0 },
            graphite_text::Selection { anchor: 4, head: 4 },
        ]);
        app.focus_editor_group(0);
        let focus = app.editor_groups.active_view().id;
        let revision = app.editor().revision();
        app.process_accessibility_action(request(
            Action::ReplaceSelectedText,
            editor_node(right),
            Some(ActionData::Value("🌎".into())),
        ));
        assert_eq!(app.editor_groups.active_view().id, focus);
        assert_eq!(app.editor_groups.active_view().scroll_line, 1);
        assert_eq!(app.editor_groups.active_view().ime_preedit, "日");
        assert_eq!(app.editor().cursor().char_idx, 8);
        assert_eq!(app.editor().revision(), revision + 1);
        assert_eq!(app.editor().text(), "🌎abc\n🌎def\n");
        app.run_ui_command("undo");
        assert_eq!(app.editor_groups.active_view().id, focus);
        assert_eq!(app.editor().cursor().char_idx, 6);
        app.run_ui_command("redo");
        assert_eq!(app.editor().cursor().char_idx, 8);
    }
}
