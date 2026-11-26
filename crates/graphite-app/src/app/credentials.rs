//! Secrets kept in the operating system's credential store: Windows Credential Manager or the
//! Linux Secret Service. Entries are namespaced by profile folder, so profiles never share them.

use std::path::Path;

/// The store entry for `id` in `service`, namespaced by `profile`.
#[cfg(any(windows, target_os = "linux"))]
pub(crate) fn system_entry(
    service: &str,
    profile: &Path,
    id: &str,
) -> Result<keyring::Entry, &'static str> {
    let profile = profile
        .canonicalize()
        .unwrap_or_else(|_| profile.to_owned());
    let digest = ring::digest::digest(&ring::digest::SHA256, profile.to_string_lossy().as_bytes());
    let namespace: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    keyring::Entry::new(service, &format!("{namespace}:{id}"))
        .map_err(|_| "The system credential store is unavailable")
}

/// Keeps the GitHub token of a profile in the system credential store.
pub(crate) struct GitHubTokenVault;

const GITHUB_SERVICE: &str = "Graphite GitHub";
const GITHUB_ID: &str = "github";

#[cfg(any(windows, target_os = "linux"))]
impl graphite_auth::TokenVault for GitHubTokenVault {
    fn get(&self, profile: &Path) -> Result<Option<String>, String> {
        match system_entry(GITHUB_SERVICE, profile, GITHUB_ID)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("Could not read the GitHub token from the system store".into()),
        }
    }

    fn set(&self, profile: &Path, token: &str) -> Result<(), String> {
        system_entry(GITHUB_SERVICE, profile, GITHUB_ID)?
            .set_password(token)
            .map_err(|_| "Could not save the GitHub token to the system store".into())
    }

    fn remove(&self, profile: &Path) -> Result<(), String> {
        match system_entry(GITHUB_SERVICE, profile, GITHUB_ID)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("Could not remove the GitHub token from the system store".into()),
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
impl graphite_auth::TokenVault for GitHubTokenVault {
    fn get(&self, _: &Path) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn set(&self, _: &Path, _: &str) -> Result<(), String> {
        Err("The system credential store is unavailable".into())
    }
    fn remove(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
}
