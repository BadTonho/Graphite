//! Finds the debug adapter for a launch configuration. A `launch.json` normally names only the
//! debug `type`; the adapter comes from the installed extension that contributes that type
//! (`contributes.debuggers`). An explicit `"adapter"` in the configuration still wins.

use super::*;
use crate::debugger::{AdapterCommand, LaunchConfig};
use graphite_plugins::manifest::{debugger_platform, DebuggerContribution, Metadata};
use std::path::Path;

/// The command that starts `contribution`'s adapter, for `platform`.
///
/// `program` is a path inside `extension_dir`. A `runtime` runs it (`node` uses the runtime that
/// ships with Graphite when there is one); without a runtime the program is executed directly.
pub(crate) fn adapter_command(
    contribution: &DebuggerContribution,
    extension_dir: &Path,
    platform: &str,
    bundled_node: Option<&Path>,
) -> Result<AdapterCommand, String> {
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
            Ok(AdapterCommand { command, args: all })
        }
        (None, Some(program)) => Ok(AdapterCommand {
            command: program,
            args,
        }),
        (None, None) => Err(format!(
            "The '{}' debugger declares no program; extensions that create their adapter in code \
             are not supported yet",
            contribution.debug_type
        )),
    }
}

impl App {
    /// Fills in `config.adapter` from the extension that provides its debug `type`.
    pub(crate) fn resolve_debug_adapter(&self, config: &mut LaunchConfig) -> Result<(), String> {
        if config.adapter.is_some() {
            return Ok(());
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
            config.adapter = Some(adapter_command(
                contribution,
                &extension_dir,
                debugger_platform(),
                bundled_node.as_deref(),
            )?);
            return Ok(());
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
        let adapter = adapter_command(
            &contribution(DebuggerExecutable {
                program: Some("bin/adapter".into()),
                args: Some(vec!["--stdio".into()]),
                ..Default::default()
            }),
            dir.path(),
            "linux",
            None,
        )
        .unwrap();
        assert_eq!(adapter.command, dir.path().join("bin/adapter"));
        assert_eq!(adapter.args, ["--stdio"]);
    }

    #[test]
    fn a_runtime_runs_the_program_after_its_own_arguments() {
        let dir = extension_with("out/adapter.js");
        let adapter = adapter_command(
            &contribution(DebuggerExecutable {
                program: Some("out/adapter.js".into()),
                runtime: Some("python".into()),
                runtime_args: Some(vec!["-u".into()]),
                args: Some(vec!["--port".into(), "0".into()]),
            }),
            dir.path(),
            "linux",
            None,
        )
        .unwrap();
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
        let adapter = adapter_command(
            &contribution(executable.clone()),
            dir.path(),
            "linux",
            Some(&bundled),
        )
        .unwrap();
        assert_eq!(adapter.command, bundled);
        let adapter =
            adapter_command(&contribution(executable), dir.path(), "linux", None).unwrap();
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
        let adapter = adapter_command(&debugger, dir.path(), "windows", None).unwrap();
        assert_eq!(adapter.command, dir.path().join("bin/adapter.exe"));
        // On another platform the shared program is used, and it is not there.
        assert!(adapter_command(&debugger, dir.path(), "linux", None)
            .unwrap_err()
            .contains("is missing from the extension"));
    }

    #[test]
    fn an_adapter_created_in_code_is_reported_as_unsupported() {
        let dir = crate::test_support::TempDir::new();
        let error = adapter_command(
            &contribution(DebuggerExecutable::default()),
            dir.path(),
            "linux",
            None,
        )
        .unwrap_err();
        assert!(
            error.contains("'demo'") && error.contains("not supported yet"),
            "{error}"
        );
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
            }),
            directory,
            enabled,
            trusted_version: None,
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
        app.resolve_debug_adapter(&mut config).unwrap();
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
}
