use super::*;
impl CommandService for App {
    fn commands(&self) -> Vec<CommandMetadata> {
        let mut commands: Vec<_> = super::super::command_registry::registry()
            .iter()
            .map(|c| CommandMetadata {
                id: c.id.into(),
                title: c.title.into(),
                category: c.category.into(),
                targets: c.targets.to_vec(),
                enablement: c.enablement.into(),
            })
            .collect();
        commands.extend(
            self.extension_host
                .commands
                .values()
                .map(|c| c.metadata.clone()),
        );
        commands.sort_by(|a, b| a.id.cmp(&b.id));
        for host in self.plugins.hosts.values() {
            commands.extend(host.commands.values().map(|c| c.metadata.clone()));
        }
        commands.sort_by(|a, b| a.id.cmp(&b.id));
        commands
    }
    fn execute(&mut self, request: CommandRequest) -> CommandTicket {
        self.execute_command(request)
    }
    fn cancel(&mut self, execution: u64) {
        self.cancel_command(execution)
    }
}
