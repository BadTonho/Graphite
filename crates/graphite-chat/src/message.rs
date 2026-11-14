//! Provider-neutral conversation model.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// One piece of a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text {
        text: String,
    },
    /// The model asks for a tool to run. `input` is the parsed JSON arguments.
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// The outcome of a tool run, sent back as a user message.
    ToolResult {
        id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub blocks: Vec<Block>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            blocks: vec![Block::Text { text: text.into() }],
        }
    }

    pub fn assistant(blocks: Vec<Block>) -> Self {
        Self {
            role: Role::Assistant,
            blocks,
        }
    }

    pub fn tool_results(results: Vec<Block>) -> Self {
        Self {
            role: Role::User,
            blocks: results,
        }
    }

    /// All the plain text of the message, joined.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                Block::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn tool_uses(&self) -> impl Iterator<Item = (&str, &str, &Value)> {
        self.blocks.iter().filter_map(|block| match block {
            Block::ToolUse { id, name, input } => Some((id.as_str(), name.as_str(), input)),
            _ => None,
        })
    }
}

/// A tool the model may call. `schema` is a JSON Schema object for the arguments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

/// Everything one model call needs.
#[derive(Debug, Clone, Default)]
pub struct Request {
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The model finished its answer.
    EndTurn,
    /// The model wants tool results before it continues.
    ToolUse,
    /// The reply hit the token limit.
    MaxTokens,
    Other,
}

/// What a streaming call reports while it runs.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    Text(String),
    /// A complete tool call, reported once its arguments have fully arrived.
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    Finished {
        stop: StopReason,
        usage: Usage,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatError {
    /// No provider is configured or the key is missing.
    NotConfigured(String),
    /// The server answered with an error status.
    Http {
        status: u16,
        message: String,
    },
    /// The connection failed or dropped.
    Network(String),
    /// The server sent something this client cannot read.
    Protocol(String),
    Cancelled,
}

impl std::fmt::Display for ChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured(text) | Self::Network(text) | Self::Protocol(text) => {
                f.write_str(text)
            }
            Self::Http { status, message } => write!(f, "{message} (HTTP {status})"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for ChatError {}
