//! Model service access for Graphite's chat.
//!
//! One provider-neutral conversation model ([`Message`], [`Request`], [`StreamEvent`]) is sent
//! to either the Anthropic Messages API or any OpenAI-compatible server, and the streamed reply
//! comes back as the same events. The crate does no UI and runs no tools: the app owns what the
//! model may read or change.

pub mod anthropic;
mod message;
pub mod openai;
mod provider;
mod sse;
mod store;

pub use message::{Block, ChatError, Message, Request, Role, StopReason, StreamEvent, ToolSpec, Usage};
pub use provider::{stream, ProviderConfig, ProviderKind};
pub use sse::{SseEvent, SseParser};
pub use store::{delete_provider, load_provider, save_provider, SETTINGS_FILE};
