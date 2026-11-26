use super::*;
use graphite_core_api::DocumentUri;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

type Settings = serde_json::Map<String, Value>;

#[derive(Default, Deserialize, Serialize)]
struct Configuration {
    profile: Settings,
    workspaces: BTreeMap<String, Settings>,
}

fn load<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(error.to_string()),
    }
}

fn persist(path: &Path, value: impl Serialize) -> Result<()> {
    fs::create_dir_all(path.parent().unwrap()).map_err(|error| error.to_string())?;
    let temp = path.with_extension("json.tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        serde_json::to_writer(&mut file, &value).map_err(|error| error.to_string())?;
        file.flush().map_err(|error| error.to_string())?;
        drop(file);
        crate::app::phase5::replace_file_atomically(&temp, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

impl App {
    fn vsix_workspace_key(&self) -> Result<String> {
        DocumentUri::from_path(self.tree.root())
            .map(|uri| uri.as_str().to_owned())
            .map_err(|error| error.to_string())
    }

    pub(super) fn vsix_configuration(&self) -> Result<Value> {
        let path = self.profile_root.join("code-oss/settings.json");
        let configuration: Configuration = load(&path)?;
        let root = self.vsix_workspace_key()?;
        Ok(json!({
            "base": self.service_configuration()?,
            "profile": configuration.profile,
            "workspace": configuration.workspaces.get(&root),
        }))
    }

    pub(super) fn change_vsix_configuration(
        &self,
        runtime: &Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        let target = args.first().and_then(Value::as_u64);
        let key = args
            .get(1)
            .and_then(Value::as_str)
            .ok_or("Invalid VSIX setting")?;
        let value = if method == "$updateConfigurationOption" {
            Some(args.get(2).cloned().ok_or("Missing VSIX setting value")?)
        } else {
            None
        };
        let language_scoped = args.last().and_then(Value::as_bool).unwrap_or(false);
        if language_scoped {
            return Err("Language-specific VSIX configuration updates are unavailable".into());
        }
        let path = self.profile_root.join("code-oss/settings.json");
        let mut configuration: Configuration = load(&path)?;
        let settings = match target {
            Some(2 | 3) => &mut configuration.profile,
            None | Some(5 | 6) => configuration
                .workspaces
                .entry(self.vsix_workspace_key()?)
                .or_default(),
            _ => return Err("Unsupported VSIX configuration target".into()),
        };
        if let Some(value) = value {
            settings.insert(key.to_owned(), value);
        } else {
            settings.remove(key);
        }
        persist(&path, configuration)?;
        runtime.session.send(
            json!({"type":"configuration", "configuration":self.vsix_configuration()?, "key":key}),
        )?;
        Ok(Value::Null)
    }

    pub(super) fn vsix_storage(&self, method: &str, args: &[Value]) -> Result<Value> {
        let shared = args
            .first()
            .and_then(Value::as_bool)
            .ok_or("Invalid VSIX storage scope")?;
        let id = args
            .get(1)
            .and_then(Value::as_str)
            .ok_or("Invalid VSIX storage owner")?;
        if !graphite_plugins::manifest::valid_id(id) {
            return Err("Invalid VSIX storage owner".into());
        }
        let path = self
            .profile_root
            .join("code-oss/state")
            .join(format!("{id}.json"));
        let mut values: BTreeMap<String, Value> = load(&path)?;
        let key = if shared {
            "global".into()
        } else {
            self.vsix_workspace_key()?
        };
        if method == "$initializeExtensionStorage" {
            return Ok(values
                .get(&key)
                .map(|value| Value::String(value.to_string()))
                .unwrap_or_default());
        }
        values.insert(
            key,
            args.get(2).cloned().ok_or("Missing VSIX storage value")?,
        );
        persist(&path, values)?;
        Ok(Value::Null)
    }
}
