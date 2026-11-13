//! Debug adapters that an extension creates in code (`registerDebugAdapterDescriptorFactory`).
//!
//! Graphite asks the extension host to activate the extension that provides the debug `type`
//! and to run its factory. The bridge answers with the adapter descriptor, which Graphite then
//! starts like any other adapter: as a process, or as a connection to a local TCP port.

use super::*;
use crate::app::debug_adapters::{adapter_from_descriptor, AdapterEndpoint};
use crate::debugger::{expand_value, LaunchConfig, VariableContext};

/// Id of the bridge reply that carries the adapter descriptor.
pub(super) const ADAPTER_CALL_ID: u64 = 5_000_000;
const ADAPTER_TIMEOUT: Duration = Duration::from_secs(30);

/// An adapter request that is waiting for the extension host.
pub(super) struct PendingAdapter {
    config: LaunchConfig,
    owner: String,
    /// The bridge message that asks for the adapter.
    request: Value,
    sent: bool,
    deadline: Instant,
}

/// The bridge message for `config`: activate `owner`, run its adapter factory for the debug type
/// and send back the descriptor. Variables in the configuration are expanded first.
pub(super) fn adapter_request(
    config: &LaunchConfig,
    owner: &str,
    context: &VariableContext,
) -> Result<Value> {
    let mut configuration = serde_json::to_value(config).map_err(|error| error.to_string())?;
    if let Some(object) = configuration.as_object_mut() {
        for key in ["adapter", "preLaunchTask", "postDebugTask"] {
            object.remove(key);
        }
    }
    expand_value(&mut configuration, context);
    Ok(json!({
        "type": "provideDebugAdapter",
        "id": ADAPTER_CALL_ID,
        "owner": owner,
        "debugType": config.adapter_type,
        "session": {
            "name": config.name,
            "type": config.adapter_type,
            "configuration": configuration,
        },
    }))
}

impl App {
    /// Asks the extension `owner` to create the adapter for `config`. The session starts from
    /// [`Self::finish_vsix_debug_adapter`] when the extension host answers.
    pub(crate) fn request_vsix_debug_adapter(
        &mut self,
        config: LaunchConfig,
        owner: &str,
    ) -> Result<()> {
        if !self.vsix_workspace_trusted() {
            return Err("Workspace is restricted. Trust it before starting this debugger.".into());
        }
        if !self
            .plugins
            .installed
            .iter()
            .any(|record| record.metadata.id() == owner && super::eligible(record))
        {
            return Err(format!(
                "Enable and approve {owner} in Plugins first; it creates the '{}' debug adapter.",
                config.adapter_type
            ));
        }
        if self
            .plugins
            .vsix
            .as_ref()
            .is_some_and(|runtime| runtime.debug_adapter.is_some())
        {
            return Err("A debug adapter is already being created".into());
        }
        let context = VariableContext {
            workspace: self.tree.root().to_path_buf(),
            active_file: self.documents[self.active_document()]
                .buffer
                .path()
                .map(Path::to_path_buf),
        };
        let request = adapter_request(&config, owner, &context)?;
        if self.plugins.vsix.is_none() {
            self.start_vsix_host(Session::bundled_runtime())?;
        }
        let runtime = self
            .plugins
            .vsix
            .as_mut()
            .ok_or("The extension host is unavailable")?;
        runtime.debug_adapter = Some(PendingAdapter {
            config,
            owner: owner.to_owned(),
            request,
            sent: false,
            deadline: Instant::now() + ADAPTER_TIMEOUT,
        });
        self.status = format!("Waiting for {owner} to create the debug adapter…");
        Ok(())
    }

    /// Forgets a pending adapter request, for example when the user stops debugging first. A late
    /// answer from the host is then ignored.
    pub(crate) fn cancel_vsix_debug_adapter(&mut self) -> bool {
        self.plugins
            .vsix
            .as_mut()
            .is_some_and(|runtime| runtime.debug_adapter.take().is_some())
    }

    pub(crate) fn vsix_debug_adapter_pending(&self) -> bool {
        self.plugins
            .vsix
            .as_ref()
            .is_some_and(|runtime| runtime.debug_adapter.is_some())
    }

    /// Sends the pending request once the host is ready, and gives up after the timeout.
    pub(super) fn drive_vsix_debug_adapter(&mut self, runtime: &mut Runtime) -> Result<()> {
        let Some(pending) = runtime.debug_adapter.as_mut() else {
            return Ok(());
        };
        if Instant::now() >= pending.deadline {
            let owner = pending.owner.clone();
            runtime.debug_adapter = None;
            self.status = format!("{owner} did not create the debug adapter in time");
            return Ok(());
        }
        if !pending.sent {
            runtime.session.send(pending.request.clone())?;
            pending.sent = true;
        }
        Ok(())
    }

    /// Handles the host's answer: starts the session with the adapter the extension described.
    pub(super) fn finish_vsix_debug_adapter(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(pending) = runtime.debug_adapter.take() else {
            // The request was cancelled before the answer arrived.
            return;
        };
        if let Some(error) = event["error"].as_str() {
            self.status = format!(
                "{} could not create the debug adapter: {error}",
                pending.owner
            );
            return;
        }
        let mut config = pending.config;
        match adapter_from_descriptor(&event["result"]) {
            Ok(AdapterEndpoint::Command(command)) => config.adapter = Some(command),
            Ok(AdapterEndpoint::Server(server)) => config.server = Some(server),
            Err(error) => {
                self.status = format!("{}: {error}", pending.owner);
                return;
            }
        }
        if let Err(error) = self.start_resolved_debug_session(config) {
            self.status = error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use graphite_extension_host::code_oss::LoopbackHost;
    use graphite_plugins::manifest::Vsix;
    use std::io::{BufRead, BufReader, Read};
    use std::net::TcpListener;

    const OWNER: &str = "pub.coder";

    fn config(debug_type: &str) -> LaunchConfig {
        serde_json::from_value(json!({
            "name": "Run it", "type": debug_type, "request": "launch",
            "program": "${workspaceFolder}/app.py", "args": ["${fileBasename}"],
            "preLaunchTask": "build",
        }))
        .unwrap()
    }

    fn installed(approved: bool) -> Installed {
        Installed {
            metadata: Metadata::Vsix(Vsix {
                id: OWNER.into(),
                version: "1.0.0".into(),
                description: String::new(),
                engines: json!({}),
                main: Some("extension.js".into()),
                contributions: vec!["debuggers".into()],
                commands: vec![],
                activity_bar: vec![],
                language_comments: vec![],
                snippets: vec![],
                debuggers: vec![],
            }),
            enabled: true,
            trusted_version: approved.then(|| "1.0.0".to_owned()),
            directory: "coder-1".into(),
        }
    }

    fn runtime(session: Session) -> Runtime {
        Runtime {
            session,
            ready: true,
            registered: BTreeSet::new(),
            command: None,
            sent: false,
            synced: None,
            prompt: None,
            languages: Default::default(),
            editor_commands: BTreeMap::new(),
            test_calls: BTreeSet::new(),
            workbench: Default::default(),
            terminals: Default::default(),
            watches: Default::default(),
            decorations: Default::default(),
            debug_adapter: None,
            unsupported: BTreeSet::new(),
        }
    }

    /// A trusted app whose extension host is a loopback the test controls.
    fn app_with_host(dir: &TempDir, approved: bool) -> (App, LoopbackHost) {
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.workspace_trust =
            Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
        app.run_ui_command("graphite.security.trustWorkspace");
        app.plugins.installed = vec![installed(approved)];
        let (session, host) = Session::loopback();
        app.plugins.vsix = Some(runtime(session));
        (app, host)
    }

    /// The next message of `kind` that Graphite sent to the host.
    fn sent_of(host: &LoopbackHost, kind: &str) -> Option<Value> {
        std::iter::from_fn(|| host.next_sent()).find(|message| message["type"] == kind)
    }

    fn answer(host: &LoopbackHost, body: Value) {
        let mut event = json!({"type": "result", "id": ADAPTER_CALL_ID});
        event
            .as_object_mut()
            .unwrap()
            .extend(body.as_object().unwrap().clone());
        host.emit(event);
    }

    #[test]
    fn the_request_expands_variables_and_hides_graphite_only_settings() {
        let dir = TempDir::new();
        let context = VariableContext {
            workspace: dir.path().to_path_buf(),
            active_file: Some(dir.path().join("src/main.py")),
        };
        let message = adapter_request(&config("python"), OWNER, &context).unwrap();
        assert_eq!(message["type"], "provideDebugAdapter");
        assert_eq!(message["id"], ADAPTER_CALL_ID);
        assert_eq!(message["owner"], OWNER);
        assert_eq!(message["debugType"], "python");
        let session = &message["session"];
        assert_eq!(session["name"], "Run it");
        assert_eq!(session["type"], "python");
        let configuration = &session["configuration"];
        assert_eq!(
            configuration["program"],
            format!("{}/app.py", dir.path().display())
        );
        assert_eq!(configuration["args"][0], "main.py");
        assert!(configuration.get("preLaunchTask").is_none());
        assert!(configuration.get("adapter").is_none());
    }

    #[test]
    fn the_request_is_sent_to_the_ready_host_once() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_host(&dir, true);
        app.request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap();
        assert!(app.vsix_debug_adapter_pending());
        assert!(
            app.status.contains("Waiting for pub.coder"),
            "{}",
            app.status
        );
        app.poll_vsix();
        app.poll_vsix();
        let message = sent_of(&host, "provideDebugAdapter").expect("request sent");
        assert_eq!(message["owner"], OWNER);
        assert!(
            sent_of(&host, "provideDebugAdapter").is_none(),
            "the same request is not repeated"
        );
    }

    #[test]
    fn a_second_request_while_one_is_pending_is_refused() {
        let dir = TempDir::new();
        let (mut app, _host) = app_with_host(&dir, true);
        app.request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap();
        let error = app
            .request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap_err();
        assert!(error.contains("already being created"), "{error}");
    }

    #[test]
    fn unapproved_extensions_and_untrusted_workspaces_cannot_provide_an_adapter() {
        let dir = TempDir::new();
        let (mut app, _host) = app_with_host(&dir, false);
        let error = app
            .request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap_err();
        assert!(error.contains("approve pub.coder"), "{error}");
        assert!(!app.vsix_debug_adapter_pending());

        let (mut app, _host) = app_with_host(&dir, true);
        app.workspace_trust = Some(crate::app::phase5::WorkspaceTrust::restricted(
            dir.path().join("restricted.json"),
        ));
        let error = app
            .request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap_err();
        assert!(error.contains("restricted"), "{error}");
    }

    #[test]
    fn a_server_descriptor_starts_a_session_over_tcp() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_host(&dir, true);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        app.request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap();
        app.poll_vsix();
        answer(&host, json!({"result": {"type": "server", "port": port}}));
        app.poll_vsix();

        assert!(!app.vsix_debug_adapter_pending());
        assert!(app.debug.session.is_some(), "status: {}", app.status);
        assert!(app.debug_panel_visible);
        // The adapter receives the DAP `initialize` request over the socket.
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let length: usize = header
            .trim()
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        let mut blank = String::new();
        reader.read_line(&mut blank).unwrap();
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        let request: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(request["command"], "initialize");
        assert_eq!(request["arguments"]["adapterID"], "python");
        app.debug.stop().unwrap();
    }

    #[test]
    fn an_executable_descriptor_fills_in_the_adapter_command() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_host(&dir, true);
        app.request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap();
        app.poll_vsix();
        answer(
            &host,
            json!({"result": {"type": "executable", "command": "graphite-no-such-adapter-binary"}}),
        );
        app.poll_vsix();
        // The command does not exist, so starting it fails, but only after the descriptor was used.
        assert!(!app.vsix_debug_adapter_pending());
        assert!(app.debug.session.is_none());
        assert!(
            !app.status.contains("did not return") && !app.status.contains("could not create"),
            "{}",
            app.status
        );
    }

    #[test]
    fn errors_and_unusable_descriptors_end_the_wait_with_a_message() {
        for (body, expected) in [
            (
                json!({"error": "Error: boom"}),
                "could not create the debug adapter: Error: boom",
            ),
            (json!({"result": null}), "did not return a debug adapter"),
            (
                json!({"result": {"type": "implementation"}}),
                "inside the extension host are not supported yet",
            ),
            (
                json!({"result": {"type": "server", "port": 0}}),
                "valid port",
            ),
        ] {
            let dir = TempDir::new();
            let (mut app, host) = app_with_host(&dir, true);
            app.request_vsix_debug_adapter(config("python"), OWNER)
                .unwrap();
            app.poll_vsix();
            answer(&host, body);
            app.poll_vsix();
            assert!(!app.vsix_debug_adapter_pending());
            assert!(app.debug.session.is_none());
            assert!(
                app.status.contains(expected),
                "{expected} not in {}",
                app.status
            );
        }
    }

    #[test]
    fn a_non_loopback_server_is_refused() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_host(&dir, true);
        app.request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap();
        app.poll_vsix();
        answer(
            &host,
            json!({"result": {"type": "server", "host": "192.0.2.1", "port": 9}}),
        );
        app.poll_vsix();
        assert!(app.debug.session.is_none());
        assert!(app.status.contains("not a local address"), "{}", app.status);
    }

    #[test]
    fn stopping_first_discards_a_late_answer() {
        let dir = TempDir::new();
        let (mut app, host) = app_with_host(&dir, true);
        app.request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap();
        app.poll_vsix();
        app.stop_debugging().unwrap();
        assert_eq!(app.status, "Debugging cancelled");
        assert!(!app.vsix_debug_adapter_pending());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        answer(&host, json!({"result": {"type": "server", "port": port}}));
        app.poll_vsix();
        assert!(
            app.debug.session.is_none(),
            "the cancelled launch must not start"
        );
    }

    #[test]
    fn a_host_that_never_answers_times_out() {
        let dir = TempDir::new();
        let (mut app, _host) = app_with_host(&dir, true);
        app.request_vsix_debug_adapter(config("python"), OWNER)
            .unwrap();
        app.plugins
            .vsix
            .as_mut()
            .unwrap()
            .debug_adapter
            .as_mut()
            .unwrap()
            .deadline = Instant::now() - Duration::from_secs(1);
        app.poll_vsix();
        assert!(!app.vsix_debug_adapter_pending());
        assert!(app
            .status
            .contains("did not create the debug adapter in time"));
    }
}
