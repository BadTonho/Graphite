//! Native plugin packages and discovery, independent of the editor and GPU.
pub mod catalog;
pub mod manifest;
pub mod open_vsx;
pub mod package;
pub mod sdk;
pub mod store;
pub type Result<T> = std::result::Result<T, String>;
pub const MAX_DOWNLOAD: u64 = 100 * 1024 * 1024;
pub const HOST: &str = include_str!("../runtime/host.cjs");
