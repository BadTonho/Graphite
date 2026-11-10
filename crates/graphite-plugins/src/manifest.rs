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
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub title: String,
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
