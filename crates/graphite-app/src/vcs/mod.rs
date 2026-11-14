//! Source control, merge editing and multi-file diffs (non-UI logic).

#[allow(unused_imports)]
use super::*;

pub(crate) mod merge;
pub(crate) mod multi_diff;
pub(crate) mod scm;
pub(crate) mod scm_branches;
pub(crate) mod scm_changes;
pub(crate) mod scm_remote;
