//! Resolving a whole file in the merge editor: accepting for every conflict at once, starting
//! over, and completing the merge by writing the result and staging the file.

use super::*;

impl App {
    /// Applies the same decision to every conflict.
    pub(crate) fn merge_accept_all(&mut self, apply: impl FnOnce(&mut crate::merge::MergeState)) {
        let Some(editor) = self.merge_editor.as_mut() else {
            return;
        };
        apply(&mut editor.state);
        editor.refresh();
        editor.close_armed = false;
        editor.active = editor.state.next_unresolved(None).or(editor.active);
        editor.scroll_to_active();
        self.status = self.t("merge.all_resolved").into();
    }

    /// Forgets every decision, so the result starts again from the unresolved conflicts.
    pub(crate) fn merge_reset_all(&mut self) {
        let Some(editor) = self.merge_editor.as_mut() else {
            return;
        };
        editor.state.reset_all();
        editor.refresh();
        editor.close_armed = false;
        editor.active = editor.state.next_unresolved(None);
        editor.scroll_to_active();
        let count = editor.state.len();
        self.status = self.tf("merge.unresolved_count", &[("count", &count)]);
    }

    /// Whether Git lists the file as conflicted, so that completing must stage it.
    fn merge_file_is_conflicted(&self, path: &Path) -> bool {
        let root = self.scm_workspace_root();
        self.scm_snapshot
            .files
            .iter()
            .any(|item| item.conflicted && crate::same_file_path(&root.join(&item.path), path))
    }

    /// Writes the merged text to the file and, for a file Git lists as conflicted, stages it.
    /// The merge editor stays open when something goes wrong, so no decision is lost.
    pub(crate) fn merge_complete(&mut self) -> Result<(), String> {
        let Some(editor) = self.merge_editor.as_ref() else {
            return Ok(());
        };
        let Some(contents) = editor.state.complete() else {
            let count = editor.state.unresolved_count();
            return Err(self.tf("merge.error.unresolved", &[("count", &count)]));
        };
        let path = editor.path.clone();
        let (encoding, ending) = editor.format;
        let unsaved = self
            .documents
            .id_for_path(&path)
            .is_some_and(|index| self.documents[index].is_dirty());
        if unsaved {
            return Err(self.t("merge.error.unsaved").into());
        }
        let bytes = crate::file_format::encode(&contents, encoding, ending);
        std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
        let staged = self.merge_file_is_conflicted(&path);
        if staged {
            let root = self.scm_workspace_root();
            crate::scm::stage_file(&root, &path, &self.translator)?;
        }
        self.merge_editor = None;
        self.status = if staged {
            self.t("scm.conflict.resolved_staged").into()
        } else {
            self.t("merge.written").into()
        };
        self.refresh_scm_async();
        self.sync_external_changes_after(std::time::Duration::ZERO);
        Ok(())
    }
}
