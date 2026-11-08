use super::*;
use serde_json::{json, Value};
use graphite_core_api::*;
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
        }),
        enabled: false,
        trusted_version: None,
        directory: "sample.vsix@1.0.0".into(),
    });
    app.plugin_action('\n');
    assert!(matches!(app.plugins.inspecting, Some(Metadata::Vsix(_))));
    app.scroll_plugins(100);
    assert_eq!(app.plugins.index, 0);
}
