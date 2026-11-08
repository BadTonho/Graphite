use super::editor_groups::View;

/// A borrowed edit publishes its changes to every peer view when the command finishes.
pub(crate) struct EditorSession<'a> {
    pub(super) editor: graphite_text::Editor<'a>,
    pub(super) peers: Vec<&'a mut View>,
    pub(super) lsp_revision: &'a mut Option<u64>,
    pub(super) schema_revision: &'a mut Option<u64>,
    pub(super) schema_manager: Option<&'a crate::json_schema::JsonSchemaManager>,
    pub(super) manager: &'a mut super::LspManager,
    pub(super) root: &'a std::path::Path,
    pub(super) sender: std::sync::mpsc::Sender<super::LspUiEvent>,
}
impl<'a> std::ops::Deref for EditorSession<'a> {
    type Target = graphite_text::Editor<'a>;
    fn deref(&self) -> &Self::Target {
        &self.editor
    }
}
impl std::ops::DerefMut for EditorSession<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.editor
    }
}
impl Drop for EditorSession<'_> {
    fn drop(&mut self) {
        for outcome in self.editor.outcomes() {
            for view in &mut self.peers {
                if let Some((origin, selections)) = &outcome.selections {
                    if view.id == *origin {
                        view.selection_state = selections.clone();
                        continue;
                    }
                }
                view.selection_state
                    .remap(&outcome.positions, outcome.len_chars);
            }
        }
        if self.editor.outcomes().is_empty() {
            return;
        }
        let Some(path) = self.editor.path() else {
            return;
        };
        let revision = self.editor.revision();
        if let Some(manager) = self.schema_manager {
            if *self.schema_revision != Some(revision) {
                manager.validate(path.to_path_buf(), self.editor.text(), revision);
                *self.schema_revision = Some(revision);
            }
        }
        if !self.manager.supports_path(path) {
            return;
        }
        if *self.lsp_revision == Some(revision) {
            return;
        }
        if self.lsp_revision.is_none() {
            self.manager.open(
                path,
                self.root,
                self.editor.text(),
                revision as i32,
                self.sender.clone(),
            );
        } else {
            self.manager
                .change(path, self.editor.text(), revision as i32);
        }
        *self.lsp_revision = Some(revision);
    }
}
