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
            {"command":"phase5.decorate","title":"Phase 5 decorate"},
            {"command":"phase5.watch","title":"Phase 5 watch"},
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
        app.plugins.vsix.as_ref().is_some_and(|runtime| {
            runtime
                .workbench_status_texts()
                .contains(&"Phase 5 status".to_owned())
        })
    });
    wait_for(&mut app, |app| {
        app.documents
            .iter()
            .any(|tab| tab.buffer.text().contains("workbench line"))
    });
    wait_for(&mut app, finished);
    assert!(!app.status.contains("findFiles missing"), "{}", app.status);
    assert!(!app.status.contains("findTextInFiles"), "{}", app.status);
    assert!(
        !app.status.contains("asExternalUri changed"),
        "{}",
        app.status
    );
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
        app.hover_text
            .as_deref()
            .unwrap_or_default()
            .contains("call(first, second)"),
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

/// Compatibility survey: installs real extensions from Open VSX into isolated temporary
/// profiles, runs each one's first command and reports what Graphite did not support.
///
/// `GRAPHITE_TEST_COMPAT_LIST` is a file with one `namespace.name` per line and
/// `GRAPHITE_TEST_COMPAT_OUT` a directory that receives `report.tsv`. Third-party extension
/// code runs during the survey, so each one gets its own temporary `HOME`.
#[test]
#[ignore = "network survey; requires the packaged Code OSS runtime"]
fn compatibility_survey() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let list =
        std::fs::read_to_string(std::env::var_os("GRAPHITE_TEST_COMPAT_LIST").unwrap()).unwrap();
    let out = PathBuf::from(std::env::var_os("GRAPHITE_TEST_COMPAT_OUT").unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let registry = graphite_plugins::open_vsx::Registry::new().unwrap();
    let mut report = String::from("extension\tversion\tresult\tunsupported\n");
    for line in list.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((namespace, name)) = line.split_once('.') else {
            continue;
        };
        let dir = TempDir::new();
        let home = dir.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        let package = dir.path().join("extension.vsix");
        let (version, result, unsupported) = (|| -> (String, String, String) {
            let metadata = match registry.download_latest(namespace, name, &package) {
                Ok(metadata) => metadata,
                Err(error) => {
                    return (
                        String::new(),
                        format!("download failed: {error}"),
                        String::new(),
                    )
                }
            };
            let version = metadata.version;
            let _ = std::fs::create_dir_all(dir.path().join("work"));
            let mut app = App::new_with_options(dir.path().join("work"), None, true);
            app.profile_root = dir.path().join("profile");
            app.workspace_trust = Some(
                crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap(),
            );
            app.run_ui_command("graphite.security.trustWorkspace");
            let store = Store::new(app.profile_root.join("plugins"));
            let record = match store.install(&package) {
                Ok(record) => record,
                Err(error) => return (version, format!("install failed: {error}"), String::new()),
            };
            let id = record.metadata.id().to_owned();
            let command = match &record.metadata {
                Metadata::Vsix(vsix) if vsix.main.is_some() => {
                    vsix.commands.first().map(|command| command.id.clone())
                }
                _ => None,
            };
            let Some(command) = command else {
                return (
                    version,
                    "no entry point or commands (declarative only)".into(),
                    String::new(),
                );
            };
            store.enable(&id, true).unwrap();
            app.plugins.store = Some(store.clone());
            app.plugins.installed = store.list().unwrap();
            let sample = dir.path().join("work/sample.txt");
            std::fs::write(&sample, "sample\n").unwrap();
            app.open_file(sample);
            let key = format!("{id}:{command}");
            if let Err(error) = app.run_vsix_command_in(&key, Some(runtime_path.clone())) {
                return (version, format!("could not start: {error}"), String::new());
            }
            let deadline = Instant::now() + Duration::from_secs(40);
            while Instant::now() < deadline {
                app.poll_plugins();
                if finished(&app) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let outcome = if !finished(&app) {
                "timed out".to_owned()
            } else if app.plugins.vsix.is_none() {
                format!("host stopped: {}", app.status)
            } else {
                format!("ran `{command}`: {}", app.status)
            };
            let log =
                std::fs::read_to_string(app.profile_root.join("code-oss/unsupported-api.log"))
                    .unwrap_or_default();
            (version, outcome, log.lines().collect::<Vec<_>>().join(","))
        })();
        let clean = |text: &str| text.replace(['\t', '\n', '\r'], " ");
        report.push_str(&format!(
            "{line}\t{version}\t{}\t{}\n",
            clean(&result),
            unsupported
        ));
        std::fs::write(out.join("report.tsv"), &report).unwrap();
    }
}

#[test]
#[ignore = "requires the packaged Code OSS runtime"]
fn official_host_delivers_file_events_to_watchers() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let dir = TempDir::new();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let mut app = App::new_with_options(work.clone(), None, true);
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
    app.run_vsix_command_in("test.phase5:phase5.watch", Some(runtime_path))
        .unwrap();
    // The extension attaches its listener some time after the command starts, so the event is
    // sent repeatedly until the command finishes. Changes outside the workspace reach nobody.
    let created = work.join("new.watched");
    std::fs::write(&created, "x").unwrap();
    let outside = dir.path().join("elsewhere.watched");
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        app.poll_plugins();
        if finished(&app) {
            break;
        }
        assert!(Instant::now() < deadline, "{}", app.status);
        app.forward_vsix_file_events(&[
            (FileChange::Created, outside.clone()),
            (FileChange::Created, created.clone()),
        ]);
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!app.status.contains("watcher"), "{}", app.status);
}

#[test]
#[ignore = "requires the packaged Code OSS runtime"]
fn official_host_sends_text_decorations_for_the_active_editor() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let dir = TempDir::new();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let mut app = App::new_with_options(work.clone(), None, true);
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
    let file = work.join("sample.txt");
    std::fs::write(&file, "sample").unwrap();
    app.open_file(file.clone());
    let uri = graphite_core_api::DocumentUri::from_path(&file).unwrap();
    let uri = uri.as_str().to_owned();
    app.run_vsix_command_in("test.phase5:phase5.decorate", Some(runtime_path))
        .unwrap();
    wait_for(&mut app, |app| {
        app.vsix_line_decorations(&uri, 0, true).len() == 2
    });
    let marks = app.vsix_line_decorations(&uri, 0, true);
    let ranged = marks.iter().find(|mark| !mark.whole_line).unwrap();
    assert_eq!((ranged.start, ranged.end), (Some(1), Some(4)));
    assert!(ranged
        .look
        .background
        .is_some_and(|color| color[0] > 0.9 && color[3] < 0.6));
    assert!(ranged.look.border.is_some());
    assert_eq!(ranged.after.as_ref().unwrap().text, " note");
    let whole = marks.iter().find(|mark| mark.whole_line).unwrap();
    assert_eq!(whole.after.as_ref().unwrap().text, " instance");
    assert!(whole.look.background.is_some(), "theme colors resolve");
    assert!(app.vsix_line_decorations(&uri, 1, true).is_empty());
    wait_for(&mut app, finished);
}

const DEBUG_FACTORY_FIXTURE: &str = r#"
const vscode = require('vscode');
exports.activate = () => {
    vscode.debug.registerDebugAdapterDescriptorFactory('fixture-debug', {
        createDebugAdapterDescriptor: () =>
            new vscode.DebugAdapterServer(Number(process.env.GRAPHITE_TEST_DAP_PORT)),
    });
};
"#;

fn debug_factory_fixture(path: &PathBuf) {
    let mut archive = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let manifest = json!({
        "publisher":"test", "name":"debugfactory", "version":"1.0.0",
        "engines":{"vscode":"^1.100.0"}, "main":"index.js",
        "contributes":{"debuggers":[{"type":"fixture-debug","label":"Fixture"}]}
    });
    let options = zip::write::SimpleFileOptions::default();
    archive
        .start_file("extension/package.json", options)
        .unwrap();
    serde_json::to_writer(&mut archive, &manifest).unwrap();
    archive.start_file("extension/index.js", options).unwrap();
    archive.write_all(DEBUG_FACTORY_FIXTURE.as_bytes()).unwrap();
    archive.finish().unwrap();
}

/// End to end: the real extension host runs an extension's `DebugAdapterDescriptorFactory`, and
/// Graphite connects to the TCP server it describes. Needs the packaged runtime
/// (`GRAPHITE_TEST_CODE_OSS`), which is why it is ignored by default.
#[test]
#[ignore = "requires the packaged Code OSS runtime"]
fn official_host_creates_a_debug_adapter_from_an_extension_factory() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std::env::set_var(
        "GRAPHITE_TEST_DAP_PORT",
        listener.local_addr().unwrap().port().to_string(),
    );
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");
    let store = Store::new(app.profile_root.join("plugins"));
    let package = dir.path().join("debugfactory.vsix");
    debug_factory_fixture(&package);
    store.install(&package).unwrap();
    store.enable("test.debugfactory", true).unwrap();
    app.plugins.store = Some(store.clone());
    app.plugins.installed = store.list().unwrap();
    std::fs::create_dir_all(dir.path().join(".vscode")).unwrap();
    std::fs::write(
        dir.path().join(".vscode/launch.json"),
        r#"{"configurations":[{"name":"Fixture","type":"fixture-debug","request":"launch"}]}"#,
    )
    .unwrap();
    app.start_vsix_host(Some(runtime_path)).unwrap();
    app.start_debugging().unwrap();
    wait_for(&mut app, |app| !app.vsix_debug_adapter_pending());
    assert!(app.debug.session.is_some(), "{}", app.status);
    let (_stream, _) = listener.accept().unwrap();
    app.debug.stop().unwrap();
}

#[test]
fn vsix_tree_view_registers_fetches_expands_refreshes_and_disposes() {
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);

    let register = json!({
        "id": 1,
        "actor": "MainThreadTreeViews",
        "method": "$registerTreeViewDataProvider",
        "args": ["ext.sampleView", { "showCollapseAll": true, "canSelectMany": false }]
    });
    app.handle_vsix_request(&mut runtime, &register).unwrap();
    let get_root = host.next_sent().expect("initial $getChildren call");
    assert_eq!(get_root["type"], "call");
    assert_eq!(get_root["actor"], "ExtHostTreeViews");
    assert_eq!(get_root["method"], "$getChildren");
    assert_eq!(get_root["args"], json!(["ext.sampleView", null]));
    let root_call_id = get_root["id"].as_u64().unwrap();
    let ack = host.next_sent().expect("register ack");
    assert_eq!(ack["type"], "reply");

    // Metadata calls.
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 2,
            "actor": "MainThreadTreeViews",
            "method": "$setTitle",
            "args": ["ext.sampleView", "Explorer Ext", "v1"]
        }),
    )
    .unwrap();
    let _ = host.next_sent();
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 3,
            "actor": "MainThreadTreeViews",
            "method": "$setMessage",
            "args": ["ext.sampleView", { "value": "Loaded from extension" }]
        }),
    )
    .unwrap();
    let _ = host.next_sent();
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 4,
            "actor": "MainThreadTreeViews",
            "method": "$setBadge",
            "args": ["ext.sampleView", { "value": 3, "tooltip": "3 items" }]
        }),
    )
    .unwrap();
    let _ = host.next_sent();

    let meta = runtime.tree_views.meta("ext.sampleView").unwrap();
    assert_eq!(meta.title.as_deref(), Some("Explorer Ext"));
    assert_eq!(meta.message.as_deref(), Some("Loaded from extension"));
    assert_eq!(meta.badge_value, Some(3));

    app.plugins.vsix = Some(runtime);

    // Deliver root items via loopback result.
    host.emit(json!({
        "type": "result",
        "id": root_call_id,
        "result": [
            { "handle": "folder-1", "label": { "label": "src" }, "collapsibleState": 1, "description": "dir" },
            { "handle": "leaf-1", "label": "README.md", "collapsibleState": 0, "command": { "id": "ext.openReadme", "title": "Open" } }
        ]
    }));
    app.poll_plugins();

    let rows = app.plugin_tree_items("ext.sampleView");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].item.label, "src");
    assert_eq!(rows[0].item.description.as_deref(), Some("dir"));
    assert_eq!(rows[1].item.label, "README.md");

    // Click collapsed folder -> emits $setExpanded, $setSelectionAndFocus, $getChildren([folder-1]).
    assert!(app.click_plugin_tree_row("ext.sampleView", 0));
    let mut saw_expand = false;
    let mut saw_select = false;
    let mut child_call_id = None;
    while let Some(msg) = host.next_sent() {
        if msg["actor"] == "ExtHostTreeViews" {
            match msg["method"].as_str() {
                Some("$setExpanded") => {
                    assert_eq!(msg["args"], json!(["ext.sampleView", "folder-1", true]));
                    saw_expand = true;
                }
                Some("$setSelectionAndFocus") => {
                    saw_select = true;
                }
                Some("$getChildren") => {
                    assert_eq!(msg["args"], json!(["ext.sampleView", ["folder-1"]]));
                    child_call_id = msg["id"].as_u64();
                }
                _ => {}
            }
        }
    }
    assert!(saw_expand && saw_select);
    let child_call_id = child_call_id.expect("child fetch id");

    host.emit(json!({
        "type": "result",
        "id": child_call_id,
        "result": [[0, { "handle": "child-1", "label": "main.rs", "collapsibleState": 0 }]]
    }));
    app.poll_plugins();

    let rows = app.plugin_tree_items("ext.sampleView");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].item.label, "main.rs");
    assert_eq!(rows[1].depth, 1);

    // Targeted refresh invalidates only folder-1 and re-requests it.
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 5,
            "actor": "MainThreadTreeViews",
            "method": "$refresh",
            "args": ["ext.sampleView", { "folder-1": {} }]
        }),
    )
    .unwrap();
    let refresh_call = host.next_sent().expect("refresh child request");
    let refresh_ack = host.next_sent();
    let (refresh_call, _ack) = if refresh_call["type"] == "call" {
        (refresh_call, refresh_ack)
    } else {
        (
            refresh_ack.expect("refresh child request"),
            Some(refresh_call),
        )
    };
    assert_eq!(refresh_call["method"], "$getChildren");
    assert_eq!(
        refresh_call["args"],
        json!(["ext.sampleView", ["folder-1"]])
    );

    // Dispose removes tree view completely.
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 6,
            "actor": "MainThreadTreeViews",
            "method": "$disposeTreeView",
            "args": ["ext.sampleView"]
        }),
    )
    .unwrap();
    app.plugins.vsix = Some(runtime);
    assert!(app.plugin_tree_items("ext.sampleView").is_empty());
}

#[test]
fn vsix_virtual_files_open_through_a_mirror_are_identified_by_uri_and_save_back() {
    use base64::Engine;
    let encode = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    // The mirror root is process-wide, so these tests share one profile (their schemes differ).
    app.profile_root = std::env::temp_dir().join("graphite-vfs-tests-profile");
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);
    let scheme_uri = "vfstest-b:/notes/a.txt";

    app.handle_vsix_request(
        &mut runtime,
        &json!({"id": 1, "actor": "MainThreadFileSystem", "method": "$registerFileSystemProvider",
            "args": [5, "vfstest-b", 2, null]}),
    )
    .unwrap();
    let watch = std::iter::from_fn(|| host.next_sent())
        .find(|message| message["method"] == "$watch")
        .expect("the provider root is watched");
    assert_eq!(watch["args"][0], 5);
    assert!(graphite_core_api::DocumentUri::parse(scheme_uri).is_ok());

    // Opening is asynchronous: the file is read from the provider, the request is answered later.
    app.handle_vsix_request(
        &mut runtime,
        &json!({"id": 7, "actor": "MainThreadDocuments", "method": "$tryOpenDocument", "args": [scheme_uri]}),
    )
    .unwrap();
    let sent: Vec<Value> = std::iter::from_fn(|| host.next_sent()).collect();
    assert!(
        sent.iter().all(|message| message["id"] != 7),
        "no early reply: {sent:?}"
    );
    let read = sent
        .iter()
        .find(|message| message["method"] == "$readFile")
        .expect("read call");
    assert_eq!(read["actor"], "ExtHostFileSystem");
    assert_eq!(read["bytes"], true);
    assert_eq!(read["args"], json!([5, {"$graphiteUri": scheme_uri}]));
    app.finish_vsix_virtual_fs(
        &mut runtime,
        &json!({"id": read["id"], "result": {"$bytes": encode(b"hello")}}),
    );
    let reply = std::iter::from_fn(|| host.next_sent())
        .find(|message| message["type"] == "reply")
        .expect("deferred reply");
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["result"], scheme_uri);

    // The document is identified by its virtual URI and its text is the provider's content.
    let documents = graphite_core_api::DocumentService::documents(&app).unwrap();
    let opened = documents
        .iter()
        .find(|document| document.uri.as_str() == scheme_uri)
        .expect("document with the virtual URI");
    assert_eq!(opened.text, "hello");
    let mirror =
        super::virtual_fs::mirror_path(&graphite_core_api::DocumentUri::parse(scheme_uri).unwrap())
            .unwrap();
    assert_eq!(std::fs::read_to_string(&mirror).unwrap(), "hello");
    assert!(mirror.starts_with(app.profile_root.join("virtual-fs")));

    // A provider failure is reported to the extension that asked.
    app.handle_vsix_request(
        &mut runtime,
        &json!({"id": 8, "actor": "MainThreadDocuments", "method": "$tryOpenDocument", "args": ["vfstest-b:/missing"]}),
    )
    .unwrap();
    let read = std::iter::from_fn(|| host.next_sent())
        .find(|message| message["method"] == "$readFile")
        .unwrap();
    app.finish_vsix_virtual_fs(
        &mut runtime,
        &json!({"id": read["id"], "error": "FileNotFound"}),
    );
    let failure = std::iter::from_fn(|| host.next_sent())
        .find(|message| message["id"] == 8)
        .unwrap();
    assert_eq!(failure["error"], "FileNotFound");

    // Saving writes the mirror back through the provider.
    app.plugins.vsix = Some(runtime);
    app.upload_virtual_document(&mirror);
    let write = std::iter::from_fn(|| host.next_sent())
        .find(|message| message["method"] == "$writeFile")
        .expect("write call");
    assert_eq!(write["args"][0], 5);
    assert_eq!(write["args"][1], json!({"$graphiteUri": scheme_uri}));
    assert_eq!(
        write["args"][2],
        json!({"$graphiteBytes": encode(b"hello")})
    );
    app.upload_virtual_document(&dir.path().join("plain.txt"));
    assert!(
        std::iter::from_fn(|| host.next_sent()).all(|message| message["method"] != "$writeFile")
    );

    // The explorer lists the provider's root and its folders once they are read.
    let rows = app.virtual_explorer_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].uri, "vfstest-b:/");
    app.activate_virtual_explorer_row(0);
    let list = std::iter::from_fn(|| host.next_sent())
        .find(|message| message["method"] == "$readdir")
        .unwrap();
    assert_eq!(list["args"], json!([5, {"$graphiteUri": "vfstest-b:/"}]));
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.finish_vsix_virtual_fs(
        &mut runtime,
        &json!({"id": list["id"], "result": [["notes", 2], ["b.txt", 1]]}),
    );
    app.plugins.vsix = Some(runtime);
    let names: Vec<String> = app
        .virtual_explorer_rows()
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, ["vfstest-b", "notes", "b.txt"]);

    // Removing the provider removes the folder and the scheme.
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.handle_vsix_request(
        &mut runtime,
        &json!({"id": 2, "actor": "MainThreadFileSystem", "method": "$unregisterProvider", "args": [5]}),
    )
    .unwrap();
    app.plugins.vsix = Some(runtime);
    assert!(app.virtual_explorer_rows().is_empty());
    assert!(graphite_core_api::DocumentUri::parse(scheme_uri).is_err());
}

#[test]
fn vsix_virtual_file_operations_call_the_provider_and_refresh_the_folder() {
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    // The mirror root is process-wide, so these tests share one profile (their schemes differ).
    app.profile_root = std::env::temp_dir().join("graphite-vfs-tests-profile");
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);
    app.handle_vsix_request(
        &mut runtime,
        &json!({"id": 1, "actor": "MainThreadFileSystem", "method": "$registerFileSystemProvider",
            "args": [9, "vfstest-c", 2048, null]}),
    )
    .unwrap();
    while host.next_sent().is_some() {}
    assert!(runtime.vfs.is_readonly("vfstest-c"));

    let call = |host: &graphite_extension_host::code_oss::LoopbackHost, method: &str| {
        std::iter::from_fn(|| host.next_sent())
            .find(|message| message["method"] == method)
            .unwrap_or_else(|| panic!("no {method} call"))
    };
    app.request_virtual_stat(&mut runtime, "vfstest-c:/d/x.txt")
        .unwrap();
    let stat = call(&host, "$stat");
    assert_eq!(
        stat["args"],
        json!([9, {"$graphiteUri": "vfstest-c:/d/x.txt"}])
    );

    app.request_virtual_delete(&mut runtime, "vfstest-c:/d/x.txt")
        .unwrap();
    let delete = call(&host, "$delete");
    assert_eq!(
        delete["args"][1],
        json!({"$graphiteUri": "vfstest-c:/d/x.txt"})
    );
    assert_eq!(delete["args"][2]["recursive"], true);
    // A finished delete asks for the parent folder again.
    app.finish_vsix_virtual_fs(&mut runtime, &json!({"id": delete["id"], "result": null}));
    let refresh = call(&host, "$readdir");
    assert_eq!(refresh["args"][1], json!({"$graphiteUri": "vfstest-c:/d"}));

    app.request_virtual_rename(&mut runtime, "vfstest-c:/d/a", "vfstest-c:/d/b")
        .unwrap();
    let rename = call(&host, "$rename");
    assert_eq!(rename["args"][2], json!({"$graphiteUri": "vfstest-c:/d/b"}));
    // A failure is reported and does not refresh.
    app.finish_vsix_virtual_fs(
        &mut runtime,
        &json!({"id": rename["id"], "error": "NoPermissions"}),
    );
    assert!(app.status.contains("NoPermissions"));
    assert!(std::iter::from_fn(|| host.next_sent()).all(|message| message["method"] != "$readdir"));

    // An unknown scheme has no provider.
    assert!(app.request_virtual_stat(&mut runtime, "nope:/x").is_err());
    // A change event refetches expanded folders only.
    runtime.vfs.dirs_for_test("vfstest-c:/", true);
    app.handle_vsix_request(
        &mut runtime,
        &json!({"id": 3, "actor": "MainThreadFileSystem", "method": "$onFileSystemChange", "args": [9, []]}),
    )
    .unwrap();
    assert_eq!(
        call(&host, "$readdir")["args"][1],
        json!({"$graphiteUri": "vfstest-c:/"})
    );
}

/// Plays the extension host for authentication tests.
struct AuthHarness {
    app: App,
    runtime: Option<Runtime>,
    host: graphite_extension_host::code_oss::LoopbackHost,
}

impl AuthHarness {
    fn new(dir: &TempDir) -> Self {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.profile_root = dir.path().join("profile");
        let (session, host) = Session::loopback();
        Self {
            app,
            runtime: Some(Runtime::for_test(session)),
            host,
        }
    }

    fn request(&mut self, id: u64, method: &str, args: Value) {
        let mut runtime = self.runtime.take().unwrap();
        self.app
            .handle_vsix_request(&mut runtime, &json!({"id": id, "actor": "MainThreadAuthentication", "method": method, "args": args}))
            .unwrap();
        self.runtime = Some(runtime);
    }

    /// Everything sent to the extension host so far.
    fn sent(&self) -> Vec<Value> {
        std::iter::from_fn(|| self.host.next_sent()).collect()
    }

    fn call(&self, method: &str) -> Value {
        self.sent()
            .into_iter()
            .find(|message| message["method"] == method)
            .unwrap_or_else(|| panic!("no {method} call"))
    }

    fn reply_of(&self, id: u64) -> Value {
        self.sent()
            .into_iter()
            .find(|message| message["type"] == "reply" && message["id"] == id)
            .unwrap_or_else(|| panic!("no reply to {id}"))
    }

    fn finish(&mut self, call: &Value, outcome: Value) {
        let mut event = json!({"id": call["id"]});
        for (key, value) in outcome.as_object().unwrap() {
            event[key] = value.clone();
        }
        let mut runtime = self.runtime.take().unwrap();
        self.app.finish_vsix_auth(&mut runtime, &event);
        self.runtime = Some(runtime);
    }

    /// Opens the next consent prompt and answers it.
    fn answer_consent(&mut self, allow: bool) -> String {
        let mut runtime = self.runtime.take().unwrap();
        self.app.drive_vsix_auth(&mut runtime);
        self.app.plugins.vsix = Some(runtime);
        let title = self
            .app
            .command_palette
            .as_ref()
            .expect("consent prompt")
            .extension_prompt
            .clone()
            .unwrap()
            .0;
        // The palette closes itself before it reports the choice.
        self.app.command_palette = None;
        self.app.answer_vsix_prompt(Some((
            String::new(),
            Some(if allow { "1" } else { "2" }.into()),
        )));
        self.runtime = self.app.plugins.vsix.take();
        title
    }
}

fn sample_session() -> Value {
    json!({"id": "s1", "accessToken": "tok", "account": {"id": "u1", "label": "User"}, "scopes": ["a"]})
}

#[test]
fn vsix_authentication_providers_release_sessions_only_after_consent() {
    use super::auth::AccessStore;
    let dir = TempDir::new();
    let mut h = AuthHarness::new(&dir);
    h.request(
        1,
        "$registerAuthenticationProvider",
        json!([{"id": "sample", "label": "Sample", "supportsMultipleAccounts": false}]),
    );
    assert_eq!(h.reply_of(1)["result"], Value::Null);
    let get = |silent: bool, create: bool| json!(["sample", ["a"], "pub.ext", "Pub Ext", {"silent": silent, "createIfNone": create}]);

    // A silent request without a session answers undefined and never asks.
    h.request(10, "$getSession", get(true, false));
    let lookup = h.call("$getSessions");
    assert_eq!(lookup["actor"], "ExtHostAuthentication");
    assert_eq!(lookup["args"], json!(["sample", ["a"], {}]));
    h.finish(&lookup, json!({"result": []}));
    assert_eq!(h.reply_of(10)["result"], Value::Null);

    // createIfNone asks the user, then creates the session and allows the extension for it.
    h.request(11, "$getSession", get(false, true));
    let lookup = h.call("$getSessions");
    h.finish(&lookup, json!({"result": []}));
    let title = h.answer_consent(true);
    assert!(
        title.contains("Pub Ext") && title.contains("Sample"),
        "{title}"
    );
    let create = h.call("$createSession");
    assert_eq!(create["args"], json!(["sample", ["a"], {}]));
    h.finish(&create, json!({"result": sample_session()}));
    let reply = h.reply_of(11);
    assert_eq!(reply["result"]["accessToken"], "tok");
    assert!(AccessStore::load(&h.app.profile_root).is_allowed("sample", "u1", "pub.ext"));

    // The same extension now gets the session without a question.
    h.request(12, "$getSession", get(true, false));
    let lookup = h.call("$getSessions");
    h.finish(&lookup, json!({"result": [sample_session()]}));
    assert_eq!(h.reply_of(12)["result"]["id"], "s1");
    assert!(h.app.command_palette.is_none());

    // Another extension has to be allowed, and a refusal is an error.
    h.request(
        13,
        "$getSession",
        json!(["sample", ["a"], "other.ext", "Other", {}]),
    );
    let lookup = h.call("$getSessions");
    h.finish(&lookup, json!({"result": [sample_session()]}));
    let title = h.answer_consent(false);
    assert!(title.contains("Other") && title.contains("User"), "{title}");
    assert_eq!(h.reply_of(13)["error"], "User did not consent to login.");
    assert!(!AccessStore::load(&h.app.profile_root).is_allowed("sample", "u1", "other.ext"));

    // A silent request from an extension that was not allowed learns nothing.
    h.request(
        14,
        "$getSession",
        json!(["sample", ["a"], "other.ext", "Other", {"silent": true}]),
    );
    let lookup = h.call("$getSessions");
    h.finish(&lookup, json!({"result": [sample_session()]}));
    assert_eq!(h.reply_of(14)["result"], Value::Null);

    // Accounts are the distinct accounts of the sessions.
    h.request(15, "$getAccounts", json!(["sample"]));
    let lookup = h.call("$getSessions");
    assert_eq!(lookup["args"], json!(["sample", null, {}]));
    h.finish(
        &lookup,
        json!({"result": [sample_session(), sample_session()]}),
    );
    assert_eq!(
        h.reply_of(15)["result"],
        json!([{"id": "u1", "label": "User"}])
    );

    // Removing a session and session changes go through to the extensions.
    h.request(16, "$removeSession", json!(["sample", "s1"]));
    let removal = h.call("$removeSession");
    assert_eq!(removal["args"], json!(["sample", "s1"]));
    h.finish(&removal, json!({"result": null}));
    assert_eq!(h.reply_of(16)["result"], Value::Null);
    h.request(
        17,
        "$sendDidChangeSessions",
        json!(["sample", {"added": [], "removed": [], "changed": []}]),
    );
    let change = h.call("$onDidChangeAuthenticationSessions");
    assert_eq!(change["args"], json!(["sample", "Sample"]));
}

#[test]
fn vsix_authentication_waits_for_a_provider_that_registers_late() {
    let dir = TempDir::new();
    let mut h = AuthHarness::new(&dir);
    h.request(
        20,
        "$getSession",
        json!(["late", ["a"], "pub.ext", "Pub", {"createIfNone": true}]),
    );
    let sent = h.sent();
    let activation = sent
        .iter()
        .find(|message| message["type"] == "activateByEvent")
        .expect("activation");
    assert_eq!(activation["event"], "onAuthenticationRequest:late");
    assert!(
        sent.iter().all(|message| message["id"] != 20),
        "no answer while waiting"
    );
    h.request(
        21,
        "$registerAuthenticationProvider",
        json!([{"id": "late", "label": "Late"}]),
    );
    assert_eq!(h.call("$getSessions")["args"][0], "late");

    // A provider that never shows up fails the call once the wait is over.
    h.request(
        22,
        "$getSession",
        json!(["never", ["a"], "pub.ext", "Pub", {}]),
    );
    let mut runtime = h.runtime.take().unwrap();
    runtime.auth.expire_waiting_for_test();
    h.app.drive_vsix_auth(&mut runtime);
    h.runtime = Some(runtime);
    assert_eq!(
        h.reply_of(22)["error"],
        "No authentication provider 'never' is currently registered."
    );
    // Silent requests for an unknown provider answer undefined at once.
    h.request(
        23,
        "$getSession",
        json!(["never", ["a"], "pub.ext", "Pub", {"silent": true}]),
    );
    assert_eq!(h.reply_of(23)["result"], Value::Null);
}

#[test]
fn vsix_github_sessions_come_from_graphites_own_sign_in() {
    use super::auth::AccessStore;
    let dir = TempDir::new();
    let mut h = AuthHarness::new(&dir);
    h.app.github_session = Some(graphite_auth::Session {
        token: "gho_x".into(),
        login: "octocat".into(),
        scope: "read:user user:email".into(),
        ..Default::default()
    });
    let get = |scopes: Value, silent: bool, create: bool| json!(["github", scopes, "pub.ext", "Pub Ext", {"silent": silent, "createIfNone": create}]);

    h.request(30, "$getSession", get(json!(["read:user"]), true, false));
    assert_eq!(h.reply_of(30)["result"], Value::Null, "not allowed yet");
    h.request(31, "$getSession", get(json!(["read:user"]), false, false));
    let title = h.answer_consent(true);
    assert!(
        title.contains("GitHub") && title.contains("octocat"),
        "{title}"
    );
    let session = h.reply_of(31)["result"].clone();
    assert_eq!(session["accessToken"], "gho_x");
    assert_eq!(session["scopes"], json!(["read:user"]));
    assert_eq!(
        session["account"],
        json!({"id": "octocat", "label": "octocat"})
    );
    assert!(AccessStore::load(&h.app.profile_root).is_allowed("github", "octocat", "pub.ext"));
    h.request(32, "$getSession", get(json!(["user:email"]), true, false));
    assert_eq!(h.reply_of(32)["result"]["accessToken"], "gho_x");

    // Scopes the sign-in does not have: silent requests get nothing; others offer a new sign-in.
    h.request(33, "$getSession", get(json!(["gist"]), true, false));
    assert_eq!(h.reply_of(33)["result"], Value::Null);
    h.request(34, "$getSession", get(json!(["gist"]), false, true));
    let title = h.answer_consent(true);
    assert!(title.contains("sign in"), "{title}");
    // Tests have no event loop, so the sign-in cannot start; the caller hears why.
    assert!(h.reply_of(34)["error"].is_string());

    // With a sign-in in progress the call waits, then receives the account that signed in.
    h.app.github_sign_in_pending = true;
    h.request(35, "$getSession", get(json!(["gist"]), false, true));
    h.answer_consent(true);
    assert!(h.sent().iter().all(|message| message["id"] != 35));
    h.app.plugins.vsix = h.runtime.take();
    h.app.handle_github_auth_event(
        crate::app::source_control::github_auth::GitHubAuthEvent::Finished(Ok(
            graphite_auth::Session {
                token: "gho_new".into(),
                login: "octocat".into(),
                scope: "read:user gist".into(),
                ..Default::default()
            },
        )),
    );
    h.runtime = h.app.plugins.vsix.take();
    let sent = h.sent();
    let reply = sent
        .iter()
        .find(|message| message["id"] == 35)
        .expect("deferred reply");
    assert_eq!(reply["result"]["accessToken"], "gho_new");
    assert!(sent.iter().any(
        |message| message["method"] == "$onDidChangeAuthenticationSessions"
            && message["args"] == json!(["github", "GitHub"])
    ));

    // Accounts and sign-out.
    h.request(36, "$getAccounts", json!(["github"]));
    assert_eq!(
        h.reply_of(36)["result"],
        json!([{"id": "octocat", "label": "octocat"}])
    );
    h.request(37, "$removeSession", json!(["github", "github-octocat"]));
    assert_eq!(h.reply_of(37)["result"], Value::Null);
    assert!(h.app.github_session.is_none());
    assert!(!AccessStore::load(&h.app.profile_root).is_allowed("github", "octocat", "pub.ext"));
    h.request(38, "$getAccounts", json!(["github"]));
    assert_eq!(h.reply_of(38)["result"], json!([]));
}

#[test]
fn vsix_custom_editors_replace_the_text_editor_and_route_save_and_undo() {
    let dir = TempDir::new();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let drawing = work.join("art.draw");
    let notes = work.join("todo.notes");
    std::fs::write(&drawing, "binary-ish").unwrap();
    std::fs::write(&notes, "- one\n").unwrap();

    let mut app = App::new_with_options(work.clone(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");
    let (session, host) = Session::loopback();
    app.plugins.vsix = Some(Runtime::for_test(session));
    let editor = |view_type: &str, pattern: &str, priority: &str| {
        graphite_plugins::manifest::CustomEditorContribution {
            view_type: view_type.into(),
            display_name: format!("{view_type} editor"),
            patterns: vec![pattern.into()],
            priority: priority.into(),
        }
    };
    app.plugins.installed_plugins_mut().push(Installed {
        metadata: Metadata::Vsix(graphite_plugins::manifest::Vsix {
            id: "test.editors".into(),
            version: "1.0.0".into(),
            main: Some("index.js".into()),
            custom_editors: vec![
                editor("sample.paint", "*.draw", "default"),
                editor("sample.notes", "*.notes", "option"),
            ],
            activation_events: vec!["onCustomEditor:sample.paint".into()],
            ..Default::default()
        }),
        enabled: true,
        trusted_version: Some("1.0.0".into()),
        directory: "test.editors@1.0.0".into(),
        dev_path: None,
    });
    let drain = |host: &graphite_extension_host::code_oss::LoopbackHost| -> Vec<Value> {
        std::iter::from_fn(|| host.next_sent()).collect()
    };
    let find = |messages: &[Value], method: &str| -> Value {
        messages
            .iter()
            .find(|message| message["method"] == method)
            .cloned()
            .unwrap_or_else(|| panic!("no {method} call in {messages:?}"))
    };
    let request = |app: &mut App, id: u64, method: &str, args: Value| {
        let mut runtime = app.plugins.vsix.take().unwrap();
        app.handle_vsix_request(
            &mut runtime,
            &json!({"id": id, "actor": "MainThreadCustomEditors", "method": method, "args": args}),
        )
        .unwrap();
        app.plugins.vsix = Some(runtime);
    };
    let drive = |app: &mut App| {
        let mut runtime = app.plugins.vsix.take().unwrap();
        app.drive_vsix_custom_editors(&mut runtime);
        app.plugins.vsix = Some(runtime);
    };
    let finish = |app: &mut App, call: &Value, outcome: Value| {
        let mut event = json!({"id": call["id"]});
        for (key, value) in outcome.as_object().unwrap() {
            event[key] = value.clone();
        }
        let mut runtime = app.plugins.vsix.take().unwrap();
        app.finish_vsix_custom_editor(&mut runtime, &event);
        app.plugins.vsix = Some(runtime);
    };

    // Files with a default custom editor open with it; the extension is activated for it.
    assert_eq!(app.custom_editor_choices(&notes).len(), 1);
    assert!(
        app.default_custom_editor(&notes).is_none(),
        "option editors do not open by default"
    );
    app.open_file(drawing.clone());
    let id = app
        .documents
        .id_for_path(&drawing)
        .expect("the file is open");
    assert!(app.custom_editor_is_bound(&drawing));
    let state = app.custom_editor_state(id).unwrap();
    assert!(!state.ready && state.display_name == "sample.paint editor");
    let activation = drain(&host);
    assert!(activation
        .iter()
        .any(|m| m["type"] == "activateByEvent" && m["event"] == "onCustomEditor:sample.paint"));
    // Nothing is asked until the provider registers.
    drive(&mut app);
    assert!(drain(&host)
        .iter()
        .all(|m| m["method"] != "$createCustomDocument"));

    // Custom documents are created, then the provider fills the page.
    request(
        &mut app,
        1,
        "$registerCustomEditorProvider",
        json!([{"id": "test.editors", "location": null}, "sample.paint", {}, {}, false, false]),
    );
    drive(&mut app);
    let sent = drain(&host);
    let create = find(&sent, "$createCustomDocument");
    assert_eq!(create["actor"], "ExtHostCustomEditors");
    let resource = create["args"][0]["$graphiteUri"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(resource.ends_with("/art.draw"));
    assert_eq!(create["args"][1], "sample.paint");
    assert!(
        app.documents[id].buffer.is_read_only(),
        "the text behind the page is not editable"
    );
    finish(&mut app, &create, json!({"result": {"editable": true}}));
    let resolve = find(&drain(&host), "$resolveCustomEditor");
    let handle = resolve["args"][1].as_str().unwrap().to_owned();
    assert_eq!(resolve["args"][0], json!({"$graphiteUri": resource}));
    assert_eq!(resolve["args"][2], "sample.paint");
    assert_eq!(resolve["args"][3]["title"], "art.draw");
    assert!(app
        .plugins
        .vsix
        .as_ref()
        .unwrap()
        .webviews
        .contains(&handle));
    finish(&mut app, &resolve, json!({"result": null}));
    let state = app.custom_editor_state(id).unwrap();
    assert!(state.ready && state.writable && !state.can_undo);

    // The page arrives over the webview bridge.
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.handle_vsix_request(
        &mut runtime,
        &json!({"id": 2, "actor": "MainThreadWebviews",
        "method": "$setHtml", "args": [handle, "<p>canvas</p>"]}),
    )
    .unwrap();
    assert_eq!(runtime.webviews.panels[&handle].html, "<p>canvas</p>");
    app.plugins.vsix = Some(runtime);
    drain(&host);

    // Edits the extension reports make the tab dirty; save goes to the extension.
    request(
        &mut app,
        3,
        "$onDidEdit",
        json!([resource, "sample.paint", 1, "Draw"]),
    );
    app.sync_custom_editor_webviews();
    assert!(app.documents[id].is_dirty());
    assert!(app.custom_editor_state(id).unwrap().can_undo);
    assert!(app.route_custom_editor_action(EditorAction::Save));
    let save = find(&drain(&host), "$onSave");
    assert_eq!(
        save["args"],
        json!([{"$graphiteUri": resource}, "sample.paint"])
    );
    assert_eq!(save["cancellable"], true);
    assert!(
        app.documents[id].is_dirty(),
        "dirty until the extension has saved"
    );
    finish(&mut app, &save, json!({"result": null}));
    app.sync_custom_editor_webviews();
    assert!(!app.documents[id].is_dirty());

    // Undo and redo go to the extension with the id of the edit.
    request(
        &mut app,
        4,
        "$onDidEdit",
        json!([resource, "sample.paint", 2, null]),
    );
    assert!(app.route_custom_editor_action(EditorAction::Undo));
    let undo = find(&drain(&host), "$undo");
    assert_eq!(
        undo["args"],
        json!([{"$graphiteUri": resource}, "sample.paint", 2, false])
    );
    assert!(app.route_custom_editor_action(EditorAction::Redo));
    let redo = find(&drain(&host), "$redo");
    assert_eq!(redo["args"][2], 2);
    assert_eq!(redo["args"][3], true);
    assert!(
        app.route_custom_editor_action(EditorAction::Redo),
        "nothing to redo is still handled"
    );
    assert!(drain(&host).iter().all(|m| m["method"] != "$redo"));

    // Open With: the text editor releases the page; the choice sticks across opens.
    app.open_with_palette();
    let ids: Vec<String> = app
        .command_palette
        .as_ref()
        .unwrap()
        .items
        .iter()
        .map(|i| i.id.clone())
        .collect();
    assert_eq!(
        ids,
        ["graphite.openWith:", "graphite.openWith:sample.paint"]
    );
    app.command_palette = None;
    app.reopen_with("");
    assert!(!app.custom_editor_is_bound(&drawing));
    assert!(
        !app.documents[id].buffer.is_read_only(),
        "the text is editable again"
    );
    assert!(!app.documents[id].is_dirty());
    let sent = drain(&host);
    assert!(sent
        .iter()
        .any(|m| m["method"] == "$onDidDisposeWebviewPanel" && m["args"][0] == json!(handle)));
    assert!(sent.iter().any(|m| m["method"] == "$disposeCustomDocument"));
    app.open_file(drawing.clone());
    assert!(
        !app.custom_editor_is_bound(&drawing),
        "the text choice is kept"
    );
    app.reopen_with("sample.paint");
    assert!(app.custom_editor_is_bound(&drawing));

    // A text editor works on the open text document and keeps its own undo and save.
    request(
        &mut app,
        5,
        "$registerTextEditorProvider",
        json!([{"id": "test.editors", "location": null}, "sample.notes", {}, {}, false]),
    );
    app.open_file_as_text(notes.clone());
    let notes_id = app.documents.id_for_path(&notes).unwrap();
    app.reopen_with("sample.notes");
    drive(&mut app);
    let sent = drain(&host);
    assert!(sent
        .iter()
        .all(|m| m["method"] != "$createCustomDocument" || m["args"][1] != "sample.notes"));
    let resolve = sent
        .iter()
        .find(|m| m["method"] == "$resolveCustomEditor" && m["args"][2] == "sample.notes")
        .expect("a text editor is resolved without a custom document");
    finish(&mut app, resolve, json!({"result": null}));
    let state = app.custom_editor_state(notes_id).unwrap();
    assert!(state.ready && state.kind == crate::app::plugins::vsix::editors::Kind::Text);
    assert!(!state.writable && !app.documents[notes_id].buffer.is_read_only());
    assert!(
        !app.route_custom_editor_action(EditorAction::Save),
        "the document saves itself"
    );

    // Removing a provider returns its files to the text editor.
    request(
        &mut app,
        6,
        "$unregisterEditorProvider",
        json!(["sample.notes"]),
    );
    assert!(!app.custom_editor_is_bound(&notes));
    app.sync_custom_editor_webviews();
    assert!(app.custom_editor_is_bound(&drawing), "other editors stay");
}

/// An app with a trusted workspace, a ready extension host and one installed extension that
/// contributes the notebook type `sample-book` for `*.sbook` files.
fn notebook_test_app(
    dir: &TempDir,
) -> (
    App,
    graphite_extension_host::code_oss::LoopbackHost,
    PathBuf,
) {
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let mut app = App::new_with_options(work.clone(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");
    let (session, host) = Session::loopback();
    app.plugins.vsix = Some(Runtime::for_test(session));
    app.plugins.installed_plugins_mut().push(Installed {
        metadata: Metadata::Vsix(graphite_plugins::manifest::Vsix {
            id: "test.books".into(),
            version: "1.0.0".into(),
            main: Some("index.js".into()),
            notebooks: vec![graphite_plugins::manifest::CustomEditorContribution {
                view_type: "sample-book".into(),
                display_name: "Sample Book".into(),
                patterns: vec!["*.sbook".into()],
                priority: "default".into(),
            }],
            activation_events: vec!["onNotebook:sample-book".into()],
            ..Default::default()
        }),
        enabled: true,
        trusted_version: Some("1.0.0".into()),
        directory: "test.books@1.0.0".into(),
        dev_path: None,
    });
    (app, host, work)
}

fn notebook_request(app: &mut App, actor: &str, id: u64, method: &str, args: Value) {
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.handle_vsix_request(
        &mut runtime,
        &json!({"id": id, "actor": actor, "method": method, "args": args}),
    )
    .unwrap();
    app.plugins.vsix = Some(runtime);
}

fn notebook_finish(app: &mut App, call: &Value, outcome: Value) {
    let mut event = json!({"id": call["id"]});
    for (key, value) in outcome.as_object().unwrap() {
        event[key] = value.clone();
    }
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.finish_vsix_notebook(&mut runtime, &event);
    app.plugins.vsix = Some(runtime);
}

fn notebook_drive(app: &mut App) {
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.drive_vsix_notebooks(&mut runtime);
    app.plugins.vsix = Some(runtime);
}

#[test]
fn vsix_notebook_controllers_run_cells_through_the_extension() {
    use crate::app::notebooks::notebook_kernel::RunTarget;
    use base64::Engine;
    let dir = TempDir::new();
    let (mut app, host, work) = notebook_test_app(&dir);
    let file = work.join("a.ipynb");
    std::fs::write(
        &file,
        r##"{"cells":[{"cell_type":"code","metadata":{},"source":["print(1)"],"outputs":[],"execution_count":null,"id":"c1"},
        {"cell_type":"markdown","metadata":{},"source":["# t"],"id":"c2"}],"metadata":{},"nbformat":4,"nbformat_minor":5}"##,
    )
    .unwrap();
    app.open_file(file.clone());
    assert!(
        app.active_notebook().is_some(),
        "the notebook opened in the notebook editor"
    );

    // A registered controller is offered for the notebook and can be started.
    notebook_request(
        &mut app,
        "MainThreadNotebookKernels",
        1,
        "$addKernel",
        json!([7, {
        "id": "ext.kernel", "notebookType": "jupyter-notebook", "extensionId": {"value": "test.books"},
        "label": "Ext Kernel", "detail": "1.0", "supportedLanguages": ["python"]}]),
    );
    notebook_request(
        &mut app,
        "MainThreadNotebookKernels",
        2,
        "$addKernel",
        json!([8, {
        "id": "ext.other", "notebookType": "another-type", "label": "Other"}]),
    );
    let specs = app.vsix_notebook_kernel_specs();
    assert_eq!(
        specs.len(),
        1,
        "only controllers of this notebook type: {specs:?}"
    );
    assert_eq!(specs[0].name, "vsix:7");
    assert_eq!(specs[0].display_name, "Ext Kernel (1.0)");
    app.start_extension_kernel(specs[0].clone()).unwrap();
    while host.next_sent().is_some() {}

    // Running a cell sends it to the extension with the notebook it belongs to.
    app.run_notebook_cells(RunTarget::Selected).unwrap();
    notebook_drive(&mut app);
    let sent: Vec<Value> = std::iter::from_fn(|| host.next_sent()).collect();
    let find = |method: &str| {
        sent.iter()
            .find(|m| m["method"] == method)
            .cloned()
            .unwrap_or_else(|| panic!("no {method}: {sent:?}"))
    };
    let cells = find("$acceptDocumentsAndEditorsDelta");
    assert_eq!(
        cells["args"][0]["addedDocuments"].as_array().unwrap().len(),
        2,
        "a document for each cell"
    );
    let notebook = find("$acceptDocumentAndEditorsDelta");
    assert_eq!(notebook["actor"], "ExtHostNotebook");
    assert_eq!(notebook["serializable"], json!([0]));
    let model = &notebook["args"][0]["addedDocuments"][0];
    assert_eq!(model["viewType"], "jupyter-notebook");
    assert_eq!(model["cells"][0]["source"], json!(["print(1)"]));
    assert_eq!(model["cells"][0]["cellKind"], 2);
    assert_eq!(model["cells"][1]["cellKind"], 1);
    let execute = find("$executeCells");
    assert_eq!(execute["actor"], "ExtHostNotebookKernels");
    assert_eq!(execute["args"][0], 7);
    let handle = model["cells"][0]["handle"].clone();
    assert_eq!(execute["args"][2], json!([handle]));
    let notebook_uri = execute["args"][1]["$graphiteUri"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(app.active_notebook().unwrap().is_running());

    // The execution the extension creates streams outputs and completes.
    let encode = |text: &str| base64::engine::general_purpose::STANDARD.encode(text);
    notebook_request(
        &mut app,
        "MainThreadNotebookKernels",
        3,
        "$createExecution",
        json!([100, "ext.kernel", notebook_uri, handle]),
    );
    notebook_request(
        &mut app,
        "MainThreadNotebookKernels",
        4,
        "$updateExecution",
        json!([100, {"value": [
        {"editType": 3, "executionOrder": 5},
        {"editType": 1, "append": false, "outputs": [{"outputId": "o1", "items": [
            {"mime": "application/vnd.code.notebook.stdout", "valueBytes": {"$bytes": encode("hello\n")}}]}]},
    ], "buffers": []}]),
    );
    notebook_request(
        &mut app,
        "MainThreadNotebookKernels",
        5,
        "$updateExecution",
        json!([100, [
        {"editType": 2, "outputId": "o1", "append": true, "items": [
            {"mime": "application/vnd.code.notebook.stdout", "valueBytes": {"$bytes": encode("world\n")}}]}]]),
    );
    notebook_request(
        &mut app,
        "MainThreadNotebookKernels",
        6,
        "$completeExecution",
        json!([100, {"value": {"lastRunSuccess": true}}]),
    );
    notebook_finish(&mut app, &execute, json!({"result": null}));
    app.poll_kernels();
    let session = app.active_notebook().unwrap();
    assert!(!session.is_running());
    assert_eq!(session.cells[0].cell.execution_count, Some(5));
    assert_eq!(session.cells[0].cell.outputs.len(), 1);
    assert_eq!(session.cells[0].cell.outputs[0]["text"], "hello\nworld\n");

    // A failing handler ends the run with an error output.
    app.run_notebook_cells(RunTarget::Selected).unwrap();
    notebook_drive(&mut app);
    let execute = std::iter::from_fn(|| host.next_sent())
        .find(|m| m["method"] == "$executeCells")
        .unwrap();
    notebook_finish(&mut app, &execute, json!({"error": "Boom"}));
    app.poll_kernels();
    let session = app.active_notebook().unwrap();
    assert!(!session.is_running());
    assert_eq!(
        session.cells[0].cell.outputs.last().unwrap()["output_type"],
        "error"
    );
    assert_eq!(
        session.cells[0].cell.outputs.last().unwrap()["evalue"],
        "Boom"
    );

    // Interrupting reaches the extension; removing the controller ends the kernel.
    app.run_notebook_cells(RunTarget::Selected).unwrap();
    notebook_drive(&mut app);
    while host.next_sent().is_some() {}
    app.with_notebook(|session| session.interrupt_kernel());
    notebook_drive(&mut app);
    let cancel = std::iter::from_fn(|| host.next_sent())
        .find(|m| m["method"] == "$cancelCells")
        .expect("cancel");
    assert_eq!(cancel["args"][0], 7);
    notebook_request(
        &mut app,
        "MainThreadNotebookKernels",
        7,
        "$removeKernel",
        json!([7]),
    );
    assert!(app.vsix_notebook_kernel_specs().is_empty());
}

#[test]
fn vsix_notebook_serializers_open_and_save_files_in_the_notebook_editor() {
    use base64::Engine;
    let encode = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let dir = TempDir::new();
    let (mut app, host, work) = notebook_test_app(&dir);
    let file = work.join("story.sbook");
    std::fs::write(&file, "abc").unwrap();
    assert_eq!(
        app.default_notebook_type(&file).as_deref(),
        Some("sample-book")
    );
    assert!(
        app.default_notebook_type(&work.join("a.ipynb")).is_none(),
        "ipynb keeps the Jupyter editor"
    );
    assert!(app.default_notebook_type(&work.join("a.txt")).is_none());

    // The provider is not there yet: the open waits for it and the extension is activated.
    app.open_file(file.clone());
    let sent: Vec<Value> = std::iter::from_fn(|| host.next_sent()).collect();
    assert!(sent
        .iter()
        .any(|m| m["type"] == "activateByEvent" && m["event"] == "onNotebook:sample-book"));
    assert!(sent.iter().all(|m| m["method"] != "$dataToNotebook"));
    assert!(
        app.documents.id_for_path(&file).is_none(),
        "the raw file is not opened as text"
    );
    notebook_request(
        &mut app,
        "MainThreadNotebook",
        1,
        "$registerNotebookSerializer",
        json!([3, {"id": "test.books", "location": null}, "sample-book", {}, null]),
    );
    let read = std::iter::from_fn(|| host.next_sent())
        .find(|m| m["method"] == "$dataToNotebook")
        .expect("read call");
    assert_eq!(read["actor"], "ExtHostNotebook");
    assert_eq!(read["args"], json!([3, {"$graphiteBytes": encode(b"abc")}]));

    // The serializer's notebook opens in the notebook editor, through a mirror.
    notebook_finish(
        &mut app,
        &read,
        json!({"result": {"value": {
        "metadata": {"language_info": {"name": "python"}},
        "cells": [{"cellKind": 2, "source": "x = 1", "language": "python", "outputs": []},
                  {"cellKind": 1, "source": "# Story", "language": "markdown", "outputs": []}]}}}),
    );
    let session = app.active_notebook().expect("the notebook opened");
    assert_eq!(session.cells.len(), 2);
    assert_eq!(session.cells[0].text(), "x = 1");
    let mirror = app.documents[app.active_document()]
        .buffer
        .path()
        .unwrap()
        .to_path_buf();
    assert!(mirror.starts_with(app.profile_root.join("notebook-mirror")));
    assert_eq!(mirror.file_name().unwrap(), "story.sbook.ipynb");
    assert_eq!(
        app.serialized_notebook_origin(&mirror).as_deref(),
        Some(file.as_path())
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "abc",
        "opening does not touch the file"
    );

    // Saving the mirror writes the real file through the serializer.
    app.upload_serialized_notebook(&mirror);
    let write = std::iter::from_fn(|| host.next_sent())
        .find(|m| m["method"] == "$notebookToData")
        .expect("write call");
    assert_eq!(write["serializable"], json!([1]));
    assert_eq!(write["bytes"], true);
    assert_eq!(write["args"][0], 3);
    let data = &write["args"][1];
    assert_eq!(data["cells"][0]["source"], "x = 1");
    assert_eq!(data["cells"][0]["cellKind"], 2);
    assert_eq!(data["cells"][1]["cellKind"], 1);
    notebook_finish(
        &mut app,
        &write,
        json!({"result": {"$bytes": encode(b"serialized!")}}),
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "serialized!");

    // A serializer error is reported and leaves the file alone.
    app.upload_serialized_notebook(&mirror);
    let write = std::iter::from_fn(|| host.next_sent())
        .find(|m| m["method"] == "$notebookToData")
        .unwrap();
    notebook_finish(&mut app, &write, json!({"error": "cannot write"}));
    assert!(app.status.contains("cannot write"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "serialized!");
    // Other files do not go through a serializer.
    app.upload_serialized_notebook(&work.join("plain.txt"));
    assert!(std::iter::from_fn(|| host.next_sent()).all(|m| m["method"] != "$notebookToData"));

    // A failing read falls back to the text editor.
    let broken = work.join("broken.sbook");
    std::fs::write(&broken, "xyz").unwrap();
    app.open_file(broken.clone());
    let read = std::iter::from_fn(|| host.next_sent())
        .find(|m| m["method"] == "$dataToNotebook")
        .unwrap();
    notebook_finish(&mut app, &read, json!({"error": "not a book"}));
    assert!(
        app.documents.id_for_path(&broken).is_some(),
        "opened as text instead"
    );
}

#[test]
fn vsix_decorations_offer_hover_messages_gutter_icons_and_ruler_marks() {
    let dir = TempDir::new();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let file = work.join("a.txt");
    std::fs::write(&file, "héllo wörld\nsecond\n").unwrap();
    let mut app = App::new_with_options(work, None, true);
    app.profile_root = dir.path().join("profile");
    app.open_file(file.clone());
    let (session, _host) = Session::loopback();
    app.plugins.vsix = Some(Runtime::for_test(session));
    let uri = app.decoration_uri(&file).unwrap();
    let call = |app: &mut App, method: &str, args: Value| {
        let mut runtime = app.plugins.vsix.take().unwrap();
        app.handle_vsix_request(
            &mut runtime,
            &json!({"id": 1, "actor": "MainThreadTextEditors", "method": method, "args": args}),
        )
        .unwrap();
        app.plugins.vsix = Some(runtime);
    };
    call(
        &mut app,
        "$registerTextEditorDecorationType",
        json!([
        "ext", "deco-1",
        {"gutterIconPath": "/icons/dot.svg", "overviewRulerColor": "#ff0000",
         "before": {"contentText": "> "}}]),
    );
    call(
        &mut app,
        "$trySetDecorations",
        json!([uri, "deco-1", [
            {"range": {"startLineNumber": 1, "startColumn": 7, "endLineNumber": 1, "endColumn": 12},
             "hoverMessage": [{"value": "about wörld"}]},
            {"range": {"startLineNumber": 2, "startColumn": 1, "endLineNumber": 2, "endColumn": 3}},
        ]]),
    );

    // The cursor inside the first range shows its message (columns count UTF-16 units).
    app.editor_mut().set_cursor_line_col(0, 8);
    assert_eq!(
        app.decoration_hover_at_cursor().as_deref(),
        Some("about wörld")
    );
    app.editor_mut().set_cursor_line_col(0, 2);
    assert_eq!(app.decoration_hover_at_cursor(), None, "outside the range");
    app.editor_mut().set_cursor_line_col(1, 0);
    assert_eq!(
        app.decoration_hover_at_cursor(),
        None,
        "a range without a message"
    );

    // The command shows it even when no provider answers, and keeps what a provider answered.
    app.editor_mut().set_cursor_line_col(0, 8);
    app.hover_text = None;
    app.run_ui_command("editor.action.showHover");
    assert_eq!(app.hover_text.as_deref(), Some("about wörld"));
    app.hover_text = Some("from a provider".into());
    app.run_ui_command("editor.action.showHover");
    assert_eq!(app.hover_text.as_deref(), Some("from a provider"));

    // Gutter icons and before text come with the first line of each range; the ruler lists lines.
    let marks = app.vsix_line_decorations(&uri, 0, true);
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].before.as_ref().unwrap().text, "> ");
    assert_eq!(
        marks[0].gutter_icon.as_deref(),
        Some(std::path::Path::new("/icons/dot.svg"))
    );
    assert_eq!(
        app.vsix_overview_marks(&uri, true)
            .iter()
            .map(|(line, _)| *line)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(app
        .vsix_overview_marks("file:///elsewhere", true)
        .is_empty());
    // A missing icon file is remembered as missing and does not fail.
    assert!(crate::icon_theme::load_file_image(std::path::Path::new("/icons/dot.svg")).is_none());
}

#[test]
fn vsix_source_control_lists_resources_forwards_the_commit_box_and_opens_resources() {
    use crate::app::scm_ui::{ScmListRow, ScmListRowKind, ScmRect};
    use winit::keyboard::{Key, NamedKey};

    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);
    let root_uri = graphite_core_api::DocumentUri::from_path(dir.path()).unwrap();
    let call = |app: &mut App, runtime: &mut Runtime, method: &str, args: Value| {
        app.handle_vsix_request(
            runtime,
            &json!({"id": 1, "actor": "MainThreadSCM", "method": method, "args": args}),
        )
        .unwrap();
    };

    call(
        &mut app,
        &mut runtime,
        "$registerSourceControl",
        json!([
            1,
            null,
            "sample",
            "Sample SCM",
            root_uri.as_str(),
            null,
            false,
            "scm://sample/input"
        ]),
    );
    call(
        &mut app,
        &mut runtime,
        "$registerGroups",
        json!([1, [[10, "changes", "Changes", {}, false]], [
            [10, [[0, 0, [
                [100, "file:///work/a.txt", [null, null], "Modified", false, false, "", {"id": "sample.open", "title": "Open"}],
                [101, "file:///work/b.txt", [null, null], "", true, true, "", null],
            ]]]]
        ]]),
    );
    call(
        &mut app,
        &mut runtime,
        "$updateSourceControl",
        json!([1, {"count": 2, "acceptInputCommand": {"id": "sample.commit", "title": "Commit"}}]),
    );
    call(
        &mut app,
        &mut runtime,
        "$setInputBoxPlaceholder",
        json!([1, "Say something"]),
    );
    app.plugins.vsix = Some(runtime);

    let rows = app.vsix_scm_rows();
    assert_eq!(rows.len(), 5, "{rows:?}");
    assert_eq!(app.vsix_scm().unwrap().providers()[0].display_count(), 2);
    assert_eq!(
        app.vsix_scm().unwrap().providers()[0]
            .placeholder
            .as_deref(),
        Some("Say something")
    );

    // The panel layout appends the extension rows after the Git ones.
    let layout = crate::app::scm_ui::scm_panel_layout(
        &crate::scm::Snapshot::default(),
        300.0,
        900.0,
        20.0,
        9.0,
        0,
        false,
        false,
        false,
        &rows,
    );
    let listed = layout
        .list_rows
        .iter()
        .filter(|row| matches!(row.kind, ScmListRowKind::Extension(_)))
        .count();
    assert_eq!(listed, 5);

    // Clicking the commit box focuses it, typing forwards the value to the extension.
    let row = |action_x: f32| ScmListRow {
        bounds: ScmRect {
            x: 0.0,
            y: 0.0,
            width: 300.0,
            height: 20.0,
        },
        chevron: ScmRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 20.0,
        },
        action: ScmRect {
            x: action_x,
            y: 0.0,
            width: 18.0,
            height: 20.0,
        },
        kind: ScmListRowKind::Extension(rows[1]),
    };
    app.click_extension_scm_row(&row(250.0), rows[1], 20.0, 5.0);
    assert_eq!(app.scm_extension_input, Some(1));
    assert!(app.handle_extension_scm_input_key(
        &Key::Character("h".into()),
        Some("h"),
        false,
        false
    ));
    assert!(app.handle_extension_scm_input_key(
        &Key::Character("i".into()),
        Some("i"),
        false,
        false
    ));
    assert!(app.handle_extension_scm_input_key(
        &Key::Named(NamedKey::Backspace),
        None,
        false,
        false
    ));
    assert_eq!(app.vsix_scm().unwrap().providers()[0].input, "h");
    let change = std::iter::from_fn(|| host.next_sent())
        .filter(|message| message["method"] == "$onInputBoxValueChange")
        .last()
        .expect("input change forwarded");
    assert_eq!(change["actor"], "ExtHostSCM");
    assert_eq!(change["args"], json!([1, "h"]));

    // A resource with a command is opened by the extension; one without opens as a file elsewhere.
    app.click_extension_scm_row(&row(250.0), rows[3], 20.0, 5.0);
    let open = std::iter::from_fn(|| host.next_sent())
        .find(|message| message["method"] == "$executeResourceCommand")
        .expect("resource command requested");
    assert_eq!(open["args"], json!([1, 10, 100, false]));

    // Collapsing the group hides its resources; escape leaves the box.
    app.toggle_vsix_scm_group(1, 10);
    assert_eq!(app.vsix_scm_rows().len(), 3);
    assert!(app.handle_extension_scm_input_key(&Key::Named(NamedKey::Escape), None, false, false));
    assert_eq!(app.scm_extension_input, None);
    assert!(!app.handle_extension_scm_input_key(
        &Key::Character("x".into()),
        Some("x"),
        false,
        false
    ));

    // Accepting without an accept command reports it; unregistering removes the provider.
    app.plugins
        .vsix
        .as_mut()
        .unwrap()
        .scm
        .clear_accept_for_test(1);
    app.accept_vsix_scm_input(1);
    assert_eq!(app.status, app.t("scm.sidebar.extension_no_accept"));
    let mut runtime = app.plugins.vsix.take().unwrap();
    call(
        &mut app,
        &mut runtime,
        "$unregisterSourceControl",
        json!([1]),
    );
    app.plugins.vsix = Some(runtime);
    assert!(app.vsix_scm_rows().is_empty());
}

#[test]
fn vsix_webview_views_resolve_update_their_header_and_are_disposed_with_the_provider() {
    let dir = TempDir::new();
    let ext_root = dir.path().join("extension");
    std::fs::create_dir_all(&ext_root).unwrap();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);
    let ext_uri = graphite_core_api::DocumentUri::from_path(&ext_root).unwrap();
    let call = |app: &mut App, runtime: &mut Runtime, method: &str, args: Value| {
        app.handle_vsix_request(
            runtime,
            &json!({"id": 1, "actor": "MainThreadWebviewViews", "method": method, "args": args}),
        )
    };

    call(
        &mut app,
        &mut runtime,
        "$registerWebviewViewProvider",
        json!([{ "location": { "$graphiteUri": ext_uri.as_str() } }, "sample.notes", { "retainContextWhenHidden": true }]),
    )
    .unwrap();
    let _ = host.next_sent();
    assert!(runtime.webview_views.has_provider("sample.notes"));
    assert!(runtime.webview_views.retains_context("sample.notes"));

    app.resolve_webview_view(&mut runtime, "sample.notes", "Notes");
    let resolve = host.next_sent().expect("resolve call");
    assert_eq!(resolve["actor"], "ExtHostWebviewViews");
    assert_eq!(resolve["method"], "$resolveWebviewView");
    assert_eq!(resolve["args"][1], "sample.notes");
    assert_eq!(resolve["args"][2], "Notes");
    assert_eq!(resolve["cancellable"], true);
    let handle = resolve["args"][0].as_str().unwrap().to_owned();
    let panel = runtime.webviews.panels.get(&handle).expect("view page");
    assert!(!panel.enable_scripts);

    // The page content arrives over the shared webview bridge.
    app.handle_vsix_request(
        &mut runtime,
        &json!({"id": 2, "actor": "MainThreadWebviews", "method": "$setHtml", "args": [handle, "<p>hi</p>"]}),
    )
    .unwrap();
    let _ = host.next_sent();
    assert_eq!(runtime.webviews.panels[&handle].html, "<p>hi</p>");

    call(
        &mut app,
        &mut runtime,
        "$setWebviewViewTitle",
        json!([handle, "Open notes"]),
    )
    .unwrap();
    call(
        &mut app,
        &mut runtime,
        "$setWebviewViewDescription",
        json!([handle, "2 pending"]),
    )
    .unwrap();
    call(
        &mut app,
        &mut runtime,
        "$setWebviewViewBadge",
        json!([handle, { "value": 2, "tooltip": "two" }]),
    )
    .unwrap();
    let state = runtime.webview_views.view("sample.notes").unwrap();
    assert_eq!(state.title.as_deref(), Some("Open notes"));
    assert_eq!(state.description.as_deref(), Some("2 pending"));
    assert_eq!(state.badge, Some((2, Some("two".into()))));
    call(
        &mut app,
        &mut runtime,
        "$setWebviewViewBadge",
        json!([handle, null]),
    )
    .unwrap();
    assert!(runtime
        .webview_views
        .view("sample.notes")
        .unwrap()
        .badge
        .is_none());

    call(&mut app, &mut runtime, "$show", json!([handle, false])).unwrap();
    assert_eq!(
        runtime.webview_views.pending_show.as_deref(),
        Some("sample.notes")
    );

    // A failed resolution is reported and not retried.
    app.finish_vsix_webview_view(&mut runtime, &json!({"id": resolve["id"], "error": "boom"}));
    assert_eq!(app.status, "boom");
    assert!(runtime.webview_views.view("sample.notes").unwrap().failed);

    call(
        &mut app,
        &mut runtime,
        "$unregisterWebviewViewProvider",
        json!(["sample.notes"]),
    )
    .unwrap();
    // Earlier replies are still queued; the dispose call is the one naming its method.
    let dispose = std::iter::from_fn(|| host.next_sent())
        .find(|message| message["method"] == "$disposeWebviewView")
        .expect("dispose call");
    assert_eq!(dispose["args"][0], json!(handle));
    assert!(!runtime.webviews.panels.contains_key(&handle));
    assert!(runtime.webview_views.view("sample.notes").is_none());
    // Unknown methods are answered with an error rather than silently accepted.
    while host.next_sent().is_some() {}
    call(&mut app, &mut runtime, "$nonexistent", json!([])).unwrap();
    let refusal = host.next_sent().expect("error reply");
    assert!(refusal["error"].is_string(), "{refusal}");
}

#[test]
fn vsix_webview_enforces_local_resource_roots_and_handles_bidirectional_messages() {
    let dir = TempDir::new();
    let ext_root = dir.path().join("extension");
    let media_dir = ext_root.join("media");
    let outside_dir = dir.path().join("outside");
    std::fs::create_dir_all(&media_dir).unwrap();
    std::fs::create_dir_all(&outside_dir).unwrap();
    let allowed_js = media_dir.join("app.js");
    let blocked_js = outside_dir.join("secret.js");
    std::fs::write(&allowed_js, "console.log('ok');").unwrap();
    std::fs::write(&blocked_js, "console.log('nope');").unwrap();

    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);

    let ext_uri = graphite_core_api::DocumentUri::from_path(&ext_root).unwrap();
    let media_uri = graphite_core_api::DocumentUri::from_path(&media_dir).unwrap();

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 10,
            "actor": "MainThreadWebviewPanels",
            "method": "$createWebviewPanel",
            "args": [
                { "id": { "value": "test.webview" }, "location": { "$graphiteUri": ext_uri.as_str() } },
                "panel-1",
                "sample.panel",
                {
                    "title": "Sample Webview",
                    "webviewOptions": {
                        "enableScripts": true,
                        "localResourceRoots": [{ "$graphiteUri": media_uri.as_str() }]
                    }
                }
            ]
        }),
    )
    .unwrap();
    let _ = host.next_sent();

    let allowed_path = allowed_js.to_string_lossy().replace('\\', "/");
    let allowed_url_path = if allowed_path.starts_with('/') {
        allowed_path.clone()
    } else {
        format!("/{allowed_path}")
    };
    let blocked_path = blocked_js.to_string_lossy().replace('\\', "/");
    let blocked_url_path = if blocked_path.starts_with('/') {
        blocked_path
    } else {
        format!("/{blocked_path}")
    };
    let media_path = media_dir.to_string_lossy().replace('\\', "/");
    let media_url_path = if media_path.starts_with('/') {
        media_path
    } else {
        format!("/{media_path}")
    };

    let html = format!(
        r#"<html><body>
            <script src="https://file+.vscode-resource.vscode-cdn.net{allowed_url_path}"></script>
            <script src="vscode-resource://file{blocked_url_path}"></script>
            <img src="https://file+.vscode-resource.vscode-cdn.net{media_url_path}/../../outside/secret.js">
        </body></html>"#
    );

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 11,
            "actor": "MainThreadWebviews",
            "method": "$setHtml",
            "args": ["panel-1", html]
        }),
    )
    .unwrap();
    let _ = host.next_sent();

    let panel_html = runtime.webviews.active_panel().unwrap().html.clone();
    assert!(
        panel_html.contains("app.js"),
        "expected allowed local asset: {panel_html}"
    );
    assert!(
        !panel_html.contains("secret.js"),
        "blocked assets must not survive rewrite: {panel_html}"
    );
    assert_eq!(panel_html.matches("about:blank#blocked").count(), 2);

    // Title update & reveal + outbound message (JSON object).
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 12,
            "actor": "MainThreadWebviewPanels",
            "method": "$setTitle",
            "args": ["panel-1", "Renamed Panel"]
        }),
    )
    .unwrap();
    let _ = host.next_sent();

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 13,
            "actor": "MainThreadWebviews",
            "method": "$postMessage",
            "args": ["panel-1", { "kind": "pong", "count": 2 }]
        }),
    )
    .unwrap();
    let _ = host.next_sent();

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 14,
            "actor": "MainThreadWebviewPanels",
            "method": "$reveal",
            "args": ["panel-1", { "viewColumn": 1 }]
        }),
    )
    .unwrap();
    let reveal_event = host.next_sent().expect("reveal view state notification");
    assert_eq!(reveal_event["actor"], "ExtHostWebviewPanels");
    assert_eq!(reveal_event["method"], "$onDidChangeWebviewPanelViewState");
    let _reveal_reply = host.next_sent().expect("reveal reply");

    assert_eq!(
        runtime.webviews.active_panel().unwrap().title,
        "Renamed Panel"
    );
    assert_eq!(
        runtime.webviews.active_panel().unwrap().outbound_messages,
        vec![json!({ "kind": "pong", "count": 2 })]
    );

    app.plugins.vsix = Some(runtime);
    app.send_vsix_webview_message("panel-1", json!({ "action": "save" }))
        .unwrap();
    let inbound = host.next_sent().expect("inbound $onMessage");
    assert_eq!(inbound["actor"], "ExtHostWebviews");
    assert_eq!(inbound["method"], "$onMessage");
    // The host's `$onMessage(handle, jsonMessage, buffers)` takes JSON text and wrapped buffers.
    assert_eq!(
        inbound["args"],
        json!(["panel-1", r#"{"action":"save"}"#, []])
    );
    assert_eq!(inbound["serializable"], json!([2]));

    app.close_vsix_webview("panel-1").unwrap();
    let disposed = host.next_sent().expect("disposal notification");
    assert_eq!(disposed["actor"], "ExtHostWebviewPanels");
    assert_eq!(disposed["method"], "$onDidDisposeWebviewPanel");
    assert!(app.vsix_webview().is_none());
}

#[test]
fn vsix_custom_pty_terminal_bridges_output_title_input_and_exit() {
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 20,
            "actor": "MainThreadTerminalService",
            "method": "$createTerminal",
            "args": ["ext-pty-1", { "name": "Ext PTY", "isExtensionCustomPtyTerminal": true }]
        }),
    )
    .unwrap();

    let opened = host.next_sent().expect("$acceptTerminalOpened");
    assert_eq!(opened["actor"], "ExtHostTerminalService");
    assert_eq!(opened["method"], "$acceptTerminalOpened");
    let term_id = opened["args"][0].as_u64().unwrap();

    let start_pty = host.next_sent().expect("$startExtensionTerminal");
    assert_eq!(start_pty["actor"], "ExtHostTerminalService");
    assert_eq!(start_pty["method"], "$startExtensionTerminal");
    assert_eq!(start_pty["args"][0], term_id);

    let _create_reply = host.next_sent().expect("create reply");

    // Rename & stream output from extension pseudoterminal.
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 21,
            "actor": "MainThreadTerminalService",
            "method": "$sendProcessTitle",
            "args": ["ext-pty-1", "Build PTY"]
        }),
    )
    .unwrap();
    let _ = host.next_sent();
    assert_eq!(app.terminal_names[0], "Build PTY");

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 22,
            "actor": "MainThreadTerminalService",
            "method": "$sendProcessData",
            "args": ["ext-pty-1", "hello pty\r\n"]
        }),
    )
    .unwrap();
    let _ = host.next_sent();

    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        let text: String = app.terminals[0]
            .snapshot()
            .rows
            .iter()
            .map(|row| row.iter().map(|c| c.character).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        if text.contains("hello pty") {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let rendered: String = app.terminals[0]
        .snapshot()
        .rows
        .iter()
        .map(|row| row.iter().map(|c| c.character).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("hello pty"), "screen was: {rendered}");

    // Keyboard input in terminal forwards to $acceptTerminalProcessInput.
    app.terminals[0].input(b"status\r".to_vec());
    app.plugins.vsix = Some(runtime);
    let mut forwarded = None;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        app.poll_plugins();
        while let Some(msg) = host.next_sent() {
            if msg["method"] == "$acceptTerminalProcessInput" {
                forwarded = Some(msg);
                break;
            }
        }
        if forwarded.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let forwarded = forwarded.expect("expected forwarded PTY input");
    assert_eq!(forwarded["args"], json!([term_id, "status\r"]));

    // Extension closes the pseudoterminal with exit code 0.
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 23,
            "actor": "MainThreadTerminalService",
            "method": "$sendProcessExit",
            "args": ["ext-pty-1", 0]
        }),
    )
    .unwrap();
    let closed_evt = host.next_sent().expect("$acceptTerminalClosed");
    assert_eq!(closed_evt["method"], "$acceptTerminalClosed");
    assert_eq!(closed_evt["args"][0], term_id);
    assert_eq!(closed_evt["args"][1], 0);
}

#[test]
fn vsix_tasks_provider_custom_execution_and_cwd_confinement() {
    let dir = TempDir::new();
    let work = dir.path().join("work");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&outside).unwrap();

    let mut app = App::new_with_options(work.clone(), None, true);
    app.profile_root = dir.path().join("profile");
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);

    // Untrusted workspace must refuse $executeTask.
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 30,
            "actor": "MainThreadTask",
            "method": "$executeTask",
            "args": [{ "type": "shell", "name": "blocked", "command": { "runtime": "shell", "commandLine": "echo no" } }]
        }),
    )
    .unwrap();
    let refused = host.next_sent().expect("untrusted error reply");
    assert!(refused["error"].as_str().unwrap().contains("restricted"));

    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");

    // Register task provider & fetch tasks.
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 31,
            "actor": "MainThreadTask",
            "method": "$registerTaskProvider",
            "args": [7, "cargo-ext"]
        }),
    )
    .unwrap();
    let _ = host.next_sent();

    app.plugins.vsix = Some(runtime);
    app.request_vsix_tasks().unwrap();
    let provide_call = host.next_sent().expect("$provideTasks");
    assert_eq!(provide_call["actor"], "ExtHostTask");
    assert_eq!(provide_call["method"], "$provideTasks");
    let provide_id = provide_call["id"].as_u64().unwrap();

    host.emit(json!({
        "type": "result",
        "id": provide_id,
        "result": {
            "tasks": [{ "type": "cargo-ext", "name": "Clippy Check", "isCustomExecution": true }]
        }
    }));
    app.poll_plugins();

    let mut runtime = app.plugins.vsix.take().unwrap();
    assert_eq!(runtime.tasks.discovered.len(), 1);
    assert_eq!(runtime.tasks.discovered[0].name, "Clippy Check");

    // Escaping workspace via cwd must be rejected cleanly.
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 32,
            "actor": "MainThreadTask",
            "method": "$executeTask",
            "args": [{
                "type": "shell",
                "name": "escape",
                "command": {
                    "runtime": "shell",
                    "commandLine": "echo hi",
                    "options": { "cwd": outside.display().to_string() }
                }
            }]
        }),
    )
    .unwrap();
    let mut escape_reply = None;
    while let Some(msg) = host.next_sent() {
        if msg["type"] == "reply" && msg["id"] == 32 {
            escape_reply = Some(msg);
            break;
        }
    }
    let escape_reply = escape_reply.expect("cwd confinement error");
    assert!(escape_reply["error"].as_str().unwrap().contains("escapes"));

    // CustomExecution starts and completes cleanly.
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 33,
            "actor": "MainThreadTask",
            "method": "$executeTask",
            "args": [{ "type": "cargo-ext", "name": "Custom Build", "isCustomExecution": true }]
        }),
    )
    .unwrap();
    let started_evt = host.next_sent().expect("$onDidStartTask");
    assert_eq!(started_evt["method"], "$onDidStartTask");
    let reply = host.next_sent().expect("$executeTask reply");
    let exec_id = reply["result"]["id"].as_str().unwrap().to_owned();

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 34,
            "actor": "MainThreadTask",
            "method": "$customExecutionComplete",
            "args": [exec_id, 0]
        }),
    )
    .unwrap();
    let ended_proc = host.next_sent().expect("$onDidEndTaskProcess");
    assert_eq!(ended_proc["method"], "$onDidEndTaskProcess");
    assert_eq!(ended_proc["args"][1], 0);
    let ended_evt = host.next_sent().expect("$onDidEndTask");
    assert_eq!(ended_evt["method"], "$onDidEndTask");
    let _complete_reply = host.next_sent();
}

#[test]
#[ignore = "manual check with a packaged Code OSS runtime and an explicit VSIX fixture"]
fn packaged_host_resolves_a_secondary_sidebar_view() {
    let runtime_path = PathBuf::from(std::env::var_os("GRAPHITE_TEST_CODE_OSS").unwrap());
    let package = PathBuf::from(std::env::var_os("GRAPHITE_TEST_SECONDARY_VSIX").unwrap());
    let view_id = std::env::var("GRAPHITE_TEST_SECONDARY_VIEW").unwrap();
    let dir = TempDir::new();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let mut app = App::new_with_options(workspace.clone(), None, true);
    app.profile_root = dir.path().join("profile");
    let mut trust =
        crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap();
    trust.trust(&workspace).unwrap();
    app.workspace_trust = Some(trust);
    let store = Store::new(app.profile_root.join("plugins"));
    let record = store.install(&package).unwrap();
    let id = record.metadata.id().to_owned();
    store
        .enable_installation(&id, &record.directory, true)
        .unwrap();
    app.plugins.installed = store.list().unwrap();
    app.plugins.store = Some(store);
    app.start_vsix_host(Some(runtime_path)).unwrap();
    assert!(app.open_secondary_extension_view(&view_id));

    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        app.poll_vsix();
        let Some(mut runtime) = app.plugins.vsix.take() else {
            panic!("Code OSS host stopped: {}", app.status);
        };
        // Resolve the same registered provider used by the native overlay, without a GPU window.
        if runtime.webview_views.has_provider(&view_id) {
            app.resolve_webview_view(&mut runtime, &view_id, "Secondary sidebar");
        }
        let html_ready = runtime
            .webview_views
            .view(&view_id)
            .and_then(|view| runtime.webviews.panels.get(&view.handle))
            .is_some_and(|panel| !panel.html.is_empty() && panel.html != webviews::DEFAULT_HTML);
        let active = runtime.active_extensions.contains(&id);
        app.plugins.vsix = Some(runtime);
        if html_ready && active {
            println!("{id}: activated and sent HTML for {view_id}");
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("{id} did not provide HTML for {view_id}: {}", app.status);
}

#[test]
fn secondary_views_activate_and_respect_extension_context_without_changing_the_primary_view() {
    use graphite_plugins::manifest::{ActivityBarContainer, ActivityBarView, Vsix};
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    let mut trust =
        crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap();
    trust.trust(dir.path()).unwrap();
    app.workspace_trust = Some(trust);
    app.active_plugin_activity = Some("primary".into());
    let view_id = "test.secondary";
    app.plugins.installed_plugins_mut().push(Installed {
        metadata: Metadata::Vsix(Vsix {
            id: "test.sidebar".into(),
            version: "1.0.0".into(),
            main: Some("index.js".into()),
            activation_events: vec![format!("onView:{view_id}")],
            secondary_sidebar: Some(vec![ActivityBarContainer {
                id: "aux".into(),
                title: "Tools".into(),
                icon: None,
                when: Some("!test:hide".into()),
                views: vec![ActivityBarView {
                    id: view_id.into(),
                    title: "Tools".into(),
                    kind: "webview".into(),
                    when: None,
                }],
            }]),
            ..Default::default()
        }),
        enabled: true,
        trusted_version: Some("1.0.0".into()),
        directory: "fixture".into(),
        dev_path: None,
    });
    let (session, host) = Session::loopback();
    app.plugins.vsix = Some(Runtime::for_test(session));
    assert!(app.open_secondary_extension_view(view_id));
    assert_eq!(
        host.next_sent().unwrap()["event"],
        format!("onView:{view_id}")
    );
    assert_eq!(app.active_plugin_activity.as_deref(), Some("primary"));
    assert!(app.visible_secondary_plugin().is_some());
    app.set_secondary_sidebar_visible(false);
    assert!(app.visible_secondary_plugin().is_none());
    app.set_secondary_sidebar_visible(true);
    assert!(app.visible_secondary_plugin().is_some());
    assert!(host.next_sent().is_none());
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id":1,"actor":"MainThreadCommands","method":"$executeCommand",
            "args":["setContext",["test:hide",true]]
        }),
    )
    .unwrap();
    app.plugins.vsix = Some(runtime);
    assert!(app.secondary_plugin_entries().is_empty());
    app.vsix_context.clear();
    app.plugins.installed_plugins_mut()[0].enabled = false;
    assert!(app.secondary_plugin_entries().is_empty());
}

#[test]
fn vsix_activation_events_menus_keybindings_and_open_uri() {
    let dir = TempDir::new();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();

    let mut app = App::new_with_options(work, None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");

    let (session, host) = Session::loopback();
    let runtime = Runtime::for_test(session);
    app.plugins.vsix = Some(runtime);

    app.plugins.installed_plugins_mut().push(Installed {
        metadata: Metadata::Vsix(graphite_plugins::manifest::Vsix {
            id: "test.rust-helper".into(),
            version: "1.0.0".into(),
            description: "Rust helper".into(),
            engines: json!({ "vscode": "^1.100.0" }),
            main: Some("index.js".into()),
            contributions: vec!["commands".into(), "keybindings".into(), "menus".into()],
            commands: vec![graphite_plugins::manifest::Command {
                id: "ext.rustFmt".into(),
                title: "Rust Ext Format".into(),
            }],
            activation_events: vec![
                "onLanguage:rust".into(),
                "workspaceContains:Cargo.toml".into(),
            ],
            keybindings: vec![graphite_plugins::manifest::Keybinding {
                command: "ext.rustFmt".into(),
                key: "ctrl+alt+f".into(),
                when: Some("editorTextFocus".into()),
            }],
            menus: graphite_plugins::manifest::Menus {
                command_palette: vec![graphite_plugins::manifest::MenuItem {
                    command: "ext.rustFmt".into(),
                    when: Some("editorTextFocus".into()),
                    group: None,
                }],
                editor_context: vec![graphite_plugins::manifest::MenuItem {
                    command: "ext.rustFmt".into(),
                    when: None,
                    group: Some("1_modification".into()),
                }],
                view_item_context: vec![],
            },
            ..Default::default()
        }),
        enabled: true,
        trusted_version: Some("1.0.0".into()),
        directory: "test.rust-helper@1.0.0".into(),
        dev_path: None,
    });

    // Menus & keybindings resolve cleanly.
    app.sync_plugin_keybindings();
    let editor_menu =
        app.plugin_menu_items(crate::app::extension_host::PluginMenuLocation::EditorContext);
    assert_eq!(editor_menu.len(), 1);
    assert_eq!(editor_menu[0].title, "Rust Ext Format");
    assert_eq!(editor_menu[0].group.as_deref(), Some("1_modification"));

    // Keybinding dispatches the VSIX command execution onto the active VSIX host,
    // and startup polling fires matching `workspaceContains` activation events automatically.
    assert!(app.handle_keybinding_stroke("ctrl+alt+f"));
    app.poll_plugins();
    let mut saw_execute = false;
    let mut saw_workspace_contains = false;
    while let Some(msg) = host.next_sent() {
        if msg["type"] == "execute" && msg["command"] == "ext.rustFmt" {
            saw_execute = true;
            assert_eq!(msg["owner"], "test.rust-helper");
        }
        if msg["type"] == "activateByEvent" && msg["event"] == "workspaceContains:Cargo.toml" {
            saw_workspace_contains = true;
        }
    }
    assert!(
        saw_execute,
        "expected VSIX keybinding to queue execute message"
    );
    assert!(
        saw_workspace_contains,
        "expected startup poll to fire workspaceContains:Cargo.toml activateByEvent"
    );

    // Explicit `workspaceContains` trigger matches cleanly and `onLanguage` emits activateByEvent.
    let activated_ws = app.trigger_vsix_activation_event("workspaceContains");
    assert_eq!(activated_ws, vec!["test.rust-helper"]);

    let activated_lang = app.activate_plugins_for_event("onLanguage:rust");
    assert!(activated_lang.contains(&"test.rust-helper".to_owned()));
    let mut saw_on_language = false;
    while let Some(msg) = host.next_sent() {
        if msg["type"] == "activateByEvent" && msg["event"] == "onLanguage:rust" {
            saw_on_language = true;
        }
    }
    assert!(
        saw_on_language,
        "expected onLanguage:rust activateByEvent message"
    );

    // MainThreadWindow.$openUri accepts https URIs and rejects file URIs.
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 40,
            "actor": "MainThreadWindow",
            "method": "$openUri",
            "args": [null, "file:///etc/passwd"]
        }),
    )
    .unwrap();
    let rejected = host.next_sent().expect("$openUri file error");
    assert!(rejected["error"].as_str().unwrap().contains("https"));
}

#[test]
fn vsix_language_model_tools_register_bridge_to_chat_and_invoke() {
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.profile_root = dir.path().join("profile");
    app.workspace_trust =
        Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
    app.run_ui_command("graphite.security.trustWorkspace");

    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 50,
            "actor": "MainThreadLanguageModelTools",
            "method": "$registerToolWithDefinition",
            "args": [{
                "id": "ext.search_docs",
                "toolReferenceName": "search_docs",
                "modelDescription": "Search extension docs",
                "inputSchema": {
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                }
            }]
        }),
    )
    .unwrap();
    let _ = host.next_sent();

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 51,
            "actor": "MainThreadLanguageModelTools",
            "method": "$getTools",
            "args": []
        }),
    )
    .unwrap();
    let tools_reply = host.next_sent().expect("$getTools reply");
    assert_eq!(tools_reply["result"].as_array().unwrap().len(), 1);

    app.plugins.vsix = Some(runtime);
    let specs = app.plugin_chat_tool_specs();
    assert!(specs.iter().any(|t| t.name == "search_docs"));

    let (tx, rx) = std::sync::mpsc::channel();
    app.invoke_plugin_chat_tool("search_docs", json!({ "query": "treeview" }), Some(tx))
        .unwrap();
    let call = host.next_sent().expect("$invokeTool call");
    assert_eq!(call["actor"], "ExtHostLanguageModelTools");
    assert_eq!(call["method"], "$invokeTool");
    assert_eq!(call["args"][0]["toolId"], "ext.search_docs");
    let call_id = call["id"].as_u64().unwrap();

    host.emit(json!({
        "type": "result",
        "id": call_id,
        "result": { "content": [{ "value": "TreeView docs result" }] }
    }));
    app.poll_plugins();

    assert_eq!(rx.try_recv().unwrap().unwrap(), "TreeView docs result");

    // Unregister removes the tool from plugin_chat_tool_specs.
    let mut runtime = app.plugins.vsix.take().unwrap();
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 52,
            "actor": "MainThreadLanguageModelTools",
            "method": "$unregisterTool",
            "args": ["ext.search_docs"]
        }),
    )
    .unwrap();
    app.plugins.vsix = Some(runtime);
    assert!(app.plugin_chat_tool_specs().is_empty());
}

#[test]
fn command_activation_notices_are_accepted_without_an_error() {
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id":7,"actor":"MainThreadCommands","method":"$fireCommandActivationEvent",
            "args":["claude-vscode.sidebar.open"]
        }),
    )
    .unwrap();
    let reply = host.next_sent().unwrap();
    assert_eq!(reply["id"], 7);
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(runtime.unsupported.is_empty());

    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id":8,"actor":"MainThreadWebviewPanels","method":"$registerSerializer",
            "args":["claudeVSCodePanel", {}]
        }),
    )
    .unwrap();
    let reply = host.next_sent().unwrap();
    assert_eq!(reply["id"], 8);
    assert!(reply.get("error").is_none(), "{reply}");
}

#[test]
fn page_messages_reach_the_host_as_json_text_with_wrapped_buffers() {
    let json = webviews::page_message_call("h", r#"{"type":"request","requestId":"1"}"#);
    assert_eq!(json["args"][1], r#"{"type":"request","requestId":"1"}"#);
    assert_eq!(json["args"][2], json!([]));
    assert_eq!(json["serializable"], json!([2]));
    // Text that is not JSON is delivered as a JSON string, so the host can still parse it.
    let text = webviews::page_message_call("h", "plain words");
    assert_eq!(text["args"][1], r#""plain words""#);
}

#[test]
fn returning_focus_from_pages_is_harmless_when_there_are_none() {
    let dir = TempDir::new();
    let app = App::new_with_options(dir.path().to_path_buf(), None, true);
    app.return_focus_from_webviews();
}

#[test]
fn binary_page_messages_carry_their_buffers_to_the_host() {
    let envelope = json!({
        "message": r#"{"bytes":{"$$vscode_array_buffer_reference$$":true,"index":0}}"#,
        "buffers": ["AQID", "BAUG"],
    });
    let call = webviews::page_message_call("h", &format!("__graphite_bin:{envelope}"));
    assert_eq!(
        call["args"][1],
        r#"{"bytes":{"$$vscode_array_buffer_reference$$":true,"index":0}}"#
    );
    assert_eq!(
        call["args"][2],
        json!([{"$graphiteBytes": "AQID"}, {"$graphiteBytes": "BAUG"}])
    );
    assert_eq!(call["serializable"], json!([2]));
    // A damaged envelope is delivered as plain text instead of being dropped.
    let broken = webviews::page_message_call("h", "__graphite_bin:{not json");
    assert_eq!(broken["args"][2], json!([]));
}

#[test]
fn messages_to_a_page_use_the_buffer_aware_delivery_only_when_they_have_buffers() {
    let plain = webviews::deliver_script(r#"{"a":1}"#, &[]);
    assert!(
        plain.starts_with("window.postMessage(JSON.parse("),
        "{plain}"
    );
    let binary = webviews::deliver_script(
        r#"{"b":{"$$vscode_array_buffer_reference$$":true,"index":0}}"#,
        &["AQID"],
    );
    assert!(
        binary.starts_with("window.__graphiteDeliver(\""),
        "{binary}"
    );
    assert!(binary.ends_with(r#",["AQID"])"#), "{binary}");
}

#[test]
fn percent_encoded_resource_hosts_are_rewritten_to_allowed_local_files() {
    let dir = TempDir::new();
    let root = dir.path().join("ext/webview");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("index.js"), "x").unwrap();
    std::fs::write(dir.path().join("secret.js"), "x").unwrap();
    let encoded = |path: &Path| {
        let text = std::fs::canonicalize(path).unwrap();
        let text = text.to_string_lossy().replace('\\', "/");
        let text = text.strip_prefix("//?/").unwrap_or(&text).to_owned();
        text.replace(':', "%3A").replace('@', "%40")
    };
    let roots = [root.clone()];
    for host in [
        "https://file%2B.vscode-resource.vscode-cdn.net",
        "https://file%2b.vscode-resource.vscode-cdn.net",
        "https://file+.vscode-resource.vscode-cdn.net",
    ] {
        let allowed = format!(
            r#"<script src="{host}/{}"></script>"#,
            encoded(&root.join("index.js")).trim_start_matches('/')
        );
        let html = webviews::sanitize_webview_html(&allowed, &roots);
        assert!(html.contains(r#"src="file:///"#), "{host}: {html}");
        assert!(!html.contains("vscode-cdn"), "{host}: {html}");

        let outside = format!(
            r#"<script src="{host}/{}"></script>"#,
            encoded(&dir.path().join("secret.js")).trim_start_matches('/')
        );
        let html = webviews::sanitize_webview_html(&outside, &roots);
        assert!(html.contains("about:blank#blocked"), "{host}: {html}");
    }
}

#[test]
fn a_base_href_keeps_its_trailing_slash_so_relative_assets_stay_inside_the_folder() {
    let dir = TempDir::new();
    let root = dir.path().join("ext/webview");
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let text = std::fs::canonicalize(&root).unwrap();
    let text = text.to_string_lossy().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text).to_owned();
    let encoded = text.replace(':', "%3A");
    let roots = [root.clone()];

    let with_slash = format!(
        r#"<base href="https://file%2B.vscode-resource.vscode-cdn.net/{}/">"#,
        encoded.trim_start_matches('/')
    );
    let html = webviews::sanitize_webview_html(&with_slash, &roots);
    assert!(html.contains(r#"<base href="file:///"#), "{html}");
    assert!(html.ends_with("/webview/\">"), "{html}");

    // Without the slash in the source, none is invented.
    let without = with_slash.replace("/\">", "\">");
    let html = webviews::sanitize_webview_html(&without, &roots);
    assert!(html.ends_with("/webview\">"), "{html}");
}

#[test]
fn extension_activation_errors_update_status_without_failing_rpc() {
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 9,
            "actor": "MainThreadExtensionService",
            "method": "$onExtensionActivationError",
            "args": [{"value": "google.google-antigravity"}, {"message": "Activating extension failed"}]
        }),
    )
    .unwrap();
    let reply = host.next_sent().unwrap();
    assert_eq!(reply["id"], 9);
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(app.status, "Activating extension failed");
    assert!(runtime.unsupported.is_empty());
}

#[test]
fn comment_controller_updates_are_accepted_without_failing_rpc() {
    let dir = TempDir::new();
    let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
    let (session, host) = Session::loopback();
    let mut runtime = Runtime::for_test(session);
    app.handle_vsix_request(
        &mut runtime,
        &json!({
            "id": 10,
            "actor": "MainThreadComments",
            "method": "$updateCommentingRanges",
            "args": [0, []]
        }),
    )
    .unwrap();
    let reply = host.next_sent().unwrap();
    assert_eq!(reply["id"], 10);
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(runtime
        .unsupported
        .contains("MainThreadComments.$updateCommentingRanges"));
}
