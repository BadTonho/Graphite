//! Editor commands, split into their handlers, the registration table, and inspection commands.

use super::*;

mod handlers;
mod inspection;
mod registration;

pub(super) use registration::register;
