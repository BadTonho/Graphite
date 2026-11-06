use super::*;

impl App {
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
            .tabs
            .iter()
            .filter_map(|tab| {
                if !tab.buffer.is_dirty() {
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
