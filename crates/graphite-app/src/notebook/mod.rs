//! Notebooks: the `.ipynb` document model, the kernel connection and the outputs.

mod model;

#[allow(unused_imports)]
pub(crate) use model::{new_cell_id, Cell, CellKind, Notebook};
