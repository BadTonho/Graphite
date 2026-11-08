//! Host ownership and routing are confined to the owning UI event loop.
use super::*;
use shoppe_core_api::{protocol::*, *};
use shoppe_extension_host::{HostEvent, HostState, Supervisor};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
mod lifecycle;
mod requests;
pub(crate) struct OwnedCommand {
    pub resource: u64,
    pub session: u64,
    pub metadata: CommandMetadata,
}
pub(crate) struct Incoming {
    pub ticket: CommandTicket,
    pub deadline: Option<Instant>,
}
#[derive(Default)]
pub(crate) struct ExtensionRuntime {
    pub native_manifest: Option<shoppe_plugins::manifest::Manifest>,
    pub native_active: bool,
    pub activation_deadline: Option<Instant>,
    pub enabled: bool,
    pub supervisor: Option<Supervisor>,
    pub next_session: u64,
    pub next_resource: u64,
    pub commands: HashMap<String, OwnedCommand>,
    pub participants: std::collections::BTreeMap<u64, (u64, SaveParticipantMetadata)>,
    pub save_replies: HashMap<u64, std::sync::mpsc::Sender<ServiceResult<serde_json::Value>>>,
    pub subscriptions: HashMap<u64, DocumentSubscription>,
    pub outbound: HashMap<u64, u64>,
    pub incoming: HashMap<u64, Incoming>,
    pub last_request: u64,
    pub executable: Option<PathBuf>,
    pub wake: Option<Arc<dyn Fn() + Send + Sync>>,
}
impl App {
    pub(crate) fn execute_host_command(
        &mut self,
        request: CommandRequest,
        cancellation: CancellationToken,
    ) -> CommandTicket {
        self.refresh_command_context();
        self.command_runtime.next += 1;
        let id = self.command_runtime.next;
        let (sender, receiver) = std::sync::mpsc::channel();
        let ticket = CommandTicket {
            id,
            cancellation: cancellation.clone(),
            receiver,
        };
        let validate = || {
            cancellation.check()?;
            if self.extension_host.native_manifest.is_some() && !self.extension_host.native_active {
                return Err(CommandError::Disabled(
                    "Plugin activation is incomplete".into(),
                ));
            }
            let descriptor = self
                .extension_host
                .commands
                .get(&request.id)
                .ok_or_else(|| CommandError::UnknownCommand(request.id.clone()))?;
            if !descriptor.metadata.targets.contains(&request.target.kind()) {
                return Err(CommandError::InvalidTarget(request.id.clone()));
            }
            self.command_target_view(request.target)?;
            let context = self.context_keys.resolve_target(request.target);
            if !crate::keybindings::Condition::parse(&descriptor.metadata.enablement)
                .unwrap()
                .evaluate(&context)
            {
                return Err(CommandError::Disabled(request.id.clone()));
            }
            Ok(())
        };
        if let Err(error) = validate() {
            let _ = sender.send(Err(error));
            return ticket;
        }
        let now = Instant::now();
        let rpc = self
            .extension_host
            .supervisor
            .as_mut()
            .ok_or(ServiceError::HostStopped)
            .and_then(|host| {
                host.request(
                    RpcRequest::Invoke {
                        command: request.id.clone(),
                        arguments: request.arguments.clone(),
                    },
                    cancellation.clone(),
                    now,
                )
            });
        let rpc = match rpc {
            Ok(rpc) => rpc,
            Err(error) => {
                let _ = sender.send(Err(command_error(error)));
                return ticket;
            }
        };
        let view = self.command_target_view(request.target).unwrap();
        self.command_runtime.pending.insert(
            id,
            super::command_execution::PendingCommand {
                target: request.target,
                command: request.id,
                workspace: self.tree.root().into(),
                revision: None,
                workspace_revisions: None,
                cancellation,
                deferred: true,
                sender,
            },
        );
        let _ = view;
        self.extension_host.outbound.insert(rpc, id);
        ticket
    }
}
fn command_error(error: ServiceError) -> CommandError {
    match error {
        ServiceError::Command(error) => error,
        ServiceError::Cancelled => CommandError::Cancelled,
        ServiceError::Timeout => CommandError::Timeout,
        ServiceError::Busy => CommandError::Busy,
        ServiceError::HostStopped => CommandError::HostStopped,
        ServiceError::StaleDocument => CommandError::StaleDocument,
        other => CommandError::Failed(other.to_string()),
    }
}

#[cfg(test)]
mod tests;
