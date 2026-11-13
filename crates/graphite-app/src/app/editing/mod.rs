//! Editor views, groups, folding, drawing and document editing.

#[allow(unused_imports)]
use super::*;

pub(crate) mod document;
pub(crate) mod document_edit;
#[cfg(test)]
pub(crate) mod document_view_tests;
pub(crate) mod editor_commands;
#[allow(dead_code)]
pub(crate) mod editor_groups;
pub(crate) mod editor_history_picker;
pub(crate) mod change_navigation;
pub(crate) mod fold_actions;
pub(crate) mod fold_commands;
pub(crate) mod folding;
pub(crate) mod smart_selection;
pub(crate) mod visual_lines;
pub(crate) mod wrap_drawing;
pub(crate) mod drawing;
pub(crate) mod drawing_helpers;
pub(crate) mod decoration_drawing;
pub(crate) mod group_drawing;
pub(crate) mod emmet;
pub(crate) mod local_history;
pub(crate) mod external_changes;
pub(crate) mod custom_editors;
pub(crate) mod workspace_edit;
pub(crate) mod preview_tabs;
pub(crate) mod outline;
pub(crate) mod timeline;
pub(crate) mod comments;
pub(crate) mod autosave;
