use super::*;
use serde_json::{json, Value};
impl App {
    pub(crate) fn handle_host_request(
        &mut self,
        session: u64,
        id: u64,
        request: RpcRequest,
        now: Instant,
    ) {
        if self
            .extension_host
            .supervisor
            .as_ref()
            .is_none_or(|host| host.session != session || host.state != HostState::Ready)
        {
            return;
        }
        if id <= self.extension_host.last_request {
            self.reply_host(
                id,
                Err(ServiceError::Protocol("Duplicate request ID".into())),
            );
            return;
        }
        self.extension_host.last_request = id;
        if self.extension_host.incoming.len() >= MAX_PENDING {
            self.reply_host(id, Err(ServiceError::Busy));
            return;
        }
        let parent = if let RpcRequest::Service { parent, .. } = &request {
            *parent
        } else {
            None
        };
        let save_parent =
            parent.is_some_and(|id| self.extension_host.save_replies.contains_key(&id));
        if save_parent
            && !matches!(
                &request,
                RpcRequest::Service {
                    request: ServiceRequest::Documents
                        | ServiceRequest::Snapshot { .. }
                        | ServiceRequest::Commands
                        | ServiceRequest::Workspace
                        | ServiceRequest::Configuration,
                    ..
                }
            )
        {
            self.reply_host(
                id,
                Err(ServiceError::InvalidArguments(
                    "Save participants may only query services".into(),
                )),
            );
            return;
        }
        let parent_view = if let Some(parent) = parent.filter(|_| !save_parent) {
            let pending = self
                .extension_host
                .outbound
                .get(&parent)
                .and_then(|id| self.command_runtime.pending.get(id));
            let Some(pending) = pending else {
                self.reply_host(id, Err(ServiceError::Cancelled));
                return;
            };
            if pending.cancellation.is_cancelled() {
                self.reply_host(id, Err(ServiceError::Cancelled));
                return;
            }
            match self.command_target_view(pending.target) {
                Ok(view) => view,
                Err(_) => {
                    self.reply_host(id, Err(ServiceError::Cancelled));
                    return;
                }
            }
        } else {
            None
        };
        let previous_view = std::mem::replace(&mut self.command_runtime.view, parent_view);
        let result = match request {
            RpcRequest::Activated if self.extension_host.native_manifest.is_some() => {
                self.extension_host.native_active = true;
                self.extension_host.activation_deadline = None;
                Ok(Value::Null)
            }
            RpcRequest::RegisterSaveParticipant(metadata) => {
                if metadata.id.is_empty()
                    || self
                        .extension_host
                        .participants
                        .values()
                        .any(|(_, p)| p.id == metadata.id)
                {
                    Err(ServiceError::InvalidArguments(
                        "Duplicate or empty save participant".into(),
                    ))
                } else if self.extension_host.commands.len()
                    + self.extension_host.subscriptions.len()
                    + self.extension_host.participants.len()
                    >= 1024
                {
                    Err(ServiceError::Busy)
                } else {
                    self.extension_host.next_resource += 1;
                    let resource = self.extension_host.next_resource;
                    self.extension_host
                        .participants
                        .insert(resource, (session, metadata));
                    Ok(json!({"resource": resource}))
                }
            }
            RpcRequest::RegisterCommand(metadata) => self
                .register_host_command(session, metadata)
                .map(|resource| json!({"resource":resource})),
            RpcRequest::SubscribeDocuments => {
                if self.extension_host.commands.len()
                    + self.extension_host.subscriptions.len()
                    + self.extension_host.participants.len()
                    >= 1024
                {
                    self.command_runtime.view = previous_view;
                    self.reply_host(id, Err(ServiceError::Busy));
                    return;
                }
                self.extension_host.next_resource += 1;
                let resource = self.extension_host.next_resource;
                match self.subscribe_documents() {
                    Ok(subscription) => {
                        self.extension_host
                            .subscriptions
                            .insert(resource, subscription);
                        Ok(json!({"resource":resource}))
                    }
                    Err(error) => Err(error),
                }
            }
            RpcRequest::Dispose { resource } => {
                let mut found = self.extension_host.participants.remove(&resource).is_some();
                found |= self
                    .extension_host
                    .subscriptions
                    .remove(&resource)
                    .is_some();
                self.extension_host.commands.retain(|_, command| {
                    let remove = command.session == session && command.resource == resource;
                    found |= remove;
                    !remove
                });
                if found {
                    self.document_events.reap();
                    Ok(Value::Null)
                } else {
                    Err(ServiceError::NotFound)
                }
            }
            RpcRequest::Service {
                request: ServiceRequest::Save(request),
                ..
            } => {
                let ticket = SaveService::save(self, request, CancellationToken::default());
                if let Some(result) = ticket.try_result() {
                    result.map_err(ServiceError::Command)
                } else {
                    self.extension_host.incoming.insert(
                        id,
                        Incoming {
                            ticket,
                            deadline: Some(now + Duration::from_secs(30)),
                        },
                    );
                    self.command_runtime.view = previous_view;
                    return;
                }
            }
            RpcRequest::Service {
                request: ServiceRequest::Execute(request),
                ..
            } => {
                let ticket = CommandService::execute(self, request);
                if let Some(result) = ticket.try_result() {
                    result.map_err(ServiceError::Command)
                } else {
                    self.extension_host.incoming.insert(
                        id,
                        Incoming {
                            ticket,
                            deadline: Some(now + Duration::from_secs(30)),
                        },
                    );
                    self.command_runtime.view = previous_view;
                    return;
                }
            }
            RpcRequest::Service {
                request:
                    prompt_req @ (ServiceRequest::PromptInput { .. } | ServiceRequest::QuickPick { .. }),
                ..
            } => {
                if let Err(error) = self.handle_native_prompt_request(id, parent, prompt_req) {
                    Err(error)
                } else {
                    self.command_runtime.view = previous_view;
                    return;
                }
            }
            RpcRequest::Service { request, .. } => self.handle_service_request(request),
            _ => Err(ServiceError::Protocol(
                "Host cannot invoke itself through the editor".into(),
            )),
        };
        self.command_runtime.view = previous_view;
        self.reply_host(id, result);
    }
    pub(crate) fn register_host_command(
        &mut self,
        session: u64,
        metadata: CommandMetadata,
    ) -> ServiceResult<u64> {
        if let Some(manifest) = &self.extension_host.native_manifest {
            if !manifest
                .commands
                .iter()
                .any(|c| c.id == metadata.id && c.title == metadata.title)
            {
                return Err(ServiceError::InvalidArguments(
                    "Command must be declared in the plugin manifest".into(),
                ));
            }
        }
        if super::super::command_registry::registry()
            .get(&metadata.id)
            .is_some()
            || self.extension_host.commands.contains_key(&metadata.id)
        {
            return Err(ServiceError::InvalidArguments("Duplicate command".into()));
        }
        if metadata.id.is_empty()
            || metadata.title.is_empty()
            || metadata.category.is_empty()
            || metadata.targets.is_empty()
        {
            return Err(ServiceError::InvalidArguments(
                "Incomplete command metadata".into(),
            ));
        }
        crate::keybindings::Condition::parse(&metadata.enablement)
            .map_err(ServiceError::InvalidArguments)?;
        if self.extension_host.commands.len()
            + self.extension_host.subscriptions.len()
            + self.extension_host.participants.len()
            >= 1024
        {
            return Err(ServiceError::Busy);
        }
        self.extension_host.next_resource += 1;
        let resource = self.extension_host.next_resource;
        self.extension_host.commands.insert(
            metadata.id.clone(),
            OwnedCommand {
                resource,
                session,
                metadata,
            },
        );
        Ok(resource)
    }
    pub(crate) fn handle_service_request(
        &mut self,
        request: ServiceRequest,
    ) -> ServiceResult<Value> {
        fn value(value: impl serde::Serialize) -> ServiceResult<Value> {
            serde_json::to_value(value).map_err(|e| ServiceError::Failed(e.to_string()))
        }
        match request {
            ServiceRequest::Documents => value(DocumentService::documents(self)?),
            ServiceRequest::Snapshot { uri } => value(DocumentService::snapshot(self, &uri)?),
            ServiceRequest::ApplyEdits { edits } => {
                DocumentService::apply_edits(self, edits)?;
                Ok(Value::Null)
            }
            ServiceRequest::Commands => value(CommandService::commands(self)),
            ServiceRequest::Workspace => value(WorkspaceService::workspace(self)?),
            ServiceRequest::Configuration => WorkspaceService::configuration(self),
            ServiceRequest::Open { uri } => value(WindowService::open_document(self, &uri)?),
            ServiceRequest::Focus { view } => {
                WindowService::focus_view(self, view)?;
                Ok(Value::Null)
            }
            ServiceRequest::Message { text } => {
                WindowService::show_message(self, text)?;
                Ok(Value::Null)
            }
            ServiceRequest::SetStatusBarItem { id, text, command } => {
                self.extension_host
                    .status_items
                    .insert(id, NativeStatusItem { text, command });
                Ok(Value::Null)
            }
            ServiceRequest::DisposeStatusBarItem { id } => {
                self.extension_host.status_items.remove(&id);
                Ok(Value::Null)
            }
            ServiceRequest::CreateOutputChannel { name } => {
                let channel = self.create_native_output_channel(name)?;
                Ok(json!({ "channel": channel }))
            }
            ServiceRequest::AppendOutput {
                channel,
                text,
                replace,
            } => {
                self.append_native_output(&channel, &text, replace)?;
                Ok(Value::Null)
            }
            ServiceRequest::ShowOutput { channel } => {
                self.show_native_output(&channel)?;
                Ok(Value::Null)
            }
            ServiceRequest::StorageGet { scope, key } => self.native_storage_get(scope, key),
            ServiceRequest::StorageSet { scope, key, value } => {
                self.native_storage_set(scope, key, value)?;
                Ok(Value::Null)
            }
            ServiceRequest::Execute(_)
            | ServiceRequest::Save(_)
            | ServiceRequest::PromptInput { .. }
            | ServiceRequest::QuickPick { .. } => Err(ServiceError::Protocol(
                "Commands require an execution ticket".into(),
            )),
        }
    }
}
