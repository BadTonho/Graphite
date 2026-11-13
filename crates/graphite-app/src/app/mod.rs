use super::*;

// Feature areas. Each group re-exports the modules other code reaches via `app::<name>`.
mod window;
pub(crate) use window::{accessibility, activity_bar, clipboard, context_keys, focus_regions, layout, titlebar_layout_controls, theme};
mod menus;
pub(crate) use menus::{menu_bar, file_menu, edit_menu, selection_menu, view_menu, go_menu, run_menu, terminal_menu, help_menu, settings_menu, profile_menu};
mod command_system;
pub(crate) use command_system::{command_center, command_execution, command_palette, command_registry, command_handlers, cli_ui};
mod editing;
pub(crate) use editing::{editor_groups, change_navigation, folding, visual_lines, drawing_helpers, group_drawing, emmet, local_history, outline, sticky_scroll};
mod navigation;
pub(crate) use navigation::{explorer_files, explorer_nav, quick_open, local_search, project_search_filters};
mod source_control;
pub(crate) use source_control::{scm_ui, scm_branch_ui, scm_remote_ui, merge_editor, github_auth, github_dialog};
mod debug_ui;
pub(crate) use debug_ui::{debug_adapters, debug_inline, debug_panel, debugging};
mod terminal_panel;
mod notebooks;
pub(crate) use notebooks::{notebook_kernel, notebook_ui};
mod language_ui;
pub(crate) use language_ui::lsp_features;
mod utilities;
pub(crate) use utilities::{phase4_tools, phase5, phase6_drawing};

pub(crate) mod plugins;

pub(crate) mod extension_host;
pub(crate) mod services;

pub(crate) mod saving;
