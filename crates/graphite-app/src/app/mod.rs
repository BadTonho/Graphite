use super::*;

// Feature areas. Each group re-exports the modules other code reaches via `app::<name>`.
mod window;
pub(crate) use window::{
    accessibility, activity_bar, clipboard, context_keys, focus_regions, layout, theme,
    titlebar_layout_controls,
};
mod menus;
pub(crate) use menus::{
    edit_menu, file_menu, go_menu, help_menu, menu_bar, profile_menu, run_menu, selection_menu,
    settings_menu, terminal_menu, view_menu,
};
mod command_system;
pub(crate) use command_system::{
    cli_ui, command_center, command_execution, command_handlers, command_palette, command_registry,
};
mod editing;
pub(crate) use editing::{
    change_navigation, drawing_helpers, editor_groups, emmet, folding, group_drawing,
    local_history, outline, sticky_scroll, visual_lines,
};
mod navigation;
pub(crate) use navigation::{
    explorer_files, explorer_nav, local_search, project_search_filters, quick_open,
};
mod source_control;
pub(crate) use source_control::{
    github_auth, github_dialog, merge_editor, scm_branch_ui, scm_remote_ui, scm_ui,
};
mod debug_ui;
pub(crate) use debug_ui::{debug_adapters, debug_inline, debug_panel, debugging};
mod notebooks;
mod terminal_panel;
pub(crate) use notebooks::{notebook_kernel, notebook_ui};
mod language_ui;
pub(crate) use language_ui::lsp_features;
mod utilities;
pub(crate) use utilities::{phase4_tools, phase5, phase6_drawing};

pub(crate) mod chat;
pub(crate) mod plugins;
pub(crate) mod remote;
pub(crate) mod user_data_sync;

pub(crate) mod extension_host;
pub(crate) mod services;

pub(crate) mod saving;
