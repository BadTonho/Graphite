use super::*;
impl App {
    pub(super) fn plugin_action(&mut self, action: char) {
        if self.plugins.worker.is_some() {
            return;
        }
        if action == 'o' || action == 'i' || action == 'g' {
            self.leave_plugin_details();
            self.plugins.tab = match action {
                'o' => PluginsTab::OpenVsx,
                'g' => PluginsTab::Graphite,
                _ => PluginsTab::Installed,
            };
            self.plugins.index = 0;
            self.plugins.scroll = 0;
            if self.plugins.path_input {
                self.plugins.query.clear();
                self.plugins.path_input = false;
            }
            if action == 'o' || action == 'g' {
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
        if self.plugins.tab == PluginsTab::OpenVsx {
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
        if self.plugins.tab == PluginsTab::Graphite {
            if action != '\n' {
                return;
            }
            if let Some((_, assets)) = &self.plugins.graphite_assets {
                let Some(asset) = assets.get(self.plugins.index).cloned() else {
                    return;
                };
                self.queue_graphite_asset_download(asset);
            } else if let Some(repository) = self.plugins.graphite.get(self.plugins.index).cloned()
            {
                let root = self.plugins.store.as_ref().unwrap().root.clone();
                let session = self.plugins.next_session;
                self.plugins.next_session += 1;
                self.plugin_job(move |_| {
                    let mut catalog = graphite_plugins::catalog::Catalog::new()?;
                    let assets = catalog.assets(&repository.full_name)?;
                    if assets.is_empty() {
                        return Err(format!(
                            "The latest stable release for {} has no .graphite-plugin or .vsix assets",
                            repository.full_name
                        ));
                    }
                    if assets.len() == 1 {
                        let destination = graphite_asset_destination(&root, session, &assets[0]);
                        download_graphite_asset(&mut catalog, &assets[0], &destination)
                    } else {
                        Ok(Outcome::GraphiteAssets(repository, assets))
                    }
                });
            }
            return;
        }
        let Some(record) = self.plugins.installed.get(self.plugins.index).cloned() else {
            return;
        };
        let id = record.metadata.id().to_owned();
        match action {
            '\n' => self.plugins.inspecting = Some(record.metadata.clone()),
            'u' if matches!(record.metadata, Metadata::Vsix(_)) => {
                let Some((namespace, name)) = id.split_once('.') else {
                    self.plugins.message = "Invalid VSIX identity".into();
                    return;
                };
                let (namespace, name) = (namespace.to_owned(), name.to_owned());
                let installed_version = record.metadata.version().to_owned();
                let root = self.plugins.store.as_ref().unwrap().root.clone();
                let destination = root.join(format!(
                    ".open-vsx-update-{}-{}.vsix",
                    std::process::id(),
                    self.plugins.next_session
                ));
                self.plugins.temporary = Some(destination.clone());
                self.plugin_job(move |_| {
                    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                    let result = (|| {
                        let registry = graphite_plugins::open_vsx::Registry::new()?;
                        let latest = registry.download_latest(&namespace, &name, &destination)?;
                        if latest.version == installed_version {
                            let _ = std::fs::remove_file(&destination);
                            return Ok(Outcome::Notice(format!(
                                "{namespace}.{name} is already up to date ({installed_version})"
                            )));
                        }
                        let metadata = graphite_plugins::package::inspect(&destination)?;
                        Ok(Outcome::Preview(destination.clone(), metadata))
                    })();
                    if result.is_err() {
                        let _ = std::fs::remove_file(destination);
                    }
                    result
                });
            }
            'e' if !matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.main.is_none()) => {
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
        let Some(record) = self
            .plugins
            .installed
            .iter()
            .find(|record| record.metadata.id() == id)
        else {
            return;
        };
        let directory = record.directory.clone();
        if matches!(record.metadata, Metadata::Native(_)) {
            self.plugins.startup.push(id.clone());
        } else {
            self.stop_vsix();
        }
        self.plugin_job(move |store| {
            store.enable_installation(&id, &directory, true)?;
            Ok(Outcome::Refreshed(store.list()?))
        });
    }

    fn queue_graphite_asset_download(&mut self, asset: graphite_plugins::catalog::Asset) {
        let root = self.plugins.store.as_ref().unwrap().root.clone();
        let destination = graphite_asset_destination(&root, self.plugins.next_session, &asset);
        self.plugins.next_session += 1;
        self.plugins.temporary = Some(destination.clone());
        self.plugin_job(move |_| {
            let mut catalog = graphite_plugins::catalog::Catalog::new()?;
            download_graphite_asset(&mut catalog, &asset, &destination)
        });
    }
}

fn graphite_asset_destination(
    root: &std::path::Path,
    session: u64,
    asset: &graphite_plugins::catalog::Asset,
) -> std::path::PathBuf {
    let extension = if asset.name.ends_with(".graphite-plugin") {
        ".graphite-plugin"
    } else {
        ".vsix"
    };
    root.join(format!(
        ".github-plugin-{}-{session}{extension}",
        std::process::id()
    ))
}

fn download_graphite_asset(
    catalog: &mut graphite_plugins::catalog::Catalog,
    asset: &graphite_plugins::catalog::Asset,
    destination: &std::path::Path,
) -> Result<Outcome> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let result = (|| {
        catalog.download(asset, destination)?;
        let metadata = graphite_plugins::package::inspect(destination)?;
        Ok(Outcome::Preview(destination.to_path_buf(), metadata))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(destination);
    }
    result
}
