use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// File name of the saved session, inside the profile folder.
pub const SESSION_FILE: &str = "github-session.json";

/// A signed-in GitHub account. The token is a secret: `Debug` never prints it.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    /// The OAuth scopes requested when the token was issued, separated by spaces. A session saved
    /// before this was recorded has none.
    #[serde(default)]
    pub scope: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("token", &"<redacted>")
            .field("login", &self.login)
            .field("name", &self.name)
            .field("scope", &self.scope)
            .finish()
    }
}

/// Writes the session into `dir`. On Unix the file is readable only by its owner.
pub fn save_session(dir: &Path, session: &Session) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let path = dir.join(SESSION_FILE);
    let bytes = serde_json::to_vec_pretty(session).map_err(|error| error.to_string())?;
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    restrict_to_owner(&path)
}

/// The saved session in `dir`, if there is a readable one.
pub fn load_session(dir: &Path) -> Option<Session> {
    let bytes = fs::read(dir.join(SESSION_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Forgets the saved session. Missing files are not an error.
pub fn delete_session(dir: &Path) -> Result<(), String> {
    match fs::remove_file(dir.join(SESSION_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) -> Result<(), String> {
    // Windows files inherit the ACL of the user's profile folder, which only that user can read.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            token: "gho_secret".into(),
            login: "octocat".into(),
            name: Some("Mona".into()),
            ..Default::default()
        }
    }

    #[test]
    fn session_round_trips_and_delete_is_idempotent() {
        let dir = crate::TestDir::new();
        assert_eq!(load_session(&dir), None);
        save_session(&dir, &session()).unwrap();
        assert_eq!(load_session(&dir), Some(session()));
        delete_session(&dir).unwrap();
        assert_eq!(load_session(&dir), None);
        delete_session(&dir).unwrap();
    }

    #[test]
    fn debug_output_never_contains_the_token() {
        let printed = format!("{:?}", session());
        assert!(printed.contains("octocat"));
        assert!(!printed.contains("gho_secret"));
    }

    #[cfg(unix)]
    #[test]
    fn saved_session_is_readable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::TestDir::new();
        save_session(&dir, &session()).unwrap();
        let mode = fs::metadata(dir.join(SESSION_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
