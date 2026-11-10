use super::*;
impl App {
    pub(super) fn plugin_action(&mut self, action: char) {
        if self.plugins.worker.is_some() {
            return;
        }
        if action == 'o' || action == 'i' {
            self.leave_plugin_details();
            self.plugins.marketplace = action == 'o';
            self.plugins.index = 0;
            self.plugins.scroll = 0;
            if self.plugins.path_input {
                self.plugins.query.clear();
                self.plugins.path_input = false;
            }
            if action == 'o' {
                self.search_plugins();
            }
            return;
        }
        if action == '/' || action == 'l' {
            self.leave_plugin_details();
            self.plugins.path_input = action == 'l';
            self.plugins.query.clear();
            self.plugins.editing = true;
            return;
        }
        if self.plugins.marketplace {
            match action {
                '\n' => {
                    if let Some(extension) = self.plugins.open_vsx.get(self.plugins.index).cloned()
                    {
                        let root = self.plugins.store.as_ref().unwrap().root.clone();
                        let destination = root.join(format!(
                            ".open-vsx-{}-{}.vsix",
                            std::process::id(),
                            self.plugins.next_session
                        ));
                        self.plugins.temporary = Some(destination.clone());
                        self.plugin_job(move |_| {
                            std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                            let registry = graphite_plugins::open_vsx::Registry::new()?;
                            registry.download_latest(
                                &extension.namespace,
                                &extension.name,
                                &destination,
                            )?;
                            match graphite_plugins::package::inspect(&destination) {
                                Ok(metadata) => Ok(Outcome::Preview(destination, metadata)),
                                Err(error) => {
                                    let _ = std::fs::remove_file(destination);
                                    Err(error)
                                }
                            }
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
                    let _ = self.execute_command(graphite_core_api::CommandRequest {
                        id,
                        arguments: serde_json::Value::Null,
                        target: graphite_core_api::CommandTarget::Global,
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
