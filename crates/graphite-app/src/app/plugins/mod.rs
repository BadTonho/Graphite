//! UI-thread plugin ownership. Blocking package/network/runtime work stays on workers.
use super::*;
use graphite_plugins::{
    catalog::{Asset, Repository, RepositoryDetails},
    manifest::Metadata,
    open_vsx::Extension,
    store::{Installed, Store},
    Result,
};
use std::{collections::BTreeMap, sync::mpsc, time::Instant};
mod accessibility;
mod actions;
pub(crate) mod drawing;
mod layout;
mod panel;
mod runtime;
#[cfg(test)]
mod tests;
pub(crate) mod vsix;
use layout::PluginsTab;
pub(crate) use vsix::VsixLanguageFeature;

pub(crate) enum Outcome {
    Refreshed(Vec<Installed>),
    OpenVsx(Vec<Extension>),
    Graphite(Vec<Repository>),
    GraphiteAssets(Repository, Vec<Asset>),
    GraphiteDetails(String, RepositoryDetails),
    Preview(PathBuf, Metadata),
    Notice(String),
    Started(String, u64, super::extension_host::ExtensionRuntime),
}
#[derive(Default)]
pub(crate) struct Plugins {
    pub hosts: BTreeMap<String, super::extension_host::ExtensionRuntime>,
    pub visible: bool,
    pub focused: bool,
    pub(in crate::app) installed: Vec<Installed>,
    open_vsx: Vec<Extension>,
    graphite: Vec<Repository>,
    graphite_assets: Option<(Repository, Vec<Asset>)>,
    pub(super) graphite_details: Vec<GraphiteDetailTab>,
    pub(super) active_graphite_detail: Option<(u64, String)>,
    graphite_details_loading: Option<String>,
    preview: Option<(PathBuf, Metadata)>,
    inspecting: Option<Metadata>,
    trust: Option<String>,
    tab: PluginsTab,
    query: String,
    path_input: bool,
    editing: bool,
    index: usize,
    scroll: usize,
    message: String,
    worker: Option<mpsc::Receiver<Result<Outcome>>>,
    pub(in crate::app) store: Option<Store>,
    next_session: u64,
    startup: Vec<String>,
    starting: Option<String>,
    trust_generation: u64,
    errors: BTreeMap<String, String>,
    temporary: Option<PathBuf>,
    pub(crate) vsix: Option<vsix::Runtime>,
    vsix_generation: u64,
}

#[derive(Clone)]
pub(crate) struct GraphiteDetailTab {
    pub repository: String,
    pub group: u64,
    pub details: Option<RepositoryDetails>,
    pub loading: bool,
    pub error: Option<String>,
    pub scroll_line: usize,
    pub choosing_asset: bool,
    pub selected_asset: usize,
}
impl Plugins {
    pub(crate) fn installed_plugins(&self) -> &[Installed] {
        &self.installed
    }

    #[cfg(test)]
    pub(crate) fn installed_plugins_mut(&mut self) -> &mut Vec<Installed> {
        &mut self.installed
    }

    pub fn pending(&self) -> bool {
        self.worker.is_some()
            || self.vsix.as_ref().is_some_and(vsix::Runtime::pending)
            || self.hosts.values().any(|h| {
                h.activation_deadline.is_some()
                    || h.supervisor.as_ref().is_some_and(|s| {
                        s.state == graphite_extension_host::HostState::Starting
                            || !s.pending.is_empty()
                    })
                    || !h.incoming.is_empty()
            })
    }
}
impl Drop for Plugins {
    fn drop(&mut self) {
        for host in self.hosts.values_mut() {
            if let Some(supervisor) = host.supervisor.as_mut() {
                supervisor.shutdown();
            }
        }
        if let Some(path) = self.temporary.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}
impl App {
    pub(crate) fn refresh_plugins_for_profile(&mut self) {
        self.stop_workspace_plugins();
        // Discard any in-flight UI result from the previous profile. Its worker
        // owns a clone of the old profile's store and may safely finish there.
        self.plugins.worker = None;
        self.plugins.store = None;
        self.plugins.installed.clear();
        self.leave_plugin_details();
        self.initialize_plugins();
    }

    pub(crate) fn start_workspace_trusted_plugins(&mut self) {
        let mut startup = std::mem::take(&mut self.plugins.startup);
        if let Some(starting) = &self.plugins.starting {
            if !startup.contains(starting) {
                startup.push(starting.clone());
            }
        }
        let approved = self
            .plugins
            .installed
            .iter()
            .filter(|record| {
                matches!(record.metadata, Metadata::Native(_))
                    && record.enabled
                    && record.trusted_version.as_deref() == Some(record.metadata.version())
            })
            .map(|record| record.metadata.id().to_owned())
            .collect::<Vec<_>>();
        for id in approved {
            if !startup.contains(&id) {
                startup.push(id);
            }
        }
        self.plugins.startup = startup;
    }

    pub(crate) fn stop_workspace_plugins(&mut self) {
        self.stop_vsix();
        self.plugins.trust_generation = self.plugins.trust_generation.wrapping_add(1);
        self.plugins.startup.clear();
        self.plugins.starting = None;
        let ids = self.plugins.hosts.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            self.stop_plugin(&id);
        }
    }

    pub(crate) fn initialize_plugins(&mut self) {
        self.plugins.store = Some(Store::new(self.profile_root.join("plugins")));
        self.plugin_job(|store| Ok(Outcome::Refreshed(store.list()?)));
    }
    #[cfg(test)]
    pub(crate) fn use_plugin_store(&mut self, path: PathBuf) {
        self.plugins.store = Some(Store::new(path));
    }
    #[cfg(test)]
    pub(crate) fn plugin_store_root(&self) -> Option<&std::path::Path> {
        self.plugins
            .store
            .as_ref()
            .map(|store| store.root.as_path())
    }
    /// Focuses the sidebar and dismisses editor popups that would steal typing.
    pub(crate) fn focus_plugins(&mut self) {
        self.plugins.focused = true;
        self.completion_items.clear();
        self.quick_fix_visible = false;
        self.snippet_query = None;
    }
    pub(crate) fn open_plugins(&mut self) {
        self.scm_visible = false;
        self.close_project_search();
        self.plugins.visible = true;
        self.focus_plugins();
        if self.plugins.store.is_none() {
            self.initialize_plugins();
        }
    }
    pub(crate) fn close_plugins(&mut self) {
        self.leave_plugin_details();
        self.plugins.visible = false;
        self.plugins.focused = false;
    }
    pub(super) fn plugin_job(
        &mut self,
        job: impl FnOnce(Store) -> Result<Outcome> + Send + 'static,
    ) {
        if self.plugins.worker.is_some() {
            return;
        }
        let Some(store) = self.plugins.store.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let proxy = self.event_proxy.clone();
        self.plugins.message = self.t("plugins.loading").into();
        self.plugins.worker = Some(rx);
        std::thread::spawn(move || {
            let result = job(store);
            let _ = tx.send(result);
            if let Some(proxy) = proxy {
                let _ = proxy.send_event(UserEvent::ExtensionHostWake);
            }
        });
    }
    pub(super) fn search_plugins(&mut self) {
        let query = self.plugins.query.clone();
        let tab = self.plugins.tab;
        self.plugins.graphite_assets = None;
        self.plugins.index = 0;
        self.plugins.scroll = 0;
        match tab {
            PluginsTab::OpenVsx => {
                self.plugins.open_vsx.clear();
                self.plugin_job(move |_| {
                    let registry = graphite_plugins::open_vsx::Registry::new()?;
                    Ok(Outcome::OpenVsx(registry.search(&query, 30)?))
                });
            }
            PluginsTab::Graphite => {
                self.plugins.graphite.clear();
                self.plugin_job(move |_| {
                    let mut catalog = graphite_plugins::catalog::Catalog::new()?;
                    Ok(Outcome::Graphite(catalog.search(&query, 1)?))
                });
            }
            PluginsTab::Installed => {}
        }
    }
}
