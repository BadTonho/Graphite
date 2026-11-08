use super::*;

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
            if let Some(document) = self.tabs.index_for(document_id) {
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
                .find(|id| **id != current_id && self.tabs.index_for(**id).is_some())
        } else {
            self.recent_documents
                .iter()
                .find(|id| **id != current_id && self.tabs.index_for(**id).is_some())
        }
        .and_then(|id| self.tabs.index_for(*id));
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
            self.tabs.replace(0, tab);
            self.active_tab = 0;
            self.editor_groups = super::editor_groups::EditorGroups::new(self.tabs[0].id);
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
                self.editor_groups.remap_other_views(
                    self.tabs[index].id,
                    self.tabs[index].buffer.last_position_edits(),
                );
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
        let (edit, command) = match action {
            lsp_types::CodeActionOrCommand::CodeAction(action) => (action.edit, action.command),
            lsp_types::CodeActionOrCommand::Command(command) => (None, Some(command)),
        };
        if edit.is_none() && command.is_none() {
            self.quick_fix_visible = false;
            self.status = "Quick fix has no workspace edit".into();
            return;
        };
        if let Some(edit) = edit {
            if let Err(error) = self.apply_workspace_edit(edit) {
                self.quick_fix_visible = false;
                self.status = format!("Cannot apply quick fix: {error}");
                return;
            }
        }
        self.quick_fix_visible = false;
        self.quick_fix_actions.clear();
        if let Some(command) = command {
            let sent = self.tabs[self.active_tab]
                .buffer
                .path()
                .is_some_and(|path| {
                    self.lsp_manager.execute_server_command(
                        path,
                        command.command,
                        command.arguments.unwrap_or_default(),
                    )
                });
            self.status = if sent {
                "Server command requested"
            } else {
                "Server command unavailable"
            }
            .into();
        } else {
            self.status = "Quick fix applied".into();
        }
    }

    pub(crate) fn close_active_tab(&mut self) {
        self.smart_selection_history.clear();
        self.smart_selection_guard = None;
        self.snapshot_editor_view();
        let document = self.active_tab;
        let active_selections = self.editor_groups.active_view().selections.clone();
        self.tabs[document].buffer.set_selections(active_selections);
        let last_view = self.editor_groups.view_count(self.tabs[document].id) == 1;
        if self.editor_groups.close_active(false) {
            if last_view {
                if let Some(path) = self.tabs[document].buffer.path().map(PathBuf::from) {
                    self.lsp_manager.close(&path);
                }
                let closed = self.tabs.remove(document);
                let closed_id = closed.id;
                self.closed_tabs.push(closed);
                if self.closed_tabs.len() > 20 {
                    self.closed_tabs.remove(0);
                }
                self.editor_groups.forget_document(closed_id);
            }
            self.restore_editor_view();
        } else {
            if let Some(path) = self.tabs[document].buffer.path().map(PathBuf::from) {
                self.lsp_manager.close(&path);
            }
            let closed = self.tabs.replace(
                document,
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
                    indentation: shoppe_text::Indentation {
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
            self.editor_groups.active_view_mut().document = self.tabs[document].id;
            self.snapshot_editor_view();
        }
        self.status = "Tab closed".into();
        self.check_cli_waiters();
    }
}

pub(super) fn strict_lsp_position_to_char(
    text: &str,
    position: lsp_types::Position,
) -> Option<usize> {
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

pub(super) fn workspace_relative_path(root: &Path, path: &Path) -> Option<PathBuf> {
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
