//! Finds the debug adapter for a launch configuration. A `launch.json` normally names only the
//! debug `type`; the adapter comes from the installed extension that contributes that type
//! (`contributes.debuggers`). An explicit `"adapter"` in the configuration still wins.
//!
//! An extension either points at an adapter program (`program`), which Graphite starts itself, or
//! creates the adapter in code with `registerDebugAdapterDescriptorFactory`; that second case is
//! resolved by asking the extension host (see `plugins::vsix::debug`).

use super::*;
use crate::debugger::{AdapterCommand, AdapterServer, LaunchConfig};
use graphite_plugins::manifest::{debugger_platform, DebuggerContribution, Metadata};
use serde_json::Value;
use std::path::Path;

/// Where the adapter of a launch configuration comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AdapterSource {
    /// `config.adapter` is set; the adapter can be started now.
    Ready,
    /// The extension with this id must create the adapter in the extension host.
    Extension(String),
}

/// How to reach an adapter an extension created in code (`IAdapterDescriptor`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AdapterEndpoint {
    Command(AdapterCommand),
    Server(AdapterServer),
    /// A named pipe (Windows) or local socket (Unix).
    Pipe(PathBuf),
    /// The adapter runs inside the extension host; its messages go through the bridge.
    Inline,
}

/// Reads the descriptor an extension's `DebugAdapterDescriptorFactory` returned.
///
/// Executables, local TCP servers, named pipes and adapters implemented inside the extension
/// host (`DebugAdapterInlineImplementation`) are supported.
pub(crate) fn adapter_from_descriptor(descriptor: &Value) -> Result<AdapterEndpoint, String> {
    let Some(kind) = descriptor["type"].as_str() else {
        return Err("The extension did not return a debug adapter".into());
    };
    match kind {
        "executable" => {
            let command = descriptor["command"]
                .as_str()
                .filter(|command| !command.trim().is_empty())
                .ok_or("The debug adapter executable has no command")?;
            let args = descriptor["args"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect();
            let env = descriptor["options"]["env"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                .collect();
            Ok(AdapterEndpoint::Command(AdapterCommand {
                command: command.into(),
                args,
                cwd: descriptor["options"]["cwd"].as_str().map(Into::into),
                env,
            }))
        }
        "server" => {
            let port = descriptor["port"]
                .as_u64()
                .and_then(|port| u16::try_from(port).ok())
                .filter(|port| *port != 0)
                .ok_or("The debug adapter server has no valid port")?;
            let host = descriptor["host"].as_str().unwrap_or("127.0.0.1");
            Ok(AdapterEndpoint::Server(AdapterServer {
                host: host.to_owned(),
                port,
            }))
        }
        "pipeServer" | "namedPipeServer" => {
            let path = descriptor["path"]
                .as_str()
                .filter(|path| !path.trim().is_empty())
                .ok_or("The debug adapter pipe has no path")?;
            Ok(AdapterEndpoint::Pipe(path.into()))
        }
        "implementation" => Ok(AdapterEndpoint::Inline),
        other => Err(format!("Unknown debug adapter kind '{other}'")),
    }
}

/// The command that starts `contribution`'s adapter, for `platform`, or `None` when the
/// contribution declares no `program` (the extension creates its adapter in code).
///
/// `program` is a path inside `extension_dir`. A `runtime` runs it (`node` uses the runtime that
/// ships with Graphite when there is one); without a runtime the program is executed directly.
pub(crate) fn adapter_command(
    contribution: &DebuggerContribution,
    extension_dir: &Path,
    platform: &str,
    bundled_node: Option<&Path>,
) -> Result<Option<AdapterCommand>, String> {
    let executable = contribution.executable_for(platform);
    let program = executable.program.as_deref().map(|p| extension_dir.join(p));
    if let Some(program) = &program {
        if !program.is_file() {
            return Err(format!(
                "The debug adapter program {} is missing from the extension",
                program.display()
            ));
        }
    }
    let args = executable.args.unwrap_or_default();
    match (executable.runtime, program) {
        (Some(runtime), program) => {
            let command = match (runtime.as_str(), bundled_node) {
                ("node", Some(node)) => node.to_path_buf(),
                _ => PathBuf::from(runtime),
            };
            let mut all = executable.runtime_args.unwrap_or_default();
            all.extend(program.map(|p| p.display().to_string()));
            all.extend(args);
            Ok(Some(AdapterCommand {
                command,
                args: all,
                ..Default::default()
            }))
        }
        (None, Some(program)) => Ok(Some(AdapterCommand {
            command: program,
            args,
            ..Default::default()
        })),
        (None, None) => Ok(None),
    }
}

impl App {
    /// Finds the adapter for `config`. When the extension that provides the debug `type` ships an
    /// adapter program, `config.adapter` is filled in. When the extension creates its adapter in
    /// code, the extension's id is returned for the caller to ask the extension host.
    pub(crate) fn resolve_debug_adapter(
        &self,
        config: &mut LaunchConfig,
    ) -> Result<AdapterSource, String> {
        if config.adapter.is_some() || config.server.is_some() {
            return Ok(AdapterSource::Ready);
        }
        let Some(root) = self.plugins.store.as_ref().map(|store| store.root.clone()) else {
            return Err(missing_adapter(&config.adapter_type, None));
        };
        let mut disabled_provider = None;
        for record in &self.plugins.installed {
            let Metadata::Vsix(vsix) = &record.metadata else {
                continue;
            };
            let Some(contribution) = vsix
                .debuggers
                .iter()
                .find(|debugger| debugger.debug_type == config.adapter_type)
            else {
                continue;
            };
            if !record.enabled {
                disabled_provider.get_or_insert_with(|| vsix.id.clone());
                continue;
            }
            let extension_dir = root.join(&record.directory).join("extension");
            let bundled_node = graphite_extension_host::code_oss::Session::bundled_runtime()
                .map(|runtime| runtime.join(if cfg!(windows) { "node.exe" } else { "node" }))
                .filter(|node| node.is_file());
            let adapter = adapter_command(
                contribution,
                &extension_dir,
                debugger_platform(),
                bundled_node.as_deref(),
            )?;
            // An approved extension with code gets to resolve the configuration (and, without a
            // `program`, to create the adapter) in the extension host.
            let runs_code = vsix.main.is_some() && super::plugins::vsix_extension_runnable(record);
            return match (adapter, runs_code) {
                (adapter, true) => {
                    config.adapter = adapter;
                    Ok(AdapterSource::Extension(vsix.id.clone()))
                }
                (Some(adapter), false) => {
                    config.adapter = Some(adapter);
                    Ok(AdapterSource::Ready)
                }
                (None, false) if vsix.main.is_some() => Err(format!(
                    "The '{}' debugger in {} creates its adapter in code. Enable and approve the \
                     extension in Plugins first",
                    config.adapter_type, vsix.id
                )),
                (None, false) => Err(format!(
                    "The '{}' debugger in {} declares no program and the extension has no code to \
                     create an adapter",
                    config.adapter_type, vsix.id
                )),
            };
        }
        Err(missing_adapter(
            &config.adapter_type,
            disabled_provider.as_deref(),
        ))
    }
}

fn missing_adapter(debug_type: &str, disabled_provider: Option<&str>) -> String {
    match disabled_provider {
        Some(extension) => format!(
            "The '{debug_type}' debugger is provided by {extension}, which is disabled. Enable it and try again"
        ),
        None => format!(
            "No installed extension provides a debugger for type '{debug_type}'. Install the \
             extension that supplies it, or set \"adapter\" in launch.json"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphite_plugins::manifest::DebuggerExecutable;

    fn contribution(executable: DebuggerExecutable) -> DebuggerContribution {
        DebuggerContribution {
            debug_type: "demo".into(),
            label: None,
            executable,
            windows: None,
            linux: None,
            osx: None,
        }
    }

    fn command_of(
        contribution: &DebuggerContribution,
        dir: &Path,
        platform: &str,
        node: Option<&Path>,
    ) -> AdapterCommand {
        adapter_command(contribution, dir, platform, node)
            .unwrap()
            .expect("the contribution declares a program")
    }

    fn extension_with(program: &str) -> crate::test_support::TempDir {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join(program);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
        dir
    }

    #[test]
    fn a_program_without_a_runtime_runs_directly_with_its_arguments() {
        let dir = extension_with("bin/adapter");
        let adapter = command_of(
            &contribution(DebuggerExecutable {
                program: Some("bin/adapter".into()),
                args: Some(vec!["--stdio".into()]),
                ..Default::default()
            }),
            dir.path(),
            "linux",
            None,
        );
        assert_eq!(adapter.command, dir.path().join("bin/adapter"));
        assert_eq!(adapter.args, ["--stdio"]);
    }

    #[test]
    fn a_runtime_runs_the_program_after_its_own_arguments() {
        let dir = extension_with("out/adapter.js");
        let adapter = command_of(
            &contribution(DebuggerExecutable {
                program: Some("out/adapter.js".into()),
                runtime: Some("python".into()),
                runtime_args: Some(vec!["-u".into()]),
                args: Some(vec!["--port".into(), "0".into()]),
            }),
            dir.path(),
            "linux",
            None,
        );
        assert_eq!(adapter.command, PathBuf::from("python"));
        assert_eq!(
            adapter.args,
            [
                "-u".to_owned(),
                dir.path().join("out/adapter.js").display().to_string(),
                "--port".into(),
                "0".into()
            ]
        );
    }

    #[test]
    fn the_node_runtime_prefers_the_bundled_node() {
        let dir = extension_with("out/adapter.js");
        let executable = DebuggerExecutable {
            program: Some("out/adapter.js".into()),
            runtime: Some("node".into()),
            ..Default::default()
        };
        let bundled = dir.path().join("runtime/node");
        let adapter = command_of(
            &contribution(executable.clone()),
            dir.path(),
            "linux",
            Some(&bundled),
        );
        assert_eq!(adapter.command, bundled);
        let adapter = command_of(&contribution(executable), dir.path(), "linux", None);
        assert_eq!(adapter.command, PathBuf::from("node"), "falls back to PATH");
    }

    #[test]
    fn platform_overrides_choose_the_program() {
        let dir = extension_with("bin/adapter.exe");
        let mut debugger = contribution(DebuggerExecutable {
            program: Some("bin/adapter".into()),
            ..Default::default()
        });
        debugger.windows = Some(DebuggerExecutable {
            program: Some("bin/adapter.exe".into()),
            ..Default::default()
        });
        let adapter = command_of(&debugger, dir.path(), "windows", None);
        assert_eq!(adapter.command, dir.path().join("bin/adapter.exe"));
        // On another platform the shared program is used, and it is not there.
        assert!(adapter_command(&debugger, dir.path(), "linux", None)
            .unwrap_err()
            .contains("is missing from the extension"));
    }

    #[test]
    fn an_adapter_created_in_code_has_no_command() {
        let dir = crate::test_support::TempDir::new();
        let command = adapter_command(
            &contribution(DebuggerExecutable::default()),
            dir.path(),
            "linux",
            None,
        )
        .unwrap();
        assert_eq!(command, None, "the extension host has to create it");
    }

    #[test]
    fn executable_descriptors_become_commands_with_options() {
        let endpoint = adapter_from_descriptor(&serde_json::json!({
            "type": "executable", "command": "debugpy-adapter",
            "args": ["--log", 7, "x"],
            "options": {"cwd": "/work", "env": {"A": "1", "B": 2}}
        }))
        .unwrap();
        let AdapterEndpoint::Command(command) = endpoint else {
            panic!("expected a command");
        };
        assert_eq!(command.command, PathBuf::from("debugpy-adapter"));
        assert_eq!(command.args, ["--log", "x"], "non-strings are dropped");
        assert_eq!(command.cwd, Some(PathBuf::from("/work")));
        assert_eq!(command.env.get("A").map(String::as_str), Some("1"));
        assert!(!command.env.contains_key("B"));
    }

    #[test]
    fn server_descriptors_default_to_the_loopback_host() {
        assert_eq!(
            adapter_from_descriptor(&serde_json::json!({"type": "server", "port": 4711})).unwrap(),
            AdapterEndpoint::Server(AdapterServer {
                host: "127.0.0.1".into(),
                port: 4711
            })
        );
        assert_eq!(
            adapter_from_descriptor(
                &serde_json::json!({"type": "server", "port": 9, "host": "localhost"})
            )
            .unwrap(),
            AdapterEndpoint::Server(AdapterServer {
                host: "localhost".into(),
                port: 9
            })
        );
    }

    #[test]
    fn pipes_and_inline_implementations_are_endpoints() {
        assert_eq!(
            adapter_from_descriptor(
                &serde_json::json!({"type": "pipeServer", "path": "/tmp/dap.sock"})
            )
            .unwrap(),
            AdapterEndpoint::Pipe("/tmp/dap.sock".into())
        );
        assert_eq!(
            adapter_from_descriptor(&serde_json::json!({"type": "implementation"})).unwrap(),
            AdapterEndpoint::Inline
        );
    }

    #[test]
    fn malformed_and_unsupported_descriptors_explain_themselves() {
        let error = |value: serde_json::Value| adapter_from_descriptor(&value).unwrap_err();
        assert!(error(serde_json::Value::Null).contains("did not return a debug adapter"));
        assert!(error(serde_json::json!({})).contains("did not return a debug adapter"));
        assert!(error(serde_json::json!({"type": "executable"})).contains("no command"));
        assert!(
            error(serde_json::json!({"type": "executable", "command": "  "}))
                .contains("no command")
        );
        for port in [
            serde_json::json!(0),
            serde_json::json!(70000),
            serde_json::json!("1"),
        ] {
            assert!(
                error(serde_json::json!({"type": "server", "port": port})).contains("valid port")
            );
        }
        assert!(error(serde_json::json!({"type": "pipeServer"})).contains("no path"));
        assert!(error(serde_json::json!({"type": "pipeServer", "path": " "})).contains("no path"));
        assert!(error(serde_json::json!({"type": "weird"}))
            .contains("Unknown debug adapter kind 'weird'"));
    }

    fn app_with_debugger(
        dir: &crate::test_support::TempDir,
        enabled: bool,
        program: Option<&str>,
    ) -> App {
        use graphite_plugins::manifest::Vsix;
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let store_root = dir.path().join("plugins");
        let directory = "ext-1".to_owned();
        let installed_dir = store_root.join(&directory).join("extension");
        std::fs::create_dir_all(installed_dir.join("bin")).unwrap();
        std::fs::write(installed_dir.join("bin/adapter"), "").unwrap();
        let record = graphite_plugins::store::Installed {
            metadata: Metadata::Vsix(Vsix {
                id: "pub.demo".into(),
                version: "1.0.0".into(),
                description: String::new(),
                engines: serde_json::json!({}),
                main: None,
                contributions: vec!["debuggers".into()],
                commands: vec![],
                activity_bar: vec![],
                language_comments: vec![],
                snippets: vec![],
                debuggers: vec![contribution(DebuggerExecutable {
                    program: program.map(str::to_owned),
                    ..Default::default()
                })],
                ..Default::default()
            }),
            directory,
            enabled,
            trusted_version: None,
            dev_path: None,
        };
        app.plugins.store = Some(graphite_plugins::store::Store::new(store_root));
        app.plugins.installed = vec![record];
        app
    }

    fn launch(debug_type: &str) -> LaunchConfig {
        serde_json::from_value(serde_json::json!({
            "name": "x", "type": debug_type, "request": "launch"
        }))
        .unwrap()
    }

    #[test]
    fn the_adapter_comes_from_the_enabled_extension_that_provides_the_type() {
        let dir = crate::test_support::TempDir::new();
        let app = app_with_debugger(&dir, true, Some("bin/adapter"));
        let mut config = launch("demo");
        assert_eq!(
            app.resolve_debug_adapter(&mut config).unwrap(),
            AdapterSource::Ready
        );
        let adapter = config.adapter.unwrap();
        assert!(
            adapter.command.ends_with("ext-1/extension/bin/adapter")
                || adapter.command.ends_with("ext-1\\extension\\bin\\adapter")
        );
    }

    #[test]
    fn an_explicit_adapter_in_launch_json_wins() {
        let dir = crate::test_support::TempDir::new();
        let app = app_with_debugger(&dir, true, Some("bin/adapter"));
        let mut config: LaunchConfig = serde_json::from_value(serde_json::json!({
            "name": "x", "type": "demo", "request": "launch",
            "adapter": {"command": "my-adapter"}
        }))
        .unwrap();
        app.resolve_debug_adapter(&mut config).unwrap();
        assert_eq!(config.adapter.unwrap().command, PathBuf::from("my-adapter"));
    }

    #[test]
    fn unknown_types_and_disabled_providers_get_distinct_messages() {
        let dir = crate::test_support::TempDir::new();
        let app = app_with_debugger(&dir, true, Some("bin/adapter"));
        let error = app.resolve_debug_adapter(&mut launch("other")).unwrap_err();
        assert!(error.contains("No installed extension provides a debugger for type 'other'"));

        let disabled = app_with_debugger(&dir, false, Some("bin/adapter"));
        let error = disabled
            .resolve_debug_adapter(&mut launch("demo"))
            .unwrap_err();
        assert!(
            error.contains("pub.demo") && error.contains("disabled"),
            "{error}"
        );
    }

    #[test]
    fn without_a_plugin_store_the_type_cannot_be_resolved() {
        let dir = crate::test_support::TempDir::new();
        let app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(app
            .resolve_debug_adapter(&mut launch("demo"))
            .unwrap_err()
            .contains("No installed extension"));
    }

    #[test]
    fn a_server_chosen_at_launch_counts_as_resolved() {
        let dir = crate::test_support::TempDir::new();
        let app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let mut config = launch("demo");
        config.server = Some(AdapterServer {
            host: "127.0.0.1".into(),
            port: 1,
        });
        assert_eq!(
            app.resolve_debug_adapter(&mut config).unwrap(),
            AdapterSource::Ready
        );
    }

    fn set_main(app: &mut App, main: Option<&str>) {
        if let Metadata::Vsix(vsix) = &mut app.plugins.installed[0].metadata {
            vsix.main = main.map(str::to_owned);
        }
    }

    /// Approves the installed extension, so it may run code in the extension host.
    fn approve(app: &mut App) {
        let version = app.plugins.installed[0].metadata.version().to_owned();
        app.plugins.installed[0].trusted_version = Some(version);
    }

    #[test]
    fn a_debugger_without_a_program_is_created_by_the_extension_code() {
        let dir = crate::test_support::TempDir::new();
        let mut app = app_with_debugger(&dir, true, None);
        set_main(&mut app, Some("extension.js"));
        approve(&mut app);
        let mut config = launch("demo");
        assert_eq!(
            app.resolve_debug_adapter(&mut config).unwrap(),
            AdapterSource::Extension("pub.demo".into())
        );
        assert!(config.adapter.is_none());
    }

    #[test]
    fn an_unapproved_extension_cannot_create_an_adapter_in_code() {
        let dir = crate::test_support::TempDir::new();
        let mut app = app_with_debugger(&dir, true, None);
        set_main(&mut app, Some("extension.js"));
        let error = app.resolve_debug_adapter(&mut launch("demo")).unwrap_err();
        assert!(
            error.contains("creates its adapter in code") && error.contains("approve"),
            "{error}"
        );
    }

    #[test]
    fn an_approved_extension_with_a_program_still_resolves_its_configuration_in_the_host() {
        let dir = crate::test_support::TempDir::new();
        let mut app = app_with_debugger(&dir, true, Some("bin/adapter"));
        set_main(&mut app, Some("extension.js"));
        approve(&mut app);
        let mut config = launch("demo");
        assert_eq!(
            app.resolve_debug_adapter(&mut config).unwrap(),
            AdapterSource::Extension("pub.demo".into())
        );
        assert!(
            config.adapter.is_some(),
            "the program is kept as the adapter unless the host returns another"
        );
    }

    #[test]
    fn an_unapproved_extension_with_a_program_still_starts_without_its_providers() {
        let dir = crate::test_support::TempDir::new();
        let mut app = app_with_debugger(&dir, true, Some("bin/adapter"));
        set_main(&mut app, Some("extension.js"));
        let mut config = launch("demo");
        assert_eq!(
            app.resolve_debug_adapter(&mut config).unwrap(),
            AdapterSource::Ready
        );
        assert!(config.adapter.is_some());
    }

    #[test]
    fn a_debugger_without_a_program_or_code_is_an_error() {
        let dir = crate::test_support::TempDir::new();
        let app = app_with_debugger(&dir, true, None);
        let error = app.resolve_debug_adapter(&mut launch("demo")).unwrap_err();
        assert!(
            error.contains("no program") && error.contains("pub.demo"),
            "{error}"
        );
    }
}
