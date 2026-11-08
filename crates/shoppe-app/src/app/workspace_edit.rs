use super::editor_commands::{strict_lsp_position_to_char, workspace_relative_path};
use super::*;
use shoppe_text::{apply_document_transactions, DocumentTransaction, TextReplacement};

struct Target {
    path: PathBuf,
    version: Option<i32>,
    edits: Vec<lsp_types::TextEdit>,
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
        let count = app.tabs.len();
        app.tabs[app.active_tab].buffer.insert("!");
        let revision = app.tabs[app.active_tab].buffer.revision();
        assert!(app.commit_workspace_edit(prepared).is_err());
        assert_eq!(app.tabs.len(), count);
        assert_eq!(app.tabs[app.active_tab].buffer.text(), "!abc");
        assert_eq!(app.tabs[app.active_tab].buffer.revision(), revision);
        assert_eq!(std::fs::read_to_string(second).unwrap(), "xyz");
    }
}

struct Prepared {
    index: Option<usize>,
    tab: Option<Tab>,
    revision: u64,
    document_id: DocumentId,
    replacements: Vec<TextReplacement>,
}

impl App {
    pub(super) fn apply_workspace_edit(
        &mut self,
        edit: lsp_types::WorkspaceEdit,
    ) -> Result<(), String> {
        let prepared = self.prepare_workspace_edit(edit)?;
        self.commit_workspace_edit(prepared)
    }

    fn prepare_workspace_edit(
        &self,
        edit: lsp_types::WorkspaceEdit,
    ) -> Result<Vec<Prepared>, String> {
        let mut targets: Vec<Target> = Vec::new();
        let mut add = |uri: &str,
                       version: Option<i32>,
                       edits: Vec<lsp_types::TextEdit>|
         -> Result<(), String> {
            let path = lsp::uri_to_path(uri).ok_or("unsupported URI")?;
            let path = path
                .canonicalize()
                .map_err(|e| format!("cannot resolve target: {e}"))?;
            if workspace_relative_path(self.tree.root(), &path).is_none() {
                return Err("target outside the workspace".into());
            }
            if let Some(target) = targets
                .iter_mut()
                .find(|target| same_file_path(&target.path, &path))
            {
                if target.version.is_some() && version.is_some() && target.version != version {
                    return Err("conflicting document versions".into());
                }
                target.version = target.version.or(version);
                target.edits.extend(edits);
            } else {
                targets.push(Target {
                    path,
                    version,
                    edits,
                });
            }
            Ok(())
        };
        if let Some(changes) = edit.changes {
            for (uri, edits) in changes {
                add(uri.as_str(), None, edits)?;
            }
        }
        if let Some(changes) = edit.document_changes {
            let documents = match changes {
                lsp_types::DocumentChanges::Edits(documents) => documents,
                lsp_types::DocumentChanges::Operations(operations) => operations
                    .into_iter()
                    .map(|operation| match operation {
                        lsp_types::DocumentChangeOperation::Edit(document) => Ok(document),
                        lsp_types::DocumentChangeOperation::Op(_) => {
                            Err("unsupported resource operations".to_string())
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            };
            for document in documents {
                let edits = document
                    .edits
                    .into_iter()
                    .map(|edit| match edit {
                        lsp_types::OneOf::Left(edit) => edit,
                        lsp_types::OneOf::Right(edit) => edit.text_edit,
                    })
                    .collect();
                add(
                    document.text_document.uri.as_str(),
                    document.text_document.version,
                    edits,
                )?;
            }
        }
        if targets.is_empty() {
            return Err("no supported text edits".into());
        }

        let mut prepared = Vec::new();
        for target in targets {
            let index = self.tabs.index_for_path(&target.path);
            if let Some(version) = target.version {
                let current = index.and_then(|index| {
                    let tab = &self.tabs[index];
                    tab.lsp_revision
                        .filter(|revision| *revision == tab.buffer.revision())
                });
                if version < 0 || current != Some(version as u64) {
                    return Err("stale or unavailable document version".into());
                }
            }
            let tab = if index.is_none() {
                let relative = workspace_relative_path(self.tree.root(), &target.path)
                    .ok_or("target outside workspace")?
                    .to_string_lossy()
                    .replace('\\', "/");
                if self.read_only_patterns.iter().any(|pattern| {
                    globset::Glob::new(pattern)
                        .is_ok_and(|glob| glob.compile_matcher().is_match(&relative))
                }) {
                    return Err("target matches a read-only pattern".into());
                }
                let decoded = crate::file_format::read(&target.path)
                    .map_err(|e| format!("cannot read target: {e}"))?;
                let mut buffer = Buffer::from_text(&target.path, &decoded.text);
                if std::fs::metadata(&target.path)
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .readonly()
                    || std::fs::OpenOptions::new()
                        .write(true)
                        .open(&target.path)
                        .is_err()
                {
                    buffer.set_read_only(true);
                }
                let indentation = if self.detect_indentation {
                    buffer.detect_indentation()
                } else {
                    None
                }
                .unwrap_or(shoppe_text::Indentation {
                    insert_spaces: self.insert_spaces,
                    tab_size: self.tab_size,
                });
                Some(Tab {
                    id: next_document_id(),
                    buffer,
                    scroll_line: 0,
                    language: Language::from_path(&target.path),
                    highlights: None,
                    sticky_headers_cache: None,
                    highlighter: IncrementalHighlighter::new(),
                    diagnostics: Vec::new(),
                    inlay_hints: Vec::new(),
                    inlay_revision: None,
                    schema_diagnostics: Vec::new(),
                    schema_revision: None,
                    lsp_revision: None,
                    indentation,
                    encoding: decoded.encoding,
                    line_ending: decoded.line_ending,
                })
            } else {
                None
            };
            let buffer = match index {
                Some(index) => &self.tabs[index].buffer,
                None => &tab.as_ref().ok_or("missing prepared document")?.buffer,
            };
            let text = buffer.text();
            let mut replacements = Vec::new();
            for edit in target.edits {
                let start = strict_lsp_position_to_char(&text, edit.range.start)
                    .ok_or("invalid text range")?;
                let end = strict_lsp_position_to_char(&text, edit.range.end)
                    .ok_or("invalid text range")?;
                if start > end {
                    return Err("invalid text range".into());
                }
                replacements.push(TextReplacement {
                    range: start..end,
                    text: edit.new_text,
                });
            }
            buffer
                .validate_transaction(&replacements)
                .map_err(|e| e.to_string())?;
            let revision = buffer.revision();
            let document_id = match index {
                Some(index) => self.tabs[index].id,
                None => tab.as_ref().ok_or("missing prepared document")?.id,
            };
            prepared.push(Prepared {
                index,
                tab,
                revision,
                document_id,
                replacements,
            });
        }
        Ok(prepared)
    }

    fn commit_workspace_edit(&mut self, prepared: Vec<Prepared>) -> Result<(), String> {
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
                    if self.tabs.get(index).is_none_or(|tab| {
                        tab.id != target.document_id || tab.buffer.revision() != target.revision
                    }) {
                        return Err("document changed before applying edits".into());
                    }
                    existing.insert(index, (edit_index, target.revision));
                    before.push((index, target.revision));
                }
                (None, Some(tab)) => {
                    staged.push(tab);
                    staged_entries.push((edit_index, target.revision));
                }
                _ => return Err("invalid prepared document".into()),
            }
        }
        let mut transactions: Vec<_> = self
            .tabs
            .iter_mut()
            .enumerate()
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
                buffer: &mut tab.buffer,
                expected_revision: Some(revision),
                replacements: &replacements[edit_index],
            },
        ));
        apply_document_transactions(&mut transactions).map_err(|error| error.to_string())?;
        drop(transactions);
        for (tab, (_, revision)) in staged.into_iter().zip(staged_entries) {
            let index = self.tabs.len();
            self.tabs.push(tab);
            self.editor_groups
                .add_background_document(self.tabs[index].id);
            before.push((index, revision));
        }
        for (index, revision) in before {
            self.update_other_editor_views(index, revision);
            self.sync_lsp_document(index);
        }
        Ok(())
    }
}
