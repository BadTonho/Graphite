//! Native Graphite plugin UI, status bar, output channel, and storage services.
use super::*;
use crate::app::command_palette::{CommandPalette, PaletteItem};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

const OUTPUT_LIMIT: usize = 1 << 20;

#[derive(Clone)]
pub(crate) struct NativePrompt {
    pub rpc_id: u64,
    pub parent: Option<u64>,
    pub kind: NativePromptKind,
}

#[derive(Clone)]
pub(crate) enum NativePromptKind {
    Input,
    QuickPick {
        can_pick_many: bool,
        items: Vec<QuickPickItem>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct NativeStatusItem {
    pub text: String,
    pub command: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
struct NativePluginStorageFile {
    #[serde(default)]
    global: serde_json::Map<String, Value>,
    #[serde(default)]
    workspaces: BTreeMap<String, serde_json::Map<String, Value>>,
}

impl ExtensionRuntime {
    pub(crate) fn status_texts(&self) -> Vec<(String, Option<String>)> {
        self.status_items
            .values()
            .map(|item| (item.text.clone(), item.command.clone()))
            .collect()
    }
}

impl App {
    pub(super) fn handle_native_prompt_request(
        &mut self,
        id: u64,
        parent: Option<u64>,
        request: ServiceRequest,
    ) -> ServiceResult<()> {
        let has_active_prompt = self.plugins.vsix.as_ref().is_some_and(|_| {
            self.command_palette
                .as_ref()
                .is_some_and(|p| p.extension_prompt.is_some())
        }) || self.extension_host.prompt.is_some()
            || self
                .plugins
                .hosts
                .values()
                .any(|host| host.prompt.is_some());
        if has_active_prompt {
            return Err(ServiceError::Busy);
        }

        let (kind, title, initial_query, password, palette_items) = match request {
            ServiceRequest::PromptInput {
                prompt,
                place_holder,
                value,
                password,
            } => {
                let title = prompt
                    .or(place_holder)
                    .unwrap_or_else(|| "Plugin input".to_owned());
                (
                    NativePromptKind::Input,
                    title,
                    value.unwrap_or_default(),
                    password,
                    Vec::new(),
                )
            }
            ServiceRequest::QuickPick {
                items,
                place_holder,
                can_pick_many,
            } => {
                let palette_items = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let source = item.description.clone().unwrap_or_else(|| {
                            self.extension_host
                                .native_manifest
                                .as_ref()
                                .map(|m| m.id.clone())
                                .unwrap_or_else(|| "Plugin".into())
                        });
                        PaletteItem {
                            id: index.to_string(),
                            title: item.label.clone(),
                            source,
                            enabled: true,
                        }
                    })
                    .collect();
                (
                    NativePromptKind::QuickPick {
                        can_pick_many,
                        items,
                    },
                    place_holder.unwrap_or_else(|| "Select an item".to_owned()),
                    String::new(),
                    false,
                    palette_items,
                )
            }
            _ => {
                return Err(ServiceError::InvalidArguments(
                    "Expected prompt request".into(),
                ))
            }
        };

        self.open_command_palette();
        self.extension_host.prompt = Some(NativePrompt {
            rpc_id: id,
            parent,
            kind,
        });
        if let Some(host) = self.extension_host.supervisor.as_mut() {
            if let Some(parent_id) = parent {
                host.pending.suspend(parent_id);
            }
            let _ = host.send(Message::WaitingForUser {
                session: host.session,
                id,
            });
        }
        self.command_palette = Some(CommandPalette {
            query: initial_query,
            selected: 0,
            items: palette_items,
            extension_prompt: Some((title, password)),
        });
        Ok(())
    }

    pub(crate) fn cancel_extension_prompt(&mut self) {
        self.answer_extension_prompt(None);
    }

    pub(crate) fn answer_extension_prompt(&mut self, value: Option<(String, Option<String>)>) {
        if self.answer_draft_review(value.as_ref().and_then(|(_, id)| id.as_deref())) {
            return;
        }
        if let Some(target) = self.take_native_prompt_owner() {
            self.finish_native_prompt(target, value);
            return;
        }
        self.answer_vsix_prompt(value);
    }

    fn take_native_prompt_owner(&mut self) -> Option<(Option<String>, NativePrompt)> {
        if let Some(prompt) = self.extension_host.prompt.take() {
            return Some((None, prompt));
        }
        let owner = self
            .plugins
            .hosts
            .iter()
            .find(|(_, host)| host.prompt.is_some())
            .map(|(id, _)| id.clone())?;
        let prompt = self.plugins.hosts.get_mut(&owner)?.prompt.take()?;
        Some((Some(owner), prompt))
    }

    fn finish_native_prompt(
        &mut self,
        (owner, prompt): (Option<String>, NativePrompt),
        value: Option<(String, Option<String>)>,
    ) {
        let payload = match (&prompt.kind, value) {
            (_, None) => Value::Null,
            (NativePromptKind::Input, Some((query, _))) => json!(query),
            (
                NativePromptKind::QuickPick {
                    can_pick_many,
                    items,
                },
                Some((_, selected)),
            ) => {
                let picked = selected
                    .and_then(|id| id.parse::<usize>().ok())
                    .and_then(|idx| items.get(idx))
                    .and_then(|item| serde_json::to_value(item).ok());
                if *can_pick_many {
                    picked.map(|v| Value::Array(vec![v])).unwrap_or(Value::Null)
                } else {
                    picked.unwrap_or(Value::Null)
                }
            }
        };

        let host = match owner.as_deref() {
            None => &mut self.extension_host,
            Some(id) => match self.plugins.hosts.get_mut(id) {
                Some(host) => host,
                None => return,
            },
        };
        if let Some(supervisor) = host.supervisor.as_mut() {
            let now = Instant::now();
            if let Some(parent_id) = prompt.parent {
                supervisor.pending.resume(parent_id, now);
            }
            let session = supervisor.session;
            let _ = supervisor.send(Message::Resumed {
                session,
                id: prompt.rpc_id,
            });
            if let Err(error) = supervisor.send(Message::Response {
                session,
                id: prompt.rpc_id,
                result: Ok(payload),
            }) {
                supervisor.stop(error);
            }
        }
    }

    pub(crate) fn extension_status_items(&self) -> Vec<(String, Option<String>)> {
        let mut items = Vec::new();
        if let Some(runtime) = &self.plugins.vsix {
            for (index, text) in runtime.workbench_status_texts().into_iter().enumerate() {
                items.push((text, runtime.workbench_status_command(index)));
            }
        }
        for host in self.plugins.hosts.values() {
            items.extend(host.status_texts());
        }
        if self.extension_host.native_manifest.is_some() {
            items.extend(self.extension_host.status_texts());
        }
        items
    }

    pub(crate) fn run_extension_status_item(&mut self, index: usize) {
        let vsix_count = self
            .plugins
            .vsix
            .as_ref()
            .map(|r| r.workbench_status_texts().len())
            .unwrap_or(0);
        if index < vsix_count {
            self.run_vsix_status_item(index);
            return;
        }
        let Some((_, Some(command))) = self.extension_status_items().into_iter().nth(index) else {
            return;
        };
        if self
            .plugins
            .hosts
            .values()
            .any(|h| h.commands.contains_key(&command))
            || self.extension_host.commands.contains_key(&command)
        {
            let _ = self.execute_command(CommandRequest {
                id: command,
                arguments: Value::Null,
                target: CommandTarget::Global,
            });
        } else if crate::app::command_registry::registry()
            .get(&command)
            .is_some()
        {
            self.run_ui_command(&command);
        } else {
            self.status = format!("Unknown command: {command}");
        }
    }

    pub(super) fn native_output_root(&self) -> PathBuf {
        let base = self
            .plugins
            .store
            .as_ref()
            .map(|s| s.root.clone())
            .unwrap_or_else(|| self.profile_root.join("plugins"));
        let plugin_id = self
            .extension_host
            .native_manifest
            .as_ref()
            .map(|m| m.id.as_str())
            .unwrap_or("extension-host");
        base.join("logs").join(plugin_id)
    }

    pub(super) fn create_native_output_channel(&mut self, name: String) -> ServiceResult<String> {
        let safe_name = sanitize_filename_component(&name);
        self.extension_host.next_output += 1;
        let id = format!("output-{}-{safe_name}", self.extension_host.next_output);
        let dir = self.native_output_root();
        std::fs::create_dir_all(&dir).map_err(|e| ServiceError::Io(e.to_string()))?;
        let path = dir.join(format!("{id}.log"));
        if !path.exists() {
            std::fs::write(&path, b"").map_err(|e| ServiceError::Io(e.to_string()))?;
        }
        self.extension_host.output_channels.insert(id.clone(), path);
        Ok(id)
    }

    pub(super) fn append_native_output(
        &mut self,
        channel: &str,
        text: &str,
        replace: bool,
    ) -> ServiceResult<()> {
        let path = self
            .extension_host
            .output_channels
            .get(channel)
            .cloned()
            .ok_or(ServiceError::NotFound)?;
        if replace {
            std::fs::write(&path, text.as_bytes()).map_err(|e| ServiceError::Io(e.to_string()))?;
        } else {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|e| ServiceError::Io(e.to_string()))?;
            file.write_all(text.as_bytes())
                .and_then(|_| file.flush())
                .map_err(|e| ServiceError::Io(e.to_string()))?;
        }
        if self.documents.id_for_path(&path).is_some() {
            let content = read_trailing_output(&path);
            self.show_output_text(&path, &content);
        }
        Ok(())
    }

    pub(super) fn show_native_output(&mut self, channel: &str) -> ServiceResult<()> {
        let path = self
            .extension_host
            .output_channels
            .get(channel)
            .cloned()
            .ok_or(ServiceError::NotFound)?;
        let content = read_trailing_output(&path);
        self.show_output_text(&path, &content);
        Ok(())
    }

    fn native_storage_path(&self) -> ServiceResult<PathBuf> {
        let plugin_id = self
            .extension_host
            .native_manifest
            .as_ref()
            .map(|m| m.id.as_str())
            .ok_or_else(|| {
                ServiceError::InvalidArguments("Storage requires an active native plugin".into())
            })?;
        let base = self
            .plugins
            .store
            .as_ref()
            .map(|s| s.root.clone())
            .unwrap_or_else(|| self.profile_root.join("plugins"));
        Ok(base.join("storage").join(format!("{plugin_id}.json")))
    }

    fn workspace_storage_key(&self) -> ServiceResult<String> {
        DocumentUri::from_path(self.tree.root())
            .map(|uri| uri.as_str().to_owned())
            .map_err(|e| ServiceError::Io(e.to_string()))
    }

    pub(super) fn native_storage_get(
        &self,
        scope: StorageScope,
        key: Option<String>,
    ) -> ServiceResult<Value> {
        let path = self.native_storage_path()?;
        let data: NativePluginStorageFile = match std::fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| ServiceError::Io(e.to_string()))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                NativePluginStorageFile::default()
            }
            Err(error) => return Err(ServiceError::Io(error.to_string())),
        };
        let map = match scope {
            StorageScope::Global => Some(&data.global),
            StorageScope::Workspace => {
                let root = self.workspace_storage_key()?;
                data.workspaces.get(&root)
            }
        };
        Ok(match (key, map) {
            (None, Some(map)) => Value::Object(map.clone()),
            (None, None) => Value::Object(Default::default()),
            (Some(k), Some(map)) => map.get(&k).cloned().unwrap_or(Value::Null),
            (Some(_), None) => Value::Null,
        })
    }

    pub(super) fn native_storage_set(
        &self,
        scope: StorageScope,
        key: String,
        value: Option<Value>,
    ) -> ServiceResult<()> {
        if key.is_empty() {
            return Err(ServiceError::InvalidArguments("Empty storage key".into()));
        }
        let path = self.native_storage_path()?;
        let mut data: NativePluginStorageFile = match std::fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| ServiceError::Io(e.to_string()))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                NativePluginStorageFile::default()
            }
            Err(error) => return Err(ServiceError::Io(error.to_string())),
        };
        let target_map = match scope {
            StorageScope::Global => &mut data.global,
            StorageScope::Workspace => {
                let root = self.workspace_storage_key()?;
                data.workspaces.entry(root).or_default()
            }
        };
        match value {
            Some(v) => {
                target_map.insert(key, v);
            }
            None => {
                target_map.remove(&key);
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ServiceError::Io(e.to_string()))?;
        }
        let temp = path.with_extension("json.tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| ServiceError::Io(e.to_string()))?;
        let write_result = (|| {
            serde_json::to_writer(&mut file, &data).map_err(|e| ServiceError::Io(e.to_string()))?;
            file.flush().map_err(|e| ServiceError::Io(e.to_string()))?;
            drop(file);
            crate::app::phase5::replace_profile_pointer(&temp, &path)
                .map_err(|e| ServiceError::Io(e.to_string()))
        })();
        if write_result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        write_result
    }
}

fn sanitize_filename_component(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = clean.trim_matches('-');
    if trimmed.is_empty() {
        "channel".to_owned()
    } else {
        trimmed.chars().take(48).collect()
    }
}

fn read_trailing_output(path: &std::path::Path) -> String {
    let Ok(bytes) = std::fs::read(path) else {
        return String::new();
    };
    let start = bytes.len().saturating_sub(OUTPUT_LIMIT);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}
