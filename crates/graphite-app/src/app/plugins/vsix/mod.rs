use super::*;
use crate::app::command_palette::PaletteItem;
use graphite_core_api::DocumentService;
use graphite_extension_host::code_oss::Session;
use serde_json::{json, Value};
use std::{collections::BTreeSet, time::Duration};

mod documents;
mod languages;
pub(crate) use languages::Feature as VsixLanguageFeature;
mod prompts;
mod rpc;
mod storage;
mod testing;
#[cfg(test)]
mod tests;

pub(super) struct Runtime {
    session: Session,
    ready: bool,
    registered: BTreeSet<String>,
    command: Option<(String, String, Instant)>,
    sent: bool,
    synced: Option<Value>,
    prompt: Option<prompts::Prompt>,
    languages: languages::Languages,
    editor_commands: BTreeMap<u64, graphite_core_api::CommandTicket>,
    test_calls: BTreeSet<u64>,
}

impl Runtime {
    pub(super) fn pending(&self) -> bool {
        self.command.is_some()
            || self.languages.pending()
            || !self.editor_commands.is_empty()
            || !self.test_calls.is_empty()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for ticket in self.editor_commands.values() {
            ticket.cancellation.cancel();
        }
    }
}

fn eligible(record: &Installed) -> bool {
    matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.main.is_some())
        && record.enabled
        && record.trusted_version.as_deref() == Some(record.metadata.version())
}

impl App {
    fn vsix_workspace_trusted(&self) -> bool {
        self.workspace_trust
            .as_ref()
            .is_some_and(|trust| trust.is_trusted(self.tree.root()))
    }

    pub(crate) fn vsix_palette_items(&self) -> Vec<PaletteItem> {
        let enabled = self.vsix_workspace_trusted()
            && self.plugins.worker.is_none()
            && Session::bundled_runtime().is_some();
        let mut declared = BTreeSet::new();
        let mut items = Vec::new();
        for record in &self.plugins.installed {
            if let Metadata::Vsix(vsix) = &record.metadata {
                for command in &vsix.commands {
                    declared.insert(command.id.as_str());
                    items.push(PaletteItem {
                        id: format!("graphite.vsix:{}:{}", vsix.id, command.id),
                        title: command.title.clone(),
                        source: format!("VSIX {}", vsix.id),
                        enabled: enabled && eligible(record),
                    });
                }
            }
        }
        if let Some(runtime) = &self.plugins.vsix {
            items.extend(
                runtime
                    .registered
                    .iter()
                    .filter(|id| !declared.contains(id.as_str()) && !id.starts_with('_'))
                    .map(|id| PaletteItem {
                        id: format!("graphite.vsix::{id}"),
                        title: id.clone(),
                        source: "VSIX".into(),
                        enabled,
                    }),
            );
        }
        items
    }

    pub(crate) fn run_vsix_command(&mut self, key: &str) -> Result<()> {
        self.run_vsix_command_in(key, Session::bundled_runtime())
    }

    fn run_vsix_command_in(&mut self, key: &str, runtime_path: Option<PathBuf>) -> Result<()> {
        if !self.vsix_workspace_trusted() {
            return Err("Workspace is restricted. Trust it before running VSIX commands.".into());
        }
        if self.plugins.worker.is_some() {
            return Err("Wait for the current plugin operation to finish.".into());
        }
        let (owner, command) = key.split_once(':').ok_or("Invalid VSIX command")?;
        if owner.is_empty() {
            if !self.plugins.vsix.as_ref().is_some_and(|runtime| runtime.registered.contains(command)) {
                return Err("VSIX command is no longer registered".into());
            }
        } else if !self.plugins.installed.iter().any(|record| {
            record.metadata.id() == owner && eligible(record)
                && matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.commands.iter().any(|item| item.id == command))
        }) {
            return Err("Enable and approve the installed VSIX version in Plugins first.".into());
        }
        if self.plugins.installed.iter().filter(|record| eligible(record)
            && matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.commands.iter().any(|item| item.id == command)))
            .take(2).count() > 1 {
            return Err(format!("Multiple enabled VSIX extensions declare {command}"));
        }
        if self.plugins.vsix.is_none() {
            let path = runtime_path
                .ok_or("Code OSS runtime is unavailable. Use the portable Graphite package.")?;
            let store = self
                .plugins
                .store
                .as_ref()
                .ok_or("Plugin store is unavailable")?;
            let packages = self
                .plugins
                .installed
                .iter()
                .filter(|record| eligible(record))
                .map(|record| {
                    json!({
                        "id": record.metadata.id(),
                        "version": record.metadata.version(),
                        "directory": store.root.join(&record.directory).join("extension"),
                    })
                })
                .collect::<Vec<_>>();
            let proxy = self.event_proxy.clone();
            let root_uri = graphite_core_api::DocumentUri::from_path(self.tree.root())
                .map_err(|error| error.to_string())?;
            let session = Session::start(path, move || {
                if let Some(proxy) = &proxy {
                    let _ = proxy.send_event(UserEvent::ExtensionHostWake);
                }
            });
            session.send(json!({
                "type": "start", "packages": packages, "root": self.tree.root(), "rootUri": root_uri,
                "storage": self.profile_root.join("code-oss"), "locale": self.translator.language().code(),
                "configuration": self.vsix_configuration()?,
            }))?;
            self.plugins.vsix = Some(Runtime {
                session,
                ready: false,
                registered: BTreeSet::new(),
                command: None,
                sent: false,
                synced: None,
                prompt: None,
                languages: Default::default(),
                editor_commands: BTreeMap::new(),
                test_calls: BTreeSet::new(),
            });
        }
        let runtime = self.plugins.vsix.as_mut().unwrap();
        if runtime.command.is_some() {
            return Err("A VSIX command is already running".into());
        }
        runtime.command = Some((
            owner.to_owned(),
            command.to_owned(),
            Instant::now() + Duration::from_secs(30),
        ));
        runtime.sent = false;
        Ok(())
    }

    pub(in crate::app::plugins) fn stop_vsix(&mut self) {
        self.plugins.vsix_generation = self.plugins.vsix_generation.wrapping_add(1);
        if self.plugins.vsix.take().is_some() {
            // An open palette may still contain rows from the old session.
            self.command_palette = None;
            self.hover_text = None;
            self.completion_items.clear();
        }
    }

    pub(super) fn poll_vsix(&mut self) {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return;
        };
        if !self.vsix_workspace_trusted() {
            self.command_palette = None;
            return;
        }
        let generation = self.plugins.vsix_generation;
        match self.update_vsix(&mut runtime, generation) {
            Ok(()) if generation == self.plugins.vsix_generation => {
                self.plugins.vsix = Some(runtime)
            }
            Ok(()) => self.command_palette = None,
            Err(error) => {
                self.status = error;
                self.command_palette = None;
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
        }
    }

    fn update_vsix(&mut self, runtime: &mut Runtime, generation: u64) -> Result<()> {
        for _ in 0..128 {
            let Some(event) = runtime.session.receive() else {
                break;
            };
            let event = event?;
            match event["type"].as_str() {
                Some("ready") => runtime.ready = true,
                Some("request") => self.handle_vsix_request(runtime, &event)?,
                Some("cancel") => {
                    if let Some(id) = event["id"].as_u64() {
                        if let Some(ticket) = runtime.editor_commands.remove(&id) {
                            ticket.cancellation.cancel();
                        }
                    }
                    if runtime
                        .prompt
                        .as_ref()
                        .is_some_and(|prompt| event["id"].as_u64() == Some(prompt.id))
                    {
                        runtime.prompt = None;
                        self.command_palette = None;
                    }
                }
                Some("result") => {
                    if event["id"].as_u64() == Some(1) {
                        runtime.command = None;
                        if let Some(error) = event["error"].as_str() {
                            self.status = error.to_owned();
                        }
                    } else if event["id"]
                        .as_u64()
                        .is_some_and(|id| runtime.test_calls.remove(&id))
                    {
                        if let Some(run) = self.test_explorer.active_run {
                            self.test_explorer.finish_run(run);
                        }
                        if let Some(error) = event["error"].as_str() {
                            self.status = error.to_owned();
                        }
                    } else {
                        self.finish_vsix_language_request(runtime, &event)?;
                    }
                }
                Some("failure") => {
                    return Err(event["error"].as_str().unwrap_or("Code OSS failed").into())
                }
                _ => return Err("Unexpected Code OSS message".into()),
            }
            if generation != self.plugins.vsix_generation {
                return Ok(());
            }
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
        if runtime.prompt.is_none()
            && runtime
                .command
                .as_ref()
                .is_some_and(|(_, _, deadline)| Instant::now() >= *deadline)
        {
            return Err("Code OSS command timed out".into());
        }
        if runtime.ready {
            self.reply_vsix_editor_commands(runtime)?;
            runtime.languages.update(&runtime.session)?;
            self.sync_vsix_documents(runtime)?;
            if let (Some(handle), Some(webview)) =
                (self.webview_handle.as_deref(), self.webview.as_ref())
            {
                for message in webview.take_messages() {
                    let value =
                        serde_json::from_str::<Value>(&message).unwrap_or(Value::String(message));
                    runtime.session.send(json!({"type":"call","actor":"ExtHostWebviews","method":"$onMessage","args":[handle,value]}))?;
                }
            }
            if let Some((owner, command, _)) = &runtime.command {
                if !runtime.sent {
                    runtime.session.send(
                        json!({"type":"execute", "id":1, "owner":owner, "command":command}),
                    )?;
                    runtime.sent = true;
                }
            }
        }
        Ok(())
    }
}
