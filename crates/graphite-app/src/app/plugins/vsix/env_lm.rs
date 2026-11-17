//! VSIX `activationEvents`, `workspaceContains:`, and `MainThreadLanguageModelTools` bridge.
//!
//! Tracks language model tools extensions register via `vscode.lm.registerTool` so Graphite AI
//! Chat can discover and invoke them alongside native plugin chat tools, and manages
//! declarative VSIX activation triggers (`*`, `onStartupFinished`, `onLanguage:`, `onView:`,
//! `workspaceContains:`).

use super::*;
use graphite_plugins::manifest::Metadata;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::mpsc;

/// Replies to VSIX language model tool invocations carry ids in `LM_TOOL_BASE..LM_TOOL_END`.
const LM_TOOL_BASE: u64 = 13_000_000;
const LM_TOOL_END: u64 = 14_000_000;

pub(super) fn is_lm_tool_reply(id: u64) -> bool {
    (LM_TOOL_BASE..LM_TOOL_END).contains(&id)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VsixLanguageModelTool {
    pub id: String,
    pub name: String,
    pub model_description: String,
    pub input_schema: Value,
}

#[derive(Default)]
pub(super) struct LanguageModelTools {
    next_id: u64,
    pub(crate) tools: BTreeMap<String, VsixLanguageModelTool>,
    replies: BTreeMap<u64, Option<mpsc::Sender<std::result::Result<String, String>>>>,
}

impl LanguageModelTools {
    pub(super) fn busy(&self) -> bool {
        !self.replies.is_empty()
    }

    fn next_call_id(&mut self) -> u64 {
        let id = LM_TOOL_BASE + self.next_id;
        self.next_id = (self.next_id + 1) % (LM_TOOL_END - LM_TOOL_BASE);
        id
    }
}

fn parse_tool_definition(args: &[Value]) -> Option<VsixLanguageModelTool> {
    let first = args.first()?;
    if let Some(id) = first.as_str() {
        if id.trim().is_empty() {
            return None;
        }
        let def = args.get(1).unwrap_or(&Value::Null);
        let name = def["toolReferenceName"]
            .as_str()
            .or_else(|| def["name"].as_str())
            .unwrap_or(id)
            .to_owned();
        let model_description = def["modelDescription"]
            .as_str()
            .or_else(|| def["userDescription"].as_str())
            .unwrap_or("")
            .to_owned();
        let input_schema = def
            .get("inputSchema")
            .filter(|v| v.is_object())
            .cloned()
            .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
        return Some(VsixLanguageModelTool {
            id: id.to_owned(),
            name,
            model_description,
            input_schema,
        });
    }
    if first.is_object() {
        let id = first["id"].as_str().filter(|s| !s.trim().is_empty())?;
        let name = first["toolReferenceName"]
            .as_str()
            .or_else(|| first["name"].as_str())
            .unwrap_or(id)
            .to_owned();
        let model_description = first["modelDescription"]
            .as_str()
            .or_else(|| first["userDescription"].as_str())
            .unwrap_or("")
            .to_owned();
        let input_schema = first
            .get("inputSchema")
            .filter(|v| v.is_object())
            .cloned()
            .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
        return Some(VsixLanguageModelTool {
            id: id.to_owned(),
            name,
            model_description,
            input_schema,
        });
    }
    None
}

fn workspace_contains_glob(root: &Path, pattern: &str) -> bool {
    let trimmed = pattern.trim().trim_start_matches("./");
    if trimmed.is_empty() || trimmed.contains("..") {
        return false;
    }
    // Common direct relative file checks (`Cargo.toml`, `.eslintrc.json`, `src/main.rs`).
    if !trimmed.contains(['*', '?']) {
        return root.join(trimmed).exists();
    }
    // Simple `**/*ext` / `*.ext` filename scan bounded shallowly so startup never stalls.
    let needle = trimmed.trim_start_matches("**/").trim_start_matches('*');
    if needle.is_empty() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries
        .filter_map(std::result::Result::ok)
        .take(512)
        .any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(needle))
        })
}

pub(crate) fn vsix_event_matches(root: &Path, configured: &str, incoming: &str) -> bool {
    if configured == incoming || configured == "*" || configured == "onStartupFinished" {
        return true;
    }
    if incoming == "workspaceContains" {
        if let Some(pattern) = configured.strip_prefix("workspaceContains:") {
            return workspace_contains_glob(root, pattern);
        }
    }
    false
}

impl App {
    pub(super) fn handle_vsix_lm_tool_request(
        &mut self,
        runtime: &mut Runtime,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        match method {
            "$registerTool" | "$registerToolWithDefinition" => {
                let tool = parse_tool_definition(args)
                    .ok_or("Invalid MainThreadLanguageModelTools registration payload")?;
                if runtime.lm_tools.tools.len() >= 64
                    && !runtime.lm_tools.tools.contains_key(&tool.id)
                {
                    return Err("Maximum number of VSIX language model tools reached".into());
                }
                runtime.lm_tools.tools.insert(tool.id.clone(), tool);
                Ok(Value::Null)
            }
            "$unregisterTool" => {
                if let Some(id) = args.first().and_then(Value::as_str) {
                    runtime.lm_tools.tools.remove(id);
                }
                Ok(Value::Null)
            }
            "$getTools" => {
                let list: Vec<Value> = runtime
                    .lm_tools
                    .tools
                    .values()
                    .map(|tool| {
                        json!({
                            "id": tool.id,
                            "name": tool.name,
                            "modelDescription": tool.model_description,
                            "inputSchema": tool.input_schema,
                        })
                    })
                    .collect();
                Ok(Value::Array(list))
            }
            "$acceptToolInvocationProgress" | "$invokeToolResult" => Ok(Value::Null),
            _ => {
                self.record_unsupported_vsix_call(runtime, "MainThreadLanguageModelTools", method);
                Err(format!(
                    "Graphite does not support MainThreadLanguageModelTools.{method}"
                ))
            }
        }
    }

    pub(crate) fn vsix_lm_tool_specs(&self) -> Vec<graphite_chat::ToolSpec> {
        let Some(runtime) = self.plugins.vsix.as_ref() else {
            return Vec::new();
        };
        runtime
            .lm_tools
            .tools
            .values()
            .map(|tool| graphite_chat::ToolSpec {
                name: tool.name.clone(),
                description: tool.model_description.clone(),
                schema: tool.input_schema.clone(),
            })
            .collect()
    }

    pub(crate) fn invoke_vsix_lm_tool(
        &mut self,
        name: &str,
        input: Value,
        reply: Option<mpsc::Sender<std::result::Result<String, String>>>,
    ) -> std::result::Result<u64, String> {
        if !self.vsix_workspace_trusted() {
            return Err("Workspace is restricted; trust it before invoking VSIX tools.".into());
        }
        let runtime = self
            .plugins
            .vsix
            .as_mut()
            .ok_or_else(|| format!("Plugin chat tool `{name}` is unavailable"))?;
        let tool_id = runtime
            .lm_tools
            .tools
            .values()
            .find(|tool| tool.name == name || tool.id == name)
            .map(|tool| tool.id.clone())
            .ok_or_else(|| format!("Plugin chat tool `{name}` is unavailable"))?;
        let call_id = runtime.lm_tools.next_call_id();
        runtime.lm_tools.replies.insert(call_id, reply);
        runtime.session.send(json!({
            "type": "call",
            "id": call_id,
            "actor": "ExtHostLanguageModelTools",
            "method": "$invokeTool",
            "args": [{
                "toolId": tool_id,
                "callId": format!("graphite-lm-{call_id}"),
                "parameters": input,
            }],
            "cancellable": true,
        }))?;
        Ok(call_id)
    }

    pub(super) fn finish_vsix_lm_tool(&mut self, runtime: &mut Runtime, event: &Value) {
        let Some(id) = event["id"].as_u64() else {
            return;
        };
        let Some(reply) = runtime.lm_tools.replies.remove(&id) else {
            return;
        };
        let outcome = if let Some(error) = event["error"].as_str() {
            Err(error.to_owned())
        } else {
            let result = &event["result"];
            if let Some(parts) = result["content"].as_array() {
                let text = parts
                    .iter()
                    .filter_map(|part| part["value"].as_str().or_else(|| part["text"].as_str()))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(text)
            } else if let Some(text) = result.as_str() {
                Ok(text.to_owned())
            } else if result.is_null() {
                Ok(String::new())
            } else {
                Ok(result.to_string())
            }
        };
        if let Some(sender) = reply {
            let _ = sender.send(outcome);
        }
    }

    pub(crate) fn trigger_vsix_activation_event(&mut self, event: &str) -> Vec<String> {
        let root = self.tree.root();
        let matched: Vec<(String, Vec<String>)> = self
            .plugins
            .installed
            .iter()
            .filter_map(|record| {
                let Metadata::Vsix(vsix) = &record.metadata else {
                    return None;
                };
                if !super::eligible(record) {
                    return None;
                }
                let matching: Vec<String> = vsix
                    .activation_events
                    .iter()
                    .filter(|configured| vsix_event_matches(root, configured, event))
                    .cloned()
                    .collect();
                if matching.is_empty() {
                    None
                } else {
                    Some((vsix.id.clone(), matching))
                }
            })
            .collect();

        if matched.is_empty() {
            return Vec::new();
        }
        if self.plugins.vsix.is_none() && self.vsix_workspace_trusted() {
            if let Some(runtime_path) = Session::bundled_runtime() {
                let _ = self.start_vsix_host(Some(runtime_path));
            }
        }
        if let Some(runtime) = self.plugins.vsix.as_mut() {
            for (_, events) in &matched {
                for ev in events {
                    if runtime.activated_events.insert(ev.clone()) {
                        let _ = runtime.session.send(json!({
                            "type": "activateByEvent",
                            "event": ev,
                        }));
                    }
                }
            }
        }
        matched.into_iter().map(|(id, _)| id).collect()
    }

    pub(super) fn trigger_vsix_startup_activations(&mut self, runtime: &mut Runtime) {
        if runtime.startup_activated {
            return;
        }
        runtime.startup_activated = true;
        let root = self.tree.root();
        let mut to_fire = Vec::new();
        for record in &self.plugins.installed {
            let Metadata::Vsix(vsix) = &record.metadata else {
                continue;
            };
            if !super::eligible(record) {
                continue;
            }
            for ev in &vsix.activation_events {
                if (ev == "*"
                    || ev == "onStartupFinished"
                    || ev
                        .strip_prefix("workspaceContains:")
                        .is_some_and(|glob| workspace_contains_glob(root, glob)))
                    && runtime.activated_events.insert(ev.clone())
                {
                    to_fire.push(ev.clone());
                }
            }
        }
        for ev in to_fire {
            let _ = runtime.session.send(json!({
                "type": "activateByEvent",
                "event": ev,
            }));
        }
    }
}
