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
}
