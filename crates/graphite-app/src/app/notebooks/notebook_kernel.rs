//! Running notebook cells: starting and choosing the kernel and sending cells to it.

use super::go_menu::{GoPicker, GoPickerKind, GoSymbolItem};
use super::*;
use crate::notebook::kernel::{KernelHandle, KernelSpec, Notify};
use crate::notebook::{CellKind, KernelStatus, Session};

/// Starts the kernel of a spec in a directory. A field of the application so tests can use a
/// scripted kernel.
pub(crate) type KernelFactory = fn(&KernelSpec, &Path, Notify) -> Result<KernelHandle, String>;

/// Which cells a run command covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunTarget {
    /// The selected cells.
    Selected,
    /// Every cell.
    All,
    /// The cells above the selection.
    Above,
    /// The selected cells and all that follow.
    SelectedAndBelow,
}

impl App {
    fn notebook_session_mut(&mut self) -> Option<&mut Session> {
        let document = self.active_document();
        self.documents[document].notebook.as_deref_mut()
    }

    /// The kernels installed on the machine; looked up again when `refresh` is set.
    pub(crate) fn kernel_specs(&mut self, refresh: bool) -> Vec<KernelSpec> {
        if refresh || self.kernel_specs.is_none() {
            self.kernel_specs = Some((self.kernel_lister)());
        }
        let mut specs = self.kernel_specs.clone().unwrap_or_default();
        // The controllers extensions provide for this kind of notebook.
        specs.extend(self.vsix_notebook_kernel_specs());
        specs
    }

    /// Called from the kernel's threads; wakes the event loop to collect what arrived.
    fn kernel_notify(&self) -> Notify {
        let proxy = self.event_proxy.clone();
        std::sync::Arc::new(move || {
            if let Some(proxy) = &proxy {
                let _ = proxy.send_event(UserEvent::KernelWake);
            }
        })
    }

    /// Starts `spec` for the active notebook, replacing the kernel it had.
    fn start_kernel(&mut self, spec: KernelSpec) -> Result<(), String> {
        if spec.name.starts_with(crate::app::plugins::vsix::SPEC_PREFIX) {
            return self.start_extension_kernel(spec);
        }
        let document = self.active_document();
        let directory = self.documents[document]
            .buffer
            .path()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.tree.root().to_path_buf());
        let handle = (self.kernel_factory)(&spec, &directory, self.kernel_notify())?;
        let Some(session) = self.notebook_session_mut() else {
            return Ok(());
        };
        let language = spec.language.clone();
        session.set_kernelspec(&spec.name, &spec.display_name, &language);
        session.attach_kernel(handle, spec);
        self.sync_notebook_buffer(document);
        Ok(())
    }

    /// Makes sure the active notebook has a running kernel, starting the one it asks for. When
    /// the choice is the user's, opens the picker and reports that nothing ran.
    pub(crate) fn ensure_kernel(&mut self) -> Result<(), String> {
        let Some(session) = self.active_notebook() else {
            return Err(self.t("notebook.error.no_notebook").into());
        };
        let alive = session.kernel_status() != &KernelStatus::None
            && !matches!(session.kernel_status(), KernelStatus::Dead(_));
        if alive {
            return Ok(());
        }
        let previous = session.kernel_spec().cloned();
        let name = session.metadata().kernel_name().map(str::to_owned);
        let language = session
            .metadata()
            .language()
            .map(str::to_owned)
            .or_else(|| {
                session
                    .interactive()
                    .and_then(|state| state.language_hint.clone())
            });
        let spec = match previous {
            Some(spec) => spec,
            None => {
                let specs = self.kernel_specs(false);
                if specs.is_empty() {
                    return Err(self.t("notebook.error.no_kernels").into());
                }
                match crate::notebook::kernel::pick_spec(
                    name.as_deref(),
                    language.as_deref(),
                    &specs,
                ) {
                    Some(index) => specs[index].clone(),
                    None => {
                        self.open_kernel_picker();
                        return Err(self.t("notebook.select_kernel").into());
                    }
                }
            }
        };
        self.start_kernel(spec)
            .map_err(|error| self.tf("notebook.error.start", &[("error", &error)]))
    }

    /// Runs the cells `target` stands for.
    pub(crate) fn run_notebook_cells(&mut self, target: RunTarget) -> Result<usize, String> {
        self.ensure_kernel()?;
        let document = self.active_document();
        let Some(session) = self.notebook_session_mut() else {
            return Ok(0);
        };
        let (low, high) = session.selected();
        let count = session.cells.len();
        let indices: Vec<usize> = match target {
            RunTarget::Selected => (low..=high).collect(),
            RunTarget::All => (0..count).collect(),
            RunTarget::Above => (0..low).collect(),
            RunTarget::SelectedAndBelow => (low..count).collect(),
        };
        let sent = session.run_cells(&indices);
        self.sync_notebook_buffer(document);
        sent
    }

    /// Collects what every running kernel has sent since the last call.
    pub(crate) fn poll_kernels(&mut self) {
        let ids: Vec<DocumentId> = self.documents.ids().collect();
        let mut changed = false;
        for id in ids {
            let Some(session) = self.documents[id].notebook.as_deref_mut() else {
                continue;
            };
            if session.poll_kernel() {
                self.sync_notebook_buffer(id);
                changed = true;
            }
        }
        if changed {
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
    }

    /// Opens the list of installed kernels.
    pub(crate) fn open_kernel_picker(&mut self) {
        if self.active_notebook().is_none() {
            self.status = self.t("notebook.error.no_notebook").into();
            return;
        }
        let specs = self.kernel_specs(true);
        let items: Vec<GoSymbolItem> = specs
            .iter()
            .enumerate()
            .map(|(index, spec)| GoSymbolItem {
                path: PathBuf::new(),
                line: index,
                name: spec.display_name.clone(),
                scope: spec.language.clone(),
            })
            .collect();
        let message = items
            .is_empty()
            .then(|| self.t("notebook.error.no_kernels").to_owned());
        self.command_palette = None;
        self.quick_open = None;
        self.local_search = None;
        self.go_picker_generation = self.go_picker_generation.wrapping_add(1);
        self.go_picker = Some(GoPicker {
            kind: GoPickerKind::Kernel,
            query: String::new(),
            items,
            selected: 0,
            loading: false,
            generation: self.go_picker_generation,
            message,
        });
    }

    /// Switches the active notebook to the picked kernel.
    pub(crate) fn select_kernel(&mut self, index: usize) {
        let Some(spec) = self.kernel_specs(false).get(index).cloned() else {
            return;
        };
        let name = spec.display_name.clone();
        match self.start_kernel(spec) {
            Ok(()) => self.status = self.tf("notebook.kernel_started", &[("name", &name)]),
            Err(error) => self.status = self.tf("notebook.error.start", &[("error", &error)]),
        }
    }

    /// Ends the kernel and starts it again from the same spec; cells that were running stop.
    pub(crate) fn restart_kernel(&mut self) -> Result<(), String> {
        let Some(spec) = self
            .active_notebook()
            .and_then(|session| session.kernel_spec().cloned())
        else {
            return Err(self.t("notebook.error.no_kernel").into());
        };
        self.start_kernel(spec)
            .map_err(|error| self.tf("notebook.error.start", &[("error", &error)]))?;
        self.status = self.t("notebook.kernel_restarted").into();
        Ok(())
    }

    pub(crate) fn shutdown_kernel(&mut self) {
        let document = self.active_document();
        let Some(session) = self.notebook_session_mut() else {
            return;
        };
        session.shutdown_kernel();
        self.sync_notebook_buffer(document);
        self.status = self.t("notebook.kernel_stopped").into();
    }

    pub(crate) fn interrupt_notebook(&mut self) {
        match self.active_notebook() {
            Some(session) if session.is_running() => {
                session.interrupt_kernel();
                self.status = self.t("notebook.interrupted").into();
            }
            _ => self.status = self.t("notebook.nothing_running").into(),
        }
    }

    /// Runs the selected cells; with `then_select_below` moves on afterwards, adding a cell
    /// when the last one ran and `insert` is set.
    pub(crate) fn run_and_advance(&mut self, insert: bool) {
        match self.run_notebook_cells(RunTarget::Selected) {
            Ok(_) => self.with_notebook(|session| {
                let (_, high) = session.selected();
                if high + 1 < session.cells.len() && !insert {
                    session.set_focus(high + 1, false);
                } else {
                    session.insert_cell(high + 1, CellKind::Code, "");
                    session.enter_edit();
                }
            }),
            Err(error) => self.status = error,
        }
    }
}
