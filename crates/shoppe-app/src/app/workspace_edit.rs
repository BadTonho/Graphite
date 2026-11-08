use super::editor_commands::{strict_lsp_position_to_char, workspace_relative_path};
use super::*;
use shoppe_text::{apply_document_transactions, DocumentTransaction, TextReplacement};

struct Target {
    path: PathBuf,
    version: Option<i32>,
    edits: Vec<lsp_types::TextEdit>,
}

struct Prepared {
    index: Option<usize>,
    tab: Option<Tab>,
    replacements: Vec<TextReplacement>,
}

impl App {
    pub(super) fn apply_workspace_edit(
        &mut self,
        edit: lsp_types::WorkspaceEdit,
    ) -> Result<(), String> {
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
            match changes {
                lsp_types::DocumentChanges::Edits(documents) => {
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
                lsp_types::DocumentChanges::Operations(_) => {
                    return Err("unsupported resource operations".into())
                }
            }
        }
        if targets.is_empty() {
            return Err("no supported text edits".into());
        }

        let mut prepared = Vec::new();
        for target in targets {
            let index = self.tabs.iter().position(|tab| {
                tab.buffer
                    .path()
                    .is_some_and(|path| same_file_path(path, &target.path))
            });
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
                None => &tab.as_ref().expect("prepared tab").buffer,
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
            prepared.push(Prepared {
                index,
                tab,
                replacements,
            });
        }
        for target in &mut prepared {
            if let Some(tab) = target.tab.take() {
                let index = self.tabs.len();
                self.tabs.push(tab);
                self.editor_groups.add_background_document(index);
                target.index = Some(index);
            }
        }
        let before: Vec<_> = prepared
            .iter()
            .map(|p| {
                (
                    p.index.expect("document index"),
                    self.tabs[p.index.unwrap()].buffer.revision(),
                )
            })
            .collect();
        let mut transactions: Vec<_> = self
            .tabs
            .iter_mut()
            .enumerate()
            .filter_map(|(index, tab)| {
                prepared.iter().find(|p| p.index == Some(index)).map(|p| {
                    let revision = tab.buffer.revision();
                    DocumentTransaction {
                        buffer: &mut tab.buffer,
                        expected_revision: Some(revision),
                        replacements: &p.replacements,
                    }
                })
            })
            .collect();
        apply_document_transactions(&mut transactions)
            .expect("all documents were validated before commit");
        for (index, revision) in before {
            self.update_other_editor_views(index, revision);
            self.sync_lsp_document(index);
        }
        Ok(())
    }
}
