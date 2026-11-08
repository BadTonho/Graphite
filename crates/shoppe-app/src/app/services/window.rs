use super::*;
impl WindowService for App {
    fn open_document(&mut self, uri: &DocumentUri) -> ServiceResult<DocumentSnapshot> {
        if uri.is_untitled() {
            let id = self
                .service_document_id(uri)
                .ok_or(ServiceError::NotFound)?;
            self.activate_editor_document(id);
        } else {
            let path = uri.to_path()?;
            self.open_file(path);
        }
        let id = self
            .service_document_id(uri)
            .ok_or_else(|| ServiceError::Failed(self.status.clone()))?;
        snapshot_document(id, &self.documents[id])
    }
    fn focus_view(&mut self, view: u64) -> ServiceResult<()> {
        let ticket = self.execute_command(CommandRequest {
            id: "shoppe.view.focus".into(),
            arguments: serde_json::Value::Null,
            target: CommandTarget::View(view),
        });
        ticket
            .try_result()
            .unwrap_or(Err(CommandError::Failed(
                "Focus unexpectedly pending".into(),
            )))
            .map(|_| ())
            .map_err(|e| ServiceError::Failed(e.to_string()))
    }
    fn show_message(&mut self, message: String) -> ServiceResult<()> {
        self.status = message;
        Ok(())
    }
}
