//! Native Graphite plugin TreeDataProvider views, WebviewPanel hosting, menus, and keybindings.
use super::*;
use graphite_plugins::manifest::Metadata;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub(crate) struct NativeTreeView {
    pub owner: String,
    pub children: BTreeMap<Option<String>, Vec<TreeItem>>,
    pub expanded: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FlattenedTreeRow {
    pub view_id: String,
    pub depth: usize,
    pub item: TreeItem,
}

#[derive(Clone, Debug)]
pub(crate) struct NativeWebviewPanel {
    pub owner: String,
    pub handle: String,
    pub view_type: String,
    pub title: String,
    pub html: String,
    pub outbound_messages: Vec<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PluginMenuLocation {
    CommandPalette,
    EditorContext,
    ViewItemContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedPluginMenuItem {
    pub plugin_id: String,
    pub command: String,
    pub title: String,
    pub group: Option<String>,
    pub enabled: bool,
}

pub(crate) struct PendingActivationCommand {
    pub plugin_id: String,
    pub request: CommandRequest,
    pub cancellation: CancellationToken,
    pub sender: std::sync::mpsc::Sender<Result<Value, CommandError>>,
}

impl App {
    fn current_plugin_id(&self) -> Option<String> {
        self.extension_host
            .native_manifest
            .as_ref()
            .map(|manifest| manifest.id.clone())
    }

    fn plugin_runtime_mut(&mut self, plugin_id: &str) -> Option<&mut ExtensionRuntime> {
        if self
            .extension_host
            .native_manifest
            .as_ref()
            .is_some_and(|manifest| manifest.id == plugin_id)
        {
            return Some(&mut self.extension_host);
        }
        self.plugins.hosts.get_mut(plugin_id)
    }

    pub(crate) fn handle_native_ui_service(
        &mut self,
        request: ServiceRequest,
    ) -> Option<ServiceResult<Value>> {
        match request {
            ServiceRequest::RegisterTreeDataProvider { view_id } => Some(
                self.register_native_tree_provider(&view_id)
                    .map(|()| Value::Null),
            ),
            ServiceRequest::RefreshTreeView {
                view_id,
                element_id,
            } => Some(
                self.refresh_native_tree_view(&view_id, element_id)
                    .map(|()| Value::Null),
            ),
            ServiceRequest::CreateWebviewPanel {
                handle,
                view_type,
                title,
                html,
            } => Some(
                self.create_native_webview_panel(handle, view_type, title, html)
                    .map(|()| Value::Null),
            ),
            ServiceRequest::SetWebviewHtml { handle, html } => Some(
                self.set_native_webview_html(&handle, html)
                    .map(|()| Value::Null),
            ),
            ServiceRequest::SetWebviewTitle { handle, title } => Some(
                self.set_native_webview_title(&handle, title)
                    .map(|()| Value::Null),
            ),
            ServiceRequest::PostWebviewMessage { handle, message } => Some(
                self.post_native_webview_message(&handle, message)
                    .map(|()| Value::Null),
            ),
            ServiceRequest::DisposeWebviewPanel { handle } => Some(
                self.dispose_native_webview_panel(&handle)
                    .map(|()| Value::Null),
            ),
            _ => None,
        }
    }

    fn register_native_tree_provider(&mut self, view_id: &str) -> ServiceResult<()> {
        let Some(manifest) = &self.extension_host.native_manifest else {
            return Err(ServiceError::Failed(
                "TreeDataProvider requires an active native plugin".into(),
            ));
        };
        let declared = manifest
            .activity_bar
            .iter()
            .flat_map(|c| &c.views)
            .any(|v| v.id == view_id)
            || view_id.starts_with(&format!("{}.", manifest.id));
        if !declared {
            return Err(ServiceError::InvalidArguments(
                "Tree view ID must belong to the active plugin".into(),
            ));
        }
        let owner = manifest.id.clone();
        self.plugins.native_tree_views.insert(
            view_id.to_owned(),
            NativeTreeView {
                owner,
                children: BTreeMap::new(),
                expanded: BTreeSet::new(),
            },
        );
        if self.extension_host.native_active {
            self.request_native_tree_children(view_id, None);
        }
        Ok(())
    }

    pub(crate) fn on_native_plugin_activated(&mut self) {
        let Some(owner) = self.current_plugin_id() else {
            return;
        };
        let pending_views: Vec<String> = self
            .plugins
            .native_tree_views
            .iter()
            .filter(|(_, view)| view.owner == owner && view.children.is_empty())
            .map(|(view_id, _)| view_id.clone())
            .collect();
        for view_id in pending_views {
            self.request_native_tree_children(&view_id, None);
        }
    }

    pub(crate) fn refresh_native_tree_view(
        &mut self,
        view_id: &str,
        element_id: Option<String>,
    ) -> ServiceResult<()> {
        let Some(owner) = self.current_plugin_id() else {
            return Err(ServiceError::HostStopped);
        };
        let Some(tree) = self.plugins.native_tree_views.get_mut(view_id) else {
            return Err(ServiceError::NotFound);
        };
        if tree.owner != owner {
            return Err(ServiceError::InvalidArguments(
                "Tree view belongs to another plugin".into(),
            ));
        }
        if let Some(element) = &element_id {
            tree.children.remove(&Some(element.clone()));
        } else {
            tree.children.clear();
        }
        self.request_native_tree_children(view_id, element_id);
        Ok(())
    }

    pub(crate) fn request_native_tree_children(
        &mut self,
        view_id: &str,
        element_id: Option<String>,
    ) {
        let Some(owner) = self
            .plugins
            .native_tree_views
            .get(view_id)
            .map(|view| view.owner.clone())
        else {
            return;
        };
        let Some(runtime) = self.plugin_runtime_mut(&owner) else {
            return;
        };
        let Some(supervisor) = runtime.supervisor.as_mut() else {
            return;
        };
        if let Ok(rpc_id) = supervisor.request(
            RpcRequest::ProvideTreeChildren {
                view_id: view_id.to_owned(),
                element_id: element_id.clone(),
            },
            CancellationToken::default(),
            Instant::now(),
        ) {
            runtime
                .tree_replies
                .insert(rpc_id, (view_id.to_owned(), element_id));
        }
    }

    pub(crate) fn apply_native_tree_children(
        &mut self,
        view_id: &str,
        element_id: Option<String>,
        result: ServiceResult<Value>,
    ) {
        let Ok(value) = result else {
            return;
        };
        let Ok(items) = serde_json::from_value::<Vec<TreeItem>>(value) else {
            return;
        };
        let Some(tree) = self.plugins.native_tree_views.get_mut(view_id) else {
            return;
        };
        for item in &items {
            if item.collapsible_state == TreeCollapsibleState::Expanded {
                tree.expanded.insert(item.id.clone());
            }
        }
        let pending_children: Vec<String> = items
            .iter()
            .filter(|item| item.collapsible_state == TreeCollapsibleState::Expanded)
            .map(|item| item.id.clone())
            .filter(|child_id| !tree.children.contains_key(&Some(child_id.clone())))
            .collect();
        tree.children.insert(element_id, items);
        for child_id in pending_children {
            self.request_native_tree_children(view_id, Some(child_id));
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    pub(crate) fn plugin_tree_items(&self, view_id: &str) -> Vec<FlattenedTreeRow> {
        let Some(tree) = self.plugins.native_tree_views.get(view_id) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        fn push_level(
            view_id: &str,
            tree: &NativeTreeView,
            parent: Option<&str>,
            depth: usize,
            rows: &mut Vec<FlattenedTreeRow>,
        ) {
            let key = parent.map(str::to_owned);
            let Some(children) = tree.children.get(&key) else {
                return;
            };
            for item in children {
                let is_expanded = tree.expanded.contains(&item.id);
                rows.push(FlattenedTreeRow {
                    view_id: view_id.to_owned(),
                    depth,
                    item: item.clone(),
                });
                if is_expanded {
                    push_level(view_id, tree, Some(&item.id), depth + 1, rows);
                }
            }
        }
        push_level(view_id, tree, None, 0, &mut rows);
        rows
    }

    pub(crate) fn click_plugin_tree_row(&mut self, view_id: &str, row_index: usize) -> bool {
        let rows = self.plugin_tree_items(view_id);
        let Some(row) = rows.get(row_index) else {
            return false;
        };
        let mut fetch_children = false;
        if row.item.collapsible_state != TreeCollapsibleState::None {
            if let Some(tree) = self.plugins.native_tree_views.get_mut(view_id) {
                if tree.expanded.contains(&row.item.id) {
                    tree.expanded.remove(&row.item.id);
                } else {
                    tree.expanded.insert(row.item.id.clone());
                    fetch_children = !tree.children.contains_key(&Some(row.item.id.clone()));
                }
            }
        }
        if fetch_children {
            self.request_native_tree_children(view_id, Some(row.item.id.clone()));
        }
        if let Some(command) = &row.item.command {
            let arguments = if command.arguments.is_empty() {
                Value::Null
            } else {
                Value::Array(command.arguments.clone())
            };
            let ticket = self.execute_command(CommandRequest {
                id: command.id.clone(),
                arguments,
                target: CommandTarget::Global,
            });
            if let Some(Err(error)) = ticket.try_result() {
                self.status = self.command_error_text(&error);
            }
        }
        true
    }

    pub(crate) fn plugin_activity_sidebar_click(
        &mut self,
        mx: f32,
        my: f32,
        _window_width: f32,
        line_height: f32,
    ) -> bool {
        let Some(key) = self.active_plugin_activity.clone() else {
            return false;
        };
        let Some(entry) = self
            .plugin_activity_entries()
            .into_iter()
            .find(|entry| entry.key == key)
        else {
            return false;
        };
        let _ = mx;
        let clicked_row = ((my / line_height.max(1.0)) - 4.0).floor();
        if clicked_row < 0.0 {
            return true;
        }
        let mut target_row = clicked_row as usize;
        for view in &entry.views {
            if target_row == 0 {
                if view.kind == "tree" {
                    self.request_native_tree_children(&view.id, None);
                }
                return true;
            }
            target_row -= 1;
            if view.kind == "tree" {
                let items = self.plugin_tree_items(&view.id);
                if target_row < items.len() {
                    self.click_plugin_tree_row(&view.id, target_row);
                    return true;
                }
                target_row -= items.len();
            }
        }
        let _ = target_row;
        true
    }

    fn create_native_webview_panel(
        &mut self,
        handle: String,
        view_type: String,
        title: String,
        html: String,
    ) -> ServiceResult<()> {
        let Some(owner) = self.current_plugin_id() else {
            return Err(ServiceError::HostStopped);
        };
        let namespaced_handle = format!("{owner}:{handle}");
        if self.gpu.is_some() {
            self.webview_handle = Some(namespaced_handle);
            let _ = self.show_webview_html(&html);
        }
        self.status = title.clone();
        self.plugins.native_webview = Some(NativeWebviewPanel {
            owner,
            handle,
            view_type,
            title,
            html,
            outbound_messages: Vec::new(),
        });
        Ok(())
    }

    fn set_native_webview_html(&mut self, handle: &str, html: String) -> ServiceResult<()> {
        let Some(owner) = self.current_plugin_id() else {
            return Err(ServiceError::HostStopped);
        };
        let Some(panel) = self.plugins.native_webview.as_mut() else {
            return Err(ServiceError::NotFound);
        };
        if panel.owner != owner || panel.handle != handle {
            return Err(ServiceError::NotFound);
        }
        panel.html = html.clone();
        if self.gpu.is_some() {
            self.webview_handle = Some(format!("{owner}:{handle}"));
            let _ = self.show_webview_html(&html);
        }
        Ok(())
    }

    fn set_native_webview_title(&mut self, handle: &str, title: String) -> ServiceResult<()> {
        let Some(owner) = self.current_plugin_id() else {
            return Err(ServiceError::HostStopped);
        };
        let Some(panel) = self.plugins.native_webview.as_mut() else {
            return Err(ServiceError::NotFound);
        };
        if panel.owner != owner || panel.handle != handle {
            return Err(ServiceError::NotFound);
        }
        panel.title = title.clone();
        self.status = title;
        Ok(())
    }

    fn post_native_webview_message(&mut self, handle: &str, message: Value) -> ServiceResult<()> {
        let Some(owner) = self.current_plugin_id() else {
            return Err(ServiceError::HostStopped);
        };
        let Some(panel) = self.plugins.native_webview.as_mut() else {
            return Err(ServiceError::NotFound);
        };
        if panel.owner != owner || panel.handle != handle {
            return Err(ServiceError::NotFound);
        }
        panel.outbound_messages.push(message.clone());
        if let (Some(expected), Some(webview)) =
            (self.webview_handle.as_deref(), self.webview.as_ref())
        {
            if expected == format!("{owner}:{handle}") {
                let encoded = serde_json::to_string(&message)
                    .map_err(|e| ServiceError::Failed(e.to_string()))?;
                let _ = webview.evaluate_script(&format!(
                    "window.postMessage(JSON.parse({}), '*')",
                    serde_json::to_string(&encoded).unwrap_or_default()
                ));
            }
        }
        Ok(())
    }

    fn dispose_native_webview_panel(&mut self, handle: &str) -> ServiceResult<()> {
        let Some(owner) = self.current_plugin_id() else {
            return Err(ServiceError::HostStopped);
        };
        let matches = self
            .plugins
            .native_webview
            .as_ref()
            .is_some_and(|panel| panel.owner == owner && panel.handle == handle);
        if !matches {
            return Err(ServiceError::NotFound);
        }
        self.plugins.native_webview = None;
        if self.webview_handle.as_deref() == Some(&format!("{owner}:{handle}")) {
            self.close_webview();
        }
        Ok(())
    }

    pub(crate) fn native_webview(&self) -> Option<&NativeWebviewPanel> {
        self.plugins.native_webview.as_ref()
    }

    pub(crate) fn send_webview_message(&mut self, handle: &str, message: Value) -> bool {
        let Some(owner) = self
            .plugins
            .native_webview
            .as_ref()
            .filter(|panel| panel.handle == handle)
            .map(|panel| panel.owner.clone())
        else {
            return false;
        };
        let Some(runtime) = self.plugin_runtime_mut(&owner) else {
            return false;
        };
        let Some(supervisor) = runtime.supervisor.as_mut() else {
            return false;
        };
        match supervisor.request(
            RpcRequest::DeliverWebviewMessage {
                handle: handle.to_owned(),
                message,
            },
            CancellationToken::default(),
            Instant::now(),
        ) {
            Ok(rpc_id) => {
                runtime.webview_deliveries.insert(rpc_id);
                true
            }
            Err(_) => false,
        }
    }

    pub(crate) fn sync_native_webview_host(&mut self) {
        let Some(panel) = self.plugins.native_webview.as_ref() else {
            return;
        };
        let namespaced = format!("{}:{}", panel.owner, panel.handle);
        let handle = panel.handle.clone();
        if self.webview_handle.as_deref() != Some(namespaced.as_str()) {
            return;
        }
        let messages = self
            .webview
            .as_ref()
            .map(|webview| webview.take_messages())
            .unwrap_or_default();
        for raw in messages {
            let value = serde_json::from_str::<Value>(&raw).unwrap_or(Value::String(raw));
            self.send_webview_message(&handle, value);
        }
    }

    pub(crate) fn clear_native_ui_for_plugin(&mut self, plugin_id: &str) {
        self.plugins
            .native_tree_views
            .retain(|_, view| view.owner != plugin_id);
        if self
            .plugins
            .native_webview
            .as_ref()
            .is_some_and(|panel| panel.owner == plugin_id)
        {
            let full_handle = self
                .plugins
                .native_webview
                .as_ref()
                .map(|panel| format!("{}:{}", panel.owner, panel.handle));
            self.plugins.native_webview = None;
            if full_handle.is_some() && self.webview_handle == full_handle {
                self.close_webview();
            }
        }
        let mut remaining = Vec::new();
        for pending in self.plugins.pending_commands.drain(..) {
            if pending.plugin_id == plugin_id {
                let _ = pending.sender.send(Err(CommandError::HostStopped));
            } else {
                remaining.push(pending);
            }
        }
        self.plugins.pending_commands = remaining;
    }

    pub(crate) fn sync_plugin_keybindings(&mut self) {
        let mut specs = Vec::new();
        for record in &self.plugins.installed {
            let Metadata::Native(manifest) = &record.metadata else {
                continue;
            };
            if !record.enabled || record.trusted_version.as_deref() != Some(&manifest.version) {
                continue;
            }
            for binding in &manifest.keybindings {
                specs.push((
                    binding.key.clone(),
                    binding.command.clone(),
                    binding.when.clone(),
                ));
            }
        }
        self.keymap.set_plugin_bindings(&specs);
    }

    pub(crate) fn plugin_menu_items(
        &self,
        location: PluginMenuLocation,
    ) -> Vec<ResolvedPluginMenuItem> {
        let target = super::super::command_registry::CommandTarget::View(
            self.editor_groups.active_view().id.0,
        );
        let context = self.context_keys.resolve_target(target);
        let mut items = Vec::new();
        for record in &self.plugins.installed {
            let Metadata::Native(manifest) = &record.metadata else {
                continue;
            };
            if !record.enabled || record.trusted_version.as_deref() != Some(&manifest.version) {
                continue;
            }
            let menu = match location {
                PluginMenuLocation::CommandPalette => &manifest.menus.command_palette,
                PluginMenuLocation::EditorContext => &manifest.menus.editor_context,
                PluginMenuLocation::ViewItemContext => &manifest.menus.view_item_context,
            };
            for entry in menu {
                let enabled = entry.when.as_deref().is_none_or(|when| {
                    crate::keybindings::Condition::parse(when)
                        .is_ok_and(|cond| cond.evaluate(&context))
                });
                if !enabled && location != PluginMenuLocation::CommandPalette {
                    continue;
                }
                let title = manifest
                    .commands
                    .iter()
                    .find(|cmd| cmd.id == entry.command)
                    .map(|cmd| cmd.title.clone())
                    .unwrap_or_else(|| entry.command.clone());
                items.push(ResolvedPluginMenuItem {
                    plugin_id: manifest.id.clone(),
                    command: entry.command.clone(),
                    title,
                    group: entry.group.clone(),
                    enabled,
                });
            }
        }
        items
    }

    pub(crate) fn manifest_matches_startup(
        manifest: &graphite_plugins::manifest::Manifest,
    ) -> bool {
        manifest.activation_events.is_empty()
            || manifest
                .activation_events
                .iter()
                .any(|event| event == "*" || event == "onStartupFinished")
    }

    pub(crate) fn activate_plugins_for_event(&mut self, event: &str) -> Vec<String> {
        let targets: Vec<String> = self
            .plugins
            .installed
            .iter()
            .filter_map(|record| {
                let Metadata::Native(manifest) = &record.metadata else {
                    return None;
                };
                if !record.enabled
                    || record.trusted_version.as_deref() != Some(&manifest.version)
                    || !manifest.activation_events.iter().any(|e| e == event)
                {
                    return None;
                }
                Some(manifest.id.clone())
            })
            .collect();
        for id in &targets {
            self.queue_plugin_start(id.clone());
        }
        targets
    }
}
