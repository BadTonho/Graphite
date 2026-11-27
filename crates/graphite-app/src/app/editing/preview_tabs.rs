//! Preview and pinned editor tabs.
//!
//! Opening a file from the explorer or Quick Open reuses the focused group's preview tab,
//! if it has one. Editing a preview tab makes it permanent. Pinned tabs stay open, and
//! they sort before unpinned tabs in their group.

use super::*;
use graphite_text::ViewId;

impl App {
    /// Opens `path` as a preview tab. An existing preview tab in the focused group is
    /// replaced. Files that are already open are simply activated, as with `open_file`.
    pub(crate) fn open_file_preview(&mut self, path: PathBuf) {
        if self.ai_mode.is_some() || self.documents.id_for_path(&path).is_some() {
            self.open_file(path);
            return;
        }
        let group = self.editor_groups.focused();
        let previous_preview = self.editor_groups.views(group).and_then(|(tabs, _)| {
            tabs.iter()
                .find(|view| view.preview_revision.is_some() && !view.pinned)
                .map(|view| view.id)
        });
        let before: std::collections::HashSet<ViewId> =
            self.editor_groups.all_views().map(|view| view.id).collect();
        self.open_file(path);
        let opened = self.editor_groups.active_view().id;
        if before.contains(&opened) {
            return;
        }
        let revision = self.documents[self.active_document()].buffer.revision();
        if let Some(view) = self.editor_groups.view_mut(opened) {
            view.preview_revision = Some(revision);
        }
        if let Some(old) = previous_preview.filter(|&id| id != opened) {
            self.editor_groups.close_view(old, false);
            self.editor_groups.focus_view(opened);
        }
    }

    /// Makes the active preview tab permanent once its document has changed.
    pub(crate) fn sync_active_preview(&mut self) {
        let revision = self.documents[self.active_document()].buffer.revision();
        let view = self.editor_groups.active_view_mut();
        if view
            .preview_revision
            .is_some_and(|opened| opened != revision)
        {
            view.preview_revision = None;
        }
    }

    /// Pins or unpins the active tab. Pinning also makes a preview tab permanent.
    pub(crate) fn toggle_pin_active_view(&mut self) {
        let id = self.editor_groups.active_view().id;
        let Some(group) = self.editor_groups.group_for_view(id) else {
            return;
        };
        if let Some(view) = self.editor_groups.view_mut(id) {
            view.pinned = !view.pinned;
            if view.pinned {
                view.preview_revision = None;
            }
        }
        self.editor_groups.order_group_tabs(group);
        self.status = if self.editor_groups.active_view().pinned {
            self.t("tabs.pinned").to_owned()
        } else {
            self.t("tabs.unpinned").to_owned()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn focused_tabs(app: &App) -> Vec<(bool, bool)> {
        let group = app.editor_groups.focused();
        app.editor_groups
            .views(group)
            .unwrap()
            .0
            .iter()
            .map(|view| (view.pinned, view.preview_revision.is_some()))
            .collect()
    }

    fn write(dir: &crate::test_support::TempDir, name: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, "fn sample() {}\n").unwrap();
        path
    }

    #[test]
    fn opening_a_second_file_in_preview_replaces_the_preview_tab() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        let first = write(&dir, "a.rs");
        let second = write(&dir, "b.rs");
        app.open_file_preview(first);
        let after_first = focused_tabs(&app).len();
        app.open_file_preview(second.clone());
        assert_eq!(focused_tabs(&app).len(), after_first);
        assert_eq!(
            app.documents[app.active_document()].buffer.path(),
            Some(second.as_path())
        );
        assert_eq!(
            focused_tabs(&app)
                .iter()
                .filter(|(_, preview)| *preview)
                .count(),
            1
        );
    }

    #[test]
    fn an_edit_makes_the_preview_tab_permanent() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file_preview(write(&dir, "a.rs"));
        app.editor_mut().insert("// note\n");
        app.sync_active_preview();
        let before = focused_tabs(&app).len();
        app.open_file_preview(write(&dir, "b.rs"));
        assert_eq!(focused_tabs(&app).len(), before + 1);
    }

    #[test]
    fn a_pinned_preview_tab_is_not_replaced() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file_preview(write(&dir, "a.rs"));
        app.toggle_pin_active_view();
        let before = focused_tabs(&app).len();
        app.open_file_preview(write(&dir, "b.rs"));
        assert_eq!(focused_tabs(&app).len(), before + 1);
        assert!(focused_tabs(&app).iter().any(|(pinned, _)| *pinned));
    }

    #[test]
    fn pinned_tabs_sort_before_unpinned_tabs_and_keep_the_active_one() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(write(&dir, "a.rs"));
        app.open_file(write(&dir, "b.rs"));
        app.toggle_pin_active_view();
        let tabs = focused_tabs(&app);
        assert!(tabs[0].0, "the pinned tab moves to the front");
        let active = app
            .editor_groups
            .views(app.editor_groups.focused())
            .unwrap()
            .1;
        assert_eq!(active, 0);
        assert_eq!(
            app.documents[app.active_document()]
                .buffer
                .path()
                .map(|p| p.file_name().unwrap().to_owned()),
            Some("b.rs".into())
        );
    }

    #[test]
    fn opening_an_already_open_file_in_preview_does_not_add_a_tab() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        let path = write(&dir, "a.rs");
        app.open_file_preview(path.clone());
        let before = focused_tabs(&app).len();
        app.open_file_preview(path);
        assert_eq!(focused_tabs(&app).len(), before);
    }
}
