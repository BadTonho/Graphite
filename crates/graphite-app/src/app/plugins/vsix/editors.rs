//! Custom editors (`MainThreadCustomEditors` / `ExtHostCustomEditors`).
//!
//! An extension contributes a custom editor for some files (`contributes.customEditors`) and
//! registers a provider for it. Graphite opens the file as a document as usual and hides the text
//! editor behind the extension's webview, hosted in an overlay over the editor group. Two kinds
//! exist:
//!
//! * A **text** editor works on the open text document, so saving, undo and redo are the
//!   document's own.
//! * A **custom** editor owns the document: edits it reports make the tab dirty, and Save, Undo
//!   and Redo are sent to the extension.

use super::webviews::{parse_webview_options, uri_value_to_path, VsixWebviewPanel, DEFAULT_HTML};
use super::*;
use graphite_core_api::DocumentUri;
use graphite_plugins::manifest::CustomEditorContribution;
use serde_json::{json, Value};

/// Replies to custom editor calls carry ids in `EDITOR_BASE..EDITOR_END`.
pub(super) const EDITOR_BASE: u64 = 17_000_000;
const EDITOR_END: u64 = 18_000_000;
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn is_editor_reply(id: u64) -> bool {
    (EDITOR_BASE..EDITOR_END).contains(&id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Text,
    Custom,
}

#[derive(Clone, Debug)]
struct Provider {
    kind: Kind,
    extension_root: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Bound to a file; waiting for the provider to register.
    Pending,
    /// A custom document is being opened.
    Creating,
    /// The provider is filling the webview.
    Resolving,
    Ready,
}

/// A file shown with an extension's custom editor.
#[derive(Clone, Debug)]
pub(crate) struct Instance {
    pub view_type: String,
    /// The webview handle of the editor's page.
    pub handle: String,
    /// The document's URI as the extension knows it.
    pub resource: String,
    pub kind: Kind,
    phase: Phase,
    deadline: Instant,
    /// A custom document the extension can save.
    pub editable: bool,
    /// Ids of the edits the extension reported, oldest first.
    edits: Vec<u64>,
    /// How many of `edits` are applied (the rest can be redone).
    cursor: usize,
    /// The `cursor` the document was last saved at; `None` once no undo or redo reaches it.
    save_point: Option<usize>,
    /// Whether the document was read-only before a custom editor took it over.
    was_read_only: bool,
}

impl Instance {
    pub(crate) fn is_ready(&self) -> bool {
        self.phase == Phase::Ready
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.kind == Kind::Custom && self.editable && self.save_point != Some(self.cursor)
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.cursor < self.edits.len()
    }

    /// Records an edit; returns the redoable edits it made obsolete.
    fn record_edit(&mut self, id: u64) -> Vec<u64> {
        let discarded = self.edits.split_off(self.cursor);
        if self.save_point.is_some_and(|point| point > self.cursor) {
            self.save_point = None;
        }
        self.edits.push(id);
        self.cursor += 1;
        discarded
    }

    /// The edit to undo, with the document being dirty afterwards.
    fn undo(&mut self) -> Option<(u64, bool)> {
        let id = *self.edits.get(self.cursor.checked_sub(1)?)?;
        self.cursor -= 1;
        Some((id, self.is_dirty()))
    }

    fn redo(&mut self) -> Option<(u64, bool)> {
        let id = *self.edits.get(self.cursor)?;
        self.cursor += 1;
        Some((id, self.is_dirty()))
    }

    fn saved(&mut self) {
        self.save_point = Some(self.cursor);
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Create(PathBuf),
    Resolve(PathBuf),
    Save(PathBuf),
    Other(&'static str),
}

#[derive(Default)]
pub(super) struct CustomEditors {
    next_id: u64,
    next_handle: u64,
    providers: BTreeMap<String, Provider>,
    instances: BTreeMap<PathBuf, Instance>,
    pending: BTreeMap<u64, (Op, Instant)>,
}

impl CustomEditors {
    pub(super) fn busy(&self) -> bool {
        !self.pending.is_empty()
    }

    fn next_id(&mut self) -> u64 {
        let id = EDITOR_BASE + self.next_id;
        self.next_id = (self.next_id + 1) % (EDITOR_END - EDITOR_BASE);
        id
    }

    pub(crate) fn instance(&self, path: &Path) -> Option<&Instance> {
        self.instances.get(path)
    }

    fn path_for_resource(&self, resource: &str, view_type: &str) -> Option<PathBuf> {
        self.instances
            .iter()
            .find(|(_, instance)| instance.resource == resource && instance.view_type == view_type)
            .map(|(path, _)| path.clone())
    }
}

/// A custom editor that can open a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EditorChoice {
    pub view_type: String,
    pub display_name: String,
    pub is_default: bool,
}

/// What a command asks of the custom editor in front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorAction {
    Save,
    Undo,
    Redo,
}

/// What the interface needs to know about the custom editor of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EditorState {
    pub ready: bool,
    pub kind: Kind,
    pub display_name: String,
    pub writable: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

/// A native overlay hosting the page of one custom editor.
pub(crate) struct EditorOverlay {
    pub host: crate::webview::WebviewHost,
    pub html: String,
    pub scripts: bool,
    pub bounds: [f64; 4],
}

pub(super) fn resource_of(path: &Path) -> Option<String> {
    super::virtual_fs::uri_for_path(path)
        .or_else(|| DocumentUri::from_path(path).ok())
        .map(|uri| uri.as_str().to_owned())
}

fn uri_argument(uri: &str) -> Value {
    json!({ "$graphiteUri": uri })
}

impl App {
    pub(super) fn handle_vsix_custom_editor_request(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        let text = |index: usize| args.get(index).and_then(Value::as_str);
        match method {
            "$registerTextEditorProvider" | "$registerCustomEditorProvider" => {
                let view_type = text(1).ok_or("Invalid custom editor type")?;
                let kind = if method == "$registerTextEditorProvider" {
                    Kind::Text
                } else {
                    Kind::Custom
                };
                runtime.editors.providers.insert(
                    view_type.to_owned(),
                    Provider {
                        kind,
                        extension_root: args
                            .first()
                            .and_then(|extension| uri_value_to_path(&extension["location"])),
                    },
                );
                Ok(Value::Null)
            }
            "$unregisterEditorProvider" => {
                let view_type = text(0).ok_or("Invalid custom editor type")?.to_owned();
                runtime.editors.providers.remove(&view_type);
                let paths: Vec<PathBuf> = runtime
                    .editors
                    .instances
                    .iter()
                    .filter(|(_, instance)| instance.view_type == view_type)
                    .map(|(path, _)| path.clone())
                    .collect();
                for path in paths {
                    self.release_custom_editor(runtime, &path);
                }
                Ok(Value::Null)
            }
            "$onDidEdit" => {
                let (resource, view_type) =
                    (text(0).unwrap_or_default(), text(1).unwrap_or_default());
                let id = args
                    .get(2)
                    .and_then(Value::as_u64)
                    .ok_or("Invalid edit id")?;
                if let Some(path) = runtime.editors.path_for_resource(resource, view_type) {
                    let discarded = runtime
                        .editors
                        .instances
                        .get_mut(&path)
                        .map(|instance| instance.record_edit(id))
                        .unwrap_or_default();
                    if !discarded.is_empty() {
                        let _ = runtime.session.send(json!({
                            "type": "call",
                            "actor": "ExtHostCustomEditors",
                            "method": "$disposeEdits",
                            "args": [uri_argument(resource), view_type, discarded],
                        }));
                    }
                }
                Ok(Value::Null)
            }
            "$onContentChange" | "$onDidChangeCustomTextEditorSelection" => Ok(Value::Null),
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadCustomEditors", method);
                Err(format!(
                    "Graphite does not support MainThreadCustomEditors.{method}"
                ))
            }
        }
    }

    /// Sends a call to the extension and remembers what to do with its answer.
    fn editor_call(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: Vec<Value>,
        cancellable: bool,
        op: Op,
    ) -> Result<u64> {
        let id = runtime.editors.next_id();
        runtime
            .editors
            .pending
            .insert(id, (op, Instant::now() + RESOLVE_TIMEOUT));
        let sent = runtime.session.send(json!({
            "type": "call",
            "id": id,
            "actor": "ExtHostCustomEditors",
            "method": method,
            "args": args,
            "cancellable": cancellable,
        }));
        if let Err(error) = sent {
            runtime.editors.pending.remove(&id);
            return Err(error);
        }
        Ok(id)
    }

    /// Moves bound editors forward: opens custom documents, asks providers to fill the webview,
    /// and gives up on a provider that never answers.
    pub(super) fn drive_vsix_custom_editors(&mut self, runtime: &mut Runtime) {
        let now = Instant::now();
        runtime
            .editors
            .pending
            .retain(|_, (_, deadline)| *deadline > now);
        let paths: Vec<PathBuf> = runtime.editors.instances.keys().cloned().collect();
        for path in paths {
            let Some(instance) = runtime.editors.instances.get(&path).cloned() else {
                continue;
            };
            if instance.phase != Phase::Ready && instance.deadline <= now {
                self.status = format!(
                    "{}: the extension did not open the editor",
                    instance.view_type
                );
                self.release_custom_editor(runtime, &path);
                continue;
            }
            if instance.phase != Phase::Pending {
                continue;
            }
            let Some(provider) = runtime.editors.providers.get(&instance.view_type).cloned() else {
                continue;
            };
            let resource = uri_argument(&instance.resource);
            let document = self.documents.id_for_path(&path);
            let was_read_only = document.is_some_and(|id| self.documents[id].buffer.is_read_only());
            if provider.kind == Kind::Custom {
                if let Some(id) = document {
                    self.documents[id].buffer.set_read_only(true);
                }
            }
            if let Some(entry) = runtime.editors.instances.get_mut(&path) {
                entry.kind = provider.kind;
                entry.phase = Phase::Creating;
                entry.was_read_only = was_read_only;
            }
            let sent = if provider.kind == Kind::Custom {
                self.editor_call(
                    runtime,
                    "$createCustomDocument",
                    vec![
                        resource,
                        json!(instance.view_type),
                        Value::Null,
                        Value::Null,
                    ],
                    true,
                    Op::Create(path.clone()),
                )
                .map(|_| ())
            } else {
                self.resolve_custom_editor(runtime, &path)
            };
            if let Err(error) = sent {
                self.status = error;
                self.release_custom_editor(runtime, &path);
            }
        }
    }

    /// Creates the page of an editor and asks its provider to fill it.
    fn resolve_custom_editor(&mut self, runtime: &mut Runtime, path: &Path) -> Result<()> {
        let Some(instance) = runtime.editors.instances.get_mut(path) else {
            return Ok(());
        };
        instance.phase = Phase::Resolving;
        instance.deadline = Instant::now() + RESOLVE_TIMEOUT;
        let (view_type, resource, handle) = (
            instance.view_type.clone(),
            instance.resource.clone(),
            instance.handle.clone(),
        );
        let extension_root = runtime
            .editors
            .providers
            .get(&view_type)
            .and_then(|provider| provider.extension_root.clone());
        let (enable_scripts, roots) =
            parse_webview_options(&Value::Null, extension_root.as_deref(), self.tree.root());
        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        runtime.webviews.panels.insert(
            handle.clone(),
            VsixWebviewPanel {
                handle: handle.clone(),
                view_type: view_type.clone(),
                title: title.clone(),
                html: DEFAULT_HTML.to_owned(),
                enable_scripts,
                local_resource_roots: roots.unwrap_or_default(),
                extension_root,
                outbound_messages: Vec::new(),
                visible: true,
            },
        );
        self.editor_call(
            runtime,
            "$resolveCustomEditor",
            vec![
                uri_argument(&resource),
                json!(handle),
                json!(view_type),
                json!({"title": title, "contentOptions": {}, "options": {}, "active": true}),
                json!(1),
            ],
            true,
            Op::Resolve(path.to_path_buf()),
        )
        .map(|_| ())
    }

    pub(super) fn finish_vsix_custom_editor(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let Some((op, _)) = runtime.editors.pending.remove(&id) else {
            return;
        };
        let error = event["error"].as_str().map(str::to_owned);
        match op {
            Op::Create(path) => match error {
                Some(error) => {
                    self.status = error;
                    self.release_custom_editor(runtime, &path);
                }
                None => {
                    if let Some(instance) = runtime.editors.instances.get_mut(&path) {
                        instance.editable = event["result"]["editable"].as_bool().unwrap_or(false);
                    }
                    if let Err(error) = self.resolve_custom_editor(runtime, &path) {
                        self.status = error;
                        self.release_custom_editor(runtime, &path);
                    }
                }
            },
            Op::Resolve(path) => match error {
                Some(error) => {
                    self.status = error;
                    self.release_custom_editor(runtime, &path);
                }
                None => {
                    if let Some(instance) = runtime.editors.instances.get_mut(&path) {
                        instance.phase = Phase::Ready;
                    }
                }
            },
            Op::Save(path) => match error {
                Some(error) => self.status = error,
                None => {
                    if let Some(instance) = runtime.editors.instances.get_mut(&path) {
                        instance.saved();
                    }
                    self.status = self.t("custom_editor.saved").to_owned();
                }
            },
            Op::Other(what) => {
                if let Some(error) = error {
                    self.status = format!("{what}: {error}");
                }
            }
        }
    }

    /// Ends the custom editor of `path`: the page goes away, the extension is told, and the file
    /// shows as text again.
    fn release_custom_editor(&mut self, runtime: &mut Runtime, path: &Path) {
        let Some(instance) = runtime.editors.instances.remove(path) else {
            return;
        };
        runtime.webviews.panels.remove(&instance.handle);
        self.custom_editor_webviews.remove(&instance.handle);
        let _ = runtime.session.send(json!({
            "type": "call",
            "actor": "ExtHostWebviewPanels",
            "method": "$onDidDisposeWebviewPanel",
            "args": [instance.handle],
        }));
        if instance.kind == Kind::Custom && instance.phase != Phase::Pending {
            let _ = runtime.session.send(json!({
                "type": "call",
                "actor": "ExtHostCustomEditors",
                "method": "$disposeCustomDocument",
                "args": [uri_argument(&instance.resource), instance.view_type],
            }));
        }
        if let Some(id) = self.documents.id_for_path(path) {
            self.documents[id].custom_dirty = false;
            if instance.kind == Kind::Custom && instance.phase != Phase::Pending {
                self.documents[id]
                    .buffer
                    .set_read_only(instance.was_read_only);
            }
        }
    }

    /// The custom editors that offer to open `path`, from the enabled extensions.
    pub(crate) fn custom_editor_choices(&self, path: &Path) -> Vec<EditorChoice> {
        self.plugins
            .installed
            .iter()
            .filter(|record| super::eligible(record))
            .filter_map(|record| match &record.metadata {
                Metadata::Vsix(vsix) => Some(&vsix.custom_editors),
                _ => None,
            })
            .flatten()
            .filter(|editor: &&CustomEditorContribution| editor.matches(path))
            .map(|editor| EditorChoice {
                view_type: editor.view_type.clone(),
                display_name: editor.display_name.clone(),
                is_default: editor.is_default(),
            })
            .collect()
    }

    /// The editor files open with when the user did not choose one.
    pub(crate) fn default_custom_editor(&self, path: &Path) -> Option<EditorChoice> {
        if self.custom_editor_text_paths.contains(path)
            || !self.vsix_workspace_trusted()
            || self.plugins.vsix_polling
        {
            return None;
        }
        self.custom_editor_choices(path)
            .into_iter()
            .find(|choice| choice.is_default)
    }

    /// Whether `path` is shown with a custom editor.
    pub(crate) fn custom_editor_is_bound(&self, path: &Path) -> bool {
        self.plugins
            .vsix
            .as_ref()
            .is_some_and(|runtime| runtime.editors.instance(path).is_some())
    }

    /// Shows the open file `path` with a custom editor.
    pub(crate) fn bind_custom_editor(&mut self, path: &Path, view_type: &str) {
        if self.documents.id_for_path(path).is_none() {
            return;
        }
        let Some(resource) = resource_of(path) else {
            return;
        };
        self.custom_editor_text_paths.remove(path);
        self.trigger_vsix_activation_event(&format!("onCustomEditor:{view_type}"));
        let Some(runtime) = self.plugins.vsix.as_mut() else {
            self.status = self.t("custom_editor.unavailable").to_owned();
            return;
        };
        runtime.editors.next_handle += 1;
        let handle = format!("graphite-editor-{}", runtime.editors.next_handle);
        runtime.editors.instances.insert(
            path.to_path_buf(),
            Instance {
                view_type: view_type.to_owned(),
                handle,
                resource,
                kind: Kind::Custom,
                phase: Phase::Pending,
                deadline: Instant::now() + RESOLVE_TIMEOUT,
                editable: false,
                edits: Vec::new(),
                cursor: 0,
                save_point: Some(0),
                was_read_only: false,
            },
        );
    }

    /// Shows the open file `path` as text again.
    pub(crate) fn unbind_custom_editor(&mut self, path: &Path) {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return;
        };
        self.release_custom_editor(&mut runtime, path);
        self.plugins.vsix = Some(runtime);
    }

    /// Opens `path` with the editor `view_type`, or as text when it is empty.
    pub(crate) fn reopen_with(&mut self, view_type: &str) {
        let Some(path) = self.documents[self.active_document()]
            .buffer
            .path()
            .map(Path::to_path_buf)
        else {
            self.status = self.t("custom_editor.no_file").to_owned();
            return;
        };
        self.unbind_custom_editor(&path);
        if view_type.is_empty() {
            self.custom_editor_text_paths.insert(path);
        } else {
            self.bind_custom_editor(&path, view_type);
        }
    }

    /// Lists the editors that can open the active file, for "Reopen Editor With…".
    pub(crate) fn open_with_palette(&mut self) {
        let Some(path) = self.documents[self.active_document()]
            .buffer
            .path()
            .map(Path::to_path_buf)
        else {
            self.status = self.t("custom_editor.no_file").to_owned();
            return;
        };
        let item = |id: String, title: String| crate::app::command_palette::PaletteItem {
            id,
            title,
            source: "VSIX".into(),
            enabled: true,
        };
        let mut items = vec![item(
            "graphite.openWith:".into(),
            self.t("custom_editor.text_editor").to_owned(),
        )];
        items.extend(self.custom_editor_choices(&path).into_iter().map(|choice| {
            item(
                format!("graphite.openWith:{}", choice.view_type),
                choice.display_name,
            )
        }));
        self.open_command_palette();
        self.command_palette = Some(crate::app::command_palette::CommandPalette {
            query: String::new(),
            selected: 0,
            items,
            extension_prompt: None,
        });
    }

    /// What the interface shows for the custom editor of `document`, if it has one.
    pub(crate) fn custom_editor_state(&self, document: DocumentId) -> Option<EditorState> {
        let path = self.documents[document].buffer.path()?;
        let runtime = self.plugins.vsix.as_ref()?;
        let instance = runtime.editors.instance(path)?;
        let display_name = self
            .custom_editor_choices(path)
            .into_iter()
            .find(|choice| choice.view_type == instance.view_type)
            .map_or_else(|| instance.view_type.clone(), |choice| choice.display_name);
        Some(EditorState {
            ready: instance.is_ready(),
            kind: instance.kind,
            display_name,
            writable: instance.kind == Kind::Custom && instance.is_ready() && instance.editable,
            can_undo: instance.can_undo(),
            can_redo: instance.can_redo(),
        })
    }

    /// Sends Save, Undo or Redo to the custom editor in front. Returns false when the active
    /// document does not have one that handles it, so the normal command runs.
    pub(crate) fn route_custom_editor_action(&mut self, action: EditorAction) -> bool {
        let Some(path) = self.documents[self.active_document()]
            .buffer
            .path()
            .map(Path::to_path_buf)
        else {
            return false;
        };
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return false;
        };
        let handled = self.route_custom_editor_action_in(&mut runtime, &path, action);
        self.plugins.vsix = Some(runtime);
        handled
    }

    fn route_custom_editor_action_in(
        &mut self,
        runtime: &mut Runtime,
        path: &Path,
        action: EditorAction,
    ) -> bool {
        let Some(instance) = runtime.editors.instances.get_mut(path) else {
            return false;
        };
        if instance.kind != Kind::Custom || !instance.is_ready() || !instance.editable {
            return false;
        }
        let (resource, view_type) = (instance.resource.clone(), instance.view_type.clone());
        let (method, extra, op) = match action {
            EditorAction::Save => ("$onSave", Vec::new(), Op::Save(path.to_path_buf())),
            EditorAction::Undo => match instance.undo() {
                Some((id, dirty)) => ("$undo", vec![json!(id), json!(dirty)], Op::Other("undo")),
                None => return true,
            },
            EditorAction::Redo => match instance.redo() {
                Some((id, dirty)) => ("$redo", vec![json!(id), json!(dirty)], Op::Other("redo")),
                None => return true,
            },
        };
        let mut args = vec![uri_argument(&resource), json!(view_type)];
        args.extend(extra);
        let cancellable = action == EditorAction::Save;
        if let Err(error) = self.editor_call(runtime, method, args, cancellable, op) {
            self.status = error;
        }
        true
    }

    /// Hosts the pages of ready editors over their editor groups and keeps tab state in step.
    /// Runs every frame of the event loop.
    pub(crate) fn sync_custom_editor_webviews(&mut self) {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            self.custom_editor_webviews.clear();
            return;
        };
        self.sync_custom_editor_webviews_with(&mut runtime);
        self.plugins.vsix = Some(runtime);
    }

    fn sync_custom_editor_webviews_with(&mut self, runtime: &mut Runtime) {
        // A closed document ends its editor.
        let closed: Vec<PathBuf> = runtime
            .editors
            .instances
            .keys()
            .filter(|path| self.documents.id_for_path(path).is_none())
            .cloned()
            .collect();
        for path in closed {
            self.release_custom_editor(runtime, &path);
        }
        // Tabs show the extension's unsaved edits.
        for (path, instance) in &runtime.editors.instances {
            if let Some(id) = self.documents.id_for_path(path) {
                self.documents[id].custom_dirty = instance.is_dirty();
            }
        }
        let Some((window, width, height, lh)) = self.gpu.as_ref().map(|gpu| {
            let (width, height) = gpu.renderer.size();
            (
                gpu.window.clone(),
                width,
                height,
                gpu.renderer.line_height(),
            )
        }) else {
            return;
        };
        let overlays_open = self.command_palette.is_some();
        let mut shown: BTreeSet<String> = BTreeSet::new();
        if !overlays_open {
            let tab_height = self.editor_tab_strip_height(lh);
            for rect in self.group_rectangles(width, height, lh) {
                let Some(path) = self
                    .editor_groups
                    .views(rect.id)
                    .and_then(|(views, active)| views.get(active))
                    .and_then(|view| self.documents[view.document].buffer.path())
                else {
                    continue;
                };
                let Some(instance) = runtime.editors.instances.get(path).filter(|i| i.is_ready())
                else {
                    continue;
                };
                let Some(panel) = runtime.webviews.panels.get(&instance.handle) else {
                    continue;
                };
                let bounds = [
                    f64::from(rect.x),
                    f64::from(rect.y + tab_height),
                    f64::from(rect.width),
                    f64::from((rect.height - tab_height).max(0.0)),
                ];
                let (html, scripts) = (panel.html.clone(), panel.enable_scripts);
                let handle = instance.handle.clone();
                shown.insert(handle.clone());
                if let Some(overlay) = self.custom_editor_webviews.get_mut(&handle) {
                    if overlay.html == html && overlay.scripts == scripts {
                        let _ = overlay.host.set_visible(true);
                        if overlay.bounds != bounds || cfg!(target_os = "linux") {
                            let _ = overlay.host.set_overlay_bounds(&window, bounds);
                            overlay.bounds = bounds;
                        }
                        overlay.host.pump();
                        continue;
                    }
                }
                match crate::webview::WebviewHost::open_extension_overlay(
                    &window, &html, bounds, scripts,
                ) {
                    Ok(host) => {
                        self.custom_editor_webviews.insert(
                            handle,
                            EditorOverlay {
                                host,
                                html,
                                scripts,
                                bounds,
                            },
                        );
                    }
                    Err(error) => self.status = error,
                }
            }
        }
        // An editor behind another tab keeps its page but is hidden.
        for (handle, overlay) in &self.custom_editor_webviews {
            if !shown.contains(handle) {
                let _ = overlay.host.set_visible(false);
            }
        }
        let live: BTreeSet<String> = runtime
            .editors
            .instances
            .values()
            .map(|instance| instance.handle.clone())
            .collect();
        self.custom_editor_webviews
            .retain(|handle, _| live.contains(handle));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(kind: Kind) -> Instance {
        Instance {
            view_type: "sample.paint".into(),
            handle: "h".into(),
            resource: "file:///a.png".into(),
            kind,
            phase: Phase::Ready,
            deadline: Instant::now(),
            editable: true,
            edits: Vec::new(),
            cursor: 0,
            save_point: Some(0),
            was_read_only: false,
        }
    }

    #[test]
    fn edits_make_the_document_dirty_and_undo_redo_walk_the_stack() {
        let mut editor = instance(Kind::Custom);
        assert!(!editor.is_dirty() && !editor.can_undo() && !editor.can_redo());
        assert!(editor.record_edit(10).is_empty());
        assert!(editor.record_edit(11).is_empty());
        assert!(editor.is_dirty() && editor.can_undo());
        assert_eq!(editor.undo(), Some((11, true)));
        assert_eq!(editor.undo(), Some((10, false)), "back at the saved state");
        assert!(editor.can_redo() && !editor.can_undo());
        assert_eq!(editor.undo(), None);
        assert_eq!(editor.redo(), Some((10, true)));
        // A new edit drops what could still be redone.
        assert_eq!(editor.record_edit(12), vec![11]);
        assert_eq!(editor.undo(), Some((12, true)), "edit 10 is still applied");
        assert_eq!(editor.record_edit(13), vec![12]);
    }

    #[test]
    fn saving_moves_the_clean_point_and_a_discarded_one_stays_dirty() {
        let mut editor = instance(Kind::Custom);
        editor.record_edit(1);
        editor.record_edit(2);
        editor.saved();
        assert!(!editor.is_dirty());
        editor.undo();
        assert!(editor.is_dirty());
        editor.redo();
        assert!(!editor.is_dirty());
        // Undoing past the save point and then editing makes the saved state unreachable.
        editor.undo();
        editor.undo();
        assert_eq!(
            editor.record_edit(3),
            vec![1, 2],
            "the old redo edits are discarded"
        );
        assert!(editor.is_dirty());
        editor.undo();
        assert!(editor.is_dirty(), "the saved state cannot be reached again");
    }

    #[test]
    fn text_and_read_only_editors_never_report_their_own_dirty_state() {
        let mut text = instance(Kind::Text);
        text.record_edit(1);
        assert!(!text.is_dirty(), "a text editor uses the document's state");
        let mut fixed = instance(Kind::Custom);
        fixed.editable = false;
        fixed.record_edit(1);
        assert!(!fixed.is_dirty());
    }
}
