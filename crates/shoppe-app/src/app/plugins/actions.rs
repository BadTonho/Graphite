use super::*;
impl App {
    pub(super) fn plugin_action(&mut self, action: char) {
        if self.plugins.worker.is_some() {
            return;
        }
        if action == 'g' || action == 'i' {
            self.plugins.github = action == 'g';
            self.plugins.index = 0;
            self.plugins.assets = None;
            self.plugins.editing = false;
            if action == 'g' {
                if self.plugins.path_input {
                    self.plugins.query.clear();
                    self.plugins.path_input = false;
                }
                self.plugins.page = 1;
                self.search_plugins();
            }
            return;
        }
        if action == '/' || action == 'l' {
            self.plugins.path_input = action == 'l';
            self.plugins.query.clear();
            self.plugins.editing = true;
            return;
        }
        if self.plugins.github {
            match action {
                'n' => {
                    self.plugins.page = self.plugins.page.max(1) + 1;
                    self.search_plugins();
                }
                'p' => {
                    self.plugins.page = self.plugins.page.saturating_sub(1).max(1);
                    self.search_plugins();
                }
                '\n' => {
                    if let Some(assets) = &self.plugins.assets {
                        if let Some(asset) = assets.get(self.plugins.index).cloned() {
                            let root = self.plugins.store.as_ref().unwrap().root.clone();
                            let suffix = if asset.name.ends_with(".vsix") {
                                "vsix"
                            } else {
                                "graphite-plugin"
                            };
                            let destination = root.join(format!(
                                ".download-{}-{}.{}",
                                std::process::id(),
                                self.plugins.next_session,
                                suffix
                            ));
                            self.plugins.temporary = Some(destination.clone());
                            self.catalog_job(move |catalog| {
                                std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                                catalog.download(&asset, &destination)?;
                                match shoppe_plugins::package::inspect(&destination) {
                                    Ok(metadata) => Ok(Outcome::Preview(destination, metadata)),
                                    Err(error) => {
                                        let _ = std::fs::remove_file(destination);
                                        Err(error)
                                    }
                                }
                            });
                        }
                    } else if let Some(repo) = self.plugins.repositories.get(self.plugins.index) {
                        let repo = repo.full_name.clone();
                        self.catalog_job(move |catalog| {
                            Ok(Outcome::Assets(catalog.assets(&repo)?))
                        });
                    }
                }
                _ => {}
            }
            return;
        }
        let Some(record) = self.plugins.installed.get(self.plugins.index).cloned() else {
            return;
        };
        let id = record.metadata.id().to_owned();
        match action {
            '\n' => self.plugins.inspecting = Some(record.metadata.clone()),
            'e' if matches!(record.metadata, Metadata::Native(_)) => {
                if record.trusted_version.as_deref() != Some(record.metadata.version()) {
                    self.plugins.trust = Some(id);
                } else {
                    self.enable_plugin(id);
                }
            }
            'd' => {
                self.stop_plugin(&id);
                self.plugin_job(move |s| {
                    s.disable(&id)?;
                    Ok(Outcome::Refreshed(s.list()?))
                });
            }
            'r' => self.start_plugin(id),
            'x' => {
                self.stop_plugin(&id);
                self.plugin_job(move |s| {
                    s.remove(&id)?;
                    Ok(Outcome::Refreshed(s.list()?))
                });
            }
            'c' => {
                let command = self
                    .plugins
                    .hosts
                    .get(&id)
                    .and_then(|h| h.commands.keys().min())
                    .cloned();
                if let Some(id) = command {
                    let _ = self.execute_command(shoppe_core_api::CommandRequest {
                        id,
                        arguments: serde_json::Value::Null,
                        target: shoppe_core_api::CommandTarget::Global,
                    });
                }
            }
            _ => {}
        }
    }
    pub(super) fn enable_plugin(&mut self, id: String) {
        self.plugins.startup.push(id.clone());
        self.plugin_job(move |store| {
            store.enable(&id, true)?;
            Ok(Outcome::Refreshed(store.list()?))
        });
    }
}
