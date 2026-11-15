//! Source control, merge editor, multi-diff and GitHub UI.

#[allow(unused_imports)]
use super::*;

pub(crate) mod extension_scm_ui;
pub(crate) mod github_auth;
pub(crate) mod github_dialog;
pub(crate) mod merge_actions;
#[cfg(test)]
pub(crate) mod merge_actions_tests;
pub(crate) mod merge_drawing;
pub(crate) mod merge_editor;
#[cfg(test)]
pub(crate) mod merge_entry_tests;
pub(crate) mod merge_flow;
#[cfg(test)]
pub(crate) mod merge_flow_tests;
pub(crate) mod merge_resolution;
#[cfg(test)]
pub(crate) mod merge_resolution_tests;
#[cfg(test)]
pub(crate) mod multi_diff_tests;
pub(crate) mod multi_diff_ui;
pub(crate) mod scm_branch_ui;
pub(crate) mod scm_commit_ui;
pub(crate) mod scm_confirm;
pub(crate) mod scm_graph_menu;
pub(crate) mod scm_graph_ui;
pub(crate) mod scm_more_menu;
pub(crate) mod scm_icons;
pub(crate) mod scm_publish;
pub(crate) mod scm_remote_ui;
pub(crate) mod scm_tooltips;
pub(crate) mod scm_ui;
pub(crate) mod scm_welcome;
