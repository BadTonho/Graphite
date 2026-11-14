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
        runtime: Default::default(),
        permissions: vec![],
        commands: vec![],
        activity_bar: vec![],
        activation_events: vec![],
        keybindings: vec![],
        menus: Default::default(),
        themes: vec![],
        snippets: vec![],
        languages: vec![],
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
        dev_path: None,
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
            dev_path: None,
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

#[test]
fn native_plugins_can_execute_commands_open_documents_and_participate_in_save() {
    graphite_plugins::sdk::check_node(Path::new("node"))
        .expect("Plugin integration tests require Node.js 24+");
    let temp = crate::test_support::TempDir::new();
    let sample = temp.path().join("sample.txt");
    std::fs::write(&sample, "initial\n").unwrap();
    let sample_uri = DocumentUri::from_path(&sample).unwrap().as_str().to_owned();

    let mut app = App::new_with_options(temp.path().into(), None, true);
    let mut trust =
        crate::app::phase5::WorkspaceTrust::open(temp.path().join("trust.json")).unwrap();
    trust.trust(temp.path()).unwrap();
    app.workspace_trust = Some(trust);
    app.plugins.store = Some(Store::new(temp.path().join("plugins")));

    let project = temp.path().join("ext");
    graphite_plugins::sdk::create(&project).unwrap();
    let id = "local.ext";
    let manifest = json!({
        "id": id,
        "version": "0.1.0",
        "apiVersion": 1,
        "main": "dist/index.js",
        "description": "Extended API test",
        "commands": [
            {"id": "local.ext.echo", "title": "Echo"},
            {"id": "local.ext.workflow", "title": "Workflow"}
        ]
    });
    std::fs::write(
        project.join("graphite-plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::create_dir(project.join("dist")).unwrap();
    let script = r#"
export async function activate(context, g) {
  await g.commands.registerCommand('local.ext.echo', args => ({ echoed: args }));
  await g.documents.onWillSave({ id: 'local.ext.trim', languages: [] }, async req => {
    return [{
      start: { line: 1, character: 0 },
      end: { line: 1, character: 0 },
      text: '// will-save\n'
    }];
  });
  await g.commands.registerCommand('local.ext.workflow', async uri => {
    const cmds = await g.commands.list();
    const opened = await g.documents.open(uri);
    const echoed = await g.commands.execute('local.ext.echo', { count: cmds.length });
    const outcome = await g.documents.save(uri);
    return { openedText: opened.text, echoed, outcome };
  });
}
export function deactivate() {}
"#;
    std::fs::write(project.join("dist/index.js"), script).unwrap();
    let package = graphite_plugins::package::pack(&project).unwrap();
    let store = app.plugins.store.as_ref().unwrap();
    store.install(&package).unwrap();
    store.enable(id, true).unwrap();
    app.plugins.installed = store.list().unwrap();
    app.start_plugin(id.into());
    pump(&mut app, |a| {
        a.plugins
            .hosts
            .get(id)
            .is_some_and(|h| h.native_active && h.commands.len() == 2)
    });
    for _ in 0..4 {
        app.poll_plugins();
        std::thread::sleep(Duration::from_millis(5));
    }

    let ticket = app.execute_command(CommandRequest {
        id: "local.ext.workflow".into(),
        arguments: json!(sample_uri),
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let result = loop {
        app.poll_plugins();
        app.poll_saves(Instant::now());
        if let Some(result) = ticket.try_result() {
            break result;
        }
        assert!(
            Instant::now() < deadline,
            "workflow deadline: {} / {}",
            app.plugins.message,
            app.status
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    .unwrap();

    assert_eq!(result["openedText"], "initial\n");
    assert!(result["echoed"]["echoed"]["count"].as_u64().unwrap_or(0) > 0);
    assert_eq!(result["outcome"]["dirty"], false);
    assert_eq!(
        std::fs::read_to_string(&sample).unwrap(),
        "initial\n// will-save\n"
    );
}

#[test]
fn native_plugins_support_prompts_status_bar_output_channels_and_persistent_storage() {
    graphite_plugins::sdk::check_node(Path::new("node"))
        .expect("Plugin integration tests require Node.js 24+");
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    let mut trust =
        crate::app::phase5::WorkspaceTrust::open(temp.path().join("trust.json")).unwrap();
    trust.trust(temp.path()).unwrap();
    app.workspace_trust = Some(trust);
    app.plugins.store = Some(Store::new(temp.path().join("plugins")));

    let project = temp.path().join("uiext");
    graphite_plugins::sdk::create(&project).unwrap();
    let id = "local.uiext";
    let manifest = json!({
        "id": id,
        "version": "0.1.0",
        "apiVersion": 1,
        "main": "dist/index.js",
        "description": "UI & Storage test",
        "commands": [
            {"id": "local.uiext.ask", "title": "Ask"},
            {"id": "local.uiext.pick", "title": "Pick"},
            {"id": "local.uiext.statusTap", "title": "StatusTap"},
            {"id": "local.uiext.storageRoundTrip", "title": "Storage"}
        ]
    });
    std::fs::write(
        project.join("graphite-plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::create_dir(project.join("dist")).unwrap();
    let script = r#"
export async function activate(context, g) {
  const status = g.window.createStatusBarItem('counter');
  status.text = 'Graphite: Ready';
  status.command = 'local.uiext.statusTap';
  await status.show();

  const out = await g.window.createOutputChannel('Plugin Log');
  await out.appendLine('boot ok');

  await g.commands.registerCommand('local.uiext.ask', async () => {
    const answer = await g.window.showInputBox({ prompt: 'Enter token', value: 'abc' });
    return answer;
  });
  await g.commands.registerCommand('local.uiext.pick', async () => {
    const picked = await g.window.showQuickPick([{ label: 'Alpha', id: 'a' }, { label: 'Beta', id: 'b' }], { placeHolder: 'Pick one' });
    return picked;
  });
  await g.commands.registerCommand('local.uiext.statusTap', async () => {
    status.text = 'Graphite: Clicked';
    await status.show();
    await out.appendLine('status clicked');
    await out.show();
    return 'tapped';
  });
  await g.commands.registerCommand('local.uiext.storageRoundTrip', async op => {
    if (op === 'write') {
      await g.storage.global.set('theme', 'dark');
      await g.storage.workspace.set('count', 42);
      return true;
    }
    if (op === 'delete') {
      await g.storage.workspace.delete('count');
      return await g.storage.workspace.get('count');
    }
    return {
      global: await g.storage.global.get('theme'),
      workspace: await g.storage.workspace.get('count'),
      allWorkspace: await g.storage.workspace.get(),
    };
  });
}
export function deactivate() {}
"#;
    std::fs::write(project.join("dist/index.js"), script).unwrap();
    let package = graphite_plugins::package::pack(&project).unwrap();
    let store = app.plugins.store.as_ref().unwrap();
    store.install(&package).unwrap();
    store.enable(id, true).unwrap();
    app.plugins.installed = store.list().unwrap();
    app.start_plugin(id.into());
    pump(&mut app, |a| {
        a.plugins
            .hosts
            .get(id)
            .is_some_and(|h| h.native_active && h.commands.len() == 4)
    });
    for _ in 0..4 {
        app.poll_plugins();
        std::thread::sleep(Duration::from_millis(5));
    }

    // 1. Status bar item registered on activation and clickable
    assert!(app.status_bar_segments().iter().any(|(seg, text)| matches!(
        seg,
        crate::app::phase4_tools::StatusSegment::Extension(_)
    ) && text == "Graphite: Ready"));
    let idx = app
        .extension_status_items()
        .iter()
        .position(|(text, _)| text == "Graphite: Ready")
        .unwrap();
    app.run_extension_status_item(idx);
    pump(&mut app, |a| {
        a.extension_status_items()
            .iter()
            .any(|(text, _)| text == "Graphite: Clicked")
            && a.editor().text().contains("status clicked")
    });
    assert!(app.editor().text().contains("boot ok"));

    // 2. showInputBox answered through CommandPalette
    let ask_ticket = app.execute_command(CommandRequest {
        id: "local.uiext.ask".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    pump(&mut app, |a| {
        a.command_palette.as_ref().is_some_and(|p| {
            p.extension_prompt
                .as_ref()
                .is_some_and(|(t, _)| t == "Enter token")
                && p.query == "abc"
        })
    });
    app.command_palette.as_mut().unwrap().query = "custom-reply".into();
    app.execute_command_palette_selection();
    let deadline = Instant::now() + Duration::from_secs(5);
    let ask_result = loop {
        app.poll_plugins();
        if let Some(res) = ask_ticket.try_result() {
            break res.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(ask_result, json!("custom-reply"));

    // 3. showQuickPick selecting Beta
    let pick_ticket = app.execute_command(CommandRequest {
        id: "local.uiext.pick".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    pump(&mut app, |a| {
        a.command_palette
            .as_ref()
            .is_some_and(|p| p.items.len() == 2)
    });
    app.command_palette.as_mut().unwrap().selected = 1;
    app.execute_command_palette_selection();
    let deadline = Instant::now() + Duration::from_secs(5);
    let pick_result = loop {
        app.poll_plugins();
        if let Some(res) = pick_ticket.try_result() {
            break res.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(pick_result["id"], "b");
    assert_eq!(pick_result["label"], "Beta");

    // 4. Persistent storage survives plugin restart
    let write_ticket = app.execute_command(CommandRequest {
        id: "local.uiext.storageRoundTrip".into(),
        arguments: json!("write"),
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_plugins();
        if let Some(res) = write_ticket.try_result() {
            res.unwrap();
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }

    app.stop_plugin(id);
    app.start_plugin(id.into());
    pump(&mut app, |a| {
        a.plugins
            .hosts
            .get(id)
            .is_some_and(|h| h.native_active && h.commands.len() == 4)
    });
    for _ in 0..4 {
        app.poll_plugins();
        std::thread::sleep(Duration::from_millis(5));
    }
    let read_ticket = app.execute_command(CommandRequest {
        id: "local.uiext.storageRoundTrip".into(),
        arguments: json!("read"),
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let stored = loop {
        app.poll_plugins();
        if let Some(res) = read_ticket.try_result() {
            break res.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(stored["global"], "dark");
    assert_eq!(stored["workspace"], 42);
    assert_eq!(stored["allWorkspace"], json!({"count": 42}));
}

#[test]
fn native_dev_plugin_captures_console_logs_and_hot_reloads_on_entry_change() {
    graphite_plugins::sdk::check_node(Path::new("node"))
        .expect("Plugin integration tests require Node.js 24+");
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    let mut trust =
        crate::app::phase5::WorkspaceTrust::open(temp.path().join("trust.json")).unwrap();
    trust.trust(temp.path()).unwrap();
    app.workspace_trust = Some(trust);

    let dev_dir = temp.path().join("dev-live-plugin");
    std::fs::create_dir_all(dev_dir.join("dist")).unwrap();
    std::fs::write(
        dev_dir.join("graphite-plugin.json"),
        serde_json::to_vec(&graphite_plugins::manifest::Manifest {
            id: "local.devlive".into(),
            version: "0.1.0".into(),
            api_version: 1,
            main: "dist/index.js".into(),
            description: "Dev live reload plugin".into(),
            runtime: Default::default(),
            permissions: Vec::new(),
            commands: vec![graphite_plugins::manifest::Command {
                id: "local.devlive.ping".into(),
                title: "Ping".into(),
            }],
            activity_bar: Vec::new(),
            activation_events: Vec::new(),
            keybindings: Vec::new(),
            menus: Default::default(),
            themes: Vec::new(),
            snippets: Vec::new(),
            languages: Vec::new(),
        })
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        dev_dir.join("dist/index.js"),
        r#"
export async function activate(_context, g) {
    console.error("dev plugin log line v1");
    await g.commands.registerCommand("local.devlive.ping", () => "v1");
}
"#,
    )
    .unwrap();

    let store = Store::new(temp.path().join("plugins"));
    let linked = store.link_dev(&dev_dir).unwrap();
    let id = linked.metadata.id().to_owned();
    app.plugins.installed = store.list().unwrap();
    app.plugins.store = Some(store);
    app.start_plugin(id.clone());

    let deadline = Instant::now() + Duration::from_secs(5);
    while !app
        .plugins
        .hosts
        .get("local.devlive")
        .is_some_and(|h| h.native_active && !h.commands.is_empty())
    {
        app.poll_plugins();
        assert!(
            Instant::now() < deadline,
            "timed out waiting for dev plugin"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    let ticket1 = app.execute_command(CommandRequest {
        id: "local.devlive.ping".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let v1 = loop {
        app.poll_plugins();
        if let Some(res) = ticket1.try_result() {
            break res.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(v1, json!("v1"));

    let log_path = app.plugin_console_log_path(&id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if std::fs::read_to_string(&log_path)
            .map(|body| body.contains("dev plugin log line v1"))
            .unwrap_or(false)
        {
            break;
        }
        assert!(Instant::now() < deadline, "console log was not captured");
        std::thread::sleep(Duration::from_millis(10));
    }

    // Update entry script & advance dev watcher past throttle window
    std::thread::sleep(Duration::from_millis(25));
    std::fs::write(
        dev_dir.join("dist/index.js"),
        r#"
export async function activate(_context, g) {
    console.error("dev plugin log line v2");
    await g.commands.registerCommand("local.devlive.ping", () => "v2");
}
"#,
    )
    .unwrap();
    if let Some((_, _, last_check)) = app
        .plugins
        .hosts
        .get_mut("local.devlive")
        .and_then(|h| h.dev_watch.as_mut())
    {
        *last_check = Instant::now() - Duration::from_secs(2);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while !(std::fs::read_to_string(&log_path)
        .map(|body| body.contains("dev plugin log line v2"))
        .unwrap_or(false)
        && app
            .plugins
            .hosts
            .get("local.devlive")
            .is_some_and(|h| h.native_active && !h.commands.is_empty()))
    {
        app.poll_plugins();
        assert!(
            Instant::now() < deadline,
            "timed out waiting for dev live reload"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    let ticket2 = app.execute_command(CommandRequest {
        id: "local.devlive.ping".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let v2 = loop {
        app.poll_plugins();
        if let Some(res) = ticket2.try_result() {
            break res.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(v2, json!("v2"));
}

#[test]
fn native_plugins_support_tree_views_webviews_menus_keybindings_and_lazy_activation() {
    graphite_plugins::sdk::check_node(Path::new("node"))
        .expect("Plugin integration tests require Node.js 24+");
    let temp = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(temp.path().into(), None, true);
    let mut trust =
        crate::app::phase5::WorkspaceTrust::open(temp.path().join("trust.json")).unwrap();
    trust.trust(temp.path()).unwrap();
    app.workspace_trust = Some(trust);
    let store = Store::new(temp.path().join("plugins"));

    // 1. Eager Rich-UI plugin: TreeDataProvider + WebviewPanel + Keybindings + Menus
    let rich = temp.path().join("richui");
    graphite_plugins::sdk::create(&rich).unwrap();
    let rich_id = "local.rich";
    std::fs::write(
        rich.join("graphite-plugin.json"),
        serde_json::to_vec(&json!({
            "id": rich_id,
            "version": "0.1.0",
            "apiVersion": 1,
            "main": "dist/index.js",
            "description": "Rich UI plugin",
            "commands": [
                { "id": "local.rich.selectNode", "title": "Select Tree Node" },
                { "id": "local.rich.openPanel", "title": "Open Webview Panel" },
                { "id": "local.rich.refreshTree", "title": "Refresh Tree" }
            ],
            "activityBar": [{
                "id": "local.rich.sidebar",
                "title": "Rich Explorer",
                "views": [{ "id": "local.rich.tree", "title": "Items", "kind": "tree" }]
            }],
            "keybindings": [{
                "command": "local.rich.openPanel",
                "key": "ctrl+alt+r"
            }],
            "menus": {
                "commandPalette": [{ "command": "local.rich.openPanel", "when": "resourceExtname == .nonexistent" }],
                "editorContext": [{ "command": "local.rich.refreshTree", "group": "navigation" }]
            }
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(rich.join("dist")).unwrap();
    std::fs::write(
        rich.join("dist/index.js"),
        r#"
export async function activate(_context, g) {
  let suffix = 'init';
  let onTreeChange;
  const tree = await g.window.registerTreeDataProvider('local.rich.tree', {
    onDidChangeTreeData(listener) { onTreeChange = listener; return { async dispose() {} }; },
    getChildren(element) {
      if (!element) {
        return [{ id: 'folder-1', label: `Folder (${suffix})`, collapsibleState: 'collapsed' }];
      }
      if (element.id === 'folder-1') {
        return [{
          id: 'leaf-1',
          label: 'Child Item',
          description: 'leaf',
          command: { id: 'local.rich.selectNode', arguments: ['leaf-arg'] }
        }];
      }
      return [];
    }
  });
  await g.commands.registerCommand('local.rich.selectNode', async arg => {
    await g.storage.global.set('selected', arg);
    return arg;
  });
  await g.commands.registerCommand('local.rich.refreshTree', async () => {
    suffix = 'reloaded';
    await tree.refresh();
    return suffix;
  });
  await g.commands.registerCommand('local.rich.openPanel', async () => {
    const panel = await g.window.createWebviewPanel('rich.view', 'Rich Preview', { html: '<h1>Boot</h1>' });
    await panel.webview.setHtml('<h1>Updated</h1>');
    await panel.setTitle('Updated Preview');
    panel.webview.onDidReceiveMessage(async msg => {
      await panel.webview.postMessage({ echo: msg });
    });
    await g.storage.global.set('openedViaShortcut', true);
    return 'panel-open';
  });
}
"#,
    )
    .unwrap();
    let rich_pkg = graphite_plugins::package::pack(&rich).unwrap();
    store.install(&rich_pkg).unwrap();
    store.enable(rich_id, true).unwrap();

    // 2. Lazy-activated plugin triggered only onCommand:local.lazy.ping
    let lazy = temp.path().join("lazyext");
    graphite_plugins::sdk::create(&lazy).unwrap();
    let lazy_id = "local.lazy";
    std::fs::write(
        lazy.join("graphite-plugin.json"),
        serde_json::to_vec(&json!({
            "id": lazy_id,
            "version": "0.1.0",
            "apiVersion": 1,
            "main": "dist/index.js",
            "description": "Lazy plugin",
            "activationEvents": ["onCommand:local.lazy.ping"],
            "commands": [{ "id": "local.lazy.ping", "title": "Lazy Ping" }]
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(lazy.join("dist")).unwrap();
    std::fs::write(
        lazy.join("dist/index.js"),
        r#"
export async function activate(_context, g) {
  await g.commands.registerCommand('local.lazy.ping', () => 'lazy-pong');
}
"#,
    )
    .unwrap();
    let lazy_pkg = graphite_plugins::package::pack(&lazy).unwrap();
    store.install(&lazy_pkg).unwrap();
    store.enable(lazy_id, true).unwrap();

    let installed = store.list().unwrap();
    let startup: Vec<_> = installed
        .iter()
        .filter(|r| match &r.metadata {
            graphite_plugins::manifest::Metadata::Native(manifest) => {
                r.enabled && App::manifest_matches_startup(manifest)
            }
            graphite_plugins::manifest::Metadata::Vsix(_) => false,
        })
        .map(|r| r.metadata.id().to_owned())
        .collect();
    app.plugins.installed = installed;
    app.plugins.store = Some(store);
    app.sync_plugin_keybindings();
    for id in startup {
        app.queue_plugin_start(id);
    }
    app.poll_plugins();

    let deadline = Instant::now() + Duration::from_secs(6);
    while !app
        .plugins
        .hosts
        .get(rich_id)
        .is_some_and(|h| h.native_active && h.commands.len() == 3)
        || app.plugin_tree_items("local.rich.tree").is_empty()
    {
        app.poll_plugins();
        assert!(
            Instant::now() < deadline,
            "rich UI plugin did not load tree"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    // Lazy plugin did NOT start eagerly!
    assert!(!app.plugins.hosts.contains_key(lazy_id));

    // TreeDataProvider: root populated, click expands folder-1 & fetches leaf-1, click leaf-1 invokes command
    let roots = app.plugin_tree_items("local.rich.tree");
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].item.label, "Folder (init)");
    assert!(app.click_plugin_tree_row("local.rich.tree", 0));

    let deadline = Instant::now() + Duration::from_secs(5);
    while app.plugin_tree_items("local.rich.tree").len() != 2 {
        app.poll_plugins();
        assert!(Instant::now() < deadline, "folder children were not loaded");
        std::thread::sleep(Duration::from_millis(5));
    }
    let rows = app.plugin_tree_items("local.rich.tree");
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[1].item.label, "Child Item");
    assert!(app.click_plugin_tree_row("local.rich.tree", 1));

    // Verify refreshTree updates tree items
    let refresh_ticket = app.execute_command(CommandRequest {
        id: "local.rich.refreshTree".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_plugins();
        if let Some(res) = refresh_ticket.try_result() {
            assert_eq!(res.unwrap(), json!("reloaded"));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app
        .plugin_tree_items("local.rich.tree")
        .first()
        .is_some_and(|r| r.item.label == "Folder (reloaded)")
    {
        app.poll_plugins();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }

    // Keybinding ctrl+alt+r triggers local.rich.openPanel and creates WebviewPanel
    assert!(app.handle_keybinding_stroke("ctrl+alt+r"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .native_webview()
        .is_none_or(|p| p.html != "<h1>Updated</h1>" || p.title != "Updated Preview")
    {
        app.poll_plugins();
        assert!(Instant::now() < deadline, "webview panel did not update");
        std::thread::sleep(Duration::from_millis(5));
    }
    let handle = app.native_webview().unwrap().handle.clone();
    assert!(app.send_webview_message(&handle, json!({"hello": "world"})));
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .native_webview()
        .is_none_or(|p| p.outbound_messages.is_empty())
    {
        app.poll_plugins();
        assert!(Instant::now() < deadline, "webview echo not received");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        app.native_webview().unwrap().outbound_messages[0],
        json!({"echo": {"hello": "world"}})
    );

    // Menus: editorContext exposes local.rich.refreshTree, commandPalette when disables openPanel
    let editor_menus =
        app.plugin_menu_items(crate::app::extension_host::PluginMenuLocation::EditorContext);
    assert!(editor_menus
        .iter()
        .any(|m| m.command == "local.rich.refreshTree" && m.enabled));
    app.open_command_palette();
    let palette = app.command_palette.as_ref().unwrap();
    assert!(palette
        .items
        .iter()
        .any(|item| item.id == "local.rich.openPanel" && !item.enabled));
    // Lazy plugin command IS discoverable in Command Palette even before activation!
    assert!(palette
        .items
        .iter()
        .any(|item| item.id == "local.lazy.ping" && item.enabled));
    app.command_palette = None;

    // Executing lazy plugin command activates local.lazy on demand and resolves ticket!
    let lazy_ticket = app.execute_command(CommandRequest {
        id: "local.lazy.ping".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(6);
    let lazy_reply = loop {
        app.poll_plugins();
        if let Some(res) = lazy_ticket.try_result() {
            break res.unwrap();
        }
        assert!(Instant::now() < deadline, "lazy activation timed out");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(lazy_reply, json!("lazy-pong"));
}

#[test]
fn native_plugins_support_editor_languages_diagnostics_snippets_and_themes() {
    graphite_plugins::sdk::check_node(Path::new("node"))
        .expect("Plugin integration tests require Node.js 24+");
    let temp = crate::test_support::TempDir::new();
    let workspace = temp.path().join("workspace");
    let store_root = temp.path().join("plugins");
    let dev_dir = temp.path().join("dsl-plugin");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(dev_dir.join("dist")).unwrap();
    std::fs::create_dir_all(dev_dir.join("themes")).unwrap();
    std::fs::create_dir_all(dev_dir.join("snippets")).unwrap();

    let dsl_file = workspace.join("sample.dsl");
    std::fs::write(&dsl_file, "fn main() {}\nlet value = 1;").unwrap();

    std::fs::write(
        dev_dir.join("themes/neon.json"),
        r##"{"name": "Native Neon", "colors": {"editor.background": "#123456"}}"##,
    )
    .unwrap();
    std::fs::write(
        dev_dir.join("snippets/dsl.json"),
        r#"{"Define fn": {"prefix": "dslfn", "body": ["fn ${1:name}() {}"]}}"#,
    )
    .unwrap();
    std::fs::write(
        dev_dir.join("graphite-plugin.json"),
        serde_json::to_vec(&serde_json::json!({
            "id": "local.dsl",
            "version": "0.1.0",
            "apiVersion": 1,
            "main": "dist/index.js",
            "description": "Editor, language, snippet and theme test",
            "commands": [
                { "id": "local.dsl.inspect", "title": "Inspect Active" },
                { "id": "local.dsl.clear", "title": "Clear State" }
            ],
            "activationEvents": ["*"],
            "themes": [{ "id": "native-neon", "label": "Native Neon", "uiTheme": "vs-dark", "path": "./themes/neon.json" }],
            "snippets": [{ "language": "dsl", "path": "./snippets/dsl.json" }],
            "languages": [{ "id": "dsl", "extensions": [".dsl"], "lineComment": "///" }]
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        dev_dir.join("dist/index.js"),
        r#"
let activeDecor = null;
let lastUri = null;
export async function activate(context, g) {
  activeDecor = await g.editor.createDecorationType({ backgroundColor: '#ff0000' });
  await g.languages.registerCompletionProvider('*', {
    provideCompletions() {
      return [{ label: 'dslKeyword', detail: 'DSL keyword', insertText: 'dslKeyword()', documentation: 'Custom native completion' }];
    }
  });
  await g.languages.registerHoverProvider('*', {
    provideHover(_uri, pos) {
      return `Hover at ${pos.line}:${pos.character}`;
    }
  });
  await g.languages.registerFormattingProvider('*', {
    provideDocumentFormatting() {
      return [{ start: { line: 0, character: 0 }, end: { line: 0, character: 2 }, text: 'FUNC' }];
    }
  });
  context.subscriptions.push(await g.commands.registerCommand('local.dsl.inspect', async () => {
    const active = await g.editor.active();
    lastUri = active.document.uri;
    await g.editor.setSelections([{ anchor: { line: 1, character: 4 }, active: { line: 1, character: 9 } }]);
    await g.editor.setDecorations(activeDecor, active.document.uri, [{ start: { line: 0, character: 0 }, end: { line: 0, character: 2 } }]);
    await g.languages.setDiagnostics('dsl-lint', active.document.uri, [{
      start: { line: 0, character: 0 },
      end: { line: 0, character: 2 },
      message: 'Sample DSL warning',
      severity: 'warning',
      code: 'DSL001'
    }]);
    return active;
  }));
  context.subscriptions.push(await g.commands.registerCommand('local.dsl.clear', async () => {
    if (activeDecor) await activeDecor.dispose();
    if (lastUri) await g.languages.clearDiagnostics('dsl-lint', lastUri);
    return 'cleared';
  }));
}
"#,
    )
    .unwrap();

    let store = Store::new(store_root);
    store.link_dev(&dev_dir).unwrap();
    store.enable("local.dsl", true).unwrap();

    let mut app = App::new_with_options(workspace.clone(), None, true);
    let mut trust =
        crate::app::phase5::WorkspaceTrust::open(temp.path().join("trust.json")).unwrap();
    trust.trust(&workspace).unwrap();
    app.workspace_trust = Some(trust);
    std::fs::write(
        app.profile_root.join("settings.json"),
        serde_json::json!({
            "workbench.colorTheme": "Native Neon"
        })
        .to_string(),
    )
    .unwrap();
    app.plugins.installed = store.list().unwrap();
    app.plugins.store = Some(store);
    app.open_file(dsl_file);

    // Declarative languages (lineComment: "///"), snippets, and themes
    app.sync_plugin_keybindings();
    app.reload_extension_snippets();
    app.sync_extension_themes();
    assert!(app.extension_snippets.iter().any(|s| s.prefix == "dslfn"));
    let resolved_theme =
        config::imported_theme_colors_in(&app.profile_root, &workspace, &app.translator);
    assert!(
        resolved_theme.is_ok(),
        "native plugin theme should resolve by label"
    );
    app.toggle_line_comment();
    assert!(app.documents[app.active_document()]
        .buffer
        .text()
        .starts_with("/// fn main()"));
    app.toggle_line_comment();
    assert!(app.documents[app.active_document()]
        .buffer
        .text()
        .starts_with("fn main()"));

    // Start plugin and wait for activation + 3 language providers
    app.queue_plugin_start("local.dsl".into());
    app.poll_plugins();
    let deadline = Instant::now() + Duration::from_secs(6);
    while !app
        .plugins
        .hosts
        .get("local.dsl")
        .is_some_and(|h| h.native_active && h.language_providers.len() == 3)
    {
        app.poll_plugins();
        assert!(
            Instant::now() < deadline,
            "native editor/language plugin did not activate"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    // Active editor snapshot, setSelections, setDecorations, setDiagnostics
    let inspect_ticket = app.execute_command(CommandRequest {
        id: "local.dsl.inspect".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let active_snapshot = loop {
        app.poll_plugins();
        if let Some(res) = inspect_ticket.try_result() {
            break res.unwrap();
        }
        assert!(Instant::now() < deadline, "inspect command timed out");
        std::thread::sleep(Duration::from_millis(5));
    };
    let uri = active_snapshot["document"]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(!app.editor().selections().is_empty());
    let sel = app.editor().selections()[0];
    let text = app.documents[app.active_document()].buffer.text();
    let anchor_pos = graphite_core_api::char_to_position(&text, sel.anchor);
    let active_pos = graphite_core_api::char_to_position(&text, sel.head);
    assert_eq!((anchor_pos.line, anchor_pos.character), (1, 4));
    assert_eq!((active_pos.line, active_pos.character), (1, 9));

    let marks = app.vsix_line_decorations(&uri, 0, true);
    assert!(!marks.is_empty(), "decoration mark expected on line 0");
    let diags = app.diagnostic_entries();
    assert!(
        diags
            .iter()
            .any(|(_, d)| d.message == "Sample DSL warning"
                && d.source.as_deref() == Some("dsl-lint")),
        "diagnostic expected from native plugin"
    );

    // Completion, Hover, Formatting providers
    let active_target = CommandTarget::View(app.editor_groups.active_view().id.0);
    let suggest_res = app
        .execute_command(CommandRequest {
            id: "editor.action.triggerSuggest".into(),
            arguments: Value::Null,
            target: active_target.clone(),
        })
        .try_result();
    assert!(
        suggest_res.as_ref().is_none_or(|r| r.is_ok()),
        "triggerSuggest dispatch failed: {:?}",
        suggest_res
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.completion_items.is_empty() {
        app.poll_plugins();
        assert!(Instant::now() < deadline, "completion provider timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(app.completion_items[0].label, "dslKeyword");

    let hover_res = app
        .execute_command(CommandRequest {
            id: "editor.action.showHover".into(),
            arguments: Value::Null,
            target: active_target.clone(),
        })
        .try_result();
    assert!(
        hover_res.as_ref().is_none_or(|r| r.is_ok()),
        "showHover dispatch failed: {:?}",
        hover_res
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.hover_text.is_none() {
        app.poll_plugins();
        assert!(Instant::now() < deadline, "hover provider timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app
        .hover_text
        .as_deref()
        .is_some_and(|t| t.starts_with("Hover at ")));

    let format_res = app
        .execute_command(CommandRequest {
            id: "editor.action.formatDocument".into(),
            arguments: Value::Null,
            target: active_target,
        })
        .try_result();
    assert!(
        format_res.as_ref().is_none_or(|r| r.is_ok()),
        "formatDocument dispatch failed: {:?}",
        format_res
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.documents[app.active_document()]
        .buffer
        .text()
        .starts_with("FUNC main()")
    {
        app.poll_plugins();
        assert!(Instant::now() < deadline, "formatting provider timed out");
        std::thread::sleep(Duration::from_millis(5));
    }

    // Disposing decoration & clearing diagnostics removes them
    let clear_ticket = app.execute_command(CommandRequest {
        id: "local.dsl.clear".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_plugins();
        if let Some(res) = clear_ticket.try_result() {
            assert_eq!(res.unwrap(), json!("cleared"));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.vsix_line_decorations(&uri, 0, true).is_empty());
    assert!(app.diagnostic_entries().is_empty());
}

#[test]
fn native_plugins_enforce_granular_permissions_and_support_system_apis() {
    graphite_plugins::sdk::check_node(Path::new("node"))
        .expect("Plugin integration tests require Node.js 24+");
    let temp = crate::test_support::TempDir::new();
    let ws_root = temp.path().join("workspace");
    let store_root = temp.path().join("store");
    let pkg_src = temp.path().join("sys-plugin");
    std::fs::create_dir_all(ws_root.join("sub")).unwrap();
    std::fs::create_dir_all(pkg_src.join("dist")).unwrap();
    std::fs::write(ws_root.join("sub/hello.txt"), "initial body\n").unwrap();

    std::fs::write(
        pkg_src.join("graphite-plugin.json"),
        serde_json::json!({
            "id": "local.sys",
            "version": "0.1.0",
            "apiVersion": 1,
            "main": "dist/index.js",
            "description": "System API & permission test plugin",
            "permissions": ["commands", "workspace.read", "workspace.write", "terminal", "chat"],
            "commands": [
                { "id": "local.sys.run", "title": "Run System Checks" }
            ]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        pkg_src.join("dist/index.js"),
        r#"
export async function activate(_ctx, g) {
  await g.chat.registerTool(
    {
      name: "echo_workspace_tool",
      description: "Read a workspace file for chat.",
      schema: { type: "object", properties: { path: { type: "string" } }, required: ["path"] }
    },
    async (input) => {
      const content = await g.workspace.readFile(String(input?.path ?? ""));
      return { content: `read:${content.trim()}`, isError: false };
    }
  );
  await g.commands.registerCommand("local.sys.run", async () => {
    const files = await g.workspace.findFiles({ maxResults: 20 });
    const content = await g.workspace.readFile("sub/hello.txt");
    await g.workspace.writeFile("sub/hello.txt", "updated by plugin\n");
    await g.workspace.writeFile("nested/created.txt", "new file");

    let traversalBlocked = false;
    try { await g.workspace.readFile("../outside.txt"); } catch (_) { traversalBlocked = true; }

    let gitBlocked = false;
    try { await g.workspace.writeFile(".git/HEAD", "bad"); } catch (_) { gitBlocked = true; }

    let uiDenied = false;
    try { await g.window.showMessage("blocked"); } catch (e) { uiDenied = String(e).includes("ui"); }

    const scm = await g.scm.status();
    const term = await g.terminal.create({ name: "PluginTerm" });
    await term.sendText("echo hi", true);
    await term.show(false);

    return {
      files,
      oldContent: content,
      traversalBlocked,
      gitBlocked,
      uiDenied,
      scmErrorOrBranch: scm.branch ?? scm.error ?? "ok",
      termId: term.id,
    };
  });
}
"#,
    )
    .unwrap();

    let pkg = graphite_plugins::package::pack(&pkg_src).unwrap();
    let store = Store::new(store_root.clone());
    let installed = store.install(&pkg).unwrap();
    store.enable(&installed.metadata.id(), true).unwrap();

    let mut app = App::new_with_options(ws_root.clone(), None, true);
    let mut trust =
        crate::app::phase5::WorkspaceTrust::open(temp.path().join("trust.json")).unwrap();
    trust.trust(&ws_root).unwrap();
    app.workspace_trust = Some(trust);
    app.open_file(ws_root.join("sub/hello.txt"));

    app.plugins.installed = store.list().unwrap();
    app.plugins.store = Some(store);
    app.start_plugin("local.sys".into());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !app
        .plugins
        .hosts
        .get("local.sys")
        .is_some_and(|h| h.native_active)
    {
        app.poll_plugins();
        assert!(
            Instant::now() < deadline,
            "system plugin failed to activate"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(app
        .plugin_chat_tool_specs()
        .iter()
        .any(|t| t.name == "echo_workspace_tool"));

    let ticket = app.execute_command(CommandRequest {
        id: "local.sys.run".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let result = loop {
        app.poll_plugins();
        if let Some(res) = ticket.try_result() {
            break res.unwrap();
        }
        assert!(Instant::now() < deadline, "local.sys.run timed out");
        std::thread::sleep(Duration::from_millis(5));
    };

    assert_eq!(result["oldContent"], json!("initial body\n"));
    assert_eq!(result["traversalBlocked"], json!(true));
    assert_eq!(result["gitBlocked"], json!(true));
    assert_eq!(result["uiDenied"], json!(true));
    assert!(result["termId"].as_u64().unwrap_or(0) >= 1);
    assert!(!app.terminals.is_empty());
    assert!(app.terminal_visible);
    assert_eq!(
        std::fs::read_to_string(ws_root.join("nested/created.txt")).unwrap(),
        "new file"
    );
    assert_eq!(app.editor().text(), "updated by plugin\n");

    let (tx, rx) = std::sync::mpsc::channel();
    app.invoke_plugin_chat_tool(
        "echo_workspace_tool",
        json!({ "path": "nested/created.txt" }),
        Some(tx),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let tool_out = loop {
        app.poll_plugins();
        if let Ok(val) = rx.try_recv() {
            break val.unwrap();
        }
        assert!(Instant::now() < deadline, "chat tool RPC timed out");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(tool_out, "read:new file");
}

#[test]
fn native_rust_binary_plugin_runs_without_node() {
    let temp = crate::test_support::TempDir::new();
    let ws_root = temp.path().join("workspace");
    let store_root = temp.path().join("store");
    let plugin_src = temp.path().join("native-plugin");
    std::fs::create_dir_all(&ws_root).unwrap();
    std::fs::create_dir_all(plugin_src.join("bin")).unwrap();

    // Compile a tiny standalone native executable implementing v2 framed protocol without Node.js.
    let rs_src = temp.path().join("native_plugin_bin.rs");
    std::fs::write(
        &rs_src,
        r#"
use std::io::{self, Read, Write};

fn read_frame() -> io::Result<String> {
    let mut len_buf = [0u8; 4];
    io::stdin().read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut body = vec![0u8; len];
    io::stdin().read_exact(&mut body)?;
    Ok(String::from_utf8(body).unwrap())
}

fn write_frame(json: &str) {
    let bytes = json.as_bytes();
    let mut out = io::stdout().lock();
    out.write_all(&(bytes.len() as u32).to_be_bytes()).unwrap();
    out.write_all(bytes).unwrap();
    out.flush().unwrap();
}

fn extract_u64(s: &str, key: &str) -> Option<u64> {
    let idx = s.find(key)? + key.len();
    let digits: String = s[idx..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn main() {
    let hello = read_frame().unwrap();
    let session = extract_u64(&hello, "\"session\":").unwrap();
    write_frame(&format!("{{\"Ready\":{{\"version\":2,\"session\":{session}}}}}"));
    write_frame(&format!(
        "{{\"Request\":{{\"session\":{session},\"id\":1,\"request\":{{\"RegisterCommand\":{{\"id\":\"local.rustbin.ping\",\"title\":\"Rust Bin Ping\",\"category\":\"Plugin\",\"targets\":[\"Global\"],\"enablement\":\"true\"}}}}}}}}"
    ));
    let reg_reply = read_frame().unwrap();
    assert!(!reg_reply.contains("\"Err\""), "register failed: {reg_reply}");
    write_frame(&format!("{{\"Request\":{{\"session\":{session},\"id\":2,\"request\":\"Activated\"}}}}"));
    let _ = read_frame().unwrap();

    while let Ok(msg) = read_frame() {
        if msg.contains("\"Shutdown\"") {
            break;
        }
        if msg.contains("\"Invoke\"") {
            let id = extract_u64(&msg, "\"id\":").unwrap();
            write_frame(&format!(
                "{{\"Response\":{{\"session\":{session},\"id\":{id},\"result\":{{\"Ok\":{{\"native\":true}}}}}}}}"
            ));
        }
    }
}
"#,
    )
    .unwrap();

    let exe_name = if cfg!(windows) {
        "native-echo.exe"
    } else {
        "native-echo"
    };
    let bin_out = plugin_src.join("bin").join(exe_name);
    let status = std::process::Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg(&rs_src)
        .arg("-o")
        .arg(&bin_out)
        .status()
        .expect("rustc must be available to run test");
    assert!(status.success(), "compiling test native plugin failed");

    std::fs::write(
        plugin_src.join("graphite-plugin.json"),
        serde_json::json!({
            "id": "local.rustbin",
            "version": "0.1.0",
            "apiVersion": 1,
            "runtime": "native",
            "main": "bin/native-echo",
            "description": "Compiled binary native plugin",
            "permissions": ["commands"],
            "commands": [{ "id": "local.rustbin.ping", "title": "Rust Bin Ping" }]
        })
        .to_string(),
    )
    .unwrap();

    let pkg = graphite_plugins::package::pack(&plugin_src).unwrap();
    let store = Store::new(store_root);
    let installed = store.install(&pkg).unwrap();
    store.enable(&installed.metadata.id(), true).unwrap();

    let mut app = App::new_with_options(ws_root.clone(), None, true);
    let mut trust =
        crate::app::phase5::WorkspaceTrust::open(temp.path().join("trust.json")).unwrap();
    trust.trust(&ws_root).unwrap();
    app.workspace_trust = Some(trust);
    // Deliberately point nodePath at a non-existent binary to prove native runtime never uses Node.js.
    std::fs::write(
        app.profile_root.join("settings.json"),
        serde_json::json!({
            "graphite.plugins.nodePath": "missing-node-executable-should-not-run"
        })
        .to_string(),
    )
    .unwrap();
    app.plugins.installed = store.list().unwrap();
    app.plugins.store = Some(store);
    app.start_plugin("local.rustbin".into());

    let deadline = Instant::now() + Duration::from_secs(10);
    while !app
        .plugins
        .hosts
        .get("local.rustbin")
        .is_some_and(|h| h.native_active)
    {
        app.poll_plugins();
        assert!(
            Instant::now() < deadline,
            "native binary plugin failed to activate"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    let ticket = app.execute_command(CommandRequest {
        id: "local.rustbin.ping".into(),
        arguments: Value::Null,
        target: CommandTarget::Global,
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_plugins();
        if let Some(res) = ticket.try_result() {
            assert_eq!(res.unwrap(), json!({ "native": true }));
            break;
        }
        assert!(Instant::now() < deadline, "native binary command timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}
