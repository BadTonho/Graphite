use super::*;
use graphite_core_api::*;
use serde_json::{json, Value};
use std::time::Duration;

fn pump(app: &mut App, condition: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        app.poll_plugins();
        if condition(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "Plugin deadline exceeded: {} / {}",
        app.plugins.message, app.status
    );
}
fn setup(app: &mut App, root: &Path, name: &str) -> String {
    if app.workspace_trust.is_none() {
        let mut trust = crate::app::phase5::WorkspaceTrust::open(root.join("trust.json")).unwrap();
        trust.trust(root).unwrap();
        app.workspace_trust = Some(trust);
    }
    let project = root.join(name);
    graphite_plugins::sdk::create(&project).unwrap();
    let id = format!("local.{name}");
    let names = ["edit", "crash", "wait", "dispose", "snapshot", "stale"];
    let commands: Vec<_> = names
        .iter()
        .map(|c| json!({"id":format!("{id}.{c}"),"title":c}))
        .collect();
    let manifest = json!({"id":id,"version":"0.1.0","apiVersion":1,"main":"dist/index.js","description":"Fixture","commands":commands});
    std::fs::write(
        project.join("graphite-plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::create_dir(project.join("dist")).unwrap();
    let script = r#"
let disposable;
export async function activate(context, g) {
  const register = (name, fn) => g.commands.registerCommand('ID.' + name, fn);
  await g.documents.onDidChange(event => { globalThis.lastEvent = event; });
  await register('edit', async () => {
    const [doc] = await g.documents.list();
    await g.documents.applyEdits([{ uri: doc.uri, version: doc.version, edits: [{start:{line:0,character:0},end:{line:0,character:0},text:'plugin:'}] }]);
    await g.window.showMessage('Plugin edited document');
    return (await g.documents.snapshot(doc.uri)).text;
  });
  await register('crash', () => process.exit(9));
  await register('wait', (_, signal) => new Promise(resolve => signal.addEventListener('abort', () => resolve('cancelled'))));
  await register('snapshot', async () => ({ documents: await g.documents.list(), event: globalThis.lastEvent ?? null, workspace: await g.workspace.get(), configuration: await g.workspace.configuration() }));
  await register('stale', async () => {
    const [doc] = await g.documents.list();
    await g.documents.applyEdits([{uri:doc.uri, version:doc.version + 100, edits:[{start:{line:0,character:0},end:{line:0,character:0},text:'bad'}]}]);
  });
  disposable = await register('dispose', async () => { await disposable.dispose(); });
  console.log('This goes to stderr, not RPC');
}
export function deactivate() {}
"#.replace("ID", &id);
    std::fs::write(project.join("dist/index.js"), script).unwrap();
    let package = graphite_plugins::package::pack(&project).unwrap();
    let store = app.plugins.store.as_ref().unwrap();
    store.install(&package).unwrap();
    store.enable(&id, true).unwrap();
    app.plugins.installed = store.list().unwrap();
    app.start_plugin(id.clone());
    pump(app, |a| {
        a.plugins
            .hosts
            .get(&id)
            .is_some_and(|h| h.native_active && h.commands.len() == names.len())
    });
    // Complete activation's final acknowledgement before invoking commands.
    for _ in 0..4 {
        app.poll_plugins();
        std::thread::sleep(Duration::from_millis(5));
    }
    id
}
fn invoke(app: &mut App, id: &str) -> CommandResult {
    let ticket = app.execute_command(CommandRequest {
        id: id.into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_plugins();
        if let Some(result) = ticket.try_result() {
            return result;
        }
        assert!(
            Instant::now() < deadline,
            "command deadline: {}",
            app.plugins.message
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn real_plugins_edit_dirty_documents_undo_dispose_cancel_and_isolate_crashes() {
    graphite_plugins::sdk::check_node(Path::new("node"))
        .expect("Plugin integration tests require Node.js 24+");
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    app.plugins.store = Some(Store::new(temp.path().join("plugins")));
    app.editor_mut().insert("unsaved");
    let one = setup(&mut app, temp.path(), "one");
    let two = setup(&mut app, temp.path(), "two");
    assert_eq!(
        invoke(&mut app, &format!("{one}.edit")).unwrap(),
        json!("plugin:unsaved")
    );
    assert_eq!(app.editor().text(), "plugin:unsaved");
    app.run_ui_command("undo");
    assert_eq!(app.editor().text(), "unsaved");
    let snapshot = invoke(&mut app, &format!("{two}.snapshot")).unwrap();
    assert_eq!(snapshot["documents"][0]["text"], "unsaved");
    assert_eq!(snapshot["documents"][0]["dirty"], true);
    assert!(!snapshot["event"].is_null());
    assert!(snapshot["workspace"]["root"]
        .as_str()
        .unwrap()
        .starts_with("file:"));
    assert!(invoke(&mut app, &format!("{one}.stale")).is_err());
    assert_eq!(app.editor().text(), "unsaved");
    app.editor_mut().set_read_only(true);
    assert!(invoke(&mut app, &format!("{one}.edit")).is_err());
    assert_eq!(app.editor().text(), "unsaved");
    app.editor_mut().set_read_only(false);
    invoke(&mut app, &format!("{one}.dispose")).unwrap();
    assert!(!app.plugins.hosts[&one]
        .commands
        .contains_key(&format!("{one}.dispose")));
    let ticket = app.execute_command(CommandRequest {
        id: format!("{two}.wait"),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    ticket.cancellation.cancel();
    pump(&mut app, |a| {
        !a.command_runtime.pending.contains_key(&ticket.id)
    });
    assert!(ticket.try_result().unwrap().is_err());
    assert!(invoke(&mut app, &format!("{one}.crash")).is_err());
    pump(&mut app, |a| a.plugins.hosts[&one].commands.is_empty());
    assert!(
        !app.plugins.hosts[&two].commands.is_empty(),
        "{} / {}",
        app.plugins.message,
        app.status
    );
    assert!(invoke(&mut app, &format!("{two}.snapshot")).is_ok());
    let old_session = app.plugins.hosts[&one].supervisor.as_ref().unwrap().session;
    app.start_plugin(one.clone());
    pump(&mut app, |a| {
        a.plugins
            .hosts
            .get(&one)
            .is_some_and(|h| h.native_active && h.commands.len() == 6)
    });
    assert_ne!(
        app.plugins.hosts[&one].supervisor.as_ref().unwrap().session,
        old_session
    );
    app.stop_plugin(&one);
    assert!(!app.plugins.hosts.contains_key(&one));
    assert!(!app.plugins.hosts[&two].commands.is_empty());
    app.stop_plugin(&two);
}

#[test]
fn workspace_revocation_stops_plugins_and_retrust_restarts_approved_plugins() {
    graphite_plugins::sdk::check_node(Path::new("node"))
        .expect("Plugin integration tests require Node.js 24+");
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    app.plugins.store = Some(Store::new(temp.path().join("plugins")));
    let id = setup(&mut app, temp.path(), "trusted");
    assert!(app.plugins.hosts.contains_key(&id));

    app.run_ui_command("graphite.security.revokeWorkspaceTrust");
    assert!(!app.plugins.hosts.contains_key(&id));
    app.start_plugin(id.clone());
    assert!(!app.plugins.hosts.contains_key(&id));

    app.run_ui_command("graphite.security.trustWorkspace");
    pump(&mut app, |a| {
        a.plugins
            .hosts
            .get(&id)
            .is_some_and(|host| host.native_active)
    });
    app.stop_plugin(&id);
}

#[test]
fn activation_result_arriving_after_workspace_revocation_is_discarded() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    let mut trust = crate::app::phase5::WorkspaceTrust::restricted(temp.path().join("trust.json"));
    trust.trust(temp.path()).unwrap();
    app.workspace_trust = Some(trust);
    // This result belongs to the previous trust period, even though the workspace
    // has since been trusted again.
    app.plugins.trust_generation = 1;
    let (sender, receiver) = std::sync::mpsc::channel();
    sender
        .send(Ok(Outcome::Started(
            "publisher.extension".into(),
            0,
            super::super::extension_host::ExtensionRuntime::default(),
        )))
        .unwrap();
    app.plugins.worker = Some(receiver);
    app.poll_plugins();
    assert!(!app.plugins.hosts.contains_key("publisher.extension"));
    assert!(app.status.contains("discarded"));
}
#[test]
fn native_registration_cannot_escape_manifest_or_shadow_builtins() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    app.extension_host.native_manifest = Some(graphite_plugins::manifest::Manifest {
        id: "local.test".into(),
        version: "1.0.0".into(),
        api_version: 1,
        main: "main.js".into(),
        description: String::new(),
        commands: vec![],
        activity_bar: vec![],
    });
    for id in ["undo", "other.plugin.command", "local.test.undeclared"] {
        let metadata = CommandMetadata {
            id: id.into(),
            title: "test".into(),
            category: "test".into(),
            targets: vec![TargetKind::Global],
            enablement: "true".into(),
        };
        // Register through a ready host is covered by the process integration; direct service registration validates metadata too.
        assert!(app.register_host_command(1, metadata).is_err());
    }
}
#[test]
fn activation_timeout_releases_only_its_resources_and_panel_binding_is_registered() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    app.plugins.store = Some(Store::new(temp.path().join("plugins")));
    let one = setup(&mut app, temp.path(), "timed");
    let two = setup(&mut app, temp.path(), "alive");
    app.plugins.hosts.get_mut(&one).unwrap().activation_deadline =
        Some(Instant::now() - Duration::from_secs(1));
    app.poll_plugins();
    assert!(app.plugins.hosts[&one].commands.is_empty());
    assert!(!app.plugins.hosts[&two].commands.is_empty());
    assert!(app.handle_keybinding_stroke("ctrl+shift+x"));
    assert!(app.plugins.visible);
    assert!(app.plugin_accessibility().nodes.len() >= 2);
    app.plugin_action('d');
    pump(&mut app, |a| a.plugins.worker.is_none());
    app.stop_plugin(&two);
}
#[test]
fn panel_details_and_ime_input_leave_background_document_untouched() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    app.plugins.visible = true;
    app.plugins.focused = true;
    app.plugins.editing = true;
    app.ime_commit("plugin path");
    assert_eq!(app.plugins.query, "plugin path");
    assert_eq!(app.editor().text(), "");
    app.plugins.editing = false;
    app.ime_commit("must not edit");
    assert_eq!(app.editor().text(), "");
    app.plugins.installed.push(Installed {
        metadata: Metadata::Vsix(graphite_plugins::manifest::Vsix {
            id: "sample.vsix".into(),
            version: "1.0.0".into(),
            description: String::new(),
            engines: Value::Null,
            main: None,
            contributions: vec![],
            commands: vec![],
            activity_bar: vec![],
            language_comments: vec![],
            snippets: vec![],
            debuggers: vec![],
        }),
        enabled: false,
        trusted_version: None,
        directory: "sample.vsix@1.0.0".into(),
    });
    app.plugin_action('\n');
    assert!(matches!(app.plugins.inspecting, Some(Metadata::Vsix(_))));
    app.scroll_plugins(100);
    assert_eq!(app.plugins.index, 0);
    assert_eq!(app.plugins.scroll, 0);
}

#[test]
fn tab_switches_leave_details_and_clear_path_input() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    app.plugins.visible = true;
    app.plugins.focused = true;
    let metadata = Metadata::Vsix(graphite_plugins::manifest::Vsix {
        id: "sample.vsix".into(),
        version: "1.0.0".into(),
        description: String::new(),
        engines: Value::Null,
        main: None,
        contributions: vec![],
        commands: vec![],
        activity_bar: vec![],
        language_comments: vec![],
        snippets: vec![],
        debuggers: vec![],
    });
    app.plugins.inspecting = Some(metadata.clone());
    app.plugin_action('i');
    assert!(app.plugins.inspecting.is_none());
    app.plugins.path_input = true;
    app.plugins.query = "plugin.vsix".into();
    app.plugins.editing = true;
    app.plugin_action('i');
    assert!(!app.plugins.path_input);
    assert!(app.plugins.query.is_empty());
    assert!(!app.plugins.editing);
    app.plugins.inspecting = Some(metadata);
    app.plugin_action('l');
    assert!(app.plugins.inspecting.is_none());
    assert!(app.plugins.path_input);
    assert!(app.plugins.editing);
    assert!(app.plugins.query.is_empty());
}

#[test]
fn graphite_catalog_is_an_additive_tab_with_repository_and_asset_rows() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    let open_vsx = graphite_plugins::open_vsx::Extension {
        namespace: "publisher".into(),
        name: "extension".into(),
        display_name: "Open VSX extension".into(),
        version: "1.0.0".into(),
        description: "Still available".into(),
    };
    app.plugins.open_vsx.push(open_vsx);

    app.plugin_action('g');
    assert_eq!(app.plugins.tab, PluginsTab::Graphite);
    assert_eq!(app.plugins.open_vsx.len(), 1);

    let repository = graphite_plugins::catalog::Repository {
        full_name: "author/plugin".into(),
        description: Some("A Graphite plugin".into()),
        stargazers_count: 12,
        html_url: "https://github.com/author/plugin".into(),
        homepage: None,
        topics: vec!["graphite-plugin".into()],
        license: None,
        open_issues_count: 0,
        default_branch: Some("main".into()),
    };
    app.plugins.graphite = vec![repository.clone()];
    assert_eq!(app.plugin_row_count(), 1);

    let (sender, receiver) = std::sync::mpsc::channel();
    sender
        .send(Ok(Outcome::GraphiteAssets(
            repository.clone(),
            vec![
                graphite_plugins::catalog::Asset {
                    name: "plugin.graphite-plugin".into(),
                    browser_download_url: "https://github.com/author/plugin/releases/download/v1/plugin.graphite-plugin".into(),
                    size: 10,
                    download_count: 1,
                },
                graphite_plugins::catalog::Asset {
                    name: "plugin.vsix".into(),
                    browser_download_url: "https://github.com/author/plugin/releases/download/v1/plugin.vsix".into(),
                    size: 20,
                    download_count: 2,
                },
            ],
        )))
        .unwrap();
    app.plugins.worker = Some(receiver);
    app.poll_plugins();
    assert_eq!(app.plugin_row_count(), 2);
    assert_eq!(
        app.plugins.graphite_assets.as_ref().unwrap().0.full_name,
        repository.full_name
    );

    app.leave_plugin_details();
    assert!(app.plugins.graphite_assets.is_none());
    assert_eq!(app.plugin_row_count(), 1);
    app.plugin_action('o');
    assert_eq!(app.plugins.tab, PluginsTab::OpenVsx);
}

#[test]
fn graphite_repository_opens_a_separate_editor_detail_tab_per_plugin() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    app.plugins.tab = PluginsTab::Graphite;
    let repository = |name: &str| graphite_plugins::catalog::Repository {
        full_name: format!("author/{name}"),
        description: Some("A Graphite plugin".into()),
        stargazers_count: 12,
        html_url: format!("https://github.com/author/{name}"),
        homepage: None,
        topics: vec!["graphite-plugin".into()],
        license: None,
        open_issues_count: 0,
        default_branch: Some("main".into()),
    };
    app.plugins.graphite = vec![repository("first"), repository("second")];

    app.plugins.index = 0;
    app.plugin_action('\n');
    assert_eq!(app.plugins.graphite_details.len(), 1);
    assert_eq!(
        app.plugins.active_graphite_detail.as_ref().unwrap().1,
        "author/first"
    );

    app.plugins.index = 1;
    app.plugin_action('\n');
    assert_eq!(app.plugins.graphite_details.len(), 2);
    assert_eq!(
        app.plugins.active_graphite_detail.as_ref().unwrap().1,
        "author/second"
    );
    app.plugin_action('\n');
    assert_eq!(app.plugins.graphite_details.len(), 2);

    let group = app.plugins.active_graphite_detail.as_ref().unwrap().0;
    app.close_graphite_detail(group, "author/second");
    assert_eq!(app.plugins.graphite_details.len(), 1);
    assert!(app.plugins.active_graphite_detail.is_none());
}

#[test]
fn sidebar_scrolls_its_viewport_and_yields_the_keyboard_to_overlays() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    for index in 0..40 {
        app.plugins.installed.push(Installed {
            metadata: Metadata::Vsix(graphite_plugins::manifest::Vsix {
                id: format!("sample.plugin{index}"),
                version: "1.0.0".into(),
                description: String::new(),
                engines: Value::Null,
                main: None,
                contributions: vec![],
                commands: vec![],
                activity_bar: vec![],
                language_comments: vec![],
                snippets: vec![],
                debuggers: vec![],
            }),
            enabled: false,
            trusted_version: None,
            directory: format!("sample.plugin{index}@1.0.0"),
        });
    }
    app.plugins.visible = true;
    app.plugins.focused = true;
    assert!(app.plugin_owns_keyboard());
    app.scroll_plugins(3);
    assert_eq!(app.plugins.scroll, 3);
    app.scroll_plugins(100);
    assert_eq!(app.plugins.scroll, 40 - app.plugin_visible_rows());
    app.scroll_plugins(-100);
    assert_eq!(app.plugins.scroll, 0);
    app.quick_open = Some(Default::default());
    assert!(!app.plugin_owns_keyboard());
    app.quick_open = None;
    app.plugins.focused = false;
    assert!(!app.plugin_owns_keyboard());
}

/// An app whose plugin store is a temporary folder and whose background jobs are switched off,
/// so commands that would search or download over the network only change state.
fn app_without_network() -> (App, crate::test_support::TempDir) {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    app.plugins.store = Some(Store::new(temp.path().join("plugins")));
    app.plugins.worker = Some(mpsc::channel().1);
    (app, temp)
}

fn run_with_argument(app: &mut App, id: &str, argument: &str) -> CommandResult {
    let ticket = app.execute_command(CommandRequest {
        id: id.into(),
        arguments: json!({"text": argument}),
        target: CommandTarget::Global,
    });
    ticket.try_result().expect("the command finishes at once")
}

#[test]
fn the_search_command_opens_the_open_vsx_tab_with_the_text() {
    let (mut app, _temp) = app_without_network();
    run_with_argument(&mut app, "workbench.extensions.search", "rust analyzer").unwrap();
    assert!(app.plugins.visible);
    assert_eq!(app.plugins.tab, PluginsTab::OpenVsx);
    assert_eq!(app.plugins.query, "rust analyzer");
    for id in [
        "workbench.extensions.action.showPopularExtensions",
        "workbench.extensions.action.showRecommendedExtensions",
        "workbench.extensions.action.recentlyPublishedExtensions",
    ] {
        app.plugins.tab = PluginsTab::Installed;
        app.run_ui_command(id);
        assert_eq!(app.plugins.tab, PluginsTab::OpenVsx, "{id}");
        assert_eq!(app.plugins.query, "", "{id}");
    }
}

#[test]
fn the_language_search_uses_the_language_of_the_active_file() {
    let (mut app, temp) = app_without_network();
    let file = temp.path().join("main.rs");
    std::fs::write(&file, "fn main() {}").unwrap();
    app.open_file(file);
    app.run_ui_command("workbench.action.showLanguageExtensions");
    assert_eq!(app.plugins.tab, PluginsTab::OpenVsx);
    assert_eq!(app.plugins.query, "rust");
}

#[test]
fn installing_needs_a_publisher_dot_name_id_and_uninstalling_a_known_extension() {
    let (mut app, _temp) = app_without_network();
    for bad in ["", "noname", "a.b.c", ".x", "x."] {
        assert!(
            run_with_argument(&mut app, "workbench.extensions.installExtension", bad).is_err(),
            "{bad:?}"
        );
    }
    let error = run_with_argument(
        &mut app,
        "workbench.extensions.uninstallExtension",
        "no.such",
    )
    .unwrap_err();
    assert!(error.to_string().contains("no.such"), "{error}");
}

#[test]
fn a_valid_extension_id_starts_a_download_preview() {
    let (mut app, _temp) = app_without_network();
    // The download job is held back by the dummy worker; the command itself succeeds.
    run_with_argument(
        &mut app,
        "workbench.extensions.installExtension",
        "redhat.vscode-yaml",
    )
    .unwrap();
    assert!(app.plugins.visible);
}

#[test]
fn disabling_and_enabling_all_extensions_changes_the_store() {
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    app.plugins.store = Some(Store::new(temp.path().join("plugins")));
    let project = temp.path().join("fixture");
    graphite_plugins::sdk::create(&project).unwrap();
    let manifest = json!({"id":"local.fixture","version":"0.1.0","apiVersion":1,
        "main":"dist/index.js","description":"Fixture","commands":[]});
    std::fs::write(
        project.join("graphite-plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::create_dir(project.join("dist")).unwrap();
    std::fs::write(
        project.join("dist/index.js"),
        "export function activate() {}",
    )
    .unwrap();
    let package = graphite_plugins::package::pack(&project).unwrap();
    let store = app.plugins.store.clone().unwrap();
    store.install(&package).unwrap();
    store.enable("local.fixture", true).unwrap();
    app.plugins.installed = store.list().unwrap();
    assert!(app.plugins.installed[0].enabled);

    app.run_ui_command("workbench.extensions.action.disableAll");
    pump(&mut app, |a| a.plugins.installed.iter().all(|r| !r.enabled));
    assert!(!store.list().unwrap()[0].enabled);

    app.run_ui_command("workbench.extensions.action.enableAll");
    pump(&mut app, |a| a.plugins.installed.iter().all(|r| r.enabled));
    assert!(store.list().unwrap()[0].enabled);

    // Nothing is left to enable, and disabling again is a no-op with a message.
    app.run_ui_command("workbench.extensions.action.enableAll");
    assert_eq!(app.plugins.message, app.t("plugins.none_to_enable"));
}

#[test]
fn disabling_all_with_nothing_enabled_says_so() {
    let (mut app, _temp) = app_without_network();
    app.run_ui_command("workbench.extensions.action.disableAll");
    assert_eq!(app.plugins.message, app.t("plugins.none_enabled"));
}
