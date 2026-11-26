use super::*;
impl App {
    pub(super) fn plugin_action(&mut self, action: char) {
        if self.plugins.worker.is_some() || self.plugins.bisect.is_some() {
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
            if action == '\n' {
                if let Some(extension) = self.plugins.open_vsx.get(self.plugins.index).cloned() {
                    self.download_open_vsx_preview(extension.namespace, extension.name);
                }
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
        self.installed_plugin_action(&id, action);
    }

    pub(super) fn installed_plugin_action(&mut self, id: &str, action: char) {
        self.installed_plugin_action_with_trust(id, action, false);
    }

    pub(super) fn installed_plugin_action_confirmed(&mut self, id: &str, action: char) {
        self.installed_plugin_action_with_trust(id, action, true);
    }

    fn installed_plugin_action_with_trust(
        &mut self,
        id: &str,
        action: char,
        trust_confirmed: bool,
    ) {
        if self.plugins.worker.is_some() || self.plugins.bisect.is_some() {
            return;
        }
        let Some(record) = self
            .plugins
            .installed
            .iter()
            .find(|record| record.metadata.id() == id)
            .cloned()
        else {
            return;
        };
        let id = record.metadata.id().to_owned();
        match action {
            '\n' => self.open_installed_plugin_details(&record),
            'u' if matches!(record.metadata, Metadata::Vsix(_)) => {
                let Some((namespace, name)) = id.split_once('.') else {
                    self.plugins.message = "Invalid VSIX identity".into();
                    return;
                };
                let (namespace, name) = (namespace.to_owned(), name.to_owned());
                let installed_version = record.metadata.version().to_owned();
                self.update_one_plugin(namespace, name, installed_version, record.directory);
            }
            'e' if !matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.main.is_none()) => {
                if !trust_confirmed
                    && record.trusted_version.as_deref() != Some(record.metadata.version())
                {
                    self.open_plugins();
                    self.plugins.tab = PluginsTab::Installed;
                    self.plugins.section_collapsed[PluginsTab::Installed.index()] = false;
                    self.plugins.trust = Some(id);
                    self.focus_plugins();
                } else {
                    if self.plugins.trust.as_deref() == Some(id.as_str()) {
                        self.plugins.trust = None;
                    }
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
                self.close_installed_plugin_details_by_id(&id);
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
    /// Downloads the latest version of an Open VSX extension and shows it for approval.
    fn download_open_vsx_preview(&mut self, namespace: String, name: String) {
        self.plugin_job(move |store| {
            let download = new_plugin_download(&store.root)?;
            let destination = download.path().join("package.vsix");
            let registry = graphite_plugins::open_vsx::Registry::new()?;
            registry.download_latest(&namespace, &name, &destination)?;
            let metadata = graphite_plugins::package::inspect(&destination)?;
            Ok(Outcome::Preview(download, destination, metadata))
        });
    }

    /// Searches Open VSX for extensions of the active file's language.
    pub(crate) fn plugins_search_language_extensions(&mut self) {
        let language = self.documents[self.command_document()]
            .language
            .map(|language| format!("{language:?}").to_lowercase())
            .unwrap_or_default();
        self.plugins_search_open_vsx(&language);
    }

    /// Shows the extensions side bar on the Open VSX tab and searches for `query`.
    pub(crate) fn plugins_search_open_vsx(&mut self, query: &str) {
        self.open_plugins();
        self.leave_plugin_details();
        self.plugins.tab = PluginsTab::OpenVsx;
        self.plugins.path_input = false;
        self.plugins.query = query.to_owned();
        self.search_plugins();
    }

    pub(crate) fn plugins_show_installed(&mut self) {
        self.open_plugins();
        self.leave_plugin_details();
        self.plugins.tab = PluginsTab::Installed;
        self.plugins.path_input = false;
        self.plugins.query.clear();
    }

    /// Downloads an extension by its `publisher.name` id and shows it for approval before it is
    /// installed.
    pub(crate) fn plugins_install_from_open_vsx(
        &mut self,
        id: &str,
    ) -> std::result::Result<(), String> {
        if self.plugins.bisect.is_some() {
            return Err(self.t("plugins.toolbar.busy").into());
        }
        let (namespace, name) = id
            .split_once('.')
            .filter(|(namespace, name)| {
                !namespace.is_empty() && !name.is_empty() && !name.contains('.')
            })
            .ok_or_else(|| self.t("plugins.invalid_id").to_owned())?;
        self.open_plugins();
        self.leave_plugin_details();
        self.plugins.tab = PluginsTab::Installed;
        self.download_open_vsx_preview(namespace.to_owned(), name.to_owned());
        Ok(())
    }

    /// Removes an installed extension by id.
    pub(crate) fn plugins_uninstall(&mut self, id: &str) -> std::result::Result<(), String> {
        if !self
            .plugins
            .installed
            .iter()
            .any(|record| record.metadata.id() == id)
        {
            return Err(self.tf("plugins.not_installed", &[("id", &id)]));
        }
        if self.plugins.bisect.is_some() || self.plugins.worker.is_some() {
            return Err(self.t("plugins.toolbar.busy").into());
        }
        let id = id.to_owned();
        self.stop_plugin(&id);
        self.plugin_job(move |store| {
            store.remove(&id)?;
            Ok(Outcome::Refreshed(store.list()?))
        });
        Ok(())
    }

    /// Disables every enabled extension.
    pub(crate) fn plugins_disable_all(&mut self) {
        if self.plugins.bisect.is_some() {
            return;
        }
        let enabled: Vec<String> = self
            .plugins
            .installed
            .iter()
            .filter(|record| record.enabled)
            .map(|record| record.metadata.id().to_owned())
            .collect();
        if enabled.is_empty() {
            self.plugins.message = self.t("plugins.none_enabled").into();
            return;
        }
        if self.plugins.worker.is_some() {
            return;
        }
        self.plugins.trust_queue.clear();
        self.plugins.trust = None;
        self.plugin_job(move |store| {
            let mut failed = 0;
            for id in &enabled {
                if store.disable(id).is_err() {
                    failed += 1;
                    crate::app::error_log::record(
                        crate::app::error_log::ErrorSeverity::Medium,
                        "plugins.disable_all",
                        format!("Could not disable {id}"),
                    );
                }
            }
            Ok(Outcome::BulkChanged(store.list()?, failed))
        });
    }

    /// Enables approved versions and queues individual reviews for executable code.
    pub(crate) fn plugins_enable_all(&mut self) {
        if self.plugins.worker.is_some() || self.plugins.bisect.is_some() {
            return;
        }
        self.plugins.trust_queue = self
            .plugins
            .installed
            .iter()
            .filter(|record| !record.enabled)
            .filter(|record| record.trusted_version.as_deref() != Some(record.metadata.version()))
            .filter(
                |record| !matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.main.is_none()),
            )
            .map(|record| record.metadata.id().to_owned())
            .collect();
        let candidates: Vec<(String, String, bool)> = self
            .plugins
            .installed
            .iter()
            .filter(|record| !record.enabled)
            .filter(|record| {
                record.trusted_version.as_deref() == Some(record.metadata.version())
                    || matches!(&record.metadata, Metadata::Vsix(vsix) if vsix.main.is_none())
            })
            .map(|record| {
                (
                    record.metadata.id().to_owned(),
                    record.directory.clone(),
                    matches!(record.metadata, Metadata::Native(_)),
                )
            })
            .collect();
        if candidates.is_empty() {
            if self.plugins.trust_queue.is_empty() {
                self.plugins.message = self.t("plugins.none_to_enable").into();
            } else {
                self.open_plugins();
                self.poll_plugin_trust_queue();
            }
            return;
        }
        if !self.plugins.trust_queue.is_empty() {
            self.open_plugins();
        }
        let mut stopped_vsix = false;
        for (id, _, native) in &candidates {
            if *native {
                self.plugins.startup.push(id.clone());
            } else if !stopped_vsix {
                self.stop_vsix();
                stopped_vsix = true;
            }
        }
        self.plugin_job(move |store| {
            let mut failed = 0;
            for (id, directory, _) in &candidates {
                if store.enable_installation(id, directory, true).is_err() {
                    failed += 1;
                    crate::app::error_log::record(
                        crate::app::error_log::ErrorSeverity::Medium,
                        "plugins.enable_all",
                        format!("Could not enable {id}"),
                    );
                }
            }
            Ok(Outcome::BulkChanged(store.list()?, failed))
        });
    }

    pub(super) fn poll_plugin_trust_queue(&mut self) {
        if self.plugins.worker.is_none()
            && self.plugins.visible
            && self.plugins.focused
            && !self.plugins.editing
            && !self.plugin_details_open()
            && self.plugins.toolbar_menu.is_none()
        {
            if let Some(id) = self.plugins.trust_queue.pop_front() {
                self.plugins.trust = Some(id);
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
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
        self.plugin_job(move |store| {
            let download = new_plugin_download(&store.root)?;
            let filename = if asset.name.ends_with(".graphite-plugin") {
                "package.graphite-plugin"
            } else {
                "package.vsix"
            };
            let destination = download.path().join(filename);
            let mut catalog = graphite_plugins::catalog::Catalog::new()?;
            catalog.download(&asset, &destination)?;
            let metadata = graphite_plugins::package::inspect(&destination)?;
            Ok(Outcome::Preview(download, destination, metadata))
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
        self.plugins.active_installed_detail = None;
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
