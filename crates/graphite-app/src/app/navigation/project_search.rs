use super::*;

impl App {
    pub(crate) fn open_project_search(&mut self) {
        self.open_project_search_mode(false);
    }

    pub(crate) fn open_project_search_mode(&mut self, replace: bool) {
        self.close_plugins();
        self.local_search = None;
        self.context_keys
            .close_widget(super::context_keys::WidgetKind::Find);
        self.context_keys
            .close_widget(super::context_keys::WidgetKind::ProjectSearch);
        self.search_query = Some(String::new());
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_cancel
            .store(self.search_generation, Ordering::Relaxed);
        self.search_results.clear();
        self.search_index = 0;
        self.search_replacement = replace.then(String::new);
        self.search_editing_replacement = false;
        self.search_filters.focus = Default::default();
        self.status = self.t("status.project_search.prompt").into();
    }

    pub(crate) fn close_project_search(&mut self) {
        if self.search_query.is_none() {
            return;
        }
        self.search_query = None;
        self.search_replacement = None;
        self.search_editing_replacement = false;
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_cancel
            .store(self.search_generation, Ordering::Relaxed);
    }

    pub(crate) fn execute_project_search_command(&mut self, command: &str) {
        if self.search_query.is_none() {
            return;
        }
        match command {
            "projectSearch.close" => {
                self.close_project_search();
                self.status = self.t("search.closed").into();
            }
            "projectSearch.deleteLeft" => self.delete_project_search_left(),
            "projectSearch.toggleReplaceField" => self.cycle_search_focus(),
            "projectSearch.toggleRegex"
            | "projectSearch.toggleCase"
            | "projectSearch.toggleWholeWord" => self.toggle_project_search_option(command),
            "projectSearch.toggleDetails" => self.toggle_project_search_details(),
            "projectSearch.replaceAll" if self.search_replacement.is_some() => {
                self.replace_project_search_all();
            }
            "projectSearch.next" if !self.search_results.is_empty() => {
                self.search_index = (self.search_index + 1) % self.search_results.len()
            }
            "projectSearch.previous" if !self.search_results.is_empty() => {
                self.search_index = self
                    .search_index
                    .checked_sub(1)
                    .unwrap_or(self.search_results.len() - 1)
            }
            "projectSearch.accept" => {
                if let Some(hit) = self.search_results.get(self.search_index).cloned() {
                    self.open_file(hit.path);
                    let mut b = self.editor_mut();
                    let col = hit.text.get(..hit.start).unwrap_or("").chars().count();
                    b.set_cursor_line_col(hit.line, col);
                    drop(b);
                    self.search_query = None;
                    self.search_replacement = None;
                    self.search_editing_replacement = false;
                    self.search_generation = self.search_generation.wrapping_add(1);
                    self.search_cancel
                        .store(self.search_generation, Ordering::Relaxed);
                } else {
                    self.status = self.t("search.no_matches").into();
                }
            }
            _ => {}
        }
    }

    pub(crate) fn replace_project_search_all(&mut self) {
        let (Some(query), Some(replacement)) = (
            self.search_query.as_deref(),
            self.search_replacement.as_deref(),
        ) else {
            return;
        };
        if query.is_empty() || self.search_results.is_empty() {
            self.status = self.t("search.no_matches").into();
            return;
        }
        for hit in &self.search_results {
            let current = self
                .documents
                .id_for_path(&hit.path)
                .map(|id| self.documents[id].buffer.text())
                .or_else(|| {
                    crate::file_format::read(&hit.path, &self.translator)
                        .ok()
                        .map(|file| file.text)
                });
            // Compare with the text the search matched (case and regex options can differ from the query).
            let matches = current
                .as_deref()
                .and_then(|text| text.lines().nth(hit.line))
                .and_then(|line| line.get(hit.start..hit.end))
                .zip(hit.text.get(hit.start..hit.end))
                .is_some_and(|(now, matched)| now == matched);
            if !matches {
                self.status = self.t("search.replace_failed").into();
                return;
            }
        }
        let mut changes = serde_json::Map::new();
        for hit in &self.search_results {
            let Some(uri) = crate::lsp::path_to_uri(&hit.path) else {
                self.status = self.t("search.replace_failed").into();
                return;
            };
            let Some(start) = hit.text.get(..hit.start) else {
                self.status = self.t("search.replace_failed").into();
                return;
            };
            let Some(end) = hit.text.get(..hit.end) else {
                self.status = self.t("search.replace_failed").into();
                return;
            };
            let edits = changes
                .entry(uri)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()));
            let Some(edits) = edits.as_array_mut() else {
                continue;
            };
            edits.push(serde_json::json!({
                "range": {
                    "start": {"line": hit.line, "character": start.encode_utf16().count()},
                    "end": {"line": hit.line, "character": end.encode_utf16().count()}
                },
                "newText": replacement
            }));
        }
        let edit = match serde_json::from_value::<lsp_types::WorkspaceEdit>(
            serde_json::json!({"changes": changes}),
        ) {
            Ok(edit) => edit,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        match self.apply_workspace_edit(edit) {
            Ok(()) => {
                let count = self.search_results.len();
                self.close_project_search();
                self.status = self.tf("search.replaced_many", &[("count", &count)]);
            }
            Err(error) => {
                self.status = self.tf("search.replace_failed_detail", &[("error", &error)])
            }
        }
    }

    pub(crate) fn accept_project_search(
        &mut self,
        query: &str,
        generation: u64,
        results: Vec<SearchResult>,
    ) -> bool {
        if self.search_generation != generation || self.search_query.as_deref() != Some(query) {
            return false;
        }
        self.search_results = results;
        self.search_index = 0;
        true
    }

    pub(crate) fn start_project_search(&mut self) {
        self.search_generation = self.search_generation.wrapping_add(1);
        let generation = self.search_generation;
        self.search_cancel.store(generation, Ordering::Relaxed);
        let Some(query) = self.search_query.clone() else {
            return;
        };
        if query.is_empty() {
            self.search_results.clear();
            self.search_index = 0;
            return;
        }
        let options = self.search_filters.options();
        if let Err(error) = graphite_workspace::validate_search(&query, &options) {
            self.search_results.clear();
            self.search_index = 0;
            self.status = self.tf("search.invalid_options", &[("error", &error)]);
            return;
        }
        let workspace = self.tree.workspace_snapshot();
        let overlays: std::collections::HashMap<PathBuf, String> = self
            .documents
            .iter()
            .filter_map(|tab| {
                if !tab.is_dirty() {
                    return None;
                }
                let path = tab.buffer.path()?;
                Some((
                    path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
                    tab.buffer.text(),
                ))
            })
            .collect();
        let cancel = self.search_cancel.clone();
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        let tracked = self
            .command_runtime
            .current
            .and_then(|id| self.command_runtime.pending.get(&id))
            .is_some_and(|pending| pending.command.ends_with("deleteLeft"));
        let job = if tracked {
            self.command_job(false)
        } else {
            None
        };
        std::thread::spawn(move || {
            let results = workspace.search_with_options(&query, &options, &overlays, || {
                cancel.load(Ordering::Relaxed) != generation
                    || job
                        .as_ref()
                        .is_some_and(|job| job.cancellation.is_cancelled())
            });
            let outcome = if cancel.load(Ordering::Relaxed) != generation {
                Err(super::command_execution::CommandError::Cancelled)
            } else {
                Ok(serde_json::Value::Null)
            };
            super::command_execution::deliver_command_event(
                &proxy,
                job.as_ref(),
                UserEvent::ProjectSearch {
                    query,
                    generation,
                    results,
                },
                outcome,
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphite_workspace::SearchResult;

    #[test]
    fn replace_all_updates_open_buffers_and_closed_files_across_workspace_roots() {
        let temp = crate::test_support::TempDir::new();
        let root_a = temp.path().join("a");
        let root_b = temp.path().join("b");
        std::fs::create_dir_all(&root_a).unwrap();
        std::fs::create_dir_all(&root_b).unwrap();
        let open = root_a.join("open.txt");
        let closed = root_b.join("closed.txt");
        std::fs::write(&open, "café 🌎 foo").unwrap();
        std::fs::write(&closed, "foo here").unwrap();
        let mut app = App::new_with_options(root_a.clone(), None, true);
        app.replace_workspace_roots(vec![root_a.clone(), root_b.clone()], None);
        app.open_file(open.clone());
        app.search_query = Some("foo".into());
        app.search_replacement = Some("bar".into());
        app.search_results = vec![
            SearchResult {
                path: open.clone(),
                line: 0,
                start: 11,
                end: 14,
                text: "café 🌎 foo".into(),
            },
            SearchResult {
                path: closed.clone(),
                line: 0,
                start: 0,
                end: 3,
                text: "foo here".into(),
            },
        ];
        app.replace_project_search_all();
        assert_eq!(
            app.documents[app.active_document()].buffer.text(),
            "café 🌎 bar"
        );
        assert!(app.documents[app.active_document()].is_dirty());
        let closed_id = app.documents.id_for_path(&closed).unwrap();
        assert_eq!(app.documents[closed_id].buffer.text(), "bar here");
        assert!(app.documents[closed_id].is_dirty());
        assert_eq!(std::fs::read_to_string(closed).unwrap(), "foo here");
        assert!(app.search_query.is_none());
    }

    #[test]
    fn replace_all_rejects_stale_results_without_partial_writes() {
        let temp = crate::test_support::TempDir::new();
        let file = temp.path().join("sample.txt");
        std::fs::write(&file, "changed").unwrap();
        let mut app = App::new_with_options(temp.path().to_path_buf(), None, true);
        app.search_query = Some("foo".into());
        app.search_replacement = Some("bar".into());
        app.search_results = vec![SearchResult {
            path: file.clone(),
            line: 0,
            start: 0,
            end: 3,
            text: "foo".into(),
        }];
        app.replace_project_search_all();
        assert_eq!(std::fs::read_to_string(file).unwrap(), "changed");
        assert!(app.search_query.is_some());
    }
}
