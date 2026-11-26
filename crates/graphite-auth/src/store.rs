use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

/// File name of the saved account, inside the profile folder. It holds the account details; the
/// token itself is kept in a [`TokenVault`].
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

/// Keeps the token of the account saved in a profile folder, outside that folder.
pub trait TokenVault {
    fn get(&self, profile: &Path) -> Result<Option<String>, String>;
    fn set(&self, profile: &Path, token: &str) -> Result<(), String>;
    fn remove(&self, profile: &Path) -> Result<(), String>;
}

/// What the session file holds. `token` is only read, from files written by earlier versions.
#[derive(Serialize, Deserialize)]
struct StoredSession {
    #[serde(default, skip_serializing)]
    token: Option<String>,
    login: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    scope: String,
}

/// Saves the account into `dir` and its token into `vault`. Nothing is written when the vault
/// refuses the token, so the token never ends up in the file.
pub fn save_session(dir: &Path, session: &Session, vault: &dyn TokenVault) -> Result<(), String> {
    vault.set(dir, &session.token)?;
    write_details(dir, session)
}

/// The account saved in `dir`, if there is a readable one and the vault holds its token. A file
/// written by an earlier version, with the token inside, has the token moved into the vault.
pub fn load_session(dir: &Path, vault: &dyn TokenVault) -> Option<Session> {
    let bytes = fs::read(dir.join(SESSION_FILE)).ok()?;
    let stored: StoredSession = serde_json::from_slice(&bytes).ok()?;
    let token = match stored.token.filter(|token| !token.is_empty()) {
        Some(token) => {
            // The file keeps the token until the vault has it, so a failure loses nothing.
            if vault.set(dir, &token).is_ok() {
                let _ = write_details(
                    dir,
                    &Session {
                        token: String::new(),
                        login: stored.login.clone(),
                        name: stored.name.clone(),
                        scope: stored.scope.clone(),
                    },
                );
            }
            token
        }
        None => vault.get(dir).ok().flatten()?,
    };
    Some(Session {
        token,
        login: stored.login,
        name: stored.name,
        scope: stored.scope,
    })
}

/// Forgets the saved account and its token. A missing file or token is not an error.
pub fn delete_session(dir: &Path, vault: &dyn TokenVault) -> Result<(), String> {
    vault.remove(dir)?;
    match fs::remove_file(dir.join(SESSION_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// Writes the account details, without the token, readable only by the owner on Unix.
fn write_details(dir: &Path, session: &Session) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let details = StoredSession {
        token: None,
        login: session.login.clone(),
        name: session.name.clone(),
        scope: session.scope.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&details).map_err(|error| error.to_string())?;
    let path = dir.join(SESSION_FILE);
    let temp = dir.join(format!("{SESSION_FILE}.tmp"));
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Owner-only from the moment the file exists.
        options.mode(0o600);
    }
    let mut file = options.open(&temp).map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())?;
    drop(file);
    fs::rename(&temp, &path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        error.to_string()
    })
}

/// A vault that keeps tokens in memory, for tests and for profiles that are never saved.
#[derive(Default)]
pub struct MemoryVault(std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, String>>);

impl TokenVault for MemoryVault {
    fn get(&self, profile: &Path) -> Result<Option<String>, String> {
        Ok(self.lock().get(profile).cloned())
    }

    fn set(&self, profile: &Path, token: &str) -> Result<(), String> {
        self.lock().insert(profile.to_path_buf(), token.to_owned());
        Ok(())
    }

    fn remove(&self, profile: &Path) -> Result<(), String> {
        self.lock().remove(profile);
        Ok(())
    }
}

impl MemoryVault {
    fn lock(
        &self,
    ) -> std::sync::MutexGuard<'_, std::collections::HashMap<std::path::PathBuf, String>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
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

    struct RefusingVault;

    impl TokenVault for RefusingVault {
        fn get(&self, _: &Path) -> Result<Option<String>, String> {
            Err("unavailable".into())
        }
        fn set(&self, _: &Path, _: &str) -> Result<(), String> {
            Err("unavailable".into())
        }
        fn remove(&self, _: &Path) -> Result<(), String> {
            Err("unavailable".into())
        }
    }

    #[test]
    fn session_round_trips_and_delete_is_idempotent() {
        let dir = crate::TestDir::new();
        let vault = MemoryVault::default();
        assert_eq!(load_session(&dir, &vault), None);
        save_session(&dir, &session(), &vault).unwrap();
        assert_eq!(load_session(&dir, &vault), Some(session()));
        delete_session(&dir, &vault).unwrap();
        assert_eq!(load_session(&dir, &vault), None);
        assert_eq!(vault.get(&dir).unwrap(), None);
        delete_session(&dir, &vault).unwrap();
    }

    #[test]
    fn the_token_is_never_written_to_the_profile_folder() {
        let dir = crate::TestDir::new();
        let vault = MemoryVault::default();
        save_session(&dir, &session(), &vault).unwrap();

        let file = fs::read_to_string(dir.join(SESSION_FILE)).unwrap();
        assert!(!file.contains("gho_secret"), "{file}");
        assert!(file.contains("octocat"));
        assert_eq!(vault.get(&dir).unwrap().as_deref(), Some("gho_secret"));
    }

    #[test]
    fn nothing_is_saved_when_the_vault_refuses_the_token() {
        let dir = crate::TestDir::new();
        assert!(save_session(&dir, &session(), &RefusingVault).is_err());
        assert!(!dir.join(SESSION_FILE).exists());
    }

    #[test]
    fn a_token_saved_in_the_file_by_an_earlier_version_moves_to_the_vault() {
        let dir = crate::TestDir::new();
        fs::create_dir_all(&*dir).unwrap();
        fs::write(
            dir.join(SESSION_FILE),
            r#"{"token":"gho_secret","login":"octocat","name":"Mona"}"#,
        )
        .unwrap();
        let vault = MemoryVault::default();

        assert_eq!(load_session(&dir, &vault), Some(session()));

        let file = fs::read_to_string(dir.join(SESSION_FILE)).unwrap();
        assert!(!file.contains("gho_secret"), "{file}");
        assert_eq!(vault.get(&dir).unwrap().as_deref(), Some("gho_secret"));
        assert_eq!(load_session(&dir, &vault), Some(session()));
    }

    #[test]
    fn an_earlier_file_keeps_its_token_while_the_vault_is_unavailable() {
        let dir = crate::TestDir::new();
        fs::create_dir_all(&*dir).unwrap();
        let legacy = r#"{"token":"gho_secret","login":"octocat","name":"Mona"}"#;
        fs::write(dir.join(SESSION_FILE), legacy).unwrap();

        assert_eq!(load_session(&dir, &RefusingVault), Some(session()));
        assert_eq!(fs::read_to_string(dir.join(SESSION_FILE)).unwrap(), legacy);
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
        save_session(&dir, &session(), &MemoryVault::default()).unwrap();
        let mode = fs::metadata(dir.join(SESSION_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
