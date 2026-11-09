use super::*;

impl App {
    pub(crate) fn push_navigation_location(&mut self) {
        let location = (self.active_document(), self.editor().cursor().char_idx);
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
            self.status = self.t("editor.navigation.no_history").into();
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
                self.status = self.t("editor.navigation.no_more").into();
                return;
            };
            index = next;
            let (document_id, position) = self.navigation_history[index];
            if let Some(document) = self.documents.resolve(document_id) {
                self.navigation_history_index = Some(index);
                self.activate_editor_document(document);
                self.editor_mut().set_selection(position, position);
                self.reveal_cursor = true;
                self.status = self.t("editor.navigation.moved").into();
                return;
            }
        }
    }

    pub(crate) fn switch_recent_document(&mut self, reverse: bool) {
        let current_id = self.active_document();
        let target = if reverse {
            self.recent_documents
                .iter()
                .rev()
                .find(|id| **id != current_id && self.documents.resolve(**id).is_some())
        } else {
            self.recent_documents
                .iter()
                .find(|id| **id != current_id && self.documents.resolve(**id).is_some())
        }
        .and_then(|id| self.documents.resolve(*id));
        if let Some(document) = target {
            self.activate_editor_document(document);
            self.status = self.t("editor.tabs.switched_recent").into();
        }
    }

    pub(crate) fn diagnostic_entries(&self) -> Vec<(DocumentId, lsp_types::Diagnostic)> {
        let mut entries = self
            .documents
            .iter()
            .flat_map(|tab| {
                let document = tab.id;
                tab.diagnostics
                    .iter()
                    .chain(&tab.schema_diagnostics)
                    .cloned()
                    .map(move |diagnostic| (document, diagnostic))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|(document, diagnostic)| {
            let path = self.documents[*document]
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
        let Some(closed) = self.closed_tabs.pop() else {
            self.status = self.t("editor.tabs.no_closed").into();
            return;
        };
        let existing = if let Some(document) = &closed.document {
            document
                .document
                .buffer
                .path()
                .and_then(|path| self.documents.id_for_path(path))
        } else {
            (self.editor_groups.view_count(closed.id) > 0).then_some(closed.id)
        };
        if let Some(id) = existing {
            self.activate_editor_document(id);
            self.status = self.t("editor.tabs.reopened").into();
            return;
        }
        let replace_empty = self.documents.len() == 1
            && self.documents[self.active_document()]
                .buffer
                .path()
                .is_none()
            && !self.documents[self.active_document()].is_dirty()
            && self.editor_groups.group_count() == 1;
        if replace_empty {
            self.documents.remove(self.active_document());
        }
        let id = if let Some(document) = closed.document {
            self.documents.reopen(document)
        } else {
            closed.id
        };
        if replace_empty {
            self.editor_groups = super::editor_groups::EditorGroups::new(id);
        } else {
            self.activate_editor_document(id);
        }
        self.editor_groups.restore_closed_view(closed.view);
        self.register_lsp_active();
        self.push_navigation_location();
        self.status = self.t("editor.tabs.reopened").into();
    }

    /// Detach closed documents only after their pending operations release them.
    pub(super) fn collect_closed_documents(&mut self) {
        self.closed_tabs.retain(|closed| {
            closed.document.is_some() || self.editor_groups.view_count(closed.id) == 0
        });
        for closed in &mut self.closed_tabs {
            if closed.document.is_none() && !self.documents.is_retained(closed.id) {
                if let Some(path) = self.documents[closed.id].buffer.path() {
                    self.lsp_manager.close(path);
                }
                closed.document = Some(self.documents.remove(closed.id));
            }
        }
    }

    fn remember_closed_view(&mut self, view: super::editor_groups::View) {
        let id = view.document;
        let document = if self.documents.is_retained(id) {
            None
        } else {
            if let Some(path) = self.documents[id].buffer.path() {
                self.lsp_manager.close(path);
            }
            Some(self.documents.remove(id))
        };
        self.closed_tabs
            .push(super::editor_groups::ClosedTab { id, document, view });
        // A retained entry stays in the history until the operation finishes.
        while self.closed_tabs.len() > 20 {
            if let Some(index) = self
                .closed_tabs
                .iter()
                .position(|closed| closed.document.is_some())
            {
                self.closed_tabs.remove(index);
            } else {
                break;
            }
        }
    }

    pub(crate) fn request_window_close(&mut self) {
        if self.documents.iter().any(|tab| tab.is_dirty()) {
            self.close_window_confirm = true;
            self.status = self.t("editor.close.unsaved_prompt").into();
        } else {
            self.exit_requested = true;
        }
    }

    #[cfg(test)]
    pub(crate) fn save_all_before_close(&mut self) {
        let documents = self
            .documents
            .iter()
            .filter(|d| d.is_dirty() || self.saving.document_pending(d.id))
            .map(|d| d.id)
            .collect();
        self.close_window_confirm = false;
        self.save_batch(
            documents,
            super::saving::AfterSave::Window { execution: None },
        );
    }

    pub(crate) fn accept_completion(&mut self) {
        let Some(item) = self.completion_items.get(self.completion_index).cloned() else {
            return;
        };
        let text = item.insert_text.unwrap_or_else(|| item.label.clone());
        if self.editor().is_read_only() {
            self.completion_items.clear();
            self.status = self.t("editor.read_only").into();
            return;
        }
        self.editor_mut().complete_word(&text);
        self.completion_items.clear();
        self.hover_text = None;
    }

    pub(crate) fn navigate_diagnostic(&mut self, direction: isize) {
        let diagnostics = self.diagnostic_entries();
        if diagnostics.is_empty() {
            self.status = self.t("editor.diagnostics.none").into();
            return;
        }
        let previous_index =
            self.diagnostic_navigation
                .as_ref()
                .and_then(|(document_id, range, message)| {
                    diagnostics.iter().position(|(document, diagnostic)| {
                        *document == *document_id
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
            let current_path = self.documents[self.active_document()]
                .buffer
                .path()
                .map(|path| path.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let current_position = self.editor().cursor().char_idx;
            let current_line = self.documents[self.active_document()]
                .buffer
                .rope()
                .char_to_line(current_position);
            let current_col = current_position
                - self.documents[self.active_document()]
                    .buffer
                    .line_start_char(current_line);
            let current_utf16_col = self.documents[self.active_document()]
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
                        let path = self.documents[*document]
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
                        let path = self.documents[*document]
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
        let document_id = document;
        self.diagnostic_navigation =
            Some((document_id, diagnostic.range, diagnostic.message.clone()));
        self.push_navigation_location();
        self.activate_editor_document(document);
        let line = (diagnostic.range.start.line as usize).min(
            self.documents[self.active_document()]
                .buffer
                .len_lines()
                .saturating_sub(1),
        );
        let col =
            utf16_column_to_chars(&self.editor().line(line), diagnostic.range.start.character);
        self.editor_mut().set_cursor_line_col(line, col);
        self.status = diagnostic.message.clone();
        self.push_navigation_location();
        self.request_quick_fixes_for(diagnostic);
    }

    pub(crate) fn remember_diagnostic(
        &mut self,
        document: DocumentId,
        diagnostic: &lsp_types::Diagnostic,
    ) {
        if self.documents.get(document).is_some() {
            self.diagnostic_navigation =
                Some((document, diagnostic.range, diagnostic.message.clone()));
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
            self.fail_current_command(super::command_execution::CommandError::Failed(
                self.t("lsp.quick_fix.no_action").into(),
            ));
            return;
        };
        if let Some(edit) = edit {
            if let Err(error) = self.apply_workspace_edit(edit) {
                self.quick_fix_visible = false;
                self.status = self.tf("lsp.quick_fix.apply_failed", &[("error", &error)]);
                self.fail_current_command(super::command_execution::CommandError::Failed(error));
                return;
            }
        }
        self.quick_fix_visible = false;
        self.quick_fix_actions.clear();
        if let Some(command) = command {
            self.lsp_request_id = self.lsp_request_id.wrapping_add(1);
            let request = self.lsp_request_id;
            let sent = self.documents[self.command_document()]
                .buffer
                .path()
                .is_some_and(|path| {
                    self.lsp_manager.execute_server_command_tracked(
                        path,
                        command.command,
                        command.arguments.unwrap_or_default(),
                        request,
                    )
                });
            if sent {
                if let Some(path) = self.documents[self.command_document()]
                    .buffer
                    .path()
                    .map(PathBuf::from)
                {
                    self.track_lsp_command(request, &path);
                }
            } else {
                self.fail_current_command(super::command_execution::CommandError::Failed(
                    self.t("lsp.command.unavailable").into(),
                ));
            }
            self.status = if sent {
                self.t("lsp.command.requested")
            } else {
                self.t("lsp.command.unavailable")
            }
            .into();
        } else {
            self.status = self.t("lsp.quick_fix.applied").into();
        }
    }

    pub(crate) fn close_active_tab(&mut self) {
        let view = self.command_view_ref().clone();
        let document = view.document;
        let last_view = self.editor_groups.view_count(document) == 1;
        if !self.editor_groups.close_view(view.id, false) {
            let replacement = self.documents.push(Document {
                buffer: Buffer::new(),

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
                saved_format: (
                    crate::file_format::Encoding::Utf8,
                    crate::file_format::LineEnding::Lf,
                ),
                encoding: crate::file_format::Encoding::Utf8,
                line_ending: crate::file_format::LineEnding::Lf,
            });
            self.editor_groups = super::editor_groups::EditorGroups::new(replacement);
        }
        if last_view {
            self.remember_closed_view(view);
        }
        self.status = self.t("editor.tabs.closed").into();
        self.check_cli_waiters();
    }
}

#[cfg(test)]
pub(super) fn strict_lsp_position_to_char(
    text: &str,
    position: lsp_types::Position,
) -> Option<usize> {
    graphite_core_api::position_to_char(
        text,
        graphite_core_api::Position {
            line: position.line,
            character: position.character,
        },
    )
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
