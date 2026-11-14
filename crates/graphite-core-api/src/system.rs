use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One changed path in source control status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScmFileStatus {
    pub path: String,
    pub status: String,
}

/// Source control summary returned to native plugins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScmStatusSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default)]
    pub files: Vec<ScmFileStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Tool definition exposed by a native plugin to Graphite Chat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatToolDefinition {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

/// Result returned by a plugin chat tool execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatToolOutcome {
    pub content: String,
    #[serde(rename = "isError", alias = "is_error", default)]
    pub is_error: bool,
}
