use super::*;
use crate::test_support::TempDir;
use std::io::Write;

#[test]
#[ignore = "requires a prepared Code OSS runtime and the pinned public Open VSX sample"]
fn open_vsx_sort_command_edits_and_undoes() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let sample = PathBuf::from(std::env::var_os("GRAPHITE_TEST_OPEN_VSX_SAMPLE").unwrap());
    let dir = TempDir::new();
    let sample =
        if std::env::var_os("GRAPHITE_TEST_OPEN_VSX_LIVE").is_some() {
            let fetched = dir.path().join("registry.vsix");
            let registry = graphite_plugins::open_vsx::Registry::new().unwrap();
            let results = registry.search("sort-lines-by-selection", 30).unwrap();
            assert!(results
                .iter()
                .any(|entry| entry.namespace == "earshinov"
                    && entry.name == "sort-lines-by-selection"));
            let metadata = registry
                .download_latest("earshinov", "sort-lines-by-selection", &fetched)
                .unwrap();
            assert_eq!(metadata.version, "1.0.0");
            assert!(
                std::fs::read(&sample).unwrap() == std::fs::read(&fetched).unwrap(),
                "Open VSX sample bytes changed"
            );
            fetched
        } else {
            sample
        };
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    let store = Store::new(app.profile_root.join("plugins"));
    let record = store.install(&sample).unwrap();
    let id = record.metadata.id().to_owned();
    assert_eq!(id, "earshinov.sort-lines-by-selection");
    assert_eq!(record.metadata.version(), "1.0.0");
    app.plugins.store = Some(store.clone());
    app.plugins.installed = store.list().unwrap();
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.open_command_palette();
    let command = app
        .command_palette
        .as_ref()
        .unwrap()
        .items
        .iter()
        .find(|item| item.title == "Sort Lines by Selection")
        .unwrap();
    let key = command
        .id
        .strip_prefix("graphite.vsix:")
        .unwrap()
        .to_owned();
    assert!(!command.enabled);
    assert!(app
        .run_vsix_command_in(&key, Some(runtime_path.clone()))
        .unwrap_err()
        .contains("restricted"));
    app.run_ui_command("graphite.security.trustWorkspace");
    assert!(app
        .run_vsix_command_in(&key, Some(runtime_path.clone()))
        .unwrap_err()
        .contains("approve"));
    store.enable(&id, true).unwrap();
    app.plugins.installed = store.list().unwrap();

    let file = dir.path().join("sort.txt");
    std::fs::write(&file, "z\na").unwrap();
    app.open_file(file);
    let view = app.editor_groups.active_view_mut();
    view.selection_state.set_selections(
        vec![
            graphite_text::Selection { anchor: 0, head: 1 },
            graphite_text::Selection { anchor: 2, head: 3 },
        ],
        3,
    );
    app.run_vsix_command_in(&key, Some(runtime_path)).unwrap();
    wait_for(&mut app, finished);
    assert_eq!(
        app.documents[app.active_document()].buffer.text(),
        "a\nz",
        "{}",
        app.status
    );
    app.run_ui_command("undo");
    assert_eq!(app.documents[app.active_document()].buffer.text(), "z\na");
    app.run_ui_command("graphite.security.revokeWorkspaceTrust");
    assert!(app.plugins.vsix.is_none());
}

fn wait_for(app: &mut App, check: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        app.poll_plugins();
        if check(app) {
            break;
        }
        assert!(Instant::now() < deadline, "{}", app.status);
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn finished(app: &App) -> bool {
    app.plugins
        .vsix
        .as_ref()
        .is_none_or(|runtime| !runtime.pending())
}

fn fixture(path: &PathBuf, version: &str) {
    let mut archive = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let manifest = json!({
        "publisher":"test", "name":"phase5", "version":version, "engines":{"vscode":"^1.100.0"},
        "main":"index.js", "enabledApiProposals":["findTextInFiles"], "contributes":{"commands":[
            {"command":"phase5.start","title":"Phase 5 start"},
            {"command":"phase5.pending","title":"Phase 5 pending"},
            {"command":"phase5.crash","title":"Phase 5 crash"},
            {"command":"phase5.workbench","title":"Phase 5 workbench"},
            {"command":"phase5.terminal","title":"Phase 5 terminal"},
        ]}
    });
    archive
        .start_file(
            "extension/package.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    serde_json::to_writer(&mut archive, &manifest).unwrap();
    archive
        .start_file(
            "extension/index.js",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    archive
        .write_all(include_bytes!("phase5_fixture.cjs"))
        .unwrap();
    archive.finish().unwrap();
}

#[test]
#[ignore = "requires the packaged Code OSS runtime"]
fn official_host_runs_local_vsix_services_and_invalidates_sessions() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.service_configuration_override = Some(json!({"phase5.setting":"configured"}));
    app.run_ui_command("graphite.security.trustWorkspace");
    let store = Store::new(app.profile_root.join("plugins"));
    let package = dir.path().join("fixture.vsix");
    fixture(&package, "1.0.0");
    store.install(&package).unwrap();
    store.enable("test.phase5", true).unwrap();
    app.plugins.store = Some(store.clone());
    app.plugins.installed = store.list().unwrap();
    let file = dir.path().join("sample.txt");
    std::fs::write(&file, "sample").unwrap();
    app.open_file(file);
    app.run_vsix_command_in("test.phase5:phase5.start", Some(runtime_path.clone()))
        .unwrap();
    wait_for(&mut app, |app| {
        app.command_palette
            .as_ref()
            .is_some_and(|palette| palette.extension_prompt.is_some())
    });
    assert!(
        app.command_palette
            .as_ref()
            .unwrap()
            .extension_prompt
            .as_ref()
            .unwrap()
            .1
    );
    app.command_palette = None;
    app.cancel_vsix_prompt();
    wait_for(&mut app, |app| {
        app.command_palette
            .as_ref()
            .is_some_and(|palette| !palette.items.is_empty())
    });
    let selected = app.command_palette.take().unwrap().items[0].id.clone();
    app.answer_vsix_prompt(Some(("".into(), Some(selected))));
    wait_for(&mut app, finished);
    assert_eq!(app.editor().text(), "configured-selected", "{}", app.status);
    assert!(app
        .diagnostic_entries()
        .iter()
        .any(|(_, diagnostic)| diagnostic.message == "VSIX diagnostic"));

    assert!(app.request_vsix_language(VsixLanguageFeature::Hover));
    wait_for(&mut app, finished);
    assert_eq!(app.hover_text.as_deref(), Some("VSIX hover"));
    assert!(app.request_vsix_language(VsixLanguageFeature::Completion));
    wait_for(&mut app, finished);
    assert_eq!(app.completion_items[0].label, "VSIX completion");
    assert!(app.request_vsix_language(VsixLanguageFeature::Format));
    wait_for(&mut app, finished);
    assert_eq!(app.editor().text(), "Formatted");

    app.run_vsix_command_in("test.phase5:phase5.pending", Some(runtime_path.clone()))
        .unwrap();
    wait_for(&mut app, |app| {
        app.command_palette
            .as_ref()
            .is_some_and(|palette| palette.extension_prompt.is_some())
    });
    app.run_ui_command("graphite.security.revokeWorkspaceTrust");
    assert!(app.plugins.vsix.is_none());
    assert!(app.command_palette.is_none());
    assert!(app.diagnostic_entries().is_empty());
    assert_eq!(app.editor().text(), "Formatted");

    app.run_ui_command("graphite.security.trustWorkspace");
    app.run_vsix_command_in("test.phase5:phase5.crash", Some(runtime_path.clone()))
        .unwrap();
    wait_for(&mut app, finished);
    assert!(app.plugins.vsix.is_none());

    app.run_vsix_command_in("test.phase5:phase5.pending", Some(runtime_path.clone()))
        .unwrap();
    app.refresh_plugins_for_profile();
    assert!(app.plugins.vsix.is_none());
    wait_for(&mut app, |app| app.plugins.worker.is_none());
    fixture(&package, "2.0.0");
    store.install(&package).unwrap();
    app.plugins.installed = store.list().unwrap();
    assert!(app
        .run_vsix_command_in("test.phase5:phase5.pending", Some(runtime_path))
        .unwrap_err()
        .contains("approve"));
}

#[test]
#[ignore = "requires the packaged Code OSS runtime"]
fn official_host_bridges_output_status_bar_file_search_and_links() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");
    let store = Store::new(app.profile_root.join("plugins"));
    let package = dir.path().join("fixture.vsix");
    fixture(&package, "1.0.0");
    store.install(&package).unwrap();
    store.enable("test.phase5", true).unwrap();
    app.plugins.store = Some(store.clone());
    app.plugins.installed = store.list().unwrap();
    let file = dir.path().join("sample.txt");
    std::fs::write(&file, "sample").unwrap();
    app.open_file(file);
    app.run_vsix_command_in("test.phase5:phase5.workbench", Some(runtime_path))
        .unwrap();
    wait_for(&mut app, |app| {
        app.plugins
            .vsix
            .as_ref()
            .is_some_and(|runtime| runtime.workbench_status_texts().contains(&"Phase 5 status".to_owned()))
    });
    wait_for(&mut app, |app| {
        app.documents
            .iter()
            .any(|tab| tab.buffer.text().contains("workbench line"))
    });
    wait_for(&mut app, finished);
    assert!(!app.status.contains("findFiles missing"), "{}", app.status);
    assert!(!app.status.contains("findTextInFiles"), "{}", app.status);
    assert!(!app.status.contains("asExternalUri changed"), "{}", app.status);
    let view = app
        .documents
        .iter()
        .find(|tab| tab.buffer.text().contains("workbench line"))
        .unwrap();
    assert!(view.buffer.is_read_only());
    // Clicking the extension's status bar item runs its command.
    app.run_vsix_status_item(0);
    wait_for(&mut app, finished);
    assert_eq!(app.status, "status clicked");
}

#[test]
#[ignore = "requires the packaged Code OSS runtime"]
fn official_host_answers_navigation_symbols_actions_rename_and_signature() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");
    let store = Store::new(app.profile_root.join("plugins"));
    let package = dir.path().join("fixture.vsix");
    fixture(&package, "1.0.0");
    store.install(&package).unwrap();
    store.enable("test.phase5", true).unwrap();
    app.plugins.store = Some(store.clone());
    app.plugins.installed = store.list().unwrap();
    let file = dir.path().join("sample.txt");
    std::fs::write(&file, "sample").unwrap();
    app.open_file(file.clone());
    // Running any command activates the extension, which registers its providers.
    app.run_vsix_command_in("test.phase5:phase5.workbench", Some(runtime_path))
        .unwrap();
    wait_for(&mut app, finished);
    wait_for(&mut app, |app| {
        app.has_vsix_language_provider(VsixLanguageFeature::SignatureHelp)
            && app.has_vsix_language_provider(VsixLanguageFeature::Rename)
    });

    // The activating command shows its output channel; return to the sample document.
    app.open_file(file);
    for feature in [
        VsixLanguageFeature::Definition,
        VsixLanguageFeature::Declaration,
        VsixLanguageFeature::TypeDefinition,
    ] {
        assert!(app.request_vsix_language(feature));
        wait_for(&mut app, finished);
        assert!(!app.status.contains("not support"), "{}", app.status);
    }

    for (feature, count) in [
        (VsixLanguageFeature::References, 2),
        (VsixLanguageFeature::Implementation, 2),
    ] {
        app.go_picker = None;
        assert!(app.request_vsix_language(feature));
        wait_for(&mut app, finished);
        assert_eq!(app.go_picker.as_ref().unwrap().items.len(), count);
    }

    assert!(app.request_vsix_language(VsixLanguageFeature::DocumentSymbols));
    wait_for(&mut app, finished);
    let picker = app.go_picker.as_ref().unwrap();
    assert_eq!(picker.items[0].name, "Outer");

    assert!(app.request_vsix_language(VsixLanguageFeature::SignatureHelp));
    wait_for(&mut app, finished);
    assert!(
        app.hover_text.as_deref().unwrap_or_default().contains("call(first, second)"),
        "{:?}",
        app.hover_text
    );

    assert!(app.request_vsix_language(VsixLanguageFeature::CodeActions));
    wait_for(&mut app, finished);
    assert!(app.quick_fix_visible);
    assert_eq!(app.quick_fix_actions.len(), 1);
    app.apply_quick_fix();
    assert_eq!(app.editor().text(), "SAMPLE", "{}", app.status);

    assert!(app.request_vsix_language_with(VsixLanguageFeature::Rename, Some("renamed".into())));
    wait_for(&mut app, finished);
    assert_eq!(app.editor().text(), "renamed", "{}", app.status);
    app.run_ui_command("undo");
    assert_eq!(app.editor().text(), "SAMPLE");
}

#[test]
#[ignore = "requires the packaged Code OSS runtime"]
fn official_host_creates_terminals_and_reports_their_exit() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");
    let store = Store::new(app.profile_root.join("plugins"));
    let package = dir.path().join("fixture.vsix");
    fixture(&package, "1.0.0");
    store.install(&package).unwrap();
    store.enable("test.phase5", true).unwrap();
    app.plugins.store = Some(store.clone());
    app.plugins.installed = store.list().unwrap();
    app.run_vsix_command_in("test.phase5:phase5.terminal", Some(runtime_path))
        .unwrap();
    wait_for(&mut app, finished);
    assert!(!app.status.contains("Terminal exit"), "{}", app.status);
    assert!(!app.status.contains("not support"), "{}", app.status);
    assert_eq!(app.terminal_names, ["Phase 5 terminal"]);
    assert!(app.terminal_visible);
}
