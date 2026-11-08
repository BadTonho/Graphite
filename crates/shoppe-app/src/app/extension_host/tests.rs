use super::*;
fn metadata(id: &str) -> CommandMetadata {
    CommandMetadata {
        id: id.into(),
        title: "Test command".into(),
        category: "Tests".into(),
        targets: vec![TargetKind::View],
        enablement: "editorWritable".into(),
    }
}
#[test]
fn session_registrations_reject_collisions_and_validate_programmatic_enablement() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    assert!(matches!(
        app.register_host_command(1, metadata("undo")),
        Err(ServiceError::InvalidArguments(_))
    ));
    let resource = app
        .register_host_command(1, metadata("example.edit"))
        .unwrap();
    assert!(resource > 0);
    assert!(app
        .register_host_command(1, metadata("example.edit"))
        .is_err());
    let mut invalid = metadata("bad");
    invalid.enablement = "a &&".into();
    assert!(app.register_host_command(1, invalid).is_err());
    assert_eq!(app.extension_host.commands.len(), 1);
    app.editor_mut().set_read_only(true);
    let focused = app.editor_groups.active_view().id;
    let ticket = app.execute_command(CommandRequest {
        id: "example.edit".into(),
        arguments: serde_json::Value::Null,
        target: CommandTarget::View(focused.0),
    });
    assert!(matches!(
        ticket.try_result(),
        Some(Err(CommandError::Disabled(_)))
    ));
    assert_eq!(app.editor().revision(), 0);
    let ticket = app.execute_command(CommandRequest {
        id: "example.edit".into(),
        arguments: serde_json::Value::Null,
        target: CommandTarget::Global,
    });
    assert!(matches!(
        ticket.try_result(),
        Some(Err(CommandError::InvalidTarget(_)))
    ));
}
