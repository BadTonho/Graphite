use crate::{DocumentSnapshot, Position};
use serde::{Deserialize, Serialize};

/// A selection range in an editor expressed as UTF-16 anchor/active positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorSelection {
    pub anchor: Position,
    pub active: Position,
}

/// Snapshot of the active text editor returned to plugins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveEditorSnapshot {
    pub document: DocumentSnapshot,
    pub selections: Vec<EditorSelection>,
}

/// Range for an editor decoration applied by a plugin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecorationRange {
    pub start: Position,
    pub end: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hover_message: Option<String>,
}

/// Severity level for diagnostics published by a plugin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    #[default]
    Error,
    Warning,
    Information,
    Hint,
}

/// A single diagnostic reported for a document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEntry {
    pub start: Position,
    pub end: Position,
    pub message: String,
    #[serde(default)]
    pub severity: DiagnosticSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Language intelligence provider category registered by a native plugin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LanguageProviderKind {
    Completion,
    Hover,
    Formatter,
}

/// Completion item returned by a native plugin completion provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionEntry {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "insert_text"
    )]
    pub insert_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}
