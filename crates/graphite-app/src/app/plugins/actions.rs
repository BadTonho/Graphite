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
            if let Some(repository) = self.plugins.graphite.get(self.plugins.index).cloned() {
                self.open_graphite_details(repository);
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

    fn open_graphite_details(&mut self, repository: graphite_plugins::catalog::Repository) {
        let existing = self
            .plugins
            .graphite_details
            .iter()
            .find(|tab| tab.repository == repository.full_name)
            .map(|tab| tab.group);
        let group = existing.unwrap_or_else(|| self.editor_groups.focused());
        if existing.is_none() {
            self.plugins.graphite_details.push(GraphiteDetailTab {
                repository: repository.full_name.clone(),
                group,
                details: None,
                loading: true,
                error: None,
                scroll_line: 0,
                choosing_asset: false,
                selected_asset: 0,
            });
            self.plugins.graphite_details_loading = Some(repository.full_name.clone());
            let full_name = repository.full_name.clone();
            self.plugin_job(move |_| {
                let mut catalog = graphite_plugins::catalog::Catalog::new()?;
                Ok(Outcome::GraphiteDetails(
                    full_name.clone(),
                    catalog.details(&full_name)?,
                ))
            });
        }
        self.focus_editor_group(group);
        self.plugins.active_graphite_detail = Some((group, repository.full_name));
        self.plugins.focused = false;
    }

    pub(crate) fn close_graphite_detail(&mut self, group: u64, repository: &str) {
        self.plugins
            .graphite_details
            .retain(|tab| !(tab.group == group && tab.repository == repository));
        if self.plugins.active_graphite_detail.as_ref() == Some(&(group, repository.to_owned())) {
            self.plugins.active_graphite_detail = None;
        }
    }

    pub(super) fn start_graphite_detail_install(&mut self) {
        let Some((group, repository)) = self.plugins.active_graphite_detail.clone() else {
            return;
        };
        let choice = self
            .plugins
            .graphite_details
            .iter_mut()
            .find(|tab| tab.group == group && tab.repository == repository)
            .map(|tab| {
                let assets = tab
                    .details
                    .as_ref()
                    .and_then(|details| details.release.as_ref())
                    .map(|release| release.assets.clone());
                match assets {
                    Some(mut assets) if assets.len() == 1 => Some(assets.remove(0)),
                    Some(assets) if assets.len() > 1 => {
                        tab.choosing_asset = true;
                        tab.selected_asset = 0;
                        None
                    }
                    _ => None,
                }
            });
        match choice {
            Some(Some(asset)) => self.queue_graphite_asset_download(asset),
            Some(None) => {
                let multiple = self.plugins.graphite_details.iter().any(|tab| {
                    tab.group == group && tab.repository == repository && tab.choosing_asset
                });
                if !multiple {
                    self.plugins.message = self.t("plugins.sidebar.no_compatible_release").into();
                }
            }
            None => {}
        }
    }

    pub(super) fn install_selected_graphite_asset(&mut self) {
        let Some((group, repository)) = self.plugins.active_graphite_detail.clone() else {
            return;
        };
        let Some(tab) = self
            .plugins
            .graphite_details
            .iter_mut()
            .find(|tab| tab.group == group && tab.repository == repository)
        else {
            return;
        };
        let asset = tab
            .details
            .as_ref()
            .and_then(|details| details.release.as_ref())
            .and_then(|release| release.assets.get(tab.selected_asset))
            .cloned();
        if let Some(asset) = asset {
            tab.choosing_asset = false;
            self.queue_graphite_asset_download(asset);
        }
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
