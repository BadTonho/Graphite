//! Notebook view, kernel and interactive window.

#[allow(unused_imports)]
use super::*;

pub(crate) mod notebook_drawing;
pub(crate) mod notebook_interactive;
#[cfg(test)]
pub(crate) mod notebook_interactive_tests;
pub(crate) mod notebook_kernel;
#[cfg(test)]
pub(crate) mod notebook_kernel_tests;
pub(crate) mod notebook_ui;
#[cfg(test)]
pub(crate) mod notebook_ui_tests;
pub(crate) mod notebook_view;
#[cfg(test)]
pub(crate) mod notebook_view_tests;
