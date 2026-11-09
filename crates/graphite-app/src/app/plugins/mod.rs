//! UI-thread plugin ownership. Blocking package/network/runtime work stays on workers.
use super::*;
use graphite_plugins::{
    catalog::{Asset, Catalog, Repository},
    manifest::Metadata,
    store::{Installed, Store},
    Result,
};
use std::{
    collections::BTreeMap,
    sync::{mpsc, Mutex},
    time::Instant,
};
mod accessibility;
mod actions;
mod drawing;
mod panel;
mod runtime;
#[cfg(test)]
mod tests;

pub(crate) enum Outcome {
    Refreshed(Vec<Installed>),
    Repositories(Vec<Repository>),
    Assets(Vec<Asset>),
    Preview(PathBuf, Metadata),
    Started(String, super::extension_host::ExtensionRuntime),
}
#[derive(Default)]
pub(crate) struct Plugins {
    pub hosts: BTreeMap<String, super::extension_host::ExtensionRuntime>,
    pub visible: bool,
    installed: Vec<Installed>,
    repositories: Vec<Repository>,
    assets: Option<Vec<Asset>>,
    preview: Option<(PathBuf, Metadata)>,
    inspecting: Option<Metadata>,
    trust: Option<String>,
    github: bool,
    query: String,
    path_input: bool,
    editing: bool,
    page: u32,
    index: usize,
    message: String,
    worker: Option<mpsc::Receiver<Result<Outcome>>>,
    catalog: Option<Arc<Mutex<Option<Catalog>>>>,
    store: Option<Store>,
    next_session: u64,
    startup: Vec<String>,
    starting: Option<String>,
    errors: BTreeMap<String, String>,
    temporary: Option<PathBuf>,
}
impl Plugins {
    pub fn pending(&self) -> bool {
        self.worker.is_some()
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
    pub(crate) fn initialize_plugins(&mut self) {
        self.plugins.store = Some(Store::new(config::app_config_dir().join("plugins")));
        self.plugin_job(|store| Ok(Outcome::Refreshed(store.list()?)));
    }
    pub(crate) fn open_plugins(&mut self) {
        self.plugins.visible = true;
        if self.plugins.store.is_none() {
            self.initialize_plugins();
        }
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
    pub(super) fn catalog_job(
        &mut self,
        job: impl FnOnce(&mut Catalog) -> Result<Outcome> + Send + 'static,
    ) {
        // Construct reqwest's blocking client on its worker, not the event loop.
        let catalog = self
            .plugins
            .catalog
            .clone()
            .unwrap_or_else(|| Arc::new(Mutex::new(None)));
        self.plugins.catalog = Some(catalog.clone());
        let translator = self.translator;
        self.plugin_job(move |_| {
            let mut guard = catalog
                .lock()
                .map_err(|_| translator.text("plugins.catalog_worker_failed"))?;
            if guard.is_none() {
                *guard = Some(Catalog::new()?);
            }
            job(guard.as_mut().unwrap())
        });
    }
    pub(super) fn search_plugins(&mut self) {
        let query = self.plugins.query.clone();
        let page = self.plugins.page.max(1);
        self.plugins.assets = None;
        self.plugins.index = 0;
        self.catalog_job(move |catalog| Ok(Outcome::Repositories(catalog.search(&query, page)?)));
    }
}
