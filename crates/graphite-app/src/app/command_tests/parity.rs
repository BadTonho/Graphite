use super::*;

#[test]
fn keyboard_widget_and_programmatic_completion_have_identical_document_effects() {
    let mut outcomes = Vec::new();
    for entry in ["keyboard", "widget", "programmatic"] {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.editor_mut().insert("a tail");
        app.editor_mut().set_selection(6, 6);
        app.editor_groups.active_view_mut().scroll_line = 2;
        let peer = app.editor_groups.active_view().id;
        app.split_editor_group(super::super::editor_groups::SplitDirection::Vertical);
        app.editor_mut().set_selection(1, 1);
        let focused = app.editor_groups.active_view().id;
        app.completion_items = vec![lsp_types::CompletionItem {
            label: "alpha".into(),
            ..Default::default()
        }];
        app.refresh_command_context();
        let widget = app.context_keys.widget_id(WidgetKind::Suggest).unwrap();
        match entry {
            "keyboard" => assert!(app.handle_keybinding_stroke("enter")),
            "widget" => app.run_ui_command("suggest.accept"),
            "programmatic" => assert_eq!(
                app.execute_command(request(
                    "suggest.accept",
                    CommandTarget::Widget(widget),
                    Value::Null
                ))
                .try_result(),
                Some(Ok(Value::Null))
            ),
            _ => unreachable!(),
        }
        assert_eq!(app.editor_groups.active_view().id, focused);
        assert!(app.completion_items.is_empty());
        assert_eq!(app.editor_groups.view(peer).unwrap().scroll_line, 2);
        outcomes.push((
            app.editor().text(),
            app.editor().revision(),
            app.editor().selections(),
            app.editor_groups
                .view(peer)
                .unwrap()
                .selection_state
                .selections(),
        ));
        app.run_ui_command("undo");
        assert_eq!(app.editor().text(), "a tail");
        assert_eq!(app.editor_groups.active_view().id, focused);
    }
    assert!(outcomes.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(outcomes[0].0, "alpha tail");
    assert_eq!(outcomes[0].1, 2);
}

#[test]
fn readonly_widget_rejection_preserves_suggestions_selection_and_history() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("a");
    app.editor_mut().insert("!");
    app.editor_mut().undo();
    app.completion_items = vec![lsp_types::CompletionItem {
        label: "alpha".into(),
        ..Default::default()
    }];
    app.refresh_command_context();
    let widget = app.context_keys.widget_id(WidgetKind::Suggest).unwrap();
    let version = app.editor().revision();
    let selection = app.editor().selections();
    app.editor_mut().set_read_only(true);
    assert!(matches!(
        app.execute_command(request(
            "suggest.accept",
            CommandTarget::Widget(widget),
            Value::Null
        ))
        .try_result(),
        Some(Err(CommandError::Disabled(_)))
    ));
    assert_eq!(app.completion_items[0].label, "alpha");
    assert_eq!(app.editor().revision(), version);
    assert_eq!(app.editor().selections(), selection);
    assert_eq!(app.editor().text(), "a");
    app.editor_mut().set_read_only(false);
    assert!(app.editor().can_redo());
}
