//! Event-loop implementations. External adapters only receive service contracts.
use super::*;
use graphite_core_api::*;
mod commands;
mod documents;
pub(super) mod lsp;
mod window;
mod workspace;
pub(super) use documents::snapshot_document;
pub(super) fn edit_error(error: String) -> ServiceError {
    if error.contains("version") || error.contains("changed") {
        ServiceError::StaleDocument
    } else if error.contains("read-only")
        || error.contains("writable")
        || error.contains("ReadOnly")
    {
        ServiceError::ReadOnly
    } else if error.contains("range") || error.contains("overlap") {
        ServiceError::InvalidRange
    } else {
        ServiceError::Failed(error)
    }
}

#[cfg(test)]
mod tests;

mod events;
pub(crate) use events::DocumentEventHub;
