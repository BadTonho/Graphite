use super::editor_groups::View;

/// A borrowed edit publishes its changes to every peer view when the command finishes.
pub(crate) struct EditorSession<'a> {
    pub(crate) document: super::DocumentId,
    pub(crate) language: Option<String>,
    pub(crate) format_dirty: bool,
    pub(crate) events: &'a mut super::services::DocumentEventHub,
    pub(crate) editor: graphite_text::Editor<'a>,
    pub(crate) peers: Vec<&'a mut View>,
    pub(crate) lsp_revision: &'a mut Option<u64>,
    pub(crate) schema_revision: &'a mut Option<u64>,
    pub(crate) schema_manager: Option<&'a crate::json_schema::JsonSchemaManager>,
    pub(crate) manager: &'a mut super::LspManager,
    pub(crate) root: &'a std::path::Path,
    pub(crate) sender: std::sync::mpsc::Sender<super::LspUiEvent>,
    pub(crate) translator: graphite_i18n::Translator,
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
        let uri = self
            .editor
            .path()
            .map(|path| {
                crate::app::plugins::vsix::virtual_fs::uri_for_path(path)
                    .map_or_else(|| graphite_core_api::DocumentUri::from_path(path), Ok)
            })
            .unwrap_or_else(|| Ok(graphite_core_api::DocumentUri::untitled(self.document)));
        if let Ok(uri) = uri {
            self.events.record(graphite_core_api::DocumentSnapshot {
                id: Some(self.document),
                uri,
                text: self.editor.text(),
                version: Some(self.editor.revision()),
                language: self.language.clone(),
                dirty: self.format_dirty || self.editor.is_dirty(),
                read_only: self.editor.is_read_only(),
            });
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
        if let Some(previous) = *self.lsp_revision {
            // The server has the document: send the edits, sharing the rope instead of copying.
            if super::services::lsp::publish_change(self.manager, path, &self.editor, previous)
                .is_ok()
            {
                *self.lsp_revision = Some(revision);
            }
            return;
        }
        let Ok(uri) = graphite_core_api::DocumentUri::from_path(path) else {
            return;
        };
        let Ok(root) = graphite_core_api::DocumentUri::from_path(self.root) else {
            return;
        };
        let snapshot = graphite_core_api::DocumentSnapshot {
            id: None,
            uri,
            text: self.editor.text(),
            version: Some(revision),
            language: None,
            dirty: true,
            read_only: self.editor.is_read_only(),
        };
        let workspace = graphite_core_api::WorkspaceSnapshot { root };
        if super::services::lsp::publish_document(
            self.manager,
            &snapshot,
            &workspace,
            self.sender.clone(),
            self.lsp_revision.is_some(),
            self.translator,
        )
        .is_err()
        {
            return;
        }
        *self.lsp_revision = Some(revision);
    }
}
