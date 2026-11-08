use super::*;

#[derive(Default)]
pub(crate) struct QuickOpen {
    pub query: String,
    pub paths: Vec<PathBuf>,
    pub index: usize,
}

impl App {
    pub(crate) fn start_quick_open(&mut self) {
        self.local_search = None;
        self.search_query = None;
        self.quick_open = Some(QuickOpen::default());
        self.request_quick_open();
    }

    pub(crate) fn request_quick_open(&mut self) {
        self.search_generation = self.search_generation.wrapping_add(1);
        let generation = self.search_generation;
        self.search_cancel.store(generation, Ordering::Relaxed);
        let Some(query) = self.quick_open.as_ref().map(|state| state.query.clone()) else {
            return;
        };
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        let root = self.tree.root().to_path_buf();
        let cancel = self.search_cancel.clone();
        std::thread::spawn(move || {
            let paths = Workspace::open(root)
                .map(|ws| {
                    ws.fuzzy_files(&query, 100, || cancel.load(Ordering::Relaxed) != generation)
                })
                .unwrap_or_default();
            let _ = proxy.send_event(UserEvent::QuickOpen {
                query,
                generation,
                paths,
            });
        });
    }

    pub(crate) fn accept_quick_open(
        &mut self,
        query: &str,
        generation: u64,
        paths: Vec<PathBuf>,
    ) -> bool {
        let Some(state) = &mut self.quick_open else {
            return false;
        };
        if self.search_generation != generation || state.query != query {
            return false;
        }
        state.paths = paths;
        state.index = 0;
        true
    }

    pub(crate) fn handle_quick_open(&mut self, event: &winit::event::KeyEvent) {
        let Some(state) = &mut self.quick_open else {
            return;
        };
        if !self.editor_groups.active_view_mut().ime_composing
            && !self.modifiers.control_key()
            && !self.modifiers.alt_key()
        {
            if let Some(text) = &event.text {
                if !text.chars().any(char::is_control) {
                    state.query.push_str(text);
                    self.request_quick_open();
                }
            }
        }
    }

    pub(super) fn execute_quick_open_command(&mut self, command: &str) {
        let Some(state) = &mut self.quick_open else {
            return;
        };
        match command {
            "quickOpen.close" | "quickOpen.accept" => {
                let path = if command == "quickOpen.accept" {
                    state.paths.get(state.index).cloned()
                } else {
                    None
                };
                self.quick_open = None;
                self.search_generation = self.search_generation.wrapping_add(1);
                self.search_cancel
                    .store(self.search_generation, Ordering::Relaxed);
                if let Some(path) = path {
                    self.open_file(path);
                }
            }
            "quickOpen.next" if !state.paths.is_empty() => {
                state.index = (state.index + 1) % state.paths.len()
            }
            "quickOpen.previous" if !state.paths.is_empty() => {
                state.index = (state.index + state.paths.len() - 1) % state.paths.len()
            }
            "quickOpen.deleteLeft" => {
                state.query.pop();
                self.request_quick_open();
            }
            _ => {}
        }
    }
}
