use super::*;

fn dev_watch_mtime(root: &std::path::Path, main: &str) -> Option<std::time::SystemTime> {
    let manifest_time = std::fs::metadata(root.join("graphite-plugin.json"))
        .and_then(|meta| meta.modified())
        .ok();
    let main_time = std::fs::metadata(root.join(main))
        .and_then(|meta| meta.modified())
        .ok();
    match (manifest_time, main_time) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

impl App {
    pub(crate) fn queue_plugin_start(&mut self, id: String) {
        if self
            .plugins
            .hosts
            .get(&id)
            .is_some_and(|h| h.supervisor.is_some())
            || self
                .extension_host
                .native_manifest
                .as_ref()
                .is_some_and(|m| m.id == id)
        {
            return;
        }
        if self.plugins.worker.is_some() {
            if !self.plugins.startup.iter().any(|pending| pending == &id) {
                self.plugins.startup.push(id);
            }
        } else {
            self.start_plugin(id);
        }
    }
    pub(crate) fn plugin_console_log_path(&self, id: &str) -> Option<PathBuf> {
        self.plugins
            .store
            .as_ref()
            .map(|store| store.root.join("logs").join(id).join("console.log"))
    }
    pub(crate) fn reload_plugin(&mut self, id: &str) {
        if let Some(store) = self.plugins.store.as_ref() {
            if let Ok(records) = store.list() {
                self.plugins.installed = records;
                self.reload_extension_snippets();
                self.sync_extension_themes();
                self.sync_plugin_keybindings();
            }
        }
        if self.plugins.worker.is_some() {
            if !self.plugins.startup.iter().any(|pending| pending == id) {
                self.plugins.startup.push(id.to_owned());
            }
            return;
        }
        self.start_plugin(id.to_owned());
    }
    pub(crate) fn start_plugin(&mut self, id: String) {
        if self
            .workspace_trust
            .as_ref()
            .is_none_or(|trust| !trust.is_trusted(self.tree.root()))
        {
            self.status = "Workspace is restricted; trust it before activating plugins".into();
            return;
        }
        let Some(record) = self
            .plugins
            .installed
            .iter()
            .find(|r| r.metadata.id() == id)
            .cloned()
        else {
            return;
        };
        let Metadata::Native(ref manifest) = record.metadata else {
            return;
        };
        let manifest = manifest.clone();
        if !record.enabled || record.trusted_version.as_deref() != Some(&manifest.version) {
            return;
        }
        self.stop_plugin(&id);
        self.plugins.errors.remove(&id);
        self.plugins.starting = Some(id.clone());
        self.plugins.next_session += 1;
        let session = self.plugins.next_session;
        let trust_generation = self.plugins.trust_generation;
        let configuration = self.service_configuration().unwrap_or_default();
        let node = configuration
            .get("graphite.plugins.nodePath")
            .and_then(|v| v.as_str())
            .unwrap_or("node")
            .to_owned();
        let proxy = self.event_proxy.clone();
        self.plugin_job(move |store| {
            let node = PathBuf::from(node);
            graphite_plugins::sdk::check_node(&node)?;
            let host_script = store.root.join("host-v1.cjs");
            std::fs::write(&host_script, graphite_plugins::HOST).map_err(|e| e.to_string())?;
            let root = record
                .install_dir(&store.root)
                .canonicalize()
                .map_err(|e| e.to_string())?;
            let dev_watch = record.dev_path.as_ref().map(|_| {
                let mtime = dev_watch_mtime(&root, &manifest.main);
                (root.clone(), mtime, Instant::now())
            });
            let log_path = Some(store.root.join("logs").join(&id).join("console.log"));
            let wake = Arc::new(move || {
                if let Some(proxy) = &proxy {
                    let _ = proxy.send_event(UserEvent::ExtensionHostWake);
                }
            });
            let supervisor = graphite_extension_host::Supervisor::spawn_with_args_and_log(
                &node,
                &[host_script.into_os_string(), root.into_os_string()],
                session,
                Instant::now(),
                wake,
                log_path,
            )
            .map_err(|e| e.to_string())?;
            let runtime = super::extension_host::ExtensionRuntime {
                enabled: true,
                native_manifest: Some(manifest),
                activation_deadline: Some(Instant::now() + std::time::Duration::from_secs(30)),
                supervisor: Some(supervisor),
                dev_watch,
                ..Default::default()
            };
            Ok(Outcome::Started(id, trust_generation, runtime))
        });
    }
    pub(crate) fn stop_plugin(&mut self, id: &str) {
        if self.plugins.installed.iter().any(|record| {
            record.metadata.id() == id && matches!(record.metadata, Metadata::Vsix(_))
        }) {
            self.stop_vsix();
        }
        if let Some(mut host) = self.plugins.hosts.remove(id) {
            if let Some(supervisor) = host.supervisor.as_mut() {
                supervisor.shutdown();
            }
            std::mem::swap(&mut self.extension_host, &mut host);
            self.clear_host_session();
            std::mem::swap(&mut self.extension_host, &mut host);
        }
        self.clear_native_ui_for_plugin(id);
    }
    pub(crate) fn poll_plugins(&mut self) {
        if let Some(result) = self
            .plugins
            .worker
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(self.t("plugins.worker_stopped").into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            })
        {
            self.plugins.worker = None;
            self.plugins.message.clear();
            let starting = self.plugins.starting.take();
            match result {
                Err(error) => {
                    self.cleanup_plugin_download();
                    if let Some(repository) = self.plugins.graphite_details_loading.take() {
                        if let Some(tab) = self
                            .plugins
                            .graphite_details
                            .iter_mut()
                            .find(|tab| tab.repository == repository)
                        {
                            tab.loading = false;
                            tab.error = Some(error.clone());
                        }
                    }
                    if let Some(id) = starting {
                        self.plugins.errors.insert(id, error.clone());
                    }
                    self.plugins.message = error;
                }
                Ok(Outcome::Refreshed(records)) => {
                    if self.plugins.installed.is_empty() {
                        self.plugins.startup = records
                            .iter()
                            .filter(|r| match &r.metadata {
                                Metadata::Native(manifest) => {
                                    r.enabled && Self::manifest_matches_startup(manifest)
                                }
                                Metadata::Vsix(_) => false,
                            })
                            .map(|r| r.metadata.id().to_owned())
                            .collect();
                    }
                    self.plugins.installed = records;
                    self.reload_extension_snippets();
                    self.sync_extension_themes();
                    self.sync_plugin_keybindings();
                    if self.active_plugin_activity.as_ref().is_some_and(|active| {
                        !self
                            .plugin_activity_entries()
                            .iter()
                            .any(|entry| &entry.key == active)
                    }) {
                        self.active_plugin_activity = None;
                        self.activate_activity(super::super::activity_bar::Activity::Explorer);
                    }
                    self.plugins.index = self
                        .plugins
                        .index
                        .min(self.plugins.installed.len().saturating_sub(1));
                    let rows = self.plugin_visible_rows();
                    self.plugins.scroll = self
                        .plugins
                        .scroll
                        .min(self.plugins.installed.len().saturating_sub(rows));
                }
                Ok(Outcome::OpenVsx(extensions)) => {
                    self.plugins.open_vsx = extensions;
                    self.plugins.graphite_assets = None;
                    self.plugins.index = 0;
                    self.plugins.scroll = 0;
                }
                Ok(Outcome::Graphite(repositories)) => {
                    self.plugins.graphite = repositories;
                    self.plugins.graphite_assets = None;
                    self.plugins.index = 0;
                    self.plugins.scroll = 0;
                }
                Ok(Outcome::GraphiteAssets(repository, assets)) => {
                    self.plugins.graphite_assets = Some((repository, assets));
                    self.plugins.index = 0;
                    self.plugins.scroll = 0;
                }
                Ok(Outcome::GraphiteDetails(repository, details)) => {
                    self.plugins.graphite_details_loading = None;
                    if let Some(tab) = self
                        .plugins
                        .graphite_details
                        .iter_mut()
                        .find(|tab| tab.repository == repository)
                    {
                        tab.loading = false;
                        tab.error = None;
                        tab.details = Some(details);
                    }
                }
                Ok(Outcome::Notice(message)) => {
                    self.cleanup_plugin_download();
                    self.plugins.message = message;
                }
                Ok(Outcome::Preview(path, metadata)) => {
                    self.plugins.temporary = Some(path.clone());
                    self.plugins.focused = true;
                    self.plugins.preview = Some((path, metadata))
                }
                Ok(Outcome::Started(id, trust_generation, runtime)) => {
                    if trust_generation != self.plugins.trust_generation
                        || self
                            .workspace_trust
                            .as_ref()
                            .is_none_or(|trust| !trust.is_trusted(self.tree.root()))
                    {
                        if let Some(mut supervisor) = runtime.supervisor {
                            supervisor.shutdown();
                        }
                        self.status = "Workspace trust changed before plugin activation completed; activation was discarded".into();
                    } else {
                        self.plugins.hosts.insert(id, runtime);
                    }
                }
            }
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
        if self.plugins.worker.is_none() {
            if let Some(id) = self.plugins.startup.pop() {
                self.start_plugin(id);
            }
        }
        let ids: Vec<_> = self.plugins.hosts.keys().cloned().collect();
        let mut dev_reload = Vec::new();
        for id in ids {
            let mut host = self.plugins.hosts.remove(&id).unwrap();
            let before = host.supervisor.as_ref().map(|s| s.state);
            let was_active = host.native_active;
            if host
                .activation_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                if let Some(supervisor) = host.supervisor.as_mut() {
                    supervisor.stop(graphite_core_api::ServiceError::Timeout);
                }
            }
            std::mem::swap(&mut self.extension_host, &mut host);
            self.poll_extension_host(Instant::now());
            std::mem::swap(&mut self.extension_host, &mut host);
            if before != host.supervisor.as_ref().map(|s| s.state)
                || was_active != host.native_active
            {
                self.plugins.message = match host.supervisor.as_ref().map(|s| s.state) {
                    Some(graphite_extension_host::HostState::Ready) if host.native_active => {
                        self.tf("plugins.runtime.running", &[("id", &id)])
                    }
                    Some(graphite_extension_host::HostState::Ready) => {
                        self.tf("plugins.runtime.activating", &[("id", &id)])
                    }
                    _ => self.tf(
                        "plugins.runtime.stopped",
                        &[("id", &id), ("status", &self.status)],
                    ),
                };
                if host
                    .supervisor
                    .as_ref()
                    .is_some_and(|s| s.state == graphite_extension_host::HostState::Stopped)
                {
                    self.plugins
                        .errors
                        .insert(id.clone(), self.plugins.message.clone());
                }
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            let mut should_reload = false;
            if let (Some(manifest), Some((dev_root, recorded_mtime, last_check))) =
                (&host.native_manifest, &mut host.dev_watch)
            {
                let now = Instant::now();
                if now.duration_since(*last_check) >= std::time::Duration::from_millis(500) {
                    *last_check = now;
                    let current = dev_watch_mtime(dev_root, &manifest.main);
                    if current != *recorded_mtime {
                        *recorded_mtime = current;
                        should_reload = true;
                    }
                }
            }
            self.plugins.hosts.insert(id.clone(), host);
            if self.plugins.hosts.get(&id).is_some_and(|h| h.native_active) {
                let mut ready_cmds = Vec::new();
                let mut rest = Vec::new();
                for pending in self.plugins.pending_commands.drain(..) {
                    if pending.plugin_id == id {
                        ready_cmds.push(pending);
                    } else {
                        rest.push(pending);
                    }
                }
                self.plugins.pending_commands = rest;
                if !ready_cmds.is_empty() {
                    let mut active_host = self.plugins.hosts.remove(&id).unwrap();
                    std::mem::swap(&mut self.extension_host, &mut active_host);
                    for pending in ready_cmds {
                        self.command_runtime.next += 1;
                        let exec_id = self.command_runtime.next;
                        self.dispatch_host_command(
                            exec_id,
                            pending.sender,
                            pending.request,
                            pending.cancellation,
                        );
                    }
                    std::mem::swap(&mut self.extension_host, &mut active_host);
                    self.plugins.hosts.insert(id.clone(), active_host);
                }
            }
            if should_reload {
                dev_reload.push(id);
            }
        }
        for id in dev_reload {
            self.reload_plugin(&id);
        }
        if let Some(language) = self
            .documents
            .get(self.active_document())
            .and_then(|doc| doc.language)
        {
            self.activate_plugins_for_event(&format!("onLanguage:{}", language.name()));
        }
        self.sync_native_webview_host();
        self.poll_vsix();
    }
}
