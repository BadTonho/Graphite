use super::*;
use graphite_text::TextReplacement;

impl App {
    pub(crate) fn push_navigation_location(&mut self) {
        let location = (
            self.tabs[self.active_tab].id,
            self.tabs[self.active_tab].buffer.cursor().char_idx,
        );
        if let Some(index) = self.navigation_history_index {
            self.navigation_history.truncate(index + 1);
        }
        if self.navigation_history.last() != Some(&location) {
            self.navigation_history.push(location);
            if self.navigation_history.len() > 100 {
                self.navigation_history.remove(0);
            }
        }
        self.navigation_history_index = self.navigation_history.len().checked_sub(1);
    }

    pub(crate) fn navigate_history(&mut self, direction: isize) {
        let Some(mut index) = self.navigation_history_index else {
            self.status = "No cursor navigation history".into();
            return;
        };
        loop {
            let candidate = if direction < 0 {
                index.checked_sub(1)
            } else if index + 1 < self.navigation_history.len() {
                Some(index + 1)
            } else {
                None
            };
            let Some(next) = candidate else {
                self.status = "No more cursor history".into();
                return;
            };
            index = next;
            let (document_id, position) = self.navigation_history[index];
            if let Some(document) = self.tabs.iter().position(|tab| tab.id == document_id) {
                self.navigation_history_index = Some(index);
                self.activate_editor_document(document);
                self.tabs[document].buffer.set_selection(position, position);
                self.reveal_cursor = true;
                self.status = "Cursor history".into();
                return;
            }
        }
    }

    pub(crate) fn switch_recent_document(&mut self, reverse: bool) {
        let current_id = self.tabs[self.active_tab].id;
        let target = if reverse {
            self.recent_documents
                .iter()
                .rev()
                .find(|id| **id != current_id && self.tabs.iter().any(|tab| tab.id == **id))
        } else {
            self.recent_documents
                .iter()
                .find(|id| **id != current_id && self.tabs.iter().any(|tab| tab.id == **id))
        }
        .and_then(|id| self.tabs.iter().position(|tab| tab.id == *id));
        if let Some(document) = target {
            self.activate_editor_document(document);
            self.status = "Switched to recent tab".into();
        }
    }

    pub(crate) fn diagnostic_entries(&self) -> Vec<(usize, lsp_types::Diagnostic)> {
        let mut entries = self
            .tabs
            .iter()
            .enumerate()
            .flat_map(|(document, tab)| {
                tab.diagnostics
                    .iter()
                    .chain(&tab.schema_diagnostics)
                    .cloned()
                    .map(move |diagnostic| (document, diagnostic))
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|(document, diagnostic)| {
            let path = self.tabs[*document]
                .buffer
                .path()
                .map(|path| path.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            (
                path,
                diagnostic.range.start.line,
                diagnostic.range.start.character,
            )
        });
        entries
    }

    pub(crate) fn reopen_closed_tab(&mut self) {
        let Some(tab) = self.closed_tabs.pop() else {
            self.status = "No closed tab to reopen".into();
            return;
        };
        self.snapshot_editor_view();
        if self.tabs.len() == 1
            && self.tabs[0].buffer.path().is_none()
            && !self.tabs[0].buffer.is_dirty()
            && self.editor_groups.group_count() == 1
        {
            self.tabs[0] = tab;
            self.active_tab = 0;
            self.editor_groups = super::editor_groups::EditorGroups::new(0);
        } else {
            self.tabs.push(tab);
            let document = self.tabs.len() - 1;
            self.activate_editor_document(document);
        }
        self.snapshot_editor_view();
        self.register_lsp_active();
        self.push_navigation_location();
        self.status = "Reopened closed tab".into();
    }

    pub(crate) fn request_window_close(&mut self) {
        if self.tabs.iter().any(|tab| tab.buffer.is_dirty()) {
            self.close_window_confirm = true;
            self.status = "Unsaved documents: S save all, D discard, Esc cancel".into();
        } else {
            self.exit_requested = true;
        }
    }

    pub(crate) fn save_all_before_close(&mut self) {
        self.snapshot_editor_view();
        let previous = self.active_tab;
        for index in 0..self.tabs.len() {
            if !self.tabs[index].buffer.is_dirty() {
                continue;
            }
            self.active_tab = index;
            let revision = self.tabs[index].buffer.revision();
            self.save();
            if self.tabs[index].buffer.revision() == revision + 1 {
                self.editor_groups
                    .remap_other_views(index, self.tabs[index].buffer.last_position_edits());
            }
            if self.tabs[index].buffer.is_dirty() {
                self.status = format!("Cannot close: {}", self.status);
                self.close_window_confirm = false;
                self.active_tab = previous;
                self.snapshot_editor_view();
                return;
            }
        }
        self.active_tab = previous;
        self.snapshot_editor_view();
        self.close_window_confirm = false;
        self.exit_requested = true;
    }

    pub(crate) fn accept_completion(&mut self) {
        let Some(item) = self.completion_items.get(self.completion_index).cloned() else {
            return;
        };
        let text = item.insert_text.unwrap_or_else(|| item.label.clone());
        let buffer = &mut self.tabs[self.active_tab].buffer;
        if buffer.is_read_only() {
            self.completion_items.clear();
            self.status = "Read-only document".into();
            return;
        }
        buffer.complete_word(&text);
        self.completion_items.clear();
        self.hover_text = None;
    }

    pub(crate) fn navigate_diagnostic(&mut self, direction: isize) {
        let diagnostics = self.diagnostic_entries();
        if diagnostics.is_empty() {
            self.status = "No diagnostics in open documents".into();
            return;
        }
        let previous_index =
            self.diagnostic_navigation
                .as_ref()
                .and_then(|(document_id, range, message)| {
                    diagnostics.iter().position(|(document, diagnostic)| {
                        self.tabs[*document].id == *document_id
                            && diagnostic.range == *range
                            && diagnostic.message == *message
                    })
                });
        let next_index = if let Some(index) = previous_index {
            if direction > 0 {
                (index + 1) % diagnostics.len()
            } else {
                (index + diagnostics.len() - 1) % diagnostics.len()
            }
        } else {
            let current_path = self.tabs[self.active_tab]
                .buffer
                .path()
                .map(|path| path.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let current_position = self.tabs[self.active_tab].buffer.cursor().char_idx;
            let current_line = self.tabs[self.active_tab]
                .buffer
                .rope()
                .char_to_line(current_position);
            let current_col = current_position
                - self.tabs[self.active_tab]
                    .buffer
                    .line_start_char(current_line);
            let current_utf16_col = self.tabs[self.active_tab]
                .buffer
                .line(current_line)
                .chars()
                .take(current_col)
                .map(|ch| ch.len_utf16() as u32)
                .sum::<u32>();
            let current = (current_path, current_line as u32, current_utf16_col);
            if direction > 0 {
                diagnostics
                    .iter()
                    .position(|(document, diagnostic)| {
                        let path = self.tabs[*document]
                            .buffer
                            .path()
                            .map(|path| path.to_string_lossy().to_lowercase())
                            .unwrap_or_default();
                        (
                            &path,
                            diagnostic.range.start.line,
                            diagnostic.range.start.character,
                        ) > (&current.0, current.1, current.2)
                    })
                    .unwrap_or(0)
            } else {
                diagnostics
                    .iter()
                    .rposition(|(document, diagnostic)| {
                        let path = self.tabs[*document]
                            .buffer
                            .path()
                            .map(|path| path.to_string_lossy().to_lowercase())
                            .unwrap_or_default();
                        (
                            &path,
                            diagnostic.range.start.line,
                            diagnostic.range.start.character,
                        ) < (&current.0, current.1, current.2)
                    })
                    .unwrap_or(diagnostics.len() - 1)
            }
        };
        let (document, diagnostic) = diagnostics[next_index].clone();
        let document_id = self.tabs[document].id;
        self.diagnostic_navigation =
            Some((document_id, diagnostic.range, diagnostic.message.clone()));
        self.push_navigation_location();
        self.activate_editor_document(document);
        let line = (diagnostic.range.start.line as usize).min(
            self.tabs[self.active_tab]
                .buffer
                .len_lines()
                .saturating_sub(1),
        );
        let col = utf16_column_to_chars(
            &self.tabs[self.active_tab].buffer.line(line),
            diagnostic.range.start.character,
        );
        self.tabs[self.active_tab]
            .buffer
            .set_cursor_line_col(line, col);
        self.status = diagnostic.message.clone();
        self.push_navigation_location();
        self.request_quick_fixes_for(diagnostic);
    }

    pub(crate) fn remember_diagnostic(
        &mut self,
        document: usize,
        diagnostic: &lsp_types::Diagnostic,
    ) {
        if let Some(tab) = self.tabs.get(document) {
            self.diagnostic_navigation =
                Some((tab.id, diagnostic.range, diagnostic.message.clone()));
        }
    }

    pub(crate) fn apply_quick_fix(&mut self) {
        let Some(action) = self.quick_fix_actions.get(self.quick_fix_index).cloned() else {
            self.quick_fix_visible = false;
            return;
        };
        let edit = match action {
            lsp_types::CodeActionOrCommand::CodeAction(action) => action.edit,
            lsp_types::CodeActionOrCommand::Command(command) => {
                self.quick_fix_visible = false;
                self.status = format!("Server command is not supported: {}", command.command);
                return;
            }
        };
        let Some(edit) = edit else {
            self.quick_fix_visible = false;
            self.status = "Quick fix has no workspace edit".into();
            return;
        };
        let mut edits_by_path: HashMap<PathBuf, Vec<TextReplacement>> = HashMap::new();
        if let Some(changes) = edit.changes {
            for (uri, edits) in changes {
                let Some(path) = lsp::uri_to_path(uri.as_str()) else {
                    self.quick_fix_visible = false;
                    self.status = "Quick fix contains an unsupported URI".into();
                    return;
                };
                for edit in edits {
                    let Some(range) = self.lsp_range_to_char_range(&path, edit.range) else {
                        self.quick_fix_visible = false;
                        self.status = "Quick fix contains an invalid text range".into();
                        return;
                    };
                    edits_by_path
                        .entry(path.clone())
                        .or_default()
                        .push(TextReplacement {
                            range,
                            text: edit.new_text,
                        });
                }
            }
        }
        if let Some(document_changes) = edit.document_changes {
            if let lsp_types::DocumentChanges::Edits(documents) = document_changes {
                for document in documents {
                    let Some(path) = lsp::uri_to_path(document.text_document.uri.as_str()) else {
                        self.quick_fix_visible = false;
                        self.status = "Quick fix contains an unsupported URI".into();
                        return;
                    };
                    if let Some(version) = document.text_document.version {
                        let current = self
                            .tabs
                            .iter()
                            .find(|tab| {
                                tab.buffer
                                    .path()
                                    .is_some_and(|open| same_file_path(open, &path))
                            })
                            .and_then(|tab| {
                                tab.lsp_revision
                                    .filter(|revision| *revision == tab.buffer.revision())
                            });
                        if version < 0 || current != Some(version as u64) {
                            self.quick_fix_visible = false;
                            self.status =
                                "Quick fix has a stale or unavailable document version".into();
                            return;
                        }
                    }
                    for edit in document.edits {
                        let text_edit = match edit {
                            lsp_types::OneOf::Left(edit) => edit,
                            lsp_types::OneOf::Right(edit) => edit.text_edit,
                        };
                        let Some(range) = self.lsp_range_to_char_range(&path, text_edit.range)
                        else {
                            self.quick_fix_visible = false;
                            self.status = "Quick fix contains an invalid text range".into();
                            return;
                        };
                        edits_by_path
                            .entry(path.clone())
                            .or_default()
                            .push(TextReplacement {
                                range,
                                text: text_edit.new_text,
                            });
                    }
                }
            } else {
                self.quick_fix_visible = false;
                self.status = "Quick fix contains unsupported resource operations".into();
                return;
            }
        }
        if edits_by_path.is_empty() {
            self.quick_fix_visible = false;
            self.status = "Quick fix contains no supported text edits".into();
            return;
        }
        let mut prepared = Vec::new();
        for (path, replacements) in edits_by_path {
            let Some(relative_path) = workspace_relative_path(self.tree.root(), &path) else {
                self.quick_fix_visible = false;
                self.status = format!(
                    "Quick fix targets a file outside the workspace: {}",
                    path.display()
                );
                return;
            };
            if let Some(index) = self.tabs.iter().position(|tab| {
                tab.buffer
                    .path()
                    .is_some_and(|open| same_file_path(open, &path))
            }) {
                if self.tabs[index].buffer.is_read_only() {
                    self.quick_fix_visible = false;
                    self.status = "Quick fix blocked: document is read-only".into();
                    return;
                }
                prepared.push((path, replacements, Some(index), None));
            } else {
                let relative = relative_path.to_string_lossy().replace('\\', "/");
                let read_only = self.read_only_patterns.iter().any(|pattern| {
                    globset::Glob::new(pattern)
                        .is_ok_and(|glob| glob.compile_matcher().is_match(&relative))
                });
                if read_only {
                    self.quick_fix_visible = false;
                    self.status = "Quick fix blocked: target matches a read-only pattern".into();
                    return;
                }
                let buffer = match Buffer::open(&path) {
                    Ok(buffer) => buffer,
                    Err(error) => {
                        self.quick_fix_visible = false;
                        self.status =
                            format!("Cannot read quick-fix target {}: {error}", path.display());
                        return;
                    }
                };
                prepared.push((path, replacements, None, Some(buffer)));
            }
        }
        // Validate every text transaction before changing any document. Disk
        // writes for closed files are still separate and are not atomic here.
        for (_, replacements, index, buffer) in &prepared {
            let target = match index {
                Some(index) => &self.tabs[*index].buffer,
                None => buffer.as_ref().expect("closed target has a buffer"),
            };
            if let Err(error) = target.validate_transaction(replacements) {
                self.quick_fix_visible = false;
                self.status = format!("Cannot apply quick fix: {error}");
                return;
            }
        }
        for (path, replacements, index, buffer) in prepared {
            if let Some(index) = index {
                let revision = self.tabs[index].buffer.revision();
                if let Err(error) = self.tabs[index].buffer.apply_transaction(&replacements) {
                    self.quick_fix_visible = false;
                    self.status = format!("Cannot apply quick fix: {error}");
                    return;
                }
                self.update_other_editor_views(index, revision);
                if index == self.active_tab {
                    self.sync_lsp_active();
                }
            } else if let Some(mut buffer) = buffer {
                if let Err(error) = buffer.apply_transaction(&replacements) {
                    self.quick_fix_visible = false;
                    self.status = format!("Cannot apply quick fix: {error}");
                    return;
                }
                if let Err(error) = buffer.save() {
                    self.quick_fix_visible = false;
                    self.status =
                        format!("Cannot save quick-fix target {}: {error}", path.display());
                    return;
                }
            }
        }
        self.quick_fix_visible = false;
        self.quick_fix_actions.clear();
        self.status = "Quick fix applied".into();
    }

    fn lsp_range_to_char_range(
        &self,
        path: &Path,
        range: lsp_types::Range,
    ) -> Option<std::ops::Range<usize>> {
        let text = self
            .tabs
            .iter()
            .find(|tab| {
                tab.buffer
                    .path()
                    .is_some_and(|open| same_file_path(open, path))
            })
            .map(|tab| tab.buffer.text())
            .or_else(|| {
                crate::file_format::read(path)
                    .ok()
                    .map(|decoded| decoded.text)
            })?;
        let start = strict_lsp_position_to_char(&text, range.start)?;
        let end = strict_lsp_position_to_char(&text, range.end)?;
        (start <= end).then_some(start..end)
    }

    pub(crate) fn close_active_tab(&mut self) {
        self.smart_selection_history.clear();
        self.smart_selection_guard = None;
        self.snapshot_editor_view();
        let document = self.active_tab;
        let active_selections = self.editor_groups.active_view().selections.clone();
        self.tabs[document].buffer.set_selections(active_selections);
        let last_view = self.editor_groups.view_count(document) == 1;
        if self.editor_groups.close_active(false) {
            if last_view {
                if let Some(path) = self.tabs[document].buffer.path().map(PathBuf::from) {
                    self.lsp_manager.close(&path);
                }
                let closed = self.tabs.remove(document);
                self.closed_tabs.push(closed);
                if self.closed_tabs.len() > 20 {
                    self.closed_tabs.remove(0);
                }
                self.editor_groups.forget_document(document);
            }
            self.restore_editor_view();
        } else {
            if let Some(path) = self.tabs[document].buffer.path().map(PathBuf::from) {
                self.lsp_manager.close(&path);
            }
            let closed = std::mem::replace(
                &mut self.tabs[document],
                Tab {
                    id: next_document_id(),
                    buffer: Buffer::new(),
                    scroll_line: 0,
                    language: None,
                    highlights: None,
                    sticky_headers_cache: None,
                    highlighter: IncrementalHighlighter::new(),
                    diagnostics: Vec::new(),
                    inlay_hints: Vec::new(),
                    inlay_revision: None,
                    schema_diagnostics: Vec::new(),
                    schema_revision: None,
                    lsp_revision: None,
                    indentation: graphite_text::Indentation {
                        insert_spaces: self.insert_spaces,
                        tab_size: self.tab_size,
                    },
                    encoding: crate::file_format::Encoding::Utf8,
                    line_ending: crate::file_format::LineEnding::Lf,
                },
            );
            self.closed_tabs.push(closed);
            if self.closed_tabs.len() > 20 {
                self.closed_tabs.remove(0);
            }
            self.snapshot_editor_view();
        }
        self.status = "Tab closed".into();
        self.check_cli_waiters();
    }
}

fn strict_lsp_position_to_char(text: &str, position: lsp_types::Position) -> Option<usize> {
    let mut offset = 0;
    let mut lines = text.split('\n');
    for _ in 0..position.line {
        offset += lines.next()?.chars().count() + 1;
    }
    let line = lines.next()?;
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut utf16 = 0;
    for ch in line.chars() {
        if utf16 == position.character {
            return Some(offset);
        }
        utf16 += ch.len_utf16() as u32;
        if utf16 > position.character {
            return None;
        }
        offset += 1;
    }
    (utf16 == position.character).then_some(offset)
}

#[cfg(test)]
fn lsp_position_to_char(text: &str, position: lsp_types::Position) -> usize {
    let lines = text.split_inclusive('\n').collect::<Vec<_>>();
    let line = (position.line as usize).min(lines.len().saturating_sub(1));
    let line_start = lines
        .iter()
        .take(line)
        .map(|part| part.chars().count())
        .sum::<usize>();
    line_start
        + utf16_column_to_chars(
            lines.get(line).copied().unwrap_or_default(),
            position.character,
        )
}

fn workspace_relative_path(root: &Path, path: &Path) -> Option<PathBuf> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    {
        let strip_prefix = |value: String| {
            let replaced = value.replace('/', "\\");
            replaced
                .strip_prefix(r"\\?\")
                .unwrap_or(&replaced)
                .to_owned()
        };
        let root_raw = strip_prefix(root.to_string_lossy().into_owned());
        let path_raw = strip_prefix(path.to_string_lossy().into_owned());
        let root_normalized = root_raw.trim_end_matches('\\').to_ascii_lowercase();
        let path_normalized = path_raw.to_ascii_lowercase();
        if path_normalized == root_normalized {
            return Some(PathBuf::new());
        }
        let remainder = path_normalized.strip_prefix(&root_normalized)?;
        if !remainder.starts_with('\\') {
            return None;
        }
        path_raw
            .get(root_raw.trim_end_matches('\\').len()..)
            .map(|relative| PathBuf::from(relative.trim_start_matches('\\')))
    }
    #[cfg(not(windows))]
    {
        path.strip_prefix(root).ok().map(Path::to_path_buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_lsp_positions_reject_surrogates_and_out_of_range_positions() {
        let text = "a🙂b\r\n";
        assert_eq!(
            strict_lsp_position_to_char(text, lsp_types::Position::new(0, 3)),
            Some(2)
        );
        assert_eq!(
            strict_lsp_position_to_char(text, lsp_types::Position::new(0, 2)),
            None
        );
        assert_eq!(
            strict_lsp_position_to_char(text, lsp_types::Position::new(0, 5)),
            None
        );
        assert_eq!(
            strict_lsp_position_to_char(text, lsp_types::Position::new(1, 0)),
            Some(5)
        );
        assert_eq!(
            strict_lsp_position_to_char(text, lsp_types::Position::new(2, 0)),
            None
        );
    }

    #[test]
    fn lsp_positions_convert_utf16_columns_to_unicode_character_offsets() {
        assert_eq!(
            lsp_position_to_char("a\u{1f642}b\n\u{00e9}", lsp_types::Position::new(0, 3)),
            2
        );
        assert_eq!(
            lsp_position_to_char("a\u{1f642}b\n\u{00e9}", lsp_types::Position::new(1, 1)),
            5
        );
    }
}
