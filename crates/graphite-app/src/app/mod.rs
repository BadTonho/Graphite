use super::*;
pub(super) mod accessibility;
mod accessibility_actions;
pub(crate) mod activity_bar;
mod autosave;
pub(crate) mod cli_ui;
mod clipboard;
pub(crate) mod command_center;
pub(crate) mod command_execution;
mod command_handlers;
pub(crate) mod command_palette;
pub(crate) mod command_registry;
mod commands;
mod comments;
pub(super) mod context_keys;
mod custom_editors;
mod decoration_drawing;
mod document;
mod drawing;
pub(super) mod drawing_helpers;
pub(crate) mod edit_menu;
mod editor_commands;
#[allow(dead_code)]
pub(super) mod editor_groups;
pub(crate) mod emmet;
pub(super) mod explorer_files;
mod extension_snippets;
mod extension_themes;
mod external_changes;
pub(crate) mod file_menu;
mod focus_regions;
mod fold_commands;
pub(crate) mod folding;
pub(crate) mod github_auth;
pub(crate) mod github_dialog;
pub(crate) mod go_menu;
mod group_drawing;
pub(crate) mod help_menu;
mod i18n;
mod ime;
mod import_settings;
mod input;
mod layout;
mod local_history;
pub(super) mod local_search;
pub(crate) mod lsp_features;
mod lsp_ui;
pub(crate) mod menu_bar;
mod mouse;
mod phase4_tools;
#[path = "../phase5.rs"]
pub(crate) mod phase5;
pub(crate) mod plugins;
mod preview_tabs;
pub(crate) mod profile_menu;
mod project_search;
pub(super) mod project_search_filters;
pub(super) mod quick_open;
pub(crate) mod run_menu;
pub(crate) mod scm_branch_ui;
pub(crate) mod scm_remote_ui;
pub(crate) mod scm_ui;
pub(crate) mod selection_menu;
pub(crate) mod settings_menu;
mod smart_selection;
mod tasks;
mod terminal_drawing;
mod terminal_input;
pub(crate) mod terminal_menu;
mod terminal_navigation;
mod terminal_ui;
pub(super) mod theme;
pub(crate) mod titlebar_layout_controls;
pub(crate) mod view_menu;
mod visual_lines;
mod window_events;
mod workspace_edit;
mod wrap_drawing;

mod debug_adapters;
pub(crate) mod debug_inline;
mod debug_panel;
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
