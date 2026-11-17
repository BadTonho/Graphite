//! GitHub sign-in for Graphite, using the OAuth device flow.
//!
//! The device flow needs only the public client ID of a GitHub OAuth App, never a client secret,
//! so it works in a binary that is distributed to many users. The user enters a short code on
//! github.com; Graphite polls for the token and keeps it in the profile folder.

mod device;
mod store;
mod transport;

pub use device::{sign_in, sign_in_with_scope, start, start_with_scope, DeviceCode, Poll};
pub use store::{delete_session, load_session, save_session, Session, SESSION_FILE};
pub use transport::{ReqwestTransport, Transport};

/// Scope requested from GitHub: read the signed-in user's public profile only.
pub const SCOPE: &str = "read:user";
/// Scope requested when User Data Sync pushes/pulls private GitHub Gists.
pub const SCOPE_GIST: &str = "read:user gist";
/// Scope requested when User Data Sync uses a private repository.
pub const SCOPE_REPO: &str = "read:user repo";
pub const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
pub const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
pub const USER_URL: &str = "https://api.github.com/user";

/// Unique temporary folder for tests, removed when dropped.
#[cfg(test)]
pub(crate) struct TestDir(std::path::PathBuf);

#[cfg(test)]
impl TestDir {
    pub(crate) fn new() -> Self {
        let name = format!(
            "graphite-auth-{}-{}",
            std::process::id(),
            std::thread::current()
                .name()
                .unwrap_or("t")
                .replace("::", "_")
        );
        let path = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

#[cfg(test)]
impl std::ops::Deref for TestDir {
    type Target = std::path::Path;
    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
