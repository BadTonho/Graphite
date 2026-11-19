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
        if request.action == Action::Click && self.plugin_accessibility_click(id) {
            return;
        }
        if request.target_node == super::accessibility::command_center_back_node() {
            if request.action == Action::Click && self.can_navigate_history(-1) {
                self.run_ui_command("workbench.action.navigateBack");
            }
            return;
        }
        if request.target_node == super::accessibility::command_center_forward_node() {
            if request.action == Action::Click && self.can_navigate_history(1) {
                self.run_ui_command("workbench.action.navigateForward");
            }
            return;
        }
        if request.target_node == super::accessibility::command_center_search_node() {
            if matches!(request.action, Action::Click | Action::Focus) {
                self.open_command_center();
            }
            return;
        }
        if let Some(index) = super::accessibility::command_center_item_target(request.target_node) {
            if request.action == Action::Click {
                self.command_center_activate_index(index);
            }
            return;
        }
        if request.target_node == super::accessibility::profile_button_node() {
            if request.action == Action::Click {
                if self.profile_menu.is_some() {
                    self.profile_menu = None;
                    return;
                }
                let (width, height, line_height) =
                    self.gpu.as_ref().map_or((1200.0, 760.0, 20.0), |gpu| {
                        let (width, height) = gpu.renderer.size();
                        (width, height, gpu.renderer.line_height())
                    });
                if let Some((x, y, w, h)) = super::activity_bar::activity_bar_profile_button_rect(
                    &self.activity_bar_preferences,
                    width,
                    height,
                    self.top_menu_height(),
                    line_height,
                ) {
                    self.open_profile_menu((x, y, w, h), width, height);
                }
            }
            return;
        }
        if let Some(index) = super::accessibility::profile_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_profile_menu_accessibility_item(index);
            }
            return;
        }
        if request.target_node == super::accessibility::settings_gear_node() {
            if request.action == Action::Click {
                if self.settings_menu.is_some() {
                    self.settings_menu = None;
                    return;
                }
                self.profile_menu = None;
                let (width, height, line_height) =
                    self.gpu.as_ref().map_or((1200.0, 760.0, 20.0), |gpu| {
                        let (width, height) = gpu.renderer.size();
                        (width, height, gpu.renderer.line_height())
                    });
                if let Some((x, y, w, h)) = super::activity_bar::activity_bar_settings_gear_rect(
                    &self.activity_bar_preferences,
                    width,
                    height,
                    self.top_menu_height(),
                    line_height,
                ) {
                    self.open_settings_menu((x, y, w, h), width, height);
                }
            }
            return;
        }
        if let Some(index) = super::accessibility::settings_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_settings_menu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::settings_submenu_item_target(request.target_node)
        {
            if request.action == Action::Click {
                self.activate_settings_submenu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::file_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_file_menu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::edit_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_edit_menu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::selection_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_selection_menu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::go_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_go_menu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::go_submenu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_go_submenu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::go_picker_item_target(request.target_node) {
            if request.action == Action::Click {
                if let Some(picker) = self.go_picker.as_mut() {
                    picker.selected = index;
                }
                self.accept_go_picker();
            }
            return;
        }
        if let Some(index) = super::accessibility::terminal_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_terminal_menu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::run_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_run_menu_accessibility_item(index, false);
            }
            return;
        }
        if let Some(index) = super::accessibility::run_breakpoint_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_run_menu_accessibility_item(index, true);
            }
            return;
        }
        if let Some(index) = super::accessibility::view_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_view_menu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::help_menu_item_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_help_menu_accessibility_item(index);
            }
            return;
        }
        if let Some(index) = super::accessibility::help_modal_action_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_help_modal_accessibility_action(index);
            }
            return;
        }
        if id == 3 && request.action == Action::Click {
            self.run_ui_command("workbench.action.terminal.toggleTerminal");
            return;
        }
        if id == 4 && request.action == Action::Click {
            self.run_ui_command("workbench.actions.view.problems");
            return;
        }
        if let Some(index) = super::accessibility::workbench_menu_target(request.target_node) {
            if request.action == Action::Click {
                let menu = if index == super::menu_bar::WorkbenchMenu::ALL.len() {
                    super::menu_bar::WorkbenchMenu::More
                } else {
                    super::menu_bar::WorkbenchMenu::ALL[index]
                };
                if self.workbench_menu == Some(menu) {
                    self.workbench_menu = None;
                    self.activity_bar_menu = None;
                } else {
                    self.workbench_menu = Some(menu);
                    self.activity_bar_menu = None;
                    self.workbench_menu_overflow = menu == super::menu_bar::WorkbenchMenu::More;
                    if menu == super::menu_bar::WorkbenchMenu::View {
                        let (width, height) = self.gpu.as_ref().map_or((1200.0, 760.0), |gpu| {
                            let (width, height) = gpu.renderer.size();
                            (width, height)
                        });
                        self.view_menu_open(0.0, width, height);
                    }
                    if menu == super::menu_bar::WorkbenchMenu::Help {
                        self.help_menu_selection = 0;
                    }
                    if menu == super::menu_bar::WorkbenchMenu::Selection {
                        self.selection_menu_selection = 0;
                    }
                    if menu == super::menu_bar::WorkbenchMenu::Go {
                        self.go_menu_selection = 0;
                        self.go_submenu = None;
                    }
                    if menu == super::menu_bar::WorkbenchMenu::Run {
                        self.open_run_menu();
                    }
                }
            }
            return;
        }
        if let Some(control) = super::accessibility::window_control_target(request.target_node) {
            if request.action == Action::Click {
                self.titlebar_control_action(control);
            }
            return;
        }
        if let Some(activity) = super::accessibility::activity_target(request.target_node) {
            if request.action == Action::Click {
                self.activate_activity(activity);
            }
            return;
        }
        if request.action == Action::Click {
            if let Some(entry) = self.plugin_activity_entries().into_iter().find(|entry| {
                !self
                    .activity_bar_preferences
                    .hidden_items
                    .contains(&entry.key)
                    && super::accessibility::plugin_activity_target(request.target_node, &entry.key)
            }) {
                self.activate_plugin_activity(entry);
                return;
            }
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
                self.plugins.focused = false;
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
                    self.plugins.focused = false;
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
    fn accessible_activity_bar_switches_views() {
        use crate::app::accessibility::{activity_button_node, activity_toolbar_node};
        use crate::app::activity_bar::Activity;

        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let tree = app.grouped_accessibility_tree(1200.0, 800.0, 20.0);
        assert!(tree
            .nodes
            .iter()
            .any(|(id, _)| *id == activity_toolbar_node()));
        let button = &tree
            .nodes
            .iter()
            .find(|(id, _)| *id == activity_button_node(Activity::SourceControl))
            .unwrap()
            .1;
        assert_eq!(button.label(), Some(app.t("activity.source_control")));
        assert!(button.supports_action(Action::Click));

        app.process_accessibility_action(request(
            Action::Click,
            activity_button_node(Activity::SourceControl),
            None,
        ));
        assert!(app.scm_visible);
        assert_eq!(app.active_activity(), Activity::SourceControl);
    }

    #[test]
    fn accessible_view_menu_can_restore_a_hidden_activity_bar() {
        use crate::app::accessibility::{workbench_menu_bar_node, workbench_menu_button_node};
        use crate::app::menu_bar::WorkbenchMenu;

        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.activity_bar_preferences.position = crate::config::ActivityBarPosition::Hidden;
        let tree = app.grouped_accessibility_tree(1200.0, 800.0, 20.0);
        assert!(tree
            .nodes
            .iter()
            .any(|(id, _)| *id == workbench_menu_bar_node()));

        let view_index = WorkbenchMenu::ALL
            .iter()
            .position(|menu| *menu == WorkbenchMenu::View)
            .unwrap();
        app.process_accessibility_action(request(
            Action::Click,
            workbench_menu_button_node(view_index),
            None,
        ));
        assert!(app.view_menu.is_some());
        app.activate_view_menu_accessibility_item(3);
        app.activate_view_menu_accessibility_item(11);
        app.activate_view_menu_accessibility_item(0);
        assert_eq!(
            app.activity_bar_preferences.position,
            crate::config::ActivityBarPosition::Default
        );
    }

    #[test]
    fn accessible_titlebar_exposes_the_overflow_menu_when_the_window_is_narrow() {
        use crate::app::accessibility::{workbench_menu_bar_node, workbench_menu_button_node};
        use crate::app::menu_bar::WorkbenchMenu;

        let temp = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        let tree = app.grouped_accessibility_tree(600.0, 800.0, 20.0);
        let more_node = workbench_menu_button_node(WorkbenchMenu::ALL.len());
        assert!(tree
            .nodes
            .iter()
            .any(|(id, node)| *id == workbench_menu_bar_node()
                && node.children().contains(&more_node)));
        let more_button = tree.nodes.iter().find(|(id, _)| *id == more_node).unwrap();
        assert_eq!(more_button.1.label(), Some(app.t("menubar.more")));

        app.process_accessibility_action(request(Action::Click, more_node, None));
        assert_eq!(app.workbench_menu, Some(WorkbenchMenu::More));
        assert!(app.workbench_menu_overflow);
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
