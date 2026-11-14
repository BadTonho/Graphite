use super::*;
use crate::app::command_palette::PaletteItem;
use graphite_core_api::DocumentService;
use graphite_extension_host::code_oss::Session;
use serde_json::{json, Value};
use std::{collections::BTreeSet, time::Duration};

mod auth;
mod code_lens;
mod colors;
mod debug;
pub(crate) mod decorations;
mod documents;
mod editors;
pub(crate) use editors::{EditorAction, EditorChoice, EditorOverlay, Kind as EditorKind};
mod env_lm;
mod file_events;
mod hierarchy;
pub(crate) use hierarchy::Kind as HierarchyKind;
mod language_results;
mod notebook_data;
mod notebooks;
pub(crate) use notebooks::{JUPYTER_TYPE, SPEC_PREFIX};
mod languages;
mod passive;
mod requested_commands;
pub(crate) use decorations::LineMark as DecorationMark;
pub(crate) use file_events::{changes as file_changes, FileChange};
pub(crate) use languages::Feature as VsixLanguageFeature;
mod prompts;
mod rpc;
mod scm;
pub(crate) use scm::{ExtensionRow, ScmProvider};
mod selection_ranges;
mod semantic;
mod storage;
mod tasks;
mod terminals;
mod testing;
#[cfg(test)]
mod tests;
mod text_search;
mod tree_views;
mod webview_views;
pub(crate) mod virtual_fs;
pub(crate) use webview_views::SidebarWebview;
mod webviews;
mod workbench;

pub(crate) struct Runtime {
    session: Session,
    ready: bool,
    registered: BTreeSet<String>,
    command: Option<(String, String, Instant)>,
    /// Arguments passed to the running command (for example a code lens's).
    command_args: Vec<Value>,
    sent: bool,
    synced: Option<Value>,
    prompt: Option<prompts::Prompt>,
    languages: languages::Languages,
    editor_commands: BTreeMap<u64, graphite_core_api::CommandTicket>,
    test_calls: BTreeSet<u64>,
    workbench: workbench::Workbench,
    terminals: terminals::Terminals,
    watches: file_events::Watches,
    decorations: decorations::Decorations,
    /// A debug adapter the extension host is being asked to create.
    debug_adapter: Option<debug::PendingAdapter>,
    /// Inlay hints and highlights that are requested automatically.
    passive: passive::Passive,
    /// A call or type hierarchy being collected.
    hierarchy: hierarchy::Hierarchies,
    /// Code lenses of the active document.
    lenses: code_lens::CodeLenses,
    /// Color swatches of the active document.
    colors: colors::Colors,
    /// Tree views registered via `MainThreadTreeViews`.
    tree_views: tree_views::TreeViews,
    /// Webview panels registered via `MainThreadWebviewPanels` / `MainThreadWebviews`.
    webviews: webviews::Webviews,
    /// Notebook serializers and controllers.
    notebooks: notebooks::Notebooks,
    /// Custom editors registered via `MainThreadCustomEditors`.
    editors: editors::CustomEditors,
    /// Authentication providers registered via `MainThreadAuthentication`.
    auth: auth::Auth,
    /// File system providers registered via `MainThreadFileSystem`.
    vfs: virtual_fs::VirtualFs,
    /// Source control providers registered via `MainThreadSCM`.
    scm: scm::Scm,
    /// Sidebar webview views registered via `MainThreadWebviewViews`.
    webview_views: webview_views::WebviewViews,
    /// Task providers and executions registered via `MainThreadTask`.
    tasks: tasks::Tasks,
    /// Language model tools registered via `MainThreadLanguageModelTools`.
    lm_tools: env_lm::LanguageModelTools,
    /// Activation events already sent to the VSIX extension host in this session.
    activated_events: BTreeSet<String>,
    /// Whether initial startup / workspaceContains activations have been dispatched.
    startup_activated: bool,
    /// Unsupported `actor.method` calls already logged this session.
    unsupported: BTreeSet<String>,
    /// Commands extensions asked for that Graphite does not have.
    requested: requested_commands::Requested,
}

impl Runtime {
    pub(super) fn pending(&self) -> bool {
        self.command.is_some()
            || self.languages.pending()
            || !self.editor_commands.is_empty()
            || !self.test_calls.is_empty()
            || self.debug_adapter.is_some()
            || self.passive.busy()
            || self.hierarchy.busy()
            || self.lenses.busy()
            || self.colors.busy()
            || self.tree_views.busy()
            || self.webview_views.busy()
            || self.vfs.busy()
            || self.auth.busy()
            || self.editors.busy()
            || self.notebooks.busy()
            || self.tasks.busy()
            || self.lm_tools.busy()
    }
}

#[cfg(test)]
impl Runtime {
    /// A ready runtime around `session`, for tests that play the extension host.
    pub(super) fn for_test(session: Session) -> Self {
        Self {
            session,
            ready: true,
            registered: BTreeSet::new(),
            command: None,
            command_args: Vec::new(),
            sent: false,
            synced: None,
            prompt: None,
            languages: Default::default(),
            editor_commands: BTreeMap::new(),
            test_calls: BTreeSet::new(),
            workbench: Default::default(),
            terminals: Default::default(),
            watches: Default::default(),
            decorations: Default::default(),
            debug_adapter: None,
            passive: Default::default(),
            hierarchy: Default::default(),
            lenses: Default::default(),
            colors: Default::default(),
            tree_views: Default::default(),
            webviews: Default::default(),
            webview_views: Default::default(),
            notebooks: Default::default(),
            editors: Default::default(),
            auth: Default::default(),
            vfs: Default::default(),
            scm: Default::default(),
            tasks: Default::default(),
            lm_tools: Default::default(),
            activated_events: BTreeSet::new(),
            startup_activated: false,
            unsupported: BTreeSet::new(),
            requested: Default::default(),
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for ticket in self.editor_commands.values() {
            ticket.cancellation.cancel();
        }
    }
}

pub(crate) fn eligible(record: &Installed) -> bool {
    matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.main.is_some())
        && record.enabled
        && record.trusted_version.as_deref() == Some(record.metadata.version())
}

impl App {
    pub(crate) fn vsix_workspace_trusted(&self) -> bool {
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

    /// Like [`Self::run_vsix_command`], passing `args` to the command.
    pub(crate) fn run_vsix_command_with_args(&mut self, key: &str, args: Vec<Value>) -> Result<()> {
        self.run_vsix_command(key)?;
        if let Some(runtime) = self.plugins.vsix.as_mut() {
            runtime.command_args = args;
        }
        Ok(())
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
            self.start_vsix_host(runtime_path)?;
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
        runtime.command_args = Vec::new();
        Ok(())
    }

    /// Starts the Code OSS extension host and loads the approved extensions into it.
    pub(crate) fn start_vsix_host(&mut self, runtime_path: Option<PathBuf>) -> Result<()> {
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
            command_args: Vec::new(),
            sent: false,
            synced: None,
            prompt: None,
            languages: Default::default(),
            editor_commands: BTreeMap::new(),
            test_calls: BTreeSet::new(),
            workbench: Default::default(),
            terminals: Default::default(),
            watches: Default::default(),
            decorations: Default::default(),
            debug_adapter: None,
            passive: Default::default(),
            hierarchy: Default::default(),
            lenses: Default::default(),
            colors: Default::default(),
            tree_views: Default::default(),
            webviews: Default::default(),
            webview_views: Default::default(),
            notebooks: Default::default(),
            editors: Default::default(),
            auth: Default::default(),
            vfs: Default::default(),
            scm: Default::default(),
            tasks: Default::default(),
            lm_tools: Default::default(),
            activated_events: BTreeSet::new(),
            startup_activated: false,
            unsupported: BTreeSet::new(),
            requested: Default::default(),
        });
        Ok(())
    }

    pub(in crate::app::plugins) fn stop_vsix(&mut self) {
        self.plugins.vsix_generation = self.plugins.vsix_generation.wrapping_add(1);
        if self.plugins.vsix.take().is_some() {
            // An open palette may still contain rows from the old session.
            self.command_palette = None;
            self.hover_text = None;
            self.completion_items.clear();
            self.clear_vsix_tree_views();
            self.stop_sidebar_webviews();
            self.scm_extension_input = None;
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
        self.plugins.vsix_polling = true;
        let outcome = self.update_vsix(&mut runtime, generation);
        self.plugins.vsix_polling = false;
        match outcome {
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
                    if event["id"].as_u64() == Some(debug::ADAPTER_CALL_ID) {
                        self.finish_vsix_debug_adapter(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(colors::is_color_reply) {
                        self.finish_vsix_colors(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(code_lens::is_lens_reply) {
                        self.finish_vsix_code_lens(runtime, &event);
                    } else if event["id"]
                        .as_u64()
                        .is_some_and(hierarchy::is_hierarchy_reply)
                    {
                        self.finish_vsix_hierarchy(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(passive::is_passive_reply) {
                        self.finish_vsix_passive(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(tree_views::is_tree_reply) {
                        self.finish_vsix_tree(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(notebooks::is_notebook_reply) {
                        self.finish_vsix_notebook(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(editors::is_editor_reply) {
                        self.finish_vsix_custom_editor(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(auth::is_auth_reply) {
                        self.finish_vsix_auth(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(virtual_fs::is_fs_reply) {
                        self.finish_vsix_virtual_fs(runtime, &event);
                    } else if event["id"]
                        .as_u64()
                        .is_some_and(webview_views::is_view_reply)
                    {
                        self.finish_vsix_webview_view(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(tasks::is_task_reply) {
                        self.finish_vsix_task(runtime, &event);
                    } else if event["id"].as_u64().is_some_and(env_lm::is_lm_tool_reply) {
                        self.finish_vsix_lm_tool(runtime, &event);
                    } else if event["id"].as_u64() == Some(1) {
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
            self.trigger_vsix_startup_activations(runtime);
            self.reply_vsix_editor_commands(runtime)?;
            self.drive_vsix_debug_adapter(runtime)?;
            runtime.languages.update(&runtime.session)?;
            self.sync_vsix_documents(runtime)?;
            self.drive_vsix_passive(runtime)?;
            self.expire_vsix_hierarchy(runtime);
            self.drive_vsix_code_lens(runtime)?;
            self.drive_vsix_colors(runtime)?;
            self.expire_vsix_trees(runtime);
            runtime.vfs.expire(Instant::now());
            self.drive_vsix_auth(runtime);
            self.drive_vsix_custom_editors(runtime);
            self.drive_vsix_notebooks(runtime);
            self.poll_vsix_tasks(runtime);
            self.refresh_vsix_outputs(runtime);
            self.sync_vsix_terminals(runtime)?;
            self.sync_vsix_webview_host(runtime)?;
            if let Some((owner, command, _)) = &runtime.command {
                if !runtime.sent {
                    runtime.session.send(
                        json!({"type":"execute", "id":1, "owner":owner, "command":command, "args":runtime.command_args}),
                    )?;
                    runtime.sent = true;
                }
            }
        }
        Ok(())
    }
}
