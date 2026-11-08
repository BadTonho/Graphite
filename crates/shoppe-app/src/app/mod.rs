use super::*;
pub(super) mod accessibility;
mod accessibility_actions;
pub(crate) mod cli_ui;
mod clipboard;
mod commands;
pub(super) mod context_keys;
mod document;
mod drawing;
pub(super) mod drawing_helpers;
mod editor_commands;
#[allow(dead_code)]
pub(super) mod editor_groups;
mod group_drawing;
mod ime;
mod import_settings;
mod input;
mod layout;
pub(super) mod local_search;
mod lsp_ui;
mod mouse;
mod phase4_tools;
mod project_search;
pub(super) mod quick_open;
pub(crate) mod scm_ui;
mod smart_selection;
mod terminal_drawing;
mod terminal_input;
mod terminal_ui;
pub(super) mod theme;
mod visual_lines;
mod window_events;
mod workspace_edit;
mod wrap_drawing;

mod document_edit;

#[cfg(test)]
mod document_view_tests;
