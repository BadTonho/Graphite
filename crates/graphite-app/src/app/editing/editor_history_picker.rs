//! A picker of the open editors in the order they were used, and the commands that accept or
//! close the quick open widgets.

use super::go_menu::{GoPicker, GoPickerKind, GoSymbolItem};
use super::*;

impl App {
    /// Lists the open editors, most recently used first (or least recently used first), and
    /// selects the previous one. With `group_only` only the focused group's editors are listed.
    pub(crate) fn open_editor_history_picker(&mut self, group_only: bool, least_recent: bool) {
        let views = if group_only {
            self.editor_groups
                .view_ids_in_group(self.editor_groups.focused())
                .unwrap_or_default()
        } else {
            self.editor_groups.ordered_view_ids()
        };
        let in_scope: Vec<DocumentId> = views
            .iter()
            .filter_map(|view| self.editor_groups.view(*view).map(|view| view.document))
            .collect();
        let mut ordered: Vec<DocumentId> = Vec::new();
        for id in self.recent_documents.clone() {
            if let Some(document) = self.documents.resolve(id) {
                if in_scope.contains(&document) && !ordered.contains(&document) {
                    ordered.push(document);
                }
            }
        }
        for document in in_scope {
            if !ordered.contains(&document) {
                ordered.push(document);
            }
        }
        if least_recent {
            ordered.reverse();
        }
        let items: Vec<GoSymbolItem> = ordered
            .into_iter()
            .filter_map(|document| self.history_item(document))
            .collect();
        let selected = if least_recent {
            0
        } else {
            1.min(items.len().saturating_sub(1))
        };
        let message = items.is_empty().then(|| self.t("go.no_editors").to_owned());
        self.command_palette = None;
        self.quick_open = None;
        self.local_search = None;
        self.go_picker_generation = self.go_picker_generation.wrapping_add(1);
        self.go_picker = Some(GoPicker {
            kind: GoPickerKind::Locations,
            query: String::new(),
            items,
            selected,
            loading: false,
            generation: self.go_picker_generation,
            message,
        });
    }

    fn history_item(&self, document: DocumentId) -> Option<GoSymbolItem> {
        let tab = &self.documents[document];
        let path = tab.buffer.path()?.to_path_buf();
        let line = self
            .editor_groups
            .all_views()
            .find(|view| view.document == document)
            .map_or(0, |view| {
                let rope = tab.buffer.rope();
                rope.char_to_line(view.selection_state.cursor().char_idx.min(rope.len_chars()))
            });
        Some(GoSymbolItem {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            scope: path
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            line,
        })
    }

    /// Accepts the selected item of whichever quick widget is open.
    pub(crate) fn accept_quick_widget(&mut self) {
        if self.quick_open.is_some() {
            self.execute_quick_open_command("quickOpen.accept");
        } else if self.go_picker.is_some() {
            self.accept_go_picker();
        }
    }

    /// Like accepting, but a file from quick open opens as a permanent tab, not a preview.
    pub(crate) fn accept_quick_widget_permanently(&mut self) {
        let path = self
            .quick_open
            .as_ref()
            .and_then(|state| state.paths.get(state.index).cloned());
        match path {
            Some(path) => {
                self.execute_quick_open_command("quickOpen.close");
                self.open_file(path);
            }
            None => self.accept_quick_widget(),
        }
    }

    /// The quick widgets take the keyboard as soon as they open, so there is nothing to move; an
    /// open widget stays as it is, and with none open the file picker opens.
    pub(crate) fn focus_quick_widgets(&mut self) {
        if self.quick_open.is_none() && self.go_picker.is_none() && self.command_palette.is_none() {
            self.start_quick_open();
        }
    }

    pub(crate) fn close_quick_widgets(&mut self) {
        if self.quick_open.is_some() {
            self.execute_quick_open_command("quickOpen.close");
        }
        self.go_picker = None;
        self.command_palette = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn app_with_files(names: &[&str]) -> (App, TempDir) {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        for name in names {
            std::fs::write(dir.path().join(name), name).unwrap();
            app.open_file(dir.path().join(name));
        }
        (app, dir)
    }

    fn names(app: &App) -> Vec<String> {
        app.go_picker
            .as_ref()
            .map(|picker| picker.items.iter().map(|item| item.name.clone()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn the_history_picker_lists_editors_most_recent_first_with_the_previous_selected() {
        let (mut app, _dir) = app_with_files(&["a.txt", "b.txt", "c.txt"]);
        app.run_ui_command("workbench.action.quickOpenPreviousRecentlyUsedEditor");
        let picker = app.go_picker.as_ref().expect("picker");
        assert_eq!(picker.selected, 1);
        let listed = names(&app);
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0], "c.txt", "the active editor comes first");
        app.accept_quick_widget();
        assert!(app.go_picker.is_none());
        let active = app.documents[app.active_document()]
            .buffer
            .path()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(active, listed[1]);
    }

    #[test]
    fn least_recently_used_lists_the_oldest_first() {
        let (mut app, _dir) = app_with_files(&["a.txt", "b.txt", "c.txt"]);
        app.run_ui_command("workbench.action.quickOpenPreviousRecentlyUsedEditor");
        let recent = names(&app);
        app.run_ui_command("workbench.action.quickOpenLeastRecentlyUsedEditor");
        let least: Vec<_> = recent.iter().rev().cloned().collect();
        assert_eq!(names(&app), least);
        assert_eq!(app.go_picker.as_ref().unwrap().selected, 0);
    }

    #[test]
    fn the_in_group_picker_only_lists_that_groups_editors() {
        let (mut app, _dir) = app_with_files(&["a.txt", "b.txt"]);
        app.run_ui_command("workbench.action.newGroupRight");
        app.run_ui_command("workbench.action.closeEditorsToTheLeft");
        app.run_ui_command("workbench.action.quickOpenPreviousRecentlyUsedEditorInGroup");
        assert_eq!(names(&app), ["b.txt"]);
        app.run_ui_command("workbench.action.quickOpenPreviousRecentlyUsedEditor");
        assert_eq!(names(&app).len(), 2, "all groups, each document once");
    }

    #[test]
    fn closing_clears_every_quick_widget() {
        let (mut app, _dir) = app_with_files(&["a.txt", "b.txt"]);
        app.run_ui_command("workbench.action.quickOpenPreviousRecentlyUsedEditor");
        assert!(app.go_picker.is_some());
        app.run_ui_command("workbench.action.closeQuickOpen");
        assert!(app.go_picker.is_none());
    }

    #[test]
    fn accepting_quick_open_opens_the_selected_file_as_a_permanent_tab_when_asked() {
        let (mut app, dir) = app_with_files(&["a.txt"]);
        std::fs::write(dir.path().join("z.txt"), "z").unwrap();
        app.quick_open = Some(crate::app::quick_open::QuickOpen {
            query: String::new(),
            paths: vec![dir.path().join("z.txt")],
            index: 0,
        });
        app.run_ui_command("workbench.action.alternativeAcceptSelectedQuickOpenItem");
        assert!(app.quick_open.is_none());
        let id = app
            .documents
            .id_for_path(&dir.path().join("z.txt"))
            .unwrap();
        let view = app
            .editor_groups
            .all_views()
            .find(|view| view.document == id)
            .unwrap();
        assert!(view.preview_revision.is_none(), "not a preview tab");
    }

    #[test]
    fn the_recently_used_ids_switch_editors_like_the_existing_ones() {
        let (mut app, _dir) = app_with_files(&["a.txt", "b.txt", "c.txt"]);
        let before = app.active_document();
        app.run_ui_command("workbench.action.openPreviousRecentlyUsedEditor");
        assert_ne!(app.active_document(), before);
        app.run_ui_command("workbench.action.openNextRecentlyUsedEditorInGroup");
        assert_eq!(app.active_document(), before);
    }
}
