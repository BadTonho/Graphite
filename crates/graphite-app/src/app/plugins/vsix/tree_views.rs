//! VSIX TreeView provider bridge (`MainThreadTreeViews` / `ExtHostTreeViews`).
//!
//! Feeds Code OSS `vscode.window.registerTreeDataProvider` / `createTreeView` trees into
//! Graphite's shared `plugins.native_tree_views` store so VSIX sidebar trees render and respond
//! identically to native plugin trees without duplicating Activity Bar layout code.

use super::*;
use crate::app::extension_host::NativeTreeView;
use graphite_core_api::{TreeCollapsibleState, TreeItem, TreeItemCommand};
use serde_json::{json, Value};

/// Replies to VSIX tree view requests carry ids in `TREE_BASE..TREE_END`.
pub(super) const TREE_BASE: u64 = 11_000_000;
const TREE_END: u64 = 12_000_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn is_tree_reply(id: u64) -> bool {
    (TREE_BASE..TREE_END).contains(&id)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct VsixTreeViewMeta {
    pub view_id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub message: Option<String>,
    pub badge_value: Option<u64>,
    pub badge_tooltip: Option<String>,
    pub show_collapse_all: bool,
    pub can_select_many: bool,
}

#[derive(Clone, Debug)]
struct InFlightTree {
    kind: TreeRequestKind,
    deadline: Instant,
}

#[derive(Clone, Debug)]
enum TreeRequestKind {
    GetChildren {
        view_id: String,
        element_id: Option<String>,
    },
    Void,
}

#[derive(Default)]
pub(super) struct TreeViews {
    next_id: u64,
    pub(crate) views: BTreeMap<String, VsixTreeViewMeta>,
    in_flight: BTreeMap<u64, InFlightTree>,
}

impl TreeViews {
    pub(super) fn busy(&self) -> bool {
        !self.in_flight.is_empty()
    }

    pub(super) fn contains(&self, view_id: &str) -> bool {
        self.views.contains_key(view_id)
    }

    #[cfg(test)]
    pub(super) fn meta(&self, view_id: &str) -> Option<&VsixTreeViewMeta> {
        self.views.get(view_id)
    }

    fn next_id(&mut self) -> u64 {
        let id = TREE_BASE + self.next_id;
        self.next_id = (self.next_id + 1) % (TREE_END - TREE_BASE);
        id
    }

    pub(super) fn expire(&mut self, now: Instant) {
        self.in_flight.retain(|_, req| req.deadline > now);
    }
}

/// Converts Code OSS `ITreeItem` DTOs (flat `[ITreeItem, ...]` or batched `[[idx, ITreeItem, ...]]`)
/// into Graphite `TreeItem` values.
pub(super) fn parse_tree_items(value: &Value) -> Vec<TreeItem> {
    fn collect_objects<'a>(node: &'a Value, out: &mut Vec<&'a Value>) {
        match node {
            Value::Array(items) => {
                for item in items {
                    collect_objects(item, out);
                }
            }
            Value::Object(_) => out.push(node),
            _ => {}
        }
    }

    let mut raw_items = Vec::new();
    collect_objects(value, &mut raw_items);

    raw_items
        .into_iter()
        .filter_map(|item| {
            let label = item["label"]
                .as_str()
                .or_else(|| item["label"]["label"].as_str())
                .map(str::to_owned)?;
            let id = item["handle"]
                .as_str()
                .or_else(|| item["id"].as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| label.clone());
            let description = item["description"].as_str().map(str::to_owned);
            let tooltip = item["tooltip"]
                .as_str()
                .or_else(|| item["tooltip"]["value"].as_str())
                .map(str::to_owned);
            let collapsible_state = match &item["collapsibleState"] {
                Value::Number(n) => match n.as_u64() {
                    Some(1) => TreeCollapsibleState::Collapsed,
                    Some(2) => TreeCollapsibleState::Expanded,
                    _ => TreeCollapsibleState::None,
                },
                Value::String(s) => match s.as_str() {
                    "collapsed" | "Collapsed" => TreeCollapsibleState::Collapsed,
                    "expanded" | "Expanded" => TreeCollapsibleState::Expanded,
                    _ => TreeCollapsibleState::None,
                },
                _ => TreeCollapsibleState::None,
            };
            let command = item.get("command").and_then(|cmd| {
                let cmd_id = cmd["id"].as_str()?;
                let arguments = cmd["arguments"].as_array().cloned().unwrap_or_default();
                let title = cmd["title"].as_str().map(str::to_owned);
                Some(TreeItemCommand {
                    id: cmd_id.to_owned(),
                    title,
                    arguments,
                })
            });
            let context_value = item["contextValue"].as_str().map(str::to_owned);
            Some(TreeItem {
                id,
                label,
                description,
                tooltip,
                collapsible_state,
                command,
                context_value,
            })
        })
        .collect()
}

fn extract_markdown_or_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| value["value"].as_str())
        .map(str::to_owned)
}

impl App {
    pub(super) fn handle_vsix_tree_request(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        let view_id = args
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Invalid {method} treeViewId"))?
            .to_owned();

        match method {
            "$registerTreeViewDataProvider" => {
                let options = args.get(1).unwrap_or(&Value::Null);
                let meta = VsixTreeViewMeta {
                    view_id: view_id.clone(),
                    title: None,
                    description: None,
                    message: None,
                    badge_value: None,
                    badge_tooltip: None,
                    show_collapse_all: options["showCollapseAll"].as_bool().unwrap_or(false),
                    can_select_many: options["canSelectMany"].as_bool().unwrap_or(false),
                };
                runtime.tree_views.views.insert(view_id.clone(), meta);
                self.plugins.native_tree_views.insert(
                    view_id.clone(),
                    NativeTreeView {
                        owner: format!("vsix:{view_id}"),
                        children: BTreeMap::new(),
                        expanded: BTreeSet::new(),
                    },
                );
                let _ = self.send_vsix_tree_get_children(runtime, &view_id, None);
                Ok(Value::Null)
            }
            "$disposeTreeView" => {
                runtime.tree_views.views.remove(&view_id);
                if self
                    .plugins
                    .native_tree_views
                    .get(&view_id)
                    .is_some_and(|tree| tree.owner.starts_with("vsix:"))
                {
                    self.plugins.native_tree_views.remove(&view_id);
                }
                Ok(Value::Null)
            }
            "$refresh" => {
                let targets = args.get(1).and_then(Value::as_object);
                if let Some(items) = targets.filter(|map| !map.is_empty()) {
                    let handles: Vec<String> = items.keys().cloned().collect();
                    if let Some(tree) = self.plugins.native_tree_views.get_mut(&view_id) {
                        for handle in &handles {
                            tree.children.remove(&Some(handle.clone()));
                        }
                    }
                    for handle in handles {
                        let _ = self.send_vsix_tree_get_children(runtime, &view_id, Some(handle));
                    }
                } else {
                    if let Some(tree) = self.plugins.native_tree_views.get_mut(&view_id) {
                        tree.children.clear();
                    }
                    let _ = self.send_vsix_tree_get_children(runtime, &view_id, None);
                }
                Ok(Value::Null)
            }
            "$reveal" => {
                if let Some(item_info) = args.get(1).filter(|v| !v.is_null()) {
                    let expand_depth = args
                        .get(2)
                        .and_then(|opts| {
                            opts["expand"]
                                .as_bool()
                                .or_else(|| opts["expand"].as_u64().map(|n| n > 0))
                        })
                        .unwrap_or(false);
                    if let Some(tree) = self.plugins.native_tree_views.get_mut(&view_id) {
                        if let Some(parents) = item_info["parentChain"].as_array() {
                            for parent in parents {
                                if let Some(handle) =
                                    parent["handle"].as_str().or_else(|| parent["id"].as_str())
                                {
                                    tree.expanded.insert(handle.to_owned());
                                }
                            }
                        }
                        if expand_depth {
                            if let Some(handle) = item_info["item"]["handle"]
                                .as_str()
                                .or_else(|| item_info["item"]["id"].as_str())
                            {
                                tree.expanded.insert(handle.to_owned());
                            }
                        }
                    }
                }
                Ok(Value::Null)
            }
            "$setMessage" => {
                if let Some(meta) = runtime.tree_views.views.get_mut(&view_id) {
                    meta.message = args.get(1).and_then(extract_markdown_or_text);
                }
                Ok(Value::Null)
            }
            "$setTitle" => {
                if let Some(meta) = runtime.tree_views.views.get_mut(&view_id) {
                    meta.title = args.get(1).and_then(Value::as_str).map(str::to_owned);
                    meta.description = args.get(2).and_then(Value::as_str).map(str::to_owned);
                }
                Ok(Value::Null)
            }
            "$setBadge" => {
                if let Some(meta) = runtime.tree_views.views.get_mut(&view_id) {
                    if let Some(badge) = args.get(1).filter(|v| !v.is_null()) {
                        meta.badge_value = badge["value"].as_u64();
                        meta.badge_tooltip = badge["tooltip"].as_str().map(str::to_owned);
                    } else {
                        meta.badge_value = None;
                        meta.badge_tooltip = None;
                    }
                }
                Ok(Value::Null)
            }
            "$setSelection" => Ok(Value::Null),
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadTreeViews", method);
                Err(format!(
                    "Graphite does not support MainThreadTreeViews.{method}"
                ))
            }
        }
    }

    pub(super) fn send_vsix_tree_get_children(
        &self,
        runtime: &mut Runtime,
        view_id: &str,
        element_id: Option<String>,
    ) -> Result<u64> {
        let id = runtime.tree_views.next_id();
        let handles_arg = match &element_id {
            Some(handle) => json!([handle]),
            None => Value::Null,
        };
        runtime.tree_views.in_flight.insert(
            id,
            InFlightTree {
                kind: TreeRequestKind::GetChildren {
                    view_id: view_id.to_owned(),
                    element_id,
                },
                deadline: Instant::now() + REQUEST_TIMEOUT,
            },
        );
        runtime.session.send(json!({
            "type": "call",
            "id": id,
            "actor": "ExtHostTreeViews",
            "method": "$getChildren",
            "args": [view_id, handles_arg],
            "cancellable": true,
        }))?;
        Ok(id)
    }

    pub(super) fn finish_vsix_tree(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let Some(req) = runtime.tree_views.in_flight.remove(&id) else {
            return;
        };
        if let Some(error) = event["error"].as_str() {
            self.status = error.to_owned();
            return;
        }
        if let TreeRequestKind::GetChildren {
            view_id,
            element_id,
        } = req.kind
        {
            let items = parse_tree_items(&event["result"]);
            let Some(tree) = self.plugins.native_tree_views.get_mut(&view_id) else {
                return;
            };
            let mut to_expand = Vec::new();
            for item in &items {
                if item.collapsible_state == TreeCollapsibleState::Expanded {
                    tree.expanded.insert(item.id.clone());
                    if !tree.children.contains_key(&Some(item.id.clone())) {
                        to_expand.push(item.id.clone());
                    }
                }
            }
            tree.children.insert(element_id, items);
            for child_id in to_expand {
                let _ = self.send_vsix_tree_get_children(runtime, &view_id, Some(child_id));
            }
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
    }

    pub(crate) fn is_vsix_tree_view(&self, view_id: &str) -> bool {
        self.plugins
            .vsix
            .as_ref()
            .is_some_and(|runtime| runtime.tree_views.contains(view_id))
            || self
                .plugins
                .native_tree_views
                .get(view_id)
                .is_some_and(|tree| tree.owner.starts_with("vsix:"))
    }

    pub(crate) fn request_vsix_tree_children(
        &mut self,
        view_id: &str,
        element_id: Option<String>,
    ) -> bool {
        let Some(mut runtime) = self.plugins.vsix.take() else {
            return false;
        };
        let owns = runtime.tree_views.contains(view_id);
        if owns {
            let _ = self.send_vsix_tree_get_children(&mut runtime, view_id, element_id);
        }
        self.plugins.vsix = Some(runtime);
        owns
    }

    pub(crate) fn notify_vsix_tree_expand(&mut self, view_id: &str, item_id: &str, expanded: bool) {
        let Some(runtime) = self.plugins.vsix.as_mut() else {
            return;
        };
        if !runtime.tree_views.contains(view_id) {
            return;
        }
        let id = runtime.tree_views.next_id();
        runtime.tree_views.in_flight.insert(
            id,
            InFlightTree {
                kind: TreeRequestKind::Void,
                deadline: Instant::now() + REQUEST_TIMEOUT,
            },
        );
        let _ = runtime.session.send(json!({
            "type": "call",
            "id": id,
            "actor": "ExtHostTreeViews",
            "method": "$setExpanded",
            "args": [view_id, item_id, expanded],
        }));
    }

    pub(crate) fn notify_vsix_tree_select(&mut self, view_id: &str, item_id: &str) {
        let Some(runtime) = self.plugins.vsix.as_mut() else {
            return;
        };
        if !runtime.tree_views.contains(view_id) {
            return;
        }
        let id = runtime.tree_views.next_id();
        runtime.tree_views.in_flight.insert(
            id,
            InFlightTree {
                kind: TreeRequestKind::Void,
                deadline: Instant::now() + REQUEST_TIMEOUT,
            },
        );
        let _ = runtime.session.send(json!({
            "type": "call",
            "id": id,
            "actor": "ExtHostTreeViews",
            "method": "$setSelectionAndFocus",
            "args": [view_id, [item_id], item_id],
        }));
    }

    pub(crate) fn notify_vsix_tree_visible(&mut self, view_id: &str, visible: bool) {
        let Some(runtime) = self.plugins.vsix.as_mut() else {
            return;
        };
        if !runtime.tree_views.contains(view_id) {
            return;
        }
        let id = runtime.tree_views.next_id();
        runtime.tree_views.in_flight.insert(
            id,
            InFlightTree {
                kind: TreeRequestKind::Void,
                deadline: Instant::now() + REQUEST_TIMEOUT,
            },
        );
        let _ = runtime.session.send(json!({
            "type": "call",
            "id": id,
            "actor": "ExtHostTreeViews",
            "method": "$setVisible",
            "args": [view_id, visible],
        }));
    }

    pub(crate) fn execute_vsix_tree_command(&mut self, command_id: &str, args: Vec<Value>) {
        let vsix_key = format!(":{command_id}");
        if let Err(err) = self.run_vsix_command_with_args(&vsix_key, args.clone()) {
            let arguments = if args.is_empty() {
                Value::Null
            } else {
                Value::Array(args)
            };
            let ticket = self.execute_command(graphite_core_api::CommandRequest {
                id: command_id.to_owned(),
                arguments,
                target: graphite_core_api::CommandTarget::Global,
            });
            if let Some(Err(_)) = ticket.try_result() {
                self.status = err;
            }
        }
    }

    pub(super) fn expire_vsix_trees(&mut self, runtime: &mut Runtime) {
        runtime.tree_views.expire(Instant::now());
    }

    pub(super) fn clear_vsix_tree_views(&mut self) {
        self.plugins
            .native_tree_views
            .retain(|_, tree| !tree.owner.starts_with("vsix:"));
    }
}
