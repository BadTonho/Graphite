use super::*;
use crate::app::error_log::{self, ErrorSeverity};

#[cfg(not(windows))]
const VSIX_EXTENSIONS: &[&str] = &[
    "vsix", "vsiX", "vsIx", "vsIX", "vSix", "vSiX", "vSIx", "vSIX", "Vsix", "VsiX", "VsIx", "VsIX",
    "VSix", "VSiX", "VSIx", "VSIX",
];

impl App {
    pub(super) fn pick_local_vsix(&mut self) {
        let Some(window) = self.gpu.as_ref().map(|gpu| gpu.window.clone()) else {
            return;
        };
        let directory = self.plugins.vsix_picker_directory.clone();
        let workspace = self.tree.root().to_path_buf();
        let dialog = rfd::FileDialog::new()
            .set_parent(window.as_ref())
            .set_title(self.t("plugins.toolbar.install"));
        #[cfg(windows)]
        let dialog = dialog
            .add_filter("VSIX", &["vsix"])
            .add_filter(self.t("plugins.toolbar.all_files"), &["*"]);
        #[cfg(not(windows))]
        let dialog = dialog.add_filter("VSIX", VSIX_EXTENSIONS);

        // Keep the owner alive and leave its event loop free to repaint and service the dialog.
        window.request_redraw();
        self.plugin_job(move |_| {
            let dialog = match initial_directory(directory.as_deref(), &workspace) {
                Some(directory) => dialog.set_directory(directory),
                None => dialog,
            };
            let selection = dialog.pick_file();
            drop(window);
            Ok(Outcome::VsixPicked(selection))
        });
    }

    pub(super) fn finish_local_vsix_pick(&mut self, path: Option<PathBuf>) {
        let Some(path) = path else { return };
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("vsix"))
        {
            self.plugins.message = self.t("plugins.toolbar.expected_vsix").into();
            error_log::record(
                ErrorSeverity::Medium,
                "plugins.install.picker",
                "The selected file does not have a VSIX extension.",
            );
            return;
        }
        self.plugins.vsix_picker_directory = path.parent().map(Path::to_path_buf);
        self.open_plugins();
        self.inspect_local_plugin(path);
    }
}

fn initial_directory<'a>(last: Option<&'a Path>, workspace: &'a Path) -> Option<&'a Path> {
    last.filter(|path| path.is_dir())
        .or_else(|| workspace.is_dir().then_some(workspace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_uses_existing_last_folder_then_workspace_then_system_default() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let downloads = root.path().join("downloads");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&downloads).unwrap();
        assert_eq!(
            initial_directory(None, &workspace),
            Some(workspace.as_path())
        );
        assert_eq!(
            initial_directory(Some(&downloads), &workspace),
            Some(downloads.as_path())
        );
        std::fs::remove_dir(&downloads).unwrap();
        assert_eq!(
            initial_directory(Some(&downloads), &workspace),
            Some(workspace.as_path())
        );
        assert_eq!(initial_directory(None, &downloads), None);
    }
}
