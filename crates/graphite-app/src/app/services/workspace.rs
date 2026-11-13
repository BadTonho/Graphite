use super::*;
impl App {
    pub(crate) fn service_configuration(&self) -> Result<serde_json::Value, String> {
        if let Some(settings) = &self.service_configuration_override {
            return Ok(settings.clone());
        }
        crate::config::workspace_configuration_in(
            &self.profile_root,
            self.tree.root(),
            &self.translator,
        )
    }
}
impl WorkspaceService for App {
    fn workspace(&self) -> ServiceResult<WorkspaceSnapshot> {
        Ok(WorkspaceSnapshot {
            root: DocumentUri::from_path(self.tree.root())?,
        })
    }
    fn configuration(&self) -> ServiceResult<serde_json::Value> {
        let mut settings = self.service_configuration().map_err(ServiceError::Failed)?;
        let effective = serde_json::json!({
            "editor.fontSize":self.font_size,"editor.tabSize":self.tab_size,"editor.insertSpaces":self.insert_spaces,
            "editor.detectIndentation":self.detect_indentation,"editor.wordWrap":if self.word_wrap{"on"}else{"off"},
            "editor.rulers":self.rulers,"files.trimTrailingWhitespace":self.trim_trailing_whitespace,"editor.formatOnSave":self.format_on_save,"editor.formatOnPaste":self.format_on_paste,"editor.formatOnType":self.format_on_type,
            "files.readonlyInclude":self.read_only_patterns,
            "editor.wrappingIndent":match self.wrapping_indent{config::WrapIndent::None=>"none",config::WrapIndent::Same=>"same",config::WrapIndent::Indent=>"indent",config::WrapIndent::DeepIndent=>"deepIndent"},
        });
        settings
            .as_object_mut()
            .ok_or_else(|| ServiceError::Failed(self.t("config.settings.invalid_object").into()))?
            .extend(effective.as_object().unwrap().clone());
        Ok(settings)
    }
}
