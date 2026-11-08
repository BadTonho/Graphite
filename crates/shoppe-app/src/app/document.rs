use super::*;

impl App {
    /// Re-highlights the buffer only when its text changed since the last run.
    pub(crate) fn refresh_highlights(&mut self) {
        self.refresh_document_highlights(self.active_tab);
    }

    pub(crate) fn refresh_document_highlights(&mut self, document: usize) {
        let tab = &mut self.tabs[document];
        let Some(lang) = tab.language else {
            tab.highlights = None;
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

    pub(crate) fn open_file(&mut self, path: PathBuf) {
        self.smart_selection_history.clear();
        self.smart_selection_guard = None;
        if let Some(index) = self.tabs.iter().position(|t| {
            t.buffer
                .path()
                .is_some_and(|open_path| same_file_path(open_path, &path))
        }) {
            if index != self.active_tab {
                self.push_navigation_location();
            }
            self.activate_editor_document(index);
            self.push_navigation_location();
            self.status = path.display().to_string();
            return;
        }
        match crate::file_format::read(&path) {
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
                    .strip_prefix(self.tree.root())
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
                .unwrap_or(shoppe_text::Indentation {
                    insert_spaces: self.insert_spaces,
                    tab_size: self.tab_size,
                });
                let tab = Tab {
                    id: next_document_id(),
                    buffer,
                    scroll_line: 0,
                    language: Language::from_path(&path),
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
                };
                if self.tabs.len() == 1
                    && self.editor_groups.group_count() == 1
                    && self.tabs[0].buffer.path().is_none()
                    && !self.tabs[0].buffer.is_dirty()
                {
                    self.tabs[0] = tab;
                    self.active_tab = 0;
                    self.editor_groups = super::editor_groups::EditorGroups::new(0);
                    self.snapshot_editor_view();
                } else {
                    self.snapshot_editor_view();
                    self.tabs.push(tab);
                    self.activate_editor_document(self.tabs.len() - 1);
                }
                self.register_lsp_active();
                self.push_navigation_location();
                self.status = path.display().to_string();
            }
            Err(e) => self.status = format!("Cannot open {}: {e}", path.display()),
        }
    }

    pub(crate) fn save(&mut self) {
        let path = self.tabs[self.active_tab].buffer.path().map(PathBuf::from);
        let previous = self.tabs[self.active_tab].buffer.revision();
        self.status = if self.tabs[self.active_tab].buffer.path().is_none() {
            "No file to save".into()
        } else {
            let tab = &mut self.tabs[self.active_tab];
            let text = if self.trim_trailing_whitespace {
                tab.buffer.text_with_trimmed_trailing_whitespace()
            } else {
                tab.buffer.text()
            };
            let bytes = crate::file_format::encode(&text, tab.encoding, tab.line_ending);
            match if self.trim_trailing_whitespace {
                tab.buffer.save_encoded_bytes_with_trim(&bytes)
            } else {
                tab.buffer.save_encoded_bytes(&bytes)
            } {
                Ok(()) => "Saved".into(),
                Err(error) => format!("Save failed: {error}"),
            }
        };
        if self.status == "Saved" {
            if self.tabs[self.active_tab].buffer.revision() != previous {
                self.sync_lsp_active();
            }
            if let Some(path) = path {
                self.lsp_manager
                    .save(&path, self.tabs[self.active_tab].buffer.text());
                self.stage_saved_conflict(&path);
            }
            self.refresh_scm_async();
        }
    }

    pub(crate) fn reload_after_branch_checkout(
        &mut self,
        documents: Vec<(PathBuf, Result<crate::file_format::DecodedFile, String>)>,
    ) {
        self.snapshot_editor_view();
        for (path, decoded) in documents {
            let Some(index) = self.tabs.iter().position(|tab| {
                tab.buffer
                    .path()
                    .is_some_and(|open_path| same_file_path(open_path, &path))
            }) else {
                continue;
            };
            let decoded = match decoded {
                Ok(decoded) => decoded,
                Err(error) => {
                    self.status = format!(
                        "Branch changed, but could not reload {}: {error}",
                        path.display()
                    );
                    continue;
                }
            };
            let (old_line, old_column) = self.tabs[index].buffer.cursor_line_col();
            let old_scroll = self.tabs[index].scroll_line;
            let was_read_only = self.tabs[index].buffer.is_read_only();
            self.lsp_manager.close(&path);
            let mut buffer = Buffer::from_text(&path, &decoded.text);
            if was_read_only
                || std::fs::metadata(&path).is_ok_and(|metadata| metadata.permissions().readonly())
            {
                buffer.set_read_only(true);
            }
            buffer.set_cursor_line_col(old_line, old_column);
            let cursor = buffer.cursor().char_idx;
            let line_count = buffer.len_lines();
            let tab = &mut self.tabs[index];
            tab.buffer = buffer;
            tab.scroll_line = old_scroll.min(line_count.saturating_sub(1));
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
                    view.selections = vec![shoppe_text::Selection {
                        anchor: cursor,
                        head: cursor,
                    }];
                    view.scroll_line = view.scroll_line.min(line_count.saturating_sub(1));
                }
            });
        }
        self.register_lsp_active();
        self.refresh_scm_async();
    }
}
