use super::*;
fn edit(uri: DocumentUri, version: Option<u64>, start: u32, end: u32, text: &str) -> DocumentEdit {
    DocumentEdit {
        uri,
        version,
        edits: vec![graphite_core_api::TextEdit {
            start: Position {
                line: 0,
                character: start,
            },
            end: Position {
                line: 0,
                character: end,
            },
            text: text.into(),
        }],
    }
}
#[test]
fn snapshots_read_memory_before_disk_and_decode_closed_files() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("encoded #.txt");
    std::fs::write(&path, [0xff, 0xfe, b'a', 0, b'b', 0]).unwrap();
    let uri = DocumentUri::from_path(&path).unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let closed = app.snapshot(&uri).unwrap();
    assert_eq!(closed.text, "ab");
    assert_eq!(closed.version, None);
    assert_eq!(app.documents.len(), 1);
    app.open_document(&uri).unwrap();
    app.editor_mut().insert("Q");
    assert_eq!(app.snapshot(&uri).unwrap().text, "Qab");
    assert_eq!(std::fs::read(&path).unwrap()[2], b'a');
    let missing = DocumentUri::untitled(DocumentId(u64::MAX));
    assert_eq!(app.snapshot(&missing), Err(ServiceError::NotFound));
}
#[test]
fn atomic_service_edits_cover_untitled_closed_utf16_and_history() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("closed.txt");
    std::fs::write(&path, "closed").unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert("a\u{1f30e}b");
    let original = app.editor_groups.active_view().id;
    let uri = DocumentUri::untitled(app.active_document());
    let closed = DocumentUri::from_path(&path).unwrap();
    let version = app.editor().revision();
    let before = app.documents.len();
    assert_eq!(
        app.apply_edits(vec![
            edit(uri.clone(), Some(version), 1, 2, "X"),
            edit(closed.clone(), None, 0, 1, "Q")
        ]),
        Err(ServiceError::InvalidRange)
    );
    assert_eq!(app.editor().text(), "a\u{1f30e}b");
    assert_eq!(app.documents.len(), before);
    app.apply_edits(vec![
        edit(uri.clone(), Some(version), 1, 3, "X"),
        edit(closed.clone(), None, 0, 1, "Q"),
    ])
    .unwrap();
    assert_eq!(app.editor_groups.active_view().id, original);
    assert_eq!(app.editor().text(), "aXb");
    assert_eq!(app.snapshot(&closed).unwrap().text, "Qlosed");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "closed");
    app.run_ui_command("undo");
    assert_eq!(app.editor().text(), "a\u{1f30e}b");
    let current = app.editor().revision();
    app.apply_edits(vec![edit(uri.clone(), Some(current), 0, 0, "")])
        .unwrap();
    assert_eq!(app.editor().revision(), current);
    assert_eq!(
        app.apply_edits(vec![edit(uri, Some(version), 0, 1, "Y")]),
        Err(ServiceError::StaleDocument)
    );
}
#[test]
fn read_only_rejects_batch_without_text_or_collection_effects() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("closed.txt");
    std::fs::write(&path, "abc").unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let uri = DocumentUri::untitled(app.active_document());
    app.editor_mut().set_read_only(true);
    assert_eq!(
        app.apply_edits(vec![
            edit(DocumentUri::from_path(&path).unwrap(), None, 0, 1, "Q"),
            edit(uri, Some(0), 0, 0, "X")
        ]),
        Err(ServiceError::ReadOnly)
    );
    assert_eq!(app.documents.len(), 1);
    assert_eq!(app.editor().revision(), 0);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "abc");
}

#[test]
fn rejected_batches_preserve_views_redo_events_leases_and_disk() {
    for failure in ["missing", "overlap", "surrogate", "stale", "readonly"] {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("closed.txt");
        std::fs::write(&path, "a🌎b").unwrap();
        let closed = DocumentUri::from_path(&path).unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.editor_mut().insert("live");
        app.editor_mut().insert("!");
        app.editor_mut().undo();
        let id = app.active_document();
        let uri = DocumentUri::untitled(id);
        app.editor_mut().set_selection(1, 3);
        app.split_editor_group(super::super::editor_groups::SplitDirection::Vertical);
        app.editor_mut().set_selection(4, 4);
        app.editor_groups.active_view_mut().scroll_line = 2;
        if failure == "readonly" {
            app.editor_mut().set_read_only(true);
        }
        let before = app.snapshot(&uri).unwrap();
        let views = app.editor_groups.view_ids();
        let focused = app.editor_groups.active_view().id;
        let selections = app.editor().selections();
        let subscription = app.subscribe_documents().unwrap();
        subscription.receiver.try_iter().for_each(drop);
        let mut batch = vec![
            edit(closed.clone(), None, 0, 1, "Q"),
            edit(uri.clone(), before.version, 0, 1, "L"),
        ];
        match failure {
            "missing" => batch.push(edit(
                DocumentUri::from_path(&dir.path().join("missing.txt")).unwrap(),
                None,
                0,
                0,
                "X",
            )),
            "overlap" => {
                let duplicate = batch[0].edits[0].clone();
                batch[0].edits.push(duplicate);
            }
            "surrogate" => batch[0] = edit(closed.clone(), None, 1, 2, "X"),
            "stale" => batch[1].version = Some(0),
            "readonly" => {}
            _ => unreachable!(),
        }
        assert!(app.apply_edits(batch).is_err(), "{failure}");
        app.reconcile_document_events();
        assert_eq!(app.snapshot(&uri).unwrap(), before, "{failure}");
        assert_eq!(app.documents.len(), 1, "{failure}");
        assert!(!app.documents.is_retained(id), "{failure}");
        assert_eq!(app.editor_groups.view_ids(), views, "{failure}");
        assert_eq!(app.editor_groups.active_view().id, focused);
        assert_eq!(app.editor_groups.active_view().scroll_line, 2);
        assert_eq!(app.editor().selections(), selections);
        assert!(subscription.receiver.try_recv().is_err(), "{failure}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a🌎b");
        app.editor_mut().set_read_only(false);
        assert!(app.editor().can_redo());
        app.editor_mut().redo();
        assert_eq!(app.editor().text(), "live!");
    }
}

#[test]
fn mixed_open_closed_untitled_batch_publishes_only_after_success() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let untitled = DocumentUri::untitled(app.active_document());
    app.editor_mut().insert("a🌎b");
    let open_path = dir.path().join("open.txt");
    let closed_path = dir.path().join("closed.txt");
    for path in [&open_path, &closed_path] {
        std::fs::write(path, "a🌎b").unwrap();
    }
    let open = DocumentUri::from_path(&open_path).unwrap();
    let closed = DocumentUri::from_path(&closed_path).unwrap();
    app.open_document(&open).unwrap();
    let focus = app.editor_groups.active_view().id;
    let subscription = app.subscribe_documents().unwrap();
    subscription.receiver.try_iter().for_each(drop);
    app.apply_edits(vec![
        edit(untitled.clone(), Some(1), 1, 3, "é"),
        edit(open.clone(), Some(0), 1, 3, "é"),
        edit(closed.clone(), None, 1, 3, "é"),
    ])
    .unwrap();
    assert_eq!(app.editor_groups.active_view().id, focus);
    let events: Vec<_> = subscription.receiver.try_iter().collect();
    assert_eq!(events.len(), 3);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, DocumentEvent::Opened(_)))
            .count(),
        1
    );
    for uri in [untitled, open, closed] {
        let snapshot = app.snapshot(&uri).unwrap();
        assert_eq!(snapshot.text, "aéb");
        assert!(snapshot.dirty);
        assert!(!app.documents.is_retained(snapshot.id.unwrap()));
    }
    for path in [&open_path, &closed_path] {
        assert_eq!(std::fs::read_to_string(path).unwrap(), "a🌎b");
    }
}
#[test]
fn service_commands_window_focus_and_catalog_share_central_entry() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let first = app.editor_groups.active_view().id;
    app.split_editor_group(super::super::editor_groups::SplitDirection::Vertical);
    let focused = app.editor_groups.active_view().id;
    let ticket = CommandService::execute(
        &mut app,
        CommandRequest {
            id: "type".into(),
            arguments: serde_json::json!({"text":"text"}),
            target: CommandTarget::View(first.0),
        },
    );
    assert_eq!(ticket.try_result(), Some(Ok(serde_json::Value::Null)));
    assert_eq!(app.editor_groups.active_view().id, focused);
    WindowService::focus_view(&mut app, first.0).unwrap();
    assert_eq!(app.editor_groups.active_view().id, first);
    assert!(app.commands().iter().any(|c| c.id == "type"));
    assert!(same_file_path(
        &app.workspace().unwrap().root.to_path().unwrap(),
        dir.path()
    ));
}

#[test]
fn document_subscriptions_observe_versions_close_and_release() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let subscription = app.subscribe_documents().unwrap();
    assert!(matches!(
        subscription.receiver.try_recv(),
        Ok(DocumentEvent::Opened(_))
    ));
    app.editor_mut().insert("a");
    app.editor_mut().insert("b");
    let versions: Vec<_> = subscription
        .receiver
        .try_iter()
        .filter_map(|event| match event {
            DocumentEvent::Changed(doc) => doc.version,
            _ => None,
        })
        .collect();
    assert_eq!(versions, vec![1, 2]);
    let path = dir.path().join("second.txt");
    std::fs::write(&path, "second").unwrap();
    app.open_file(path);
    app.reconcile_document_events();
    assert!(matches!(
        subscription.receiver.try_recv(),
        Ok(DocumentEvent::Opened(_))
    ));
    app.close_active_tab();
    app.reconcile_document_events();
    assert!(matches!(
        subscription.receiver.try_recv(),
        Ok(DocumentEvent::Closed(_))
    ));
    let cancellation = subscription.cancellation.clone();
    drop(subscription);
    assert!(cancellation.is_cancelled());
    app.document_events.reap();
}
#[test]
fn slow_document_subscriber_is_bounded_and_reports_overflow() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let subscription = app.subscribe_documents().unwrap();
    for _ in 0..graphite_core_api::protocol::MAX_PENDING {
        app.editor_mut().insert("x");
    }
    assert!(subscription
        .overflow
        .load(std::sync::atomic::Ordering::Acquire));
    assert!(subscription.receiver.try_iter().count() <= graphite_core_api::protocol::MAX_PENDING);
}

#[test]
fn lsp_workspace_queries_use_the_service_contract() {
    struct Workspace {
        root: DocumentUri,
    }
    impl WorkspaceService for Workspace {
        fn workspace(&self) -> ServiceResult<WorkspaceSnapshot> {
            Ok(WorkspaceSnapshot {
                root: self.root.clone(),
            })
        }
        fn configuration(&self) -> ServiceResult<serde_json::Value> {
            Ok(serde_json::json!({"editor.tabSize":4,"custom":{"enabled":true}}))
        }
    }
    let dir = crate::test_support::TempDir::new();
    let workspace = Workspace {
        root: DocumentUri::from_path(dir.path()).unwrap(),
    };
    let result=super::lsp::workspace_request(&workspace,"workspace/configuration",&serde_json::json!({"items":[{"section":"editor"},{"section":"editor.tabSize"},{"section":"missing"},{"section":"custom"}]})).unwrap();
    assert_eq!(
        result,
        serde_json::json!([{"tabSize":4},4,null,{"enabled":true}])
    );
    let folders = super::lsp::workspace_request(
        &workspace,
        "workspace/workspaceFolders",
        &serde_json::Value::Null,
    )
    .unwrap();
    assert_eq!(folders[0]["uri"], workspace.root.as_str());
}
