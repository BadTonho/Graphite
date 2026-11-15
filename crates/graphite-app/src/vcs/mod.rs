//! Source control, merge editing and multi-file diffs (non-UI logic).

#[allow(unused_imports)]
use super::*;

pub(crate) mod merge;
pub(crate) mod multi_diff;
pub(crate) mod scm;
pub(crate) mod scm_branches;
pub(crate) mod scm_changes;
pub(crate) mod scm_commit;
pub(crate) mod scm_discard;
pub(crate) mod scm_graph;
pub(crate) mod scm_ignore;
pub(crate) mod scm_refs;
pub(crate) mod scm_remote;
pub(crate) mod scm_stash;
