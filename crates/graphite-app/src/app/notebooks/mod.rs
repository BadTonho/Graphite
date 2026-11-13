//! Notebook view, kernel and interactive window.

#[allow(unused_imports)]
use super::*;

pub(crate) mod notebook_drawing;
pub(crate) mod notebook_view;
pub(crate) mod notebook_interactive;
pub(crate) mod notebook_kernel;
pub(crate) mod notebook_ui;
#[cfg(test)]
pub(crate) mod notebook_kernel_tests;
#[cfg(test)]
pub(crate) mod notebook_view_tests;
#[cfg(test)]
pub(crate) mod notebook_interactive_tests;
#[cfg(test)]
pub(crate) mod notebook_ui_tests;
