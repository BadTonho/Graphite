//! Command registry, palette, execution and handlers.

#[allow(unused_imports)]
use super::*;

pub(crate) mod commands;
pub(crate) mod command_center;
pub(crate) mod command_execution;
pub(crate) mod command_palette;
pub(crate) mod command_registry;
pub(crate) mod command_handlers;
#[cfg(test)]
pub(crate) mod command_tests;
pub(crate) mod cli_ui;
