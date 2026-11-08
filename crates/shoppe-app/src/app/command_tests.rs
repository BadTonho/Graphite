use super::command_execution::{
    CancellationToken, CommandError, CommandInvocation, CommandRequest,
};
use super::command_registry::{
    registry, ArgumentKind, CommandDescriptor, CommandRegistry, CommandTarget, TargetKind,
};
use super::context_keys::{ContextKeyService, WidgetKind};
use super::*;
use serde_json::{json, Value};

pub(super) fn register_test_commands(registry: &mut CommandRegistry) {
    fn defer(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
        invocation.cancellation.check()?;
        app.command_job(true).unwrap();
        Ok(())
    }
    registry
        .register(CommandDescriptor {
            id: "shoppe.test.defer",
            title: "Simulated worker",
            category: "Tests",
            targets: &[TargetKind::View, TargetKind::Widget],
            enablement: "documentAvailable",
            menu_when: None,
            arguments: ArgumentKind::None,
            widget: None,
            handler: defer,
        })
        .unwrap();
}
fn request(id: &str, target: CommandTarget, arguments: Value) -> CommandRequest {
    CommandRequest {
        id: id.into(),
        target,
        arguments,
    }
}
fn target(app: &App) -> CommandTarget {
    CommandTarget::View(app.editor_groups.active_view().id.0)
}
fn diff() -> Option<Box<UserEvent>> {
    Some(Box::new(UserEvent::DiffReady(super::scm_ui::DiffView {
        title: "Simulated diff".into(),
        left_name: "old".into(),
        right_name: "new".into(),
        left: "a".into(),
        right: "b".into(),
        inline: false,
    })))
}

#[test]
fn inventory_preserves_every_legacy_id_and_default_binding() {
    let legacy: Vec<String> =
        serde_json::from_str(include_str!("command_tests/legacy_ids.json")).unwrap();
    assert_eq!(legacy.len(), 106);
    for id in legacy {
        assert!(registry().get(&id).is_some(), "missing legacy command {id}");
    }
    for id in crate::keybindings::default_command_ids() {
        assert!(registry().get(id).is_some(), "missing bound command {id}");
    }
    let entries: Vec<_> = registry().iter().collect();
    let ids: std::collections::HashSet<_> = entries.iter().map(|entry| entry.id).collect();
    assert_eq!(entries.len(), ids.len());
    assert!(entries.iter().all(|entry| !entry.title.is_empty()
        && !entry.category.is_empty()
        && !entry.targets.is_empty()));
}

#[test]
fn duplicate_or_invalid_registration_preserves_existing_catalog() {
    let descriptor = registry().get("undo").unwrap().clone();
    let mut local = CommandRegistry::default();
    local.register(descriptor.clone()).unwrap();
    assert!(local.register(descriptor).is_err());
    let mut invalid = registry().get("redo").unwrap().clone();
    invalid.enablement = "a &&";
    assert!(local.register(invalid).is_err());
    assert_eq!(local.iter().count(), 1);
}

#[test]
fn keybinding_arguments_reach_the_same_handler_as_programmatic_execution() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.keymap.reload(r#"[{"key":"ctrl+alt+x","command":"type","args":{"text":"é🌎"},"when":"editorTextFocus"}]"#).unwrap();
    assert!(app.handle_keybinding_stroke("ctrl+alt+x"));
    let result = app
        .execute_command(request("type", target(&app), json!({"text":"!"})))
        .try_result()
        .unwrap();
    assert_eq!(result, Ok(Value::Null));
    assert_eq!(app.editor().text(), "é🌎!");
    assert_eq!(app.editor().revision(), 2);
}

#[test]
fn inactive_view_edits_and_history_preserve_focus_and_remap_peers() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("abcd");
    app.editor_mut().set_selection(1, 1);
    let first = target(&app);
    let group = app.split_editor_group(super::editor_groups::SplitDirection::Vertical);
    app.editor_mut().set_selection(3, 3);
    assert_eq!(
        app.execute_command(request("type", first, json!({"text":"Q"})))
            .try_result(),
        Some(Ok(Value::Null))
    );
    assert_eq!(app.editor_groups.focused(), group);
    assert_eq!(app.editor().text(), "aQbcd");
    assert_eq!(app.editor().cursor().char_idx, 4);
    app.run_ui_command("undo");
    assert_eq!(app.editor_groups.focused(), group);
    assert_eq!(app.editor().cursor().char_idx, 3);
    let CommandTarget::View(first) = first else {
        unreachable!()
    };
    assert_eq!(
        app.editor_groups
            .view(shoppe_text::ViewId(first))
            .unwrap()
            .selection_state
            .cursor()
            .char_idx,
        1
    );
}

#[test]
fn rejected_commands_leave_text_selection_and_suggestions_untouched() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("abc");
    app.editor_mut().set_selection(2, 1);
    app.completion_items.push(lsp_types::CompletionItem {
        label: "keep".into(),
        ..Default::default()
    });
    let selections = app.editor().selections();
    let revision = app.editor().revision();
    assert!(matches!(
        app.execute_command(request("unknown", target(&app), Value::Null))
            .try_result(),
        Some(Err(CommandError::UnknownCommand(_)))
    ));
    assert!(matches!(
        app.execute_command(request("type", target(&app), json!({"text":42})))
            .try_result(),
        Some(Err(CommandError::InvalidArguments(_)))
    ));
    app.editor_mut().set_read_only(true);
    assert!(matches!(
        app.execute_command(request("type", target(&app), json!({"text":"x"})))
            .try_result(),
        Some(Err(CommandError::Disabled(_)))
    ));
    assert_eq!(app.editor().text(), "abc");
    assert_eq!(app.editor().revision(), revision);
    assert_eq!(app.editor().selections(), selections);
    assert_eq!(app.completion_items[0].label, "keep");
}

#[test]
fn omitted_target_uses_global_scope_and_never_implies_an_editor() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let command: CommandRequest =
        serde_json::from_value(json!({"id":"type","arguments":{"text":"x"}})).unwrap();
    assert_eq!(command.target, CommandTarget::Global);
    assert!(matches!(
        app.execute_command(command).try_result(),
        Some(Err(CommandError::InvalidTarget(_)))
    ));
    assert_eq!(
        app.execute_command(request(
            "workbench.action.toggleZenMode",
            CommandTarget::Global,
            Value::Null
        ))
        .try_result(),
        Some(Ok(Value::Null))
    );
    assert!(app.zen_mode);
    assert!(app.editor().text().is_empty());
}

#[test]
fn widget_scope_precedence_lifetime_and_identity_are_independent() {
    let mut context = ContextKeyService::default();
    let first = shoppe_text::ViewId(1);
    let second = shoppe_text::ViewId(2);
    context.replace_global(HashMap::from([("key".into(), "global".into())]));
    context.replace_view(first, HashMap::from([("key".into(), "view".into())]));
    context.sync_widget(
        WidgetKind::Find,
        true,
        Some(first),
        HashMap::from([("key".into(), "widget".into())]),
    );
    let id = context.widget_id(WidgetKind::Find).unwrap();
    assert_eq!(
        context.resolve_target(CommandTarget::Widget(id))["key"],
        "widget"
    );
    assert_eq!(
        context.resolve_target(CommandTarget::View(first.0))["key"],
        "view"
    );
    assert_eq!(
        context.resolve_target(CommandTarget::View(second.0))["key"],
        "global"
    );
    context.sync_widget(WidgetKind::Find, false, None, HashMap::new());
    context.sync_widget(WidgetKind::Find, true, Some(second), HashMap::new());
    assert_ne!(context.widget_id(WidgetKind::Find), Some(id));
    assert!(context.widget(id).is_none());
}

#[test]
fn enablement_is_distinct_from_shortcut_when_and_menu_visibility() {
    let mut local = CommandRegistry::default();
    let mut descriptor = registry().get("type").unwrap().clone();
    descriptor.menu_when = Some("false");
    local.register(descriptor).unwrap();
    let context = HashMap::from([("editorWritable".into(), "true".into())]);
    assert!(!local.visible("type", &context));
    assert!(local.enabled("type", &context));
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.keymap
        .reload(r#"[{"key":"ctrl+alt+x","command":"type","args":{"text":"x"},"when":"false"}]"#)
        .unwrap();
    assert!(!app.handle_keybinding_stroke("ctrl+alt+x"));
    assert_eq!(
        app.execute_command(request("type", target(&app), json!({"text":"x"})))
            .try_result(),
        Some(Ok(Value::Null))
    );
}

#[test]
fn widget_commands_require_the_correct_live_instance_and_refresh_without_keyboard() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.run_ui_command("actions.find");
    let id = app.context_keys.widget_id(WidgetKind::Find).unwrap();
    assert!(matches!(
        app.execute_command(request(
            "quickOpen.close",
            CommandTarget::Widget(id),
            Value::Null
        ))
        .try_result(),
        Some(Err(CommandError::InvalidTarget(_)))
    ));
    app.local_search = None;
    assert_eq!(
        app.execute_command(request(
            "closeFindWidget",
            CommandTarget::Widget(id),
            Value::Null
        ))
        .try_result(),
        Some(Err(CommandError::TargetClosed))
    );
    assert!(app.context_keys.widget(id).is_none());
}

#[test]
fn cancellation_before_dispatch_has_no_side_effects() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let cancellation = CancellationToken::default();
    cancellation.cancel();
    let ticket = app.execute_command_with_cancellation(
        request("type", target(&app), json!({"text":"x"})),
        cancellation,
    );
    assert_eq!(ticket.try_result(), Some(Err(CommandError::Cancelled)));
    assert_eq!(app.editor().revision(), 0);
}

#[test]
fn asynchronous_ticket_does_not_block_interface_and_finishes_on_event_loop() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let ticket = app.execute_command(request("shoppe.test.defer", target(&app), Value::Null));
    assert!(ticket.try_result().is_none());
    let (sender, receiver) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        sender.send(diff()).unwrap();
    });
    app.run_ui_command("workbench.action.toggleZenMode");
    assert!(app.zen_mode);
    assert!(app.diff_view.is_none());
    let event = receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    worker.join().unwrap();
    app.accept_command_event(ticket.id, event, Ok(json!({"done":true})));
    assert_eq!(ticket.try_result(), Some(Ok(json!({"done":true}))));
    assert!(app.diff_view.is_some());
}

#[test]
fn cancellation_and_failure_finish_once_and_ignore_late_effects() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let ticket = app.execute_command(request("shoppe.test.defer", target(&app), Value::Null));
    ticket.cancellation.cancel();
    app.reap_commands();
    assert_eq!(ticket.try_result(), Some(Err(CommandError::Cancelled)));
    app.accept_command_event(ticket.id, diff(), Ok(Value::Null));
    assert!(app.diff_view.is_none());
    let failure = app.execute_command(request("shoppe.test.defer", target(&app), Value::Null));
    app.accept_command_event(
        failure.id,
        diff(),
        Err(CommandError::Failed("worker failed".into())),
    );
    assert_eq!(
        failure.try_result(),
        Some(Err(CommandError::Failed("worker failed".into())))
    );
    assert!(app.diff_view.is_none());
    assert!(app.command_runtime.pending.is_empty());
}

#[test]
fn stale_worker_result_cannot_overwrite_intervening_typing() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let ticket = app.execute_command(request("shoppe.test.defer", target(&app), Value::Null));
    app.editor_mut().insert("new text");
    app.accept_command_event(ticket.id, diff(), Ok(Value::Null));
    assert_eq!(ticket.try_result(), Some(Err(CommandError::StaleDocument)));
    assert_eq!(app.editor().text(), "new text");
    assert!(app.diff_view.is_none());
}

#[test]
fn closing_an_inactive_target_cancels_pending_work_without_changing_focus() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let first = target(&app);
    let group = app.split_editor_group(super::editor_groups::SplitDirection::Vertical);
    let ticket = app.execute_command(request("shoppe.test.defer", first, Value::Null));
    assert_eq!(
        app.execute_command(request(
            "workbench.action.closeActiveEditor",
            first,
            Value::Null
        ))
        .try_result(),
        Some(Ok(Value::Null))
    );
    assert_eq!(app.editor_groups.focused(), group);
    assert_eq!(ticket.try_result(), Some(Err(CommandError::TargetClosed)));
    app.accept_command_event(ticket.id, diff(), Ok(Value::Null));
    assert!(app.diff_view.is_none());
}

#[test]
fn dirty_close_waits_for_decision_and_preserves_the_other_document_focus() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("second.txt");
    std::fs::write(&path, "second").unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("unsaved");
    let first = target(&app);
    app.open_file(path);
    let focused = app.editor_groups.active_view().id;
    let ticket = app.execute_command(request(
        "workbench.action.closeActiveEditor",
        first,
        Value::Null,
    ));
    assert!(ticket.try_result().is_none());
    assert!(app.close_confirm);
    app.run_ui_command("shoppe.close.cancel");
    assert_eq!(ticket.try_result(), Some(Err(CommandError::Cancelled)));
    assert_eq!(app.editor_groups.active_view().id, focused);
    let ticket = app.execute_command(request(
        "workbench.action.closeActiveEditor",
        first,
        Value::Null,
    ));
    app.run_ui_command("shoppe.close.discard");
    assert_eq!(ticket.try_result(), Some(Ok(Value::Null)));
    assert_eq!(app.editor_groups.active_view().id, focused);
    assert_eq!(app.editor().text(), "second");
}

#[test]
fn workspace_snapshot_rejects_checkout_result_after_typing() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let ticket = app.execute_command(request("shoppe.test.defer", target(&app), Value::Null));
    let expected = app
        .documents
        .iter()
        .map(|doc| (doc.id, doc.buffer.revision()))
        .collect();
    let pending = app.command_runtime.pending.get_mut(&ticket.id).unwrap();
    pending.revision = None;
    pending.workspace_revisions = Some(expected);
    app.editor_mut().insert("typed while Git was running");
    app.accept_command_event(ticket.id, diff(), Ok(Value::Null));
    assert_eq!(ticket.try_result(), Some(Err(CommandError::StaleDocument)));
    assert_eq!(app.editor().text(), "typed while Git was running");
    assert!(app.diff_view.is_none());
}

#[test]
fn invalid_index_rejection_preserves_suggestions_and_widget_state() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.completion_items.push(lsp_types::CompletionItem {
        label: "keep".into(),
        ..Default::default()
    });
    let selected = app.active_terminal;
    let ticket = app.execute_command(request(
        "shoppe.terminal.select",
        CommandTarget::Global,
        json!({"index":999}),
    ));
    assert!(matches!(
        ticket.try_result(),
        Some(Err(CommandError::InvalidArguments(_)))
    ));
    assert_eq!(app.completion_items[0].label, "keep");
    assert_eq!(app.active_terminal, selected);
}

#[test]
fn reopening_find_on_another_view_replaces_its_scope_and_target() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let first = app.editor_groups.active_view().id;
    app.run_ui_command("actions.find");
    let old = app.context_keys.widget_id(WidgetKind::Find).unwrap();
    app.split_editor_group(super::editor_groups::SplitDirection::Vertical);
    let second = app.editor_groups.active_view().id;
    app.execute_command(request(
        "actions.find",
        CommandTarget::View(second.0),
        Value::Null,
    ));
    let new = app.context_keys.widget_id(WidgetKind::Find).unwrap();
    assert_ne!(old, new);
    assert_eq!(app.context_keys.widget(new).unwrap().parent, Some(second));
    assert_eq!(
        app.execute_command(request(
            "closeFindWidget",
            CommandTarget::Widget(old),
            Value::Null
        ))
        .try_result(),
        Some(Err(CommandError::TargetClosed))
    );
    assert_ne!(first, second);
    assert!(app.local_search.is_some());
}

#[test]
fn closing_widget_cancels_its_worker_and_ignores_late_results() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.run_ui_command("actions.find");
    let widget = CommandTarget::Widget(app.context_keys.widget_id(WidgetKind::Find).unwrap());
    let ticket = app.execute_command(request("shoppe.test.defer", widget, Value::Null));
    app.execute_command(request("closeFindWidget", widget, Value::Null));
    assert_eq!(ticket.try_result(), Some(Err(CommandError::TargetClosed)));
    assert!(ticket.cancellation.is_cancelled());
    app.accept_command_event(ticket.id, diff(), Ok(Value::Null));
    assert!(app.diff_view.is_none());
    assert!(app.command_runtime.pending.is_empty());
    let error = serde_json::to_value(CommandError::TargetClosed).unwrap();
    assert_eq!(error["kind"], "targetClosed");
}

#[test]
fn failed_save_and_window_decisions_finish_the_original_ticket() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("untitled");
    let close = app.execute_command(request(
        "workbench.action.closeActiveEditor",
        target(&app),
        Value::Null,
    ));
    app.run_ui_command("shoppe.close.save");
    assert!(matches!(
        close.try_result(),
        Some(Err(CommandError::Failed(_)))
    ));
    assert_eq!(app.editor().text(), "untitled");
    assert!(!app.close_confirm);
    let quit = app.execute_command(request(
        "workbench.action.quit",
        CommandTarget::Global,
        Value::Null,
    ));
    assert!(quit.try_result().is_none());
    app.run_ui_command("shoppe.window.cancelClose");
    assert_eq!(quit.try_result(), Some(Err(CommandError::Cancelled)));
    assert!(!app.exit_requested);
}
