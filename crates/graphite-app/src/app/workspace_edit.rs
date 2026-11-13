use super::editor_commands::workspace_relative_path;
use super::*;
use graphite_core_api::{
    position_to_char, DocumentEdit, DocumentUri, FileSystem, LocalFileSystem,
    TextEdit as ApiTextEdit,
};
use graphite_text::{apply_document_transactions, DocumentTransaction, TextReplacement};

struct Target {
    uri: DocumentUri,
    path: Option<PathBuf>,
    version: Option<u64>,
    edits: Vec<ApiTextEdit>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_prepared_edit_does_not_add_closed_documents_or_change_history() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("first.rs");
        let second = dir.path().join("second.rs");
        std::fs::write(&first, "abc").unwrap();
        std::fs::write(&second, "xyz").unwrap();
        let mut changes = serde_json::Map::new();
        let edit = serde_json::json!([{ "range": {"start": {"line":0,"character":0}, "end": {"line":0,"character":1}}, "newText":"Q" }]);
        changes.insert(lsp::path_to_uri(&first).unwrap(), edit.clone());
        changes.insert(lsp::path_to_uri(&second).unwrap(), edit);
        let workspace = serde_json::from_value(serde_json::json!({"changes": changes})).unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(first);
        let prepared = app.prepare_workspace_edit(workspace).unwrap();
        let count = app.documents.len();
        app.editor_mut().insert("!");
        let revision = app.documents[app.active_document()].buffer.revision();
        assert!(app.commit_workspace_edit(prepared).is_err());
        assert_eq!(app.documents.len(), count);
        assert_eq!(app.documents[app.active_document()].buffer.text(), "!abc");
        assert_eq!(
            app.documents[app.active_document()].buffer.revision(),
            revision
        );
        assert_eq!(std::fs::read_to_string(second).unwrap(), "xyz");
    }
    fn edit_targets(paths: &[&Path], replacement: &str) -> lsp_types::WorkspaceEdit {
        let changes: serde_json::Map<_, _> = paths.iter().map(|path| (
            lsp::path_to_uri(path).unwrap(),
            serde_json::json!([{ "range": {"start": {"line":0,"character":0}, "end": {"line":0,"character":1}}, "newText":replacement }])
        )).collect();
        serde_json::from_value(serde_json::json!({"changes": changes})).unwrap()
    }

    #[test]
    fn closed_file_change_rejects_every_target_without_touching_redo() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("first.txt");
        let second = dir.path().join("second.txt");
        std::fs::write(&first, "abc").unwrap();
        std::fs::write(&second, "xyz").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(first.clone());
        app.editor_mut().insert("!");
        app.editor_mut().undo();
        let revision = app.editor().revision();
        let prepared = app
            .prepare_workspace_edit(edit_targets(&[&first, &second], "Q"))
            .unwrap();
        std::fs::write(&second, "external change").unwrap();
        assert!(app.commit_workspace_edit(prepared).is_err());
        assert_eq!(app.documents.len(), 1);
        assert_eq!(app.editor().revision(), revision);
        assert_eq!(app.editor().text(), "abc");
        app.editor_mut().redo();
        assert_eq!(app.editor().text(), "!abc");
    }

    #[test]
    fn prepared_edit_retains_closed_document_and_publishes_dirty_result() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("file.txt");
        std::fs::write(&path, "abc").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(path.clone());
        let id = app.active_document();
        let prepared = app
            .prepare_workspace_edit(edit_targets(&[&path], "Q"))
            .unwrap();
        assert!(app.documents.is_retained(id));
        app.close_active_tab();
        assert!(app.documents.get(id).is_some());
        assert_eq!(app.editor_groups.view_count(id), 0);
        app.commit_workspace_edit(prepared).unwrap();
        assert!(app.documents.get(id).is_some());
        assert_eq!(app.editor_groups.view_count(id), 1);
        app.activate_editor_document(id);
        assert_eq!(app.active_document(), id);
        assert_eq!(app.editor().text(), "Qbc");
        app.editor_mut().undo();
        assert_eq!(app.editor().text(), "abc");
        assert!(!app.documents[id].is_dirty());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "abc");
    }

    #[test]
    fn cancelled_preparation_releases_document_retention() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("file.txt");
        std::fs::write(&path, "abc").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(path.clone());
        let id = app.active_document();
        let prepared = app
            .prepare_workspace_edit(edit_targets(&[&path], "Q"))
            .unwrap();
        app.close_active_tab();
        drop(prepared);
        app.collect_closed_documents();
        assert!(app.documents.get(id).is_none());
        app.reopen_closed_tab();
        assert_eq!(app.editor().text(), "abc");
        app.editor_mut().undo();
        assert_eq!(app.editor().text(), "abc");
    }

    #[test]
    fn permission_change_or_opened_target_rejects_prepared_batch() {
        let dir = crate::test_support::TempDir::new();
        let first = dir.path().join("first.txt");
        let second = dir.path().join("second.txt");
        std::fs::write(&first, "abc").unwrap();
        std::fs::write(&second, "xyz").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(first.clone());
        let prepared = app
            .prepare_workspace_edit(edit_targets(&[&first, &second], "Q"))
            .unwrap();
        let original = std::fs::metadata(&second).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        std::fs::set_permissions(&second, readonly).unwrap();
        let result = app.commit_workspace_edit(prepared);
        std::fs::set_permissions(&second, original).unwrap();
        assert!(result.is_err());
        assert_eq!(app.editor().text(), "abc");
        assert_eq!(app.documents.len(), 1);
        let prepared = app
            .prepare_workspace_edit(edit_targets(&[&first, &second], "Q"))
            .unwrap();
        app.open_file(second);
        assert!(app.commit_workspace_edit(prepared).is_err());
        assert!(app.documents.iter().all(|d| d.buffer.revision() == 0));
    }

    #[test]
    fn no_op_closed_target_does_not_publish_a_document_or_view() {
        let dir = crate::test_support::TempDir::new();
        let path = dir.path().join("file.txt");
        std::fs::write(&path, "abc").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        let id = app.active_document();
        app.apply_workspace_edit(edit_targets(&[&path], "a"))
            .unwrap();
        assert_eq!(app.documents.len(), 1);
        assert_eq!(app.active_document(), id);
        assert_eq!(app.editor_groups.views(0).unwrap().0.len(), 1);
    }
}

pub(super) struct Prepared {
    index: Option<DocumentId>,
    tab: Option<Document>,
    revision: u64,
    document_id: DocumentId,
    replacements: Vec<TextReplacement>,
    source_bytes: Option<Vec<u8>>,
    path: Option<PathBuf>,
    _lease: Option<crate::document_store::DocumentLease>,
}

impl App {
    pub(super) fn apply_workspace_edit(
        &mut self,
        edit: lsp_types::WorkspaceEdit,
    ) -> Result<(), String> {
        use graphite_core_api::DocumentService;
        let edits = super::services::lsp::workspace_edits(edit).map_err(|e| e.to_string())?;
        self.apply_edits(edits).map_err(|e| e.to_string())
    }

    #[cfg(test)]
    fn prepare_workspace_edit(
        &self,
        edit: lsp_types::WorkspaceEdit,
    ) -> Result<Vec<Prepared>, String> {
        self.prepare_document_edits(
            super::services::lsp::workspace_edits(edit).map_err(|e| e.to_string())?,
        )
    }

    pub(super) fn prepare_document_edits(
        &self,
        edits: Vec<DocumentEdit>,
    ) -> Result<Vec<Prepared>, String> {
        let mut targets: Vec<Target> = Vec::new();
        for edit in edits {
            let path = if edit.uri.is_untitled() {
                None
            } else {
                let path = edit
                    .uri
                    .to_path()
                    .map_err(|e| e.to_string())?
                    .canonicalize()
                    .map_err(|e| format!("cannot resolve target: {e}"))?;
                if workspace_relative_path(self.tree.root_for(&path), &path).is_none() {
                    return Err("target outside the workspace".into());
                }
                Some(path)
            };
            if let Some(target) = targets.iter_mut().find(|target| {
                target.uri == edit.uri
                    || target
                        .path
                        .as_ref()
                        .zip(path.as_ref())
                        .is_some_and(|(a, b)| same_file_path(a, b))
            }) {
                if target.version.is_some()
                    && edit.version.is_some()
                    && target.version != edit.version
                {
                    return Err("conflicting document versions".into());
                }
                target.version = target.version.or(edit.version);
                target.edits.extend(edit.edits);
            } else {
                targets.push(Target {
                    uri: edit.uri,
                    path,
                    version: edit.version,
                    edits: edit.edits,
                });
            }
        }
        if targets.is_empty() {
            return Err("no supported text edits".into());
        }

        let mut prepared = Vec::new();
        for target in targets {
            let index = if let Some(path) = &target.path {
                self.documents.id_for_path(path)
            } else {
                self.service_document_id(&target.uri)
            };
            if target.path.is_none() && index.is_none() {
                return Err("document no longer exists".into());
            }
            if let Some(version) = target.version {
                let current = index.map(|id| self.documents[id].buffer.revision());
                if current != Some(version) {
                    return Err("stale or unavailable document version".into());
                }
            }
            let mut source_bytes = None;
            let path = target.path.as_deref();
            let tab = if index.is_none() {
                let relative = workspace_relative_path(
                    self.tree.root_for(path.ok_or("missing document path")?),
                    path.ok_or("missing document path")?,
                )
                .ok_or("target outside workspace")?
                .to_string_lossy()
                .replace('\\', "/");
                if self.read_only_patterns.iter().any(|pattern| {
                    globset::Glob::new(pattern)
                        .is_ok_and(|glob| glob.compile_matcher().is_match(&relative))
                }) {
                    return Err("target matches a read-only pattern".into());
                }
                let bytes = LocalFileSystem
                    .read(&target.uri)
                    .map_err(|e| format!("cannot read target: {e}"))?;
                let decoded = crate::file_format::decode(&bytes, &self.translator)?;
                source_bytes = Some(bytes);
                let mut buffer =
                    Buffer::from_text(path.ok_or("missing document path")?, &decoded.text);
                if LocalFileSystem
                    .read_only(&target.uri)
                    .map_err(|e| e.to_string())?
                {
                    buffer.set_read_only(true);
                }
                let indentation = if self.detect_indentation {
                    buffer.detect_indentation()
                } else {
                    None
                }
                .unwrap_or(graphite_text::Indentation {
                    insert_spaces: self.insert_spaces,
                    tab_size: self.tab_size,
                });
                Some(Document {
                    buffer,

                    language: Language::from_path(path.ok_or("missing document path")?),
                    highlights: None,
                    sticky_headers_cache: None,
                    highlighter: IncrementalHighlighter::new(),
                    diagnostics: Vec::new(),
                    inlay_hints: Vec::new(),
                    inlay_revision: None,
                    provided_folds: None,
                    schema_diagnostics: Vec::new(),
                    schema_revision: None,
                    lsp_revision: None,
                    indentation,
                    saved_format: (decoded.encoding, decoded.line_ending),
                    encoding: decoded.encoding,
                    line_ending: decoded.line_ending,
                    notebook: None,
                })
            } else {
                None
            };
            let buffer = match index {
                Some(index) => &self.documents[index].buffer,
                None => &tab.as_ref().ok_or("missing prepared document")?.buffer,
            };
            let text = buffer.text();
            let mut replacements = Vec::new();
            for edit in target.edits {
                let start = position_to_char(&text, edit.start).ok_or("invalid text range")?;
                let end = position_to_char(&text, edit.end).ok_or("invalid text range")?;
                if start > end {
                    return Err("invalid text range".into());
                }
                replacements.push(TextReplacement {
                    range: start..end,
                    text: edit.text,
                });
            }
            buffer
                .validate_transaction(&replacements)
                .map_err(|e| e.to_string())?;
            let revision = buffer.revision();
            let document_id = match index {
                Some(index) => index,
                None => next_document_id(),
            };
            prepared.push(Prepared {
                index,
                tab,
                revision,
                document_id,
                replacements,
                source_bytes,
                path: target.path,
                _lease: index.and_then(|id| self.documents.lease(id)),
            });
        }
        Ok(prepared)
    }

    pub(super) fn commit_workspace_edit(&mut self, prepared: Vec<Prepared>) -> Result<(), String> {
        let result = self.commit_prepared_workspace_edit(prepared);
        self.collect_closed_documents();
        result
    }

    fn commit_prepared_workspace_edit(&mut self, prepared: Vec<Prepared>) -> Result<(), String> {
        for target in &prepared {
            let document = match target.index {
                Some(id) => self.documents.get(id).ok_or("document no longer exists")?,
                None => target.tab.as_ref().ok_or("missing prepared document")?,
            };
            let Some(path) = document.buffer.path() else {
                if target.path.is_some() {
                    return Err("document path changed after preparation".into());
                }
                continue;
            };
            if target
                .path
                .as_ref()
                .is_none_or(|expected| !same_file_path(path, expected))
            {
                return Err("document path changed after preparation".into());
            }
            if std::fs::metadata(path)
                .map_err(|e| e.to_string())?
                .permissions()
                .readonly()
                || std::fs::OpenOptions::new().write(true).open(path).is_err()
            {
                return Err("target is no longer writable".into());
            }
            if let Some(bytes) = &target.source_bytes {
                if self.documents.id_for_path(path).is_some() {
                    return Err("target opened after preparation".into());
                }
                if std::fs::read(path).map_err(|e| e.to_string())? != *bytes {
                    return Err("target changed after preparation".into());
                }
            }
        }
        let origin_document = self.command_document();
        let origin = self.command_view_ref().id;
        let origin_before = self.command_view_ref().selection_state.clone();

        let mut staged = Vec::new();
        let mut replacements = Vec::new();
        let mut existing = HashMap::new();
        let mut staged_entries = Vec::new();
        let mut before = Vec::new();
        for target in prepared {
            let edit_index = replacements.len();
            replacements.push(target.replacements);
            match (target.index, target.tab) {
                (Some(index), None) => {
                    if self
                        .documents
                        .get(index)
                        .is_none_or(|tab| tab.buffer.revision() != target.revision)
                    {
                        return Err("document changed before applying edits".into());
                    }
                    existing.insert(index, (edit_index, target.revision));
                    before.push((index, target.revision));
                }
                (None, Some(tab)) => {
                    staged.push(crate::document_store::ClosedDocument {
                        id: target.document_id,
                        document: tab,
                    });
                    staged_entries.push((edit_index, target.revision));
                }
                _ => return Err("invalid prepared document".into()),
            }
        }
        let mut transactions: Vec<_> = self
            .documents
            .iter_mut()
            .filter_map(|(index, tab)| {
                existing
                    .get(&index)
                    .map(|&(edit_index, revision)| DocumentTransaction {
                        buffer: &mut tab.buffer,
                        expected_revision: Some(revision),
                        replacements: &replacements[edit_index],
                    })
            })
            .collect();
        transactions.extend(staged.iter_mut().zip(&staged_entries).map(
            |(tab, &(edit_index, revision))| DocumentTransaction {
                buffer: &mut tab.document.buffer,
                expected_revision: Some(revision),
                replacements: &replacements[edit_index],
            },
        ));
        apply_document_transactions(&mut transactions).map_err(|error| error.to_string())?;
        drop(transactions);
        for (tab, (_, revision)) in staged.into_iter().zip(staged_entries) {
            if tab.document.buffer.revision() == revision {
                continue;
            }
            let index = self.documents.reopen(tab);
            self.editor_groups.add_background_document(index);
            before.push((index, revision));
        }
        for (index, revision) in before {
            if self.documents[index].buffer.revision() != revision
                && self.editor_groups.view_count(index) == 0
            {
                self.editor_groups.add_background_document(index);
            }
            self.remap_document_views(index, revision);
            if index == origin_document && self.documents[index].buffer.revision() != revision {
                let after = self.command_view_ref().selection_state.clone();
                self.documents[index].buffer.record_selection_history(
                    origin,
                    origin_before.clone(),
                    after,
                );
            }
            self.sync_lsp_document(index);
        }
        Ok(())
    }
}
