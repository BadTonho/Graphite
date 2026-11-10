use super::*;
pub(super) mod accessibility;
mod accessibility_actions;
pub(crate) mod activity_bar;
pub(crate) mod cli_ui;
mod clipboard;
pub(crate) mod command_execution;
mod command_handlers;
pub(crate) mod command_palette;
pub(crate) mod command_registry;
mod commands;
pub(super) mod context_keys;
mod document;
mod drawing;
pub(super) mod drawing_helpers;
mod editor_commands;
#[allow(dead_code)]
pub(super) mod editor_groups;
pub(crate) mod emmet;
mod group_drawing;
mod i18n;
mod ime;
mod import_settings;
mod input;
mod layout;
pub(super) mod local_search;
mod lsp_ui;
pub(crate) mod menu_bar;
mod mouse;
mod phase4_tools;
#[path = "../phase5.rs"]
pub(crate) mod phase5;
pub(crate) mod plugins;
mod project_search;
pub(super) mod quick_open;
pub(crate) mod scm_ui;
mod smart_selection;
mod tasks;
mod terminal_drawing;
mod terminal_input;
mod terminal_ui;
pub(super) mod theme;
mod visual_lines;
mod window_events;
mod workspace_edit;
mod wrap_drawing;

pub(crate) mod debugging;
mod document_edit;
mod phase6_drawing;
mod ports_ui;
mod previews_ui;

#[cfg(test)]
mod document_view_tests;

#[cfg(test)]
mod command_tests;

pub(crate) mod extension_host;
pub(crate) mod services;

pub(crate) mod saving;
