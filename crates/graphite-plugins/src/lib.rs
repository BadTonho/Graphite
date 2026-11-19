//! Native plugin packages and discovery, independent of the editor and GPU.
pub mod catalog;
pub mod manifest;
pub mod open_vsx;
pub mod package;
pub mod sdk;
pub mod store;
pub type Result<T> = std::result::Result<T, String>;
pub const MAX_DOWNLOAD: u64 = 100 * 1024 * 1024;
pub(crate) const MAX_VSIX_DOWNLOAD: u64 = 1024 * 1024 * 1024;
pub(crate) const MAX_VSIX_EXPANDED: u64 = 4 * MAX_VSIX_DOWNLOAD;

pub(crate) fn is_vsix(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vsix"))
}

pub(crate) fn download_limit(is_vsix: bool) -> u64 {
    if is_vsix {
        MAX_VSIX_DOWNLOAD
    } else {
        MAX_DOWNLOAD
    }
}

pub(crate) fn size_limit_label(is_vsix: bool) -> &'static str {
    if is_vsix {
        "1 GiB"
    } else {
        "100 MiB"
    }
}
pub const HOST: &str = include_str!("../runtime/host.cjs");
