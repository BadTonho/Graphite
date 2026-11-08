use super::*;
impl SaveService for App {
    fn save(&mut self, request: SaveRequest, cancellation: CancellationToken) -> CommandTicket {
        self.command_runtime.next += 1;
        let id = self.command_runtime.next;
        let (sender, receiver) = std::sync::mpsc::channel();
        let ticket = CommandTicket {
            id,
            receiver,
            cancellation: cancellation.clone(),
        };
        if let Err(error) = cancellation.check() {
            let _ = sender.send(Err(error));
            return ticket;
        }
        let Some(document) = self.service_document_id(&request.uri) else {
            let _ = sender.send(Err(CommandError::TargetClosed));
            return ticket;
        };
        self.command_runtime.pending.insert(
            id,
            super::super::command_execution::PendingCommand {
                target: CommandTarget::Global,
                command: "document.save".into(),
                workspace: self.tree.root().into(),
                revision: None,
                workspace_revisions: None,
                cancellation: cancellation.clone(),
                deferred: true,
                sender,
            },
        );
        let job = super::super::command_execution::CommandJob { id, cancellation };
        self.save_batch(
            vec![document],
            super::super::saving::AfterSave::Service {
                job,
                reason: request.reason,
            },
        );
        ticket
    }
}
