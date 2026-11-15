use super::*;

impl App {
    /// Re-highlights the buffer only when its text changed since the last run.
    pub(crate) fn refresh_highlights(&mut self) {
        self.refresh_document_highlights(self.active_document());
    }

    pub(crate) fn refresh_document_highlights(&mut self, document: DocumentId) {
        let tab = &mut self.documents[document];
        let Some(lang) = tab.language else {
            // Not a language tree-sitter knows: an extension may contribute a grammar for it.
            self.refresh_textmate_highlights(document);
            return;
        };
        let revision = tab.buffer.revision();
        if tab
            .highlights
            .as_ref()
            .is_some_and(|h| h.revision == revision)
        {
            return;
        }
        let edit = if tab
            .highlights
            .as_ref()
            .is_some_and(|h| h.revision + 1 == revision)
        {
            tab.buffer.last_change().map(|c| TextEdit {
                start_byte: c.start_byte,
                old_end_byte: c.old_end_byte,
                new_end_byte: c.new_end_byte,
                start_position: (c.start_row, c.start_column),
                old_end_position: (c.old_end_row, c.old_end_column),
                new_end_position: (c.new_end_row, c.new_end_column),
            })
        } else {
            None
        };
        let lines = tab
            .highlighter
            .highlight_rope(lang, tab.buffer.rope(), edit);
        tab.highlights = Some(HighlightCache { revision, lines });
    }

    /// Opens `path`, with the custom editor an extension offers for it by default, if any.
    pub(crate) fn open_file(&mut self, path: PathBuf) {
        if self.documents.id_for_path(&path).is_none() {
            if let Some(view_type) = self.default_notebook_type(&path) {
                // The notebook serializer reads the file; the notebook opens when it answers.
                if self.open_serialized_notebook(&path, &view_type) {
                    return;
                }
            }
        }
        let editor = self.default_custom_editor(&path);
        self.open_file_as_text(path.clone());
        if let Some(editor) = editor {
            if self.documents.id_for_path(&path).is_some() && !self.custom_editor_is_bound(&path) {
                self.bind_custom_editor(&path, &editor.view_type);
            }
        }
    }

    /// Opens `path` in the text editor.
    pub(crate) fn open_file_as_text(&mut self, path: PathBuf) {
        self.editor_groups
            .active_view_mut()
            .smart_selection_history
            .clear();
        self.editor_groups.active_view_mut().smart_selection_guard = None;
        if let Some(index) = self.documents.id_for_path(&path) {
            if index != self.active_document() {
                self.push_navigation_location();
            }
            self.activate_editor_document(index);
            self.push_navigation_location();
            self.status = path.display().to_string();
            return;
        }
        match crate::file_format::read(&path, &self.translator) {
            Ok(decoded) => {
                let mut buffer = Buffer::from_text(&path, &decoded.text);
                if std::fs::metadata(&path).is_ok_and(|metadata| metadata.permissions().readonly())
                {
                    buffer.set_read_only(true);
                }
                #[cfg(unix)]
                if std::fs::OpenOptions::new().write(true).open(&path).is_err() {
                    buffer.set_read_only(true);
                }
                #[cfg(windows)]
                if std::fs::OpenOptions::new().write(true).open(&path).is_err() {
                    buffer.set_read_only(true);
                }
                self.push_navigation_location();
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                let relative = canonical
                    .strip_prefix(self.tree.root_for(&canonical))
                    .unwrap_or(&canonical)
                    .to_string_lossy()
                    .replace('\\', "/");
                let matches_pattern = self.read_only_patterns.iter().any(|pattern| {
                    globset::Glob::new(pattern)
                        .is_ok_and(|glob| glob.compile_matcher().is_match(&relative))
                });
                if matches_pattern {
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
                let tab = Document {
                    buffer,

                    language: Language::from_path(&path),
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
                    custom_dirty: false,
                };
                let mut tab = tab;
                super::notebook_ui::attach_notebook(&mut tab);
                if self.documents.len() == 1
                    && self.editor_groups.group_count() == 1
                    && self.documents[self.active_document()]
                        .buffer
                        .path()
                        .is_none()
                    && !self.documents[self.active_document()].is_dirty()
                {
                    let (id, _) = self.documents.replace(self.active_document(), tab);
                    self.editor_groups = super::editor_groups::EditorGroups::new(id);
                } else {
                    let id = self.documents.push(tab);
                    self.activate_editor_document(id);
                }
                self.register_lsp_active();
                self.push_navigation_location();
                self.status = path.display().to_string();
            }
            Err(e) => {
                // Not text (binary, or not UTF-8): show the bytes in a read-only hex view.
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        let text = super::custom_editors::hex_view(&bytes);
                        self.open_read_only_view(path, &text);
                    }
                    Err(_) => {
                        self.status = self.tf(
                            "document.open_failed",
                            &[("path", &path.display()), ("error", &e)],
                        );
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn save(&mut self) {
        self.save_document(self.active_document());
        self.wait_for_saves();
    }

    pub(crate) fn reload_after_branch_checkout(
        &mut self,
        documents: Vec<(PathBuf, Result<crate::file_format::DecodedFile, String>)>,
    ) {
        for (path, decoded) in documents {
            let Some(index) = self
                .documents
                .iter()
                .find(|tab| {
                    tab.buffer
                        .path()
                        .is_some_and(|open_path| same_file_path(open_path, &path))
                })
                .map(|document| document.id)
            else {
                continue;
            };
            let decoded = match decoded {
                Ok(decoded) => decoded,
                Err(error) => {
                    self.status = self.tf(
                        "document.reload_failed",
                        &[("path", &path.display()), ("error", &error)],
                    );
                    continue;
                }
            };
            let was_read_only = self.documents[index].buffer.is_read_only();
            self.lsp_manager.close(&path);
            if was_read_only
                || std::fs::metadata(&path).is_ok_and(|metadata| metadata.permissions().readonly())
            {
                self.documents[index].buffer.set_read_only(true);
            }
            let tab = &mut self.documents[index];
            let outcome = tab.buffer.reload_text(&decoded.text);
            let line_count = tab.buffer.len_lines();
            tab.saved_format = (decoded.encoding, decoded.line_ending);
            tab.encoding = decoded.encoding;
            tab.line_ending = decoded.line_ending;
            tab.indentation = if self.detect_indentation {
                tab.buffer.detect_indentation().unwrap_or(tab.indentation)
            } else {
                tab.indentation
            };
            tab.highlights = None;
            tab.sticky_headers_cache = None;
            tab.highlighter = IncrementalHighlighter::new();
            tab.diagnostics.clear();
            tab.inlay_hints.clear();
            tab.inlay_revision = None;
            tab.schema_diagnostics.clear();
            tab.schema_revision = None;
            tab.lsp_revision = None;
            self.editor_groups.for_each_view_mut(|_, view| {
                if view.document == index {
                    view.selection_state
                        .remap(&outcome.positions, outcome.len_chars);
                    view.scroll_line = view.scroll_line.min(line_count.saturating_sub(1));
                }
            });
        }
        self.register_lsp_active();
        self.refresh_scm_async();
    }
}
