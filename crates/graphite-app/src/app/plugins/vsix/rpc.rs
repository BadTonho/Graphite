use super::*;
use graphite_core_api::{DocumentUri, WindowService};

impl App {
    pub(super) fn handle_vsix_request(
        &mut self,
        runtime: &mut Runtime,
        event: &Value,
    ) -> Result<()> {
        let id = event["id"].as_u64().ok_or("Invalid Code OSS request ID")?;
        let actor = event["actor"].as_str().ok_or("Invalid Code OSS actor")?;
        let method = event["method"].as_str().ok_or("Invalid Code OSS method")?;
        let args = event["args"]
            .as_array()
            .ok_or("Invalid Code OSS arguments")?;
        if actor == "MainThreadCommands" && method == "$executeCommand" {
            let Some(command) = args.first().and_then(Value::as_str) else {
                return runtime
                    .session
                    .send(json!({"type":"reply","id":id,"error":"Invalid command ID"}));
            };
            if crate::app::command_registry::registry()
                .get(command)
                .is_none()
            {
                return runtime.session.send(json!({"type":"reply","id":id,"error":format!("Unknown Graphite command: {command}")}));
            }
            let values = args.get(1).unwrap_or(&Value::Null);
            let values = values.as_array().or_else(|| values["value"].as_array());
            let arguments = values
                .and_then(|args| args.first())
                .cloned()
                .unwrap_or_default();
            self.refresh_command_context();
            let target = self.ui_target_for_command(command);
            let ticket = self.execute_command(graphite_core_api::CommandRequest {
                id: command.to_owned(),
                arguments,
                target,
            });
            runtime.editor_commands.insert(id, ticket);
            return Ok(());
        }
        match self.start_vsix_prompt(runtime, id, actor, method, args) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => {
                return runtime
                    .session
                    .send(json!({"type":"reply", "id":id, "error":error}))
            }
        }
        let result = self.dispatch_vsix_request(runtime, actor, method, args);
        match result {
            Ok(result) => runtime
                .session
                .send(json!({"type":"reply", "id":id, "result":result})),
            Err(error) => runtime
                .session
                .send(json!({"type":"reply", "id":id, "error":error})),
        }
    }

    pub(super) fn reply_vsix_editor_commands(&self, runtime: &mut Runtime) -> Result<()> {
        let completed = runtime
            .editor_commands
            .iter()
            .filter_map(|(id, ticket)| ticket.try_result().map(|result| (*id, result)))
            .collect::<Vec<_>>();
        for (id, result) in completed {
            runtime.editor_commands.remove(&id);
            let reply = match result {
                Ok(result) => json!({"type":"reply","id":id,"result":result}),
                Err(error) => json!({"type":"reply","id":id,"error":error.to_string()}),
            };
            runtime.session.send(reply)?;
        }
        Ok(())
    }

    fn dispatch_vsix_request(
        &mut self,
        runtime: &mut Runtime,
        actor: &str,
        method: &str,
        args: &[Value],
    ) -> Result<Value> {
        if let Some(result) = testing::dispatch(self, runtime, actor, method, args) {
            return result.map_err(|error| error.to_owned());
        }
        if let Some(result) = decorations::dispatch(runtime, actor, method, args) {
            return result;
        }
        if let Some(result) = file_events::dispatch(runtime, actor, method, args) {
            return result;
        }
        if let Some(result) = text_search::dispatch(self, runtime, actor, method, args) {
            return result;
        }
        if let Some(result) = terminals::dispatch(self, runtime, actor, method, args) {
            return result;
        }
        if let Some(result) = workbench::dispatch(self, runtime, actor, method, args) {
            return result;
        }
        if let Some(result) = languages::language_rpc(&mut runtime.languages, actor, method, args) {
            return result;
        }
        let text = |index| {
            args.get(index)
                .and_then(Value::as_str)
                .ok_or("Invalid Code OSS string argument")
        };
        let arg = |index| args.get(index).ok_or("Missing Code OSS argument");
        match (actor, method) {
            ("MainThreadCommands", "$registerCommand") => {
                runtime.registered.insert(text(0)?.to_owned());
                Ok(Value::Null)
            }
            ("MainThreadCommands", "$unregisterCommand") => {
                runtime.registered.remove(text(0)?);
                Ok(Value::Null)
            }
            ("MainThreadCommands", "$getCommands") => {
                let commands = crate::app::command_registry::registry()
                    .iter()
                    .map(|command| command.id)
                    .chain(runtime.registered.iter().map(String::as_str))
                    .collect::<BTreeSet<_>>();
                Ok(json!(commands))
            }
            (
                "MainThreadConfiguration",
                "$updateConfigurationOption" | "$removeConfigurationOption",
            ) => self.change_vsix_configuration(runtime, method, args),
            ("MainThreadStorage", "$initializeExtensionStorage" | "$setValue") => {
                self.vsix_storage(method, args)
            }
            ("MainThreadQuickOpen", "$setItems") => {
                let instance = arg(0)?.as_u64().ok_or("Invalid quick pick instance")?;
                self.set_vsix_prompt_items(runtime, instance, arg(1)?);
                Ok(Value::Null)
            }
            ("MainThreadTextEditors", "$tryApplyEdits") => {
                let version = arg(1)?.as_u64().ok_or("Invalid Code OSS editor version")?;
                self.apply_vsix_editor_edits(text(0)?, version, arg(2)?)
            }
            ("MainThreadBulkEdits", "$tryApplyWorkspaceEdit") => {
                self.apply_vsix_workspace_edits(arg(0)?)
            }
            ("MainThreadDocuments", "$tryOpenDocument")
            | ("MainThreadTextEditors", "$tryShowTextDocument") => {
                let uri = DocumentUri::parse(text(0)?).map_err(|error| error.to_string())?;
                let opened = self
                    .open_document(&uri)
                    .map_err(|error| error.to_string())?;
                self.sync_vsix_documents(runtime)?;
                Ok(json!(opened.uri))
            }
            ("MainThreadMessageService", "$showMessage") => {
                self.show_message(text(1)?.to_owned())
                    .map_err(|error| error.to_string())?;
                Ok(Value::Null)
            }
            ("MainThreadWindow", "$getInitialState") => {
                Ok(json!({"isFocused":true,"isActive":true}))
            }
            ("MainThreadLanguageModelTools", "$getTools") => Ok(json!([])),
            ("MainThreadWebviewPanels", "$createWebviewPanel") => {
                let handle = text(1)?;
                self.webview_handle = Some(handle.to_owned());
                self.status = args
                    .get(3)
                    .and_then(|data| data["title"].as_str())
                    .unwrap_or("Webview")
                    .to_owned();
                self.show_webview_html("<!doctype html><html><head><title>Graphite Webview</title></head><body></body></html>")?;
                Ok(Value::Null)
            }
            ("MainThreadWebviewPanels", "$disposeWebview") => {
                let handle = text(0)?;
                if self.webview_handle.as_deref() == Some(handle) {
                    self.close_webview();
                }
                Ok(Value::Null)
            }
            ("MainThreadWebviewPanels", "$setTitle" | "$reveal") => Ok(Value::Null),
            ("MainThreadWebviews", "$setHtml") => {
                let handle = text(0)?;
                if self.webview_handle.as_deref() == Some(handle) {
                    let html = text(1)?;
                    self.show_webview_html(html)?;
                }
                Ok(Value::Null)
            }
            ("MainThreadWebviews", "$setOptions") => Ok(Value::Null),
            ("MainThreadWebviews", "$postMessage") => {
                let handle = text(0)?;
                if self.webview_handle.as_deref() == Some(handle) {
                    let message = text(1)?;
                    let encoded =
                        serde_json::to_string(message).map_err(|error| error.to_string())?;
                    if let Some(webview) = &self.webview {
                        webview.evaluate_script(&format!(
                            "window.postMessage(JSON.parse({encoded}), '*')"
                        ))?;
                    }
                }
                Ok(json!(true))
            }
            ("MainThreadProgress", "$startProgress") => {
                if let Some(title) = arg(1)?["title"].as_str() {
                    self.status = title.to_owned();
                }
                Ok(Value::Null)
            }
            ("MainThreadProgress", "$progressReport") => {
                if let Some(message) = arg(1)?["message"].as_str() {
                    self.status = message.to_owned();
                }
                Ok(Value::Null)
            }
            ("MainThreadExtensionService", "$onExtensionActivationError")
            | ("MainThreadExtensionService", "$onExtensionRuntimeError")
            | ("MainThreadErrors", "$onUnexpectedError") => {
                let error = args.iter().find_map(|value| value["message"].as_str());
                if let Some(error) = error {
                    self.status = error.to_owned();
                }
                Ok(Value::Null)
            }
            (
                "MainThreadExtensionService",
                "$onWillActivateExtension" | "$onDidActivateExtension",
            )
            | ("MainThreadExtensionService", "$setPerformanceMarks")
            | ("MainThreadConsole", "$logExtensionHostMessage")
            | ("MainThreadFileSystem", "$ensureActivation")
            | ("MainThreadDebugService", "$registerDebugTypes")
            | ("MainThreadTask", "$registerSupportedExecutions" | "$registerTaskSystem")
            | ("MainThreadLogger", "$registerLogger" | "$deregisterLogger")
            | ("MainThreadProgress", "$progressEnd")
            | ("MainThreadTelemetry", "$publicLog" | "$publicLog2") => Ok(Value::Null),
            _ => {
                self.record_unsupported_vsix_call(runtime, actor, method);
                if is_silent_registration(method) {
                    Ok(Value::Null)
                } else {
                    Err(format!("Graphite does not support {actor}.{method}"))
                }
            }
        }
    }

    /// Notes each unsupported `actor.method` once per session in
    /// `<profile>/code-oss/unsupported-api.log`, so compatibility gaps can be measured.
    fn record_unsupported_vsix_call(&self, runtime: &mut Runtime, actor: &str, method: &str) {
        let call = format!("{actor}.{method}");
        if !runtime.unsupported.insert(call.clone()) {
            return;
        }
        let directory = self.profile_root.join("code-oss");
        if std::fs::create_dir_all(&directory).is_err() {
            return;
        }
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("unsupported-api.log"))
        {
            let _ = writeln!(file, "{call}");
        }
    }
}

/// Registrations of capabilities Graphite does not implement. The host sends many of them while
/// starting, and an extension cannot act on a refusal, so they are accepted without an error.
fn is_silent_registration(method: &str) -> bool {
    ["$register", "$unregister", "$emit"]
        .iter()
        .any(|prefix| method.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_capability_registrations_are_accepted_silently() {
        assert!(is_silent_registration("$registerTextSearchProvider"));
        assert!(is_silent_registration("$unregisterCommand"));
        assert!(is_silent_registration("$emitCodeLensEvent"));
        assert!(!is_silent_registration("$executeTask"));
        assert!(!is_silent_registration("$tryApplyEdits"));
    }
}
