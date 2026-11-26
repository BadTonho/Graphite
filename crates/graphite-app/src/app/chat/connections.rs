//! Profile connection metadata and operating-system credentials.

use graphite_chat::{ProviderConfig, ProviderKind};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

const FILE: &str = "chat-connections.json";
const LEGACY: &str = "chat-provider.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Provider {
    #[default]
    OpenAi,
    Claude,
    OpenCodeConsole,
    OpenCodeGo,
    Gemini,
    OpenRouter,
    Compatible,
}

impl Provider {
    pub(super) const ALL: [Self; 7] = [
        Self::OpenAi,
        Self::Claude,
        Self::OpenCodeConsole,
        Self::OpenCodeGo,
        Self::Gemini,
        Self::OpenRouter,
        Self::Compatible,
    ];

    pub fn cycle(self, backwards: bool) -> Self {
        let index = Self::ALL
            .iter()
            .position(|provider| *provider == self)
            .unwrap_or(0);
        let offset = if backwards { Self::ALL.len() - 1 } else { 1 };
        Self::ALL[(index + offset) % Self::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Claude => "Claude",
            Self::OpenCodeConsole => "OpenCode Console",
            Self::OpenCodeGo => "OpenCode Go",
            Self::Gemini => "Gemini",
            Self::OpenRouter => "OpenRouter",
            Self::Compatible => "OpenAI compatible",
        }
    }

    pub fn requires_key(self) -> bool {
        self != Self::Compatible
    }

    pub fn kind(self) -> ProviderKind {
        match self {
            Self::OpenAi | Self::Compatible => ProviderKind::OpenAiCompatible,
            Self::Claude => ProviderKind::Anthropic,
            Self::OpenCodeConsole => ProviderKind::OpenCodeConsole,
            Self::OpenCodeGo => ProviderKind::OpenCodeGo,
            Self::Gemini => ProviderKind::Gemini,
            Self::OpenRouter => ProviderKind::OpenRouter,
        }
    }

    pub fn has_fixed_endpoint(self) -> bool {
        matches!(
            self,
            Self::OpenCodeConsole | Self::OpenCodeGo | Self::Gemini | Self::OpenRouter
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Connection {
    pub id: String,
    pub name: String,
    pub provider: Provider,
    pub model: String,
    pub base_url: String,
    pub credential: Option<String>,
}

impl Connection {
    pub fn config(&self, api_key: Option<String>) -> ProviderConfig {
        ProviderConfig {
            kind: self.provider.kind(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            api_key,
            opencode_session: None,
        }
    }

    pub fn destination(&self) -> String {
        let mut hash = digest::Context::new(&digest::SHA256);
        let endpoint = if self.provider.has_fixed_endpoint() {
            self.provider.kind().default_base_url()
        } else {
            &self.base_url
        };
        for value in [self.provider.label(), endpoint, &self.model] {
            hash.update(&(value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        hex(hash.finish().as_ref())
    }

    pub fn same_destination(&self, other: &Self) -> bool {
        self.provider == other.provider
            && self.model == other.model
            && self.base_url == other.base_url
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Connections {
    pub items: Vec<Connection>,
    pub preferred: Option<String>,
    #[serde(default)]
    migrated: bool,
}

impl Connections {
    pub fn get(&self, id: &str) -> Option<&Connection> {
        self.items.iter().find(|connection| connection.id == id)
    }

    pub fn save(&self, profile: &Path) -> Result<(), &'static str> {
        const ERROR: &str = "Could not save chat connections";
        fs::create_dir_all(profile).map_err(|_| ERROR)?;
        let mut pending = tempfile::NamedTempFile::new_in(profile).map_err(|_| ERROR)?;
        serde_json::to_writer_pretty(&mut pending, self).map_err(|_| ERROR)?;
        pending.flush().map_err(|_| ERROR)?;
        pending.as_file().sync_all().map_err(|_| ERROR)?;
        pending.persist(profile.join(FILE)).map_err(|_| ERROR)?;
        Ok(())
    }

    pub fn load(profile: &Path) -> Result<Self, &'static str> {
        match fs::read(profile.join(FILE)) {
            Ok(data) => {
                serde_json::from_slice(&data).map_err(|_| "Invalid chat connection metadata")
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err("Could not read chat connection metadata"),
        }
    }

    pub fn migrate(&mut self, profile: &Path, vault: &impl Vault) -> Result<(), &'static str> {
        let path = profile.join(LEGACY);
        let legacy = match fs::read(&path) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err("Could not read the previous chat configuration"),
        };
        if !self.migrated {
            let old: ProviderConfig = serde_json::from_slice(&legacy)
                .map_err(|_| "Invalid previous chat configuration")?;
            let provider = match old.kind {
                ProviderKind::Anthropic => Provider::Claude,
                ProviderKind::OpenAiCompatible if old.base_url.trim().is_empty() => {
                    Provider::OpenAi
                }
                ProviderKind::OpenAiCompatible => Provider::Compatible,
                ProviderKind::OpenCodeConsole => Provider::OpenCodeConsole,
                ProviderKind::OpenCodeGo => Provider::OpenCodeGo,
                ProviderKind::Gemini => Provider::Gemini,
                ProviderKind::OpenRouter => Provider::OpenRouter,
            };
            let mut connection = Connection {
                id: "legacy".into(),
                name: provider.label().into(),
                provider,
                model: old.model,
                base_url: old.base_url,
                credential: None,
            };
            connection
                .config(Some("validation".into()))
                .check()
                .map_err(|_| "Invalid endpoint or model in the previous chat configuration")?;
            if let Some(key) = old.api_key.filter(|key| !key.trim().is_empty()) {
                vault.set(profile, "legacy", &key)?;
                connection.credential = Some("legacy".into());
            }
            let mut pending = self.clone();
            if pending.preferred.is_none() {
                pending.preferred = Some(connection.id.clone());
            }
            pending.items.push(connection);
            pending.migrated = true;
            pending.save(profile)?;
            *self = pending;
        }
        fs::remove_file(path).map_err(|_| "Could not remove the previous chat configuration")
    }
}

pub(super) fn new_id() -> Result<String, &'static str> {
    let mut bytes = [0u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "Could not generate a connection identifier")?;
    Ok(hex(&bytes))
}

pub(super) fn new_opencode_session() -> Result<String, &'static str> {
    new_id().map(|id| format!("ses_{id}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) trait Vault {
    fn get(&self, profile: &Path, id: &str) -> Result<Option<String>, &'static str>;
    fn set(&self, profile: &Path, id: &str, value: &str) -> Result<(), &'static str>;
    fn remove(&self, profile: &Path, id: &str) -> Result<(), &'static str>;
}

pub(super) struct SystemVault;

#[cfg(any(windows, target_os = "linux"))]
fn entry(profile: &Path, id: &str) -> Result<keyring::Entry, &'static str> {
    crate::app::credentials::system_entry("Graphite Chat", profile, id)
}

#[cfg(any(windows, target_os = "linux"))]
impl Vault for SystemVault {
    fn get(&self, profile: &Path, id: &str) -> Result<Option<String>, &'static str> {
        match entry(profile, id)?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("Could not read the chat credential from the system store"),
        }
    }

    fn set(&self, profile: &Path, id: &str, value: &str) -> Result<(), &'static str> {
        entry(profile, id)?
            .set_password(value)
            .map_err(|_| "Could not save the chat credential to the system store")
    }

    fn remove(&self, profile: &Path, id: &str) -> Result<(), &'static str> {
        match entry(profile, id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("Could not remove the chat credential from the system store"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        path::PathBuf,
    };

    #[derive(Default)]
    struct FakeVault {
        unavailable: Cell<bool>,
        secrets: RefCell<HashMap<(PathBuf, String), String>>,
    }

    impl Vault for FakeVault {
        fn get(&self, profile: &Path, id: &str) -> Result<Option<String>, &'static str> {
            Ok(self
                .secrets
                .borrow()
                .get(&(profile.to_owned(), id.to_owned()))
                .cloned())
        }

        fn set(&self, profile: &Path, id: &str, value: &str) -> Result<(), &'static str> {
            if self.unavailable.get() {
                return Err("Fixture vault unavailable");
            }
            self.secrets
                .borrow_mut()
                .insert((profile.to_owned(), id.to_owned()), value.to_owned());
            Ok(())
        }

        fn remove(&self, profile: &Path, id: &str) -> Result<(), &'static str> {
            self.secrets
                .borrow_mut()
                .remove(&(profile.to_owned(), id.to_owned()));
            Ok(())
        }
    }

    #[test]
    fn legacy_credentials_migrate_only_after_successful_vault_storage() {
        let profile = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let legacy = profile.path().join(LEGACY);
        fs::write(
            &legacy,
            r#"{"kind":"anthropic","model":"fixture-model","api_key":"fixture-only-value"}"#,
        )
        .unwrap();
        let vault = FakeVault::default();
        vault.unavailable.set(true);
        let mut saved = Connections::default();
        assert!(saved.migrate(profile.path(), &vault).is_err());
        assert!(legacy.is_file());
        assert!(saved.items.is_empty());
        vault.unavailable.set(false);
        saved.migrate(profile.path(), &vault).unwrap();
        saved.migrate(profile.path(), &vault).unwrap();
        assert!(!legacy.exists());
        let raw = fs::read_to_string(profile.path().join(FILE)).unwrap();
        assert!(!raw.contains("fixture-only-value"));
        let loaded = Connections::load(profile.path()).unwrap();
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].provider, Provider::Claude);
        assert_eq!(
            vault.get(profile.path(), "legacy").unwrap().as_deref(),
            Some("fixture-only-value")
        );
        assert_eq!(vault.get(other.path(), "legacy").unwrap(), None);
        assert!(Connections::load(other.path()).unwrap().items.is_empty());
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
impl Vault for SystemVault {
    fn get(&self, _: &Path, _: &str) -> Result<Option<String>, &'static str> {
        Ok(None)
    }
    fn set(&self, _: &Path, _: &str, _: &str) -> Result<(), &'static str> {
        Err("The system credential store is unavailable")
    }
    fn remove(&self, _: &Path, _: &str) -> Result<(), &'static str> {
        Ok(())
    }
}
