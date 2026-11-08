use super::*;

impl App {
    pub(super) fn execute_project_search_command(&mut self, command: &str) {
        if self.search_query.is_none() {
            return;
        }
        match command {
            "projectSearch.close" => {
                self.search_query = None;
                self.search_generation = self.search_generation.wrapping_add(1);
                self.search_cancel
                    .store(self.search_generation, Ordering::Relaxed);
                self.status = "Search closed".into();
            }
            "projectSearch.deleteLeft" => {
                if let Some(query) = &mut self.search_query {
                    query.pop();
                }
                self.start_project_search();
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
                    self.search_generation = self.search_generation.wrapping_add(1);
                    self.search_cancel
                        .store(self.search_generation, Ordering::Relaxed);
                } else {
                    self.status = "No matches".into();
                }
            }
            _ => {}
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
        let root = self.tree.root().to_path_buf();
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
        std::thread::spawn(move || {
            let results = Workspace::open(root)
                .map(|workspace| {
                    workspace.search_with_overlays(&query, &overlays, || {
                        cancel.load(Ordering::Relaxed) != generation
                    })
                })
                .unwrap_or_default();
            let _ = proxy.send_event(UserEvent::ProjectSearch {
                query,
                generation,
                results,
            });
        });
    }
}
