//! Merge model: conflict parsing, per-block resolution and the aligned rows the merge editor
//! draws. Pure text logic; nothing here touches the window, the disk or Git.

mod align;
mod parse;
mod resolve;

#[allow(unused_imports)]
pub(crate) use align::{aligned_rows, Cell, Pane, Row};
pub(crate) use parse::{parse, ConflictBlock, Parsed, Segment};
#[allow(unused_imports)]
pub(crate) use resolve::{Choice, MergeState, Resolution, Side};
