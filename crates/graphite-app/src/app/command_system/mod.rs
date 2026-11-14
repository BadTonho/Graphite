//! Command registry, palette, execution and handlers.

#[allow(unused_imports)]
use super::*;

pub(crate) mod cli_ui;
pub(crate) mod command_center;
pub(crate) mod command_execution;
pub(crate) mod command_handlers;
pub(crate) mod command_palette;
pub(crate) mod command_registry;
#[cfg(test)]
pub(crate) mod command_tests;
pub(crate) mod commands;
