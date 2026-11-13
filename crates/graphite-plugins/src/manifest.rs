use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub api_version: u32,
    pub main: String,
    pub description: String,
    #[serde(default)]
    pub commands: Vec<Command>,
    #[serde(default)]
    pub activity_bar: Vec<ActivityBarContainer>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityBarContainer {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub views: Vec<ActivityBarView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityBarView {
    pub id: String,
    pub title: String,
    #[serde(default = "default_view_kind")]
    pub kind: String,
}

fn default_view_kind() -> String {
    "tree".into()
}
pub fn valid_id(id: &str) -> bool {
    let parts: Vec<_> = id.split('.').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
}
impl Manifest {
    pub fn validate(&self) -> Result<()> {
        if !valid_id(&self.id) {
            return Err(
                "Plugin ID must be publisher.name (lowercase letters, digits, hyphens)".into(),
            );
        }
        semver::Version::parse(&self.version).map_err(|e| e.to_string())?;
        if self.api_version != 1 {
            return Err("Unsupported Graphite API version".into());
        }
        if !crate::package::safe_path(&self.main)
            || ![".js", ".cjs", ".mjs"]
                .iter()
                .any(|s| self.main.ends_with(s))
        {
            return Err("Plugin main must be a relative JavaScript file".into());
        }
        let mut ids = std::collections::HashSet::new();
        for command in &self.commands {
            if !command.id.starts_with(&format!("{}.", self.id))
                || command.title.trim().is_empty()
                || !ids.insert(&command.id)
            {
                return Err(
                    "Commands need unique IDs prefixed by the plugin ID and nonempty titles".into(),
                );
            }
        }
        for container in &self.activity_bar {
            if !container.id.starts_with(&format!("{}.", self.id))
                || container.title.trim().is_empty()
                || !ids.insert(&container.id)
            {
                return Err(
                    "Activity bar containers need unique plugin-prefixed IDs and nonempty titles"
                        .into(),
                );
            }
            for view in &container.views {
                if !view.id.starts_with(&format!("{}.", self.id))
                    || view.title.trim().is_empty()
                    || !["tree", "webview"].contains(&view.kind.as_str())
                    || !ids.insert(&view.id)
                {
                    return Err("Activity bar views need unique plugin-prefixed IDs, titles and valid kinds".into());
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "format", content = "manifest", rename_all = "camelCase")]
pub enum Metadata {
    Native(Manifest),
    Vsix(Vsix),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vsix {
    pub id: String,
    pub version: String,
    pub description: String,
    pub engines: Value,
    pub main: Option<String>,
    pub contributions: Vec<String>,
    #[serde(default)]
    pub commands: Vec<Command>,
    #[serde(default)]
    pub activity_bar: Vec<ActivityBarContainer>,
    #[serde(default)]
    pub language_comments: Vec<LanguageComments>,
    /// Snippet files the extension contributes (`contributes.snippets`).
    #[serde(default)]
    pub snippets: Vec<SnippetContribution>,
    /// Debug adapters the extension contributes (`contributes.debuggers`).
    #[serde(default)]
    pub debuggers: Vec<DebuggerContribution>,
}

/// How to start a debug adapter: `runtime` (if any) runs `program`, a path inside the extension.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DebuggerExecutable {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

/// One entry of `contributes.debuggers`: the debug `type` it serves and how to launch its adapter,
/// with optional per-platform overrides.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DebuggerContribution {
    #[serde(rename = "type")]
    pub debug_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub executable: DebuggerExecutable,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<DebuggerExecutable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<DebuggerExecutable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osx: Option<DebuggerExecutable>,
}

impl DebuggerContribution {
    /// The executable for `platform` (`"windows"`, `"linux"` or `"osx"`): the platform's fields
    /// replace the shared ones, field by field.
    pub fn executable_for(&self, platform: &str) -> DebuggerExecutable {
        let specific = match platform {
            "windows" => &self.windows,
            "linux" => &self.linux,
            "osx" => &self.osx,
            _ => &None,
        };
        let base = &self.executable;
        let Some(over) = specific else {
            return base.clone();
        };
        DebuggerExecutable {
            program: over.program.clone().or_else(|| base.program.clone()),
            runtime: over.runtime.clone().or_else(|| base.runtime.clone()),
            runtime_args: over
                .runtime_args
                .clone()
                .or_else(|| base.runtime_args.clone()),
            args: over.args.clone().or_else(|| base.args.clone()),
        }
    }
}

/// The platform key VS Code uses in `contributes.debuggers` overrides.
pub fn debugger_platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Reads `contributes.debuggers`. Entries without a `type`, and `program` paths that leave the
/// extension directory, are dropped.
pub(crate) fn vsix_debuggers(manifest: &Value) -> Vec<DebuggerContribution> {
    fn safe(executable: &mut DebuggerExecutable) -> bool {
        if let Some(program) = executable.program.as_deref() {
            let relative = program.strip_prefix("./").unwrap_or(program);
            if !crate::package::safe_path(relative) {
                return false;
            }
            executable.program = Some(relative.to_owned());
        }
        true
    }
    manifest
        .pointer("/contributes/debuggers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let mut debugger: DebuggerContribution = serde_json::from_value(entry.clone()).ok()?;
            if debugger.debug_type.trim().is_empty() || !safe(&mut debugger.executable) {
                return None;
            }
            for platform in [
                &mut debugger.windows,
                &mut debugger.linux,
                &mut debugger.osx,
            ] {
                if let Some(over) = platform {
                    if !safe(over) {
                        *platform = None;
                    }
                }
            }
            Some(debugger)
        })
        .collect()
}

/// A snippet file inside the extension for one language.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnippetContribution {
    pub language: String,
    /// Path relative to the extension root, validated by [`crate::package::safe_path`].
    pub path: String,
}

pub(crate) fn vsix_snippets(manifest: &Value) -> Vec<SnippetContribution> {
    manifest
        .pointer("/contributes/snippets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let language = entry.get("language")?.as_str()?.trim();
            let path = entry.get("path")?.as_str()?;
            let path = path.strip_prefix("./").unwrap_or(path);
            (!language.is_empty() && crate::package::safe_path(path)).then(|| SnippetContribution {
                language: language.to_owned(),
                path: path.to_owned(),
            })
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageComments {
    pub id: String,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub line_comment: Option<String>,
    #[serde(default)]
    pub block_comment: Option<[String; 2]>,
}

pub(crate) fn vsix_language_comments(
    manifest: &Value,
    mut read_configuration: impl FnMut(&str) -> Option<Value>,
) -> Vec<LanguageComments> {
    let mut result = Vec::new();
    for language in manifest
        .pointer("/contributes/languages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = language.get("id").and_then(Value::as_str) else {
            continue;
        };
        let configuration = language
            .get("configuration")
            .and_then(Value::as_str)
            .map(|path| path.strip_prefix("./").unwrap_or(path))
            .filter(|path| crate::package::safe_path(path))
            .and_then(&mut read_configuration);
        let comments = configuration
            .as_ref()
            .and_then(|configuration| configuration.get("comments"));
        let line_comment = comments
            .and_then(|comments| comments.get("lineComment"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let block_comment = comments
            .and_then(|comments| comments.get("blockComment"))
            .and_then(Value::as_array)
            .filter(|markers| markers.len() == 2)
            .and_then(|markers| {
                Some([
                    markers[0].as_str()?.to_owned(),
                    markers[1].as_str()?.to_owned(),
                ])
            })
            .filter(|markers| !markers[0].is_empty() && !markers[1].is_empty());
        if line_comment.is_none() && block_comment.is_none() {
            continue;
        }
        let extensions = language
            .get("extensions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
            .filter(|extension| !extension.is_empty())
            .collect();
        result.push(LanguageComments {
            id: id.to_owned(),
            extensions,
            line_comment,
            block_comment,
        });
    }
    result
}

pub(crate) fn vsix_activity_bar(manifest: &Value) -> Vec<ActivityBarContainer> {
    let containers = manifest
        .pointer("/contributes/viewsContainers/activitybar")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    containers
        .into_iter()
        .filter_map(|container| {
            let id = container.get("id")?.as_str()?.trim();
            let title = container.get("title")?.as_str()?.trim();
            if id.is_empty() || title.is_empty() {
                return None;
            }
            let views = manifest
                .pointer(&format!(
                    "/contributes/views/{}",
                    id.replace('~', "~0").replace('/', "~1")
                ))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|view| {
                    let view_id = view.get("id")?.as_str()?.trim();
                    let view_title = view.get("name")?.as_str()?.trim();
                    (!view_id.is_empty() && !view_title.is_empty()).then(|| ActivityBarView {
                        id: view_id.into(),
                        title: view_title.into(),
                        kind: "tree".into(),
                    })
                })
                .collect();
            Some(ActivityBarContainer {
                id: id.into(),
                title: title.into(),
                icon: container
                    .get("icon")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                views,
            })
        })
        .collect()
}

pub(crate) fn vsix_commands(manifest: &Value) -> Vec<Command> {
    let mut ids = std::collections::HashSet::new();
    manifest
        .pointer("/contributes/commands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry.get("command")?.as_str()?;
            let title = entry.get("title")?;
            let title = title.as_str().or_else(|| title.get("value")?.as_str())?;
            if id.trim().is_empty() || title.trim().is_empty() || !ids.insert(id) {
                return None;
            }
            let title = match entry.get("category").and_then(Value::as_str) {
                Some(category) if !category.is_empty() => format!("{category}: {title}"),
                _ => title.to_owned(),
            };
            Some(Command {
                id: id.into(),
                title,
            })
        })
        .collect()
}
impl Metadata {
    pub fn id(&self) -> &str {
        match self {
            Self::Native(m) => &m.id,
            Self::Vsix(m) => &m.id,
        }
    }
    pub fn version(&self) -> &str {
        match self {
            Self::Native(m) => &m.version,
            Self::Vsix(m) => &m.version,
        }
    }
}

#[cfg(test)]
mod activity_bar_tests {
    use super::*;

    #[test]
    fn vsix_activity_bar_keeps_only_valid_containers_and_views() {
        let manifest = serde_json::json!({
            "contributes": {
                "viewsContainers": {"activitybar": [
                    {"id":"sample.tools","title":"Tools","icon":"media/tools.svg"},
                    {"id":"","title":"Invalid"}
                ]},
                "views": {"sample.tools": [
                    {"id":"sample.tree","name":"Tree"},
                    {"id":"sample.bad","name":""}
                ]}
            }
        });
        let containers = vsix_activity_bar(&manifest);
        assert_eq!(containers.len(), 1);
        assert_eq!(containers[0].id, "sample.tools");
        assert_eq!(containers[0].title, "Tools");
        assert_eq!(containers[0].icon.as_deref(), Some("media/tools.svg"));
        assert_eq!(containers[0].views.len(), 1);
        assert_eq!(containers[0].views[0].id, "sample.tree");
    }

    #[test]
    fn native_activity_bar_ids_are_scoped_and_kinds_are_checked() {
        let mut manifest: Manifest = serde_json::from_value(serde_json::json!({
            "id":"sample.plugin", "version":"1.0.0", "apiVersion":1,
            "main":"main.js", "description":"",
            "activityBar":[{"id":"sample.plugin.tools","title":"Tools","views":[
                {"id":"sample.plugin.tree","title":"Tree","kind":"tree"},
                {"id":"sample.plugin.panel","title":"Panel","kind":"webview"}
            ]}]
        }))
        .unwrap();
        assert!(manifest.validate().is_ok());
        manifest.activity_bar[0].views[1].kind = "unknown".into();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn vsix_language_comments_read_safe_configuration_and_ignore_invalid_markers() {
        let manifest = serde_json::json!({
            "contributes": {"languages": [
                {"id":"sample","extensions":[".smp"],"configuration":"./language.json"},
                {"id":"unsafe","extensions":[".bad"],"configuration":"../escape.json"}
            ]}
        });
        let comments = vsix_language_comments(&manifest, |path| {
            (path == "language.json").then(|| {
                serde_json::json!({
                    "comments":{"lineComment":"#","blockComment":["/*","*/"]}
                })
            })
        });
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].id, "sample");
        assert_eq!(comments[0].extensions, ["smp"]);
        assert_eq!(comments[0].line_comment.as_deref(), Some("#"));
        assert_eq!(comments[0].block_comment.as_ref().unwrap(), &["/*", "*/"]);
    }

    #[test]
    fn vsix_snippets_keep_only_safe_relative_paths_with_a_language() {
        let manifest = serde_json::json!({"contributes": {"snippets": [
            {"language": "rust", "path": "./snippets/rust.json"},
            {"language": "go", "path": "snippets/go.json"},
            {"language": "evil", "path": "../outside.json"},
            {"language": "abs", "path": "/etc/passwd"},
            {"language": "", "path": "snippets/none.json"},
            {"path": "snippets/no-language.json"},
        ]}});
        let snippets = vsix_snippets(&manifest);
        assert_eq!(
            snippets,
            vec![
                SnippetContribution {
                    language: "rust".into(),
                    path: "snippets/rust.json".into()
                },
                SnippetContribution {
                    language: "go".into(),
                    path: "snippets/go.json".into()
                },
            ]
        );
        assert!(vsix_snippets(&serde_json::json!({})).is_empty());
    }

    fn debugger_manifest() -> Value {
        serde_json::json!({"contributes": {"debuggers": [
            {
                "type": "node", "label": "Node Debug", "runtime": "node",
                "program": "./out/adapter.js", "args": ["--serve"],
                "runtimeArgs": ["--no-warnings"],
                "windows": {"program": "./out/adapter-win.js"},
                "osx": {"runtime": "mono", "args": []}
            },
            {"type": "native", "program": "bin/adapter"},
            {"type": "", "program": "bin/blank"},
            {"program": "bin/no-type"},
            {"type": "escape", "program": "../outside"},
            {"type": "absolute", "program": "/usr/bin/adapter"},
            {"type": "partial", "program": "bin/ok", "linux": {"program": "../bad"}}
        ]}})
    }

    #[test]
    fn vsix_debuggers_keep_safe_entries_with_a_type() {
        let debuggers = vsix_debuggers(&debugger_manifest());
        let types: Vec<_> = debuggers.iter().map(|d| d.debug_type.as_str()).collect();
        assert_eq!(types, ["node", "native", "partial"]);
        let node = &debuggers[0];
        assert_eq!(node.label.as_deref(), Some("Node Debug"));
        assert_eq!(node.executable.program.as_deref(), Some("out/adapter.js"));
        assert_eq!(node.executable.runtime.as_deref(), Some("node"));
        assert_eq!(node.executable.args, Some(vec!["--serve".into()]));
        assert_eq!(
            node.executable.runtime_args,
            Some(vec!["--no-warnings".into()])
        );
        assert_eq!(
            debuggers[2].linux, None,
            "an unsafe platform override is dropped, the shared program stays"
        );
        assert_eq!(debuggers[2].executable.program.as_deref(), Some("bin/ok"));
        assert!(vsix_debuggers(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn platform_overrides_replace_shared_fields_one_by_one() {
        let debuggers = vsix_debuggers(&debugger_manifest());
        let node = &debuggers[0];
        let windows = node.executable_for("windows");
        assert_eq!(windows.program.as_deref(), Some("out/adapter-win.js"));
        assert_eq!(windows.runtime.as_deref(), Some("node"), "not overridden");
        assert_eq!(windows.args, Some(vec!["--serve".into()]));
        let osx = node.executable_for("osx");
        assert_eq!(osx.runtime.as_deref(), Some("mono"));
        assert_eq!(
            osx.args,
            Some(Vec::new()),
            "an empty override still replaces"
        );
        assert_eq!(osx.program.as_deref(), Some("out/adapter.js"));
        assert_eq!(node.executable_for("linux"), node.executable);
        assert_eq!(node.executable_for("plan9"), node.executable);
    }

    #[test]
    fn records_saved_before_debuggers_existed_load_with_none() {
        let old = serde_json::json!({
            "id": "a.b", "version": "1.0.0", "description": "", "engines": {},
            "main": null, "contributions": ["debuggers"]
        });
        let vsix: Vsix = serde_json::from_value(old).unwrap();
        assert!(vsix.debuggers.is_empty());
        let round_trip: Vsix =
            serde_json::from_value(serde_json::to_value(&vsix).unwrap()).unwrap();
        assert!(round_trip.debuggers.is_empty());
    }

    #[test]
    fn debugger_contributions_survive_serialization() {
        let debuggers = vsix_debuggers(&debugger_manifest());
        let text = serde_json::to_string(&debuggers).unwrap();
        let back: Vec<DebuggerContribution> = serde_json::from_str(&text).unwrap();
        assert_eq!(back, debuggers);
    }
}
