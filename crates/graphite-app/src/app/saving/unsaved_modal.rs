//! Centered modal that lists unsaved documents while a destructive decision is pending
//! (switching workspace, closing the window or reverting a file).
use super::*;

/// Maximum number of file names listed before the rest is summarized.
const MAX_LISTED: usize = 6;
const ROW_HEIGHT: f32 = 20.0;
const BUTTON_HEIGHT: f32 = 28.0;

type Rect = (f32, f32, f32, f32);

/// Geometry of the modal for a window size and a number of listed rows.
pub(crate) struct UnsavedModalLayout {
    pub panel: Rect,
    pub save: Rect,
    pub discard: Rect,
    pub cancel: Rect,
    pub list_top: f32,
}

fn inside(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

impl App {
    /// Whether the unsaved-changes modal must be shown.
    pub(crate) fn unsaved_modal_visible(&self) -> bool {
        self.close_window_confirm
    }

    /// Documents inside the repository that differ from what is on disk.
    fn commit_unsaved_documents(&self) -> Vec<DocumentId> {
        let root = self.scm_workspace_root();
        let root = root.canonicalize().unwrap_or(root);
        self.documents
            .iter()
            .filter(|d| d.is_dirty() || self.saving.document_pending(d.id))
            .filter(|d| {
                d.buffer.path().is_some_and(|path| {
                    path.canonicalize()
                        .unwrap_or_else(|_| path.to_path_buf())
                        .starts_with(&root)
                })
            })
            .map(|d| d.id)
            .collect()
    }

    /// Asks what to do about unsaved documents before a commit. Returns true when the commit
    /// is held back until the user answers.
    pub(crate) fn defer_commit_for_unsaved(
        &mut self,
        options: crate::scm_commit::CommitOptions,
    ) -> bool {
        if std::mem::take(&mut self.scm_commit_save_checked) {
            return false;
        }
        if self.commit_unsaved_documents().is_empty() {
            return false;
        }
        if self.close_window_confirm || self.pending_commit_save.is_some() {
            self.status = self.t("workspace.decision_pending").into();
            return true;
        }
        self.pending_commit_save = Some(options);
        self.close_window_confirm = true;
        self.status = self.t("unsaved_modal.commit_prompt").into();
        true
    }

    /// Answers the modal: save and commit, commit what is on disk, or cancel.
    pub(crate) fn resolve_commit_save(&mut self, save: bool, cancel: bool) {
        let Some(options) = self.pending_commit_save.take() else {
            return;
        };
        self.close_window_confirm = false;
        if cancel {
            self.status = self.t("unsaved_modal.commit_cancelled").into();
        } else if save {
            let documents = self.commit_unsaved_documents();
            self.save_batch(documents, AfterSave::Commit(options));
        } else {
            self.scm_commit_save_checked = true;
            self.perform_scm_action(crate::app::scm_ui::ScmAction::Commit(options));
        }
    }

    /// Display names of the documents with unsaved changes (or a save in flight).
    pub(crate) fn unsaved_document_names(&self) -> Vec<String> {
        let commit_ids = self
            .pending_commit_save
            .is_some()
            .then(|| self.commit_unsaved_documents());
        self.documents
            .iter()
            .filter(|d| match &commit_ids {
                Some(ids) => ids.contains(&d.id),
                None => d.is_dirty() || self.saving.document_pending(d.id),
            })
            .map(|d| match d.buffer.path() {
                Some(path) => path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
                None => self.t("unsaved_modal.untitled").to_owned(),
            })
            .collect()
    }

    fn unsaved_modal_title_key(&self) -> &'static str {
        if self.pending_commit_save.is_some() {
            "unsaved_modal.title_commit"
        } else if self.pending_revert_confirmation.is_some() {
            "unsaved_modal.title_revert"
        } else if self.workspace_decision_pending() {
            "unsaved_modal.title_workspace"
        } else {
            "unsaved_modal.title_close"
        }
    }

    pub(crate) fn unsaved_modal_layout(
        width: f32,
        height: f32,
        listed: usize,
    ) -> UnsavedModalLayout {
        let panel_w = (width * 0.62)
            .clamp(360.0, 520.0)
            .min((width - 24.0).max(0.0));
        let rows = listed.min(MAX_LISTED + 1) as f32;
        let list_top = 74.0;
        let panel_h = list_top + rows * ROW_HEIGHT + 18.0 + BUTTON_HEIGHT + 16.0;
        let x = ((width - panel_w) * 0.5).max(12.0);
        let y = ((height - panel_h) * 0.5).max(12.0);
        let button_y = y + panel_h - BUTTON_HEIGHT - 14.0;
        let button_w = ((panel_w - 32.0 - 16.0) / 3.0).max(40.0);
        let button = |index: f32| {
            (
                x + 16.0 + index * (button_w + 8.0),
                button_y,
                button_w,
                BUTTON_HEIGHT,
            )
        };
        UnsavedModalLayout {
            panel: (x, y, panel_w, panel_h),
            save: button(0.0),
            discard: button(1.0),
            cancel: button(2.0),
            list_top,
        }
    }

    /// Handles a click while the modal is visible. Always consumes the click.
    pub(crate) fn click_unsaved_modal(
        &mut self,
        mx: f32,
        my: f32,
        width: f32,
        height: f32,
    ) -> bool {
        if !self.unsaved_modal_visible() {
            return false;
        }
        let layout = Self::unsaved_modal_layout(width, height, self.unsaved_document_names().len());
        if inside(layout.save, mx, my) {
            self.run_ui_command("graphite.window.saveAndClose");
        } else if inside(layout.discard, mx, my) {
            self.run_ui_command("graphite.window.discardAndClose");
        } else if inside(layout.cancel, mx, my) {
            self.run_ui_command("graphite.window.cancelClose");
        }
        true
    }

    pub(crate) fn draw_unsaved_modal(&self, renderer: &mut Renderer) {
        if !self.unsaved_modal_visible() {
            return;
        }
        let names = self.unsaved_document_names();
        let (width, height) = renderer.size();
        let cw = renderer.char_width();
        let layout = Self::unsaved_modal_layout(width, height, names.len());
        let (x, y, panel_w, panel_h) = layout.panel;
        let border = rgb(Theme::color(Theme::DIM, 0x454545));
        let bg = rgb(Theme::color(Theme::SIDEBAR, 0x252526));
        let text = rgb(Theme::color(Theme::TEXT, 0xd4d4d4));
        let dim = rgb(Theme::color(Theme::DIM, 0x858585));
        let primary_bg = rgb(Theme::color(Theme::STATUS, 0x0e639c));
        let secondary_bg = rgb(Theme::color(Theme::ACTIVE_ROW, 0x37373d));
        let button_fg = rgb(0xffffff);

        renderer.draw_rect(x, y, panel_w, panel_h, border);
        renderer.draw_rect(
            x + 1.0,
            y + 1.0,
            (panel_w - 2.0).max(0.0),
            (panel_h - 2.0).max(0.0),
            bg,
        );
        renderer.draw_text(
            self.t(self.unsaved_modal_title_key()),
            x + 16.0,
            y + 14.0,
            text,
        );
        let subtitle = self.tf("unsaved_modal.subtitle", &[("count", &names.len())]);
        let inner = (panel_w - 32.0).max(0.0);
        renderer.draw_text(
            &Self::help_menu_visible_label(&subtitle, cw, inner),
            x + 16.0,
            y + 40.0,
            dim,
        );

        let mut row_y = y + layout.list_top;
        for name in names.iter().take(MAX_LISTED) {
            let label = format!("• {name}");
            renderer.draw_text(
                &Self::help_menu_visible_label(&label, cw, inner),
                x + 24.0,
                row_y,
                text,
            );
            row_y += ROW_HEIGHT;
        }
        if names.len() > MAX_LISTED {
            let more = self.tf(
                "unsaved_modal.more",
                &[("count", &(names.len() - MAX_LISTED))],
            );
            renderer.draw_text(&more, x + 24.0, row_y, dim);
        }

        let (save_key, discard_key) = if self.pending_commit_save.is_some() {
            ("unsaved_modal.commit_save", "unsaved_modal.commit_anyway")
        } else {
            ("unsaved_modal.save_all", "unsaved_modal.discard")
        };
        for (rect, key, background, foreground) in [
            (layout.save, save_key, primary_bg, button_fg),
            (layout.discard, discard_key, secondary_bg, text),
            (layout.cancel, "unsaved_modal.cancel", secondary_bg, text),
        ] {
            renderer.draw_rect(rect.0, rect.1, rect.2, rect.3, background);
            let label = Self::help_menu_visible_label(self.t(key), cw, rect.2 - 8.0);
            let label_x = rect.0 + (rect.2 - label.chars().count() as f32 * cw).max(0.0) * 0.5;
            renderer.draw_text(&label, label_x, rect.1 + 5.0, foreground);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn modal_is_hidden_without_a_pending_decision() {
        let dir = TempDir::new();
        let app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(!app.unsaved_modal_visible());
    }

    #[test]
    fn workspace_switch_with_dirty_document_shows_modal_with_its_name() {
        let dir = TempDir::new();
        let other = TempDir::new();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "a").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file);
        app.editor_mut().insert("edit");
        assert!(app.documents.iter().any(|d| d.is_dirty()));

        app.replace_workspace_roots(vec![other.path().to_path_buf()], None);

        assert!(app.unsaved_modal_visible());
        assert!(app.workspace_decision_pending());
        assert!(app
            .unsaved_document_names()
            .iter()
            .any(|n| n == "notes.txt"));
        assert_eq!(
            app.unsaved_modal_title_key(),
            "unsaved_modal.title_workspace"
        );
    }

    #[test]
    fn clicking_cancel_closes_the_modal_and_keeps_the_workspace() {
        let dir = TempDir::new();
        let other = TempDir::new();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "a").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        let root = app.tree.root().to_path_buf();
        app.open_file(file);
        app.editor_mut().insert("edit");
        app.replace_workspace_roots(vec![other.path().to_path_buf()], None);

        let layout = App::unsaved_modal_layout(1200.0, 800.0, app.unsaved_document_names().len());
        let (x, y, w, h) = layout.cancel;
        assert!(app.click_unsaved_modal(x + w / 2.0, y + h / 2.0, 1200.0, 800.0));

        assert!(!app.unsaved_modal_visible());
        assert!(!app.workspace_decision_pending());
        assert_eq!(app.tree.root(), root);
    }

    #[test]
    fn clicks_outside_buttons_are_consumed_without_resolving() {
        let dir = TempDir::new();
        let other = TempDir::new();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "a").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_file(file);
        app.editor_mut().insert("edit");
        app.replace_workspace_roots(vec![other.path().to_path_buf()], None);

        assert!(app.click_unsaved_modal(1.0, 1.0, 1200.0, 800.0));
        assert!(app.unsaved_modal_visible());
        assert!(app.workspace_decision_pending());
    }

    #[test]
    fn layout_buttons_stay_inside_the_panel_and_do_not_overlap() {
        for listed in [1, 3, 20] {
            let l = App::unsaved_modal_layout(1000.0, 700.0, listed);
            for b in [l.save, l.discard, l.cancel] {
                assert!(b.0 >= l.panel.0 && b.0 + b.2 <= l.panel.0 + l.panel.2);
                assert!(b.1 + b.3 <= l.panel.1 + l.panel.3);
            }
            assert!(l.save.0 + l.save.2 <= l.discard.0);
            assert!(l.discard.0 + l.discard.2 <= l.cancel.0);
        }
    }

    fn dirty_file_app(dir: &TempDir) -> App {
        let file = dir.path().join("teste3.cpp");
        std::fs::write(&file, "").unwrap();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.scm_commit_message = "msg".into();
        app.open_file(file);
        app.editor_mut().insert("int main() {}");
        app
    }

    #[test]
    fn commit_with_unsaved_file_asks_first_and_lists_it() {
        let dir = TempDir::new();
        let mut app = dirty_file_app(&dir);
        app.perform_scm_action(crate::app::scm_ui::ScmAction::Commit(
            crate::scm_commit::CommitOptions::PLAIN,
        ));
        assert!(app.pending_commit_save.is_some());
        assert!(app.unsaved_modal_visible());
        assert_eq!(app.unsaved_modal_title_key(), "unsaved_modal.title_commit");
        assert_eq!(app.unsaved_document_names(), vec!["teste3.cpp".to_string()]);
    }

    #[test]
    fn commit_without_unsaved_files_does_not_ask() {
        let dir = TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        assert!(!app.defer_commit_for_unsaved(crate::scm_commit::CommitOptions::PLAIN));
        assert!(app.pending_commit_save.is_none());
    }

    #[test]
    fn cancelling_the_commit_prompt_keeps_the_file_unsaved_and_the_message() {
        let dir = TempDir::new();
        let mut app = dirty_file_app(&dir);
        app.perform_scm_action(crate::app::scm_ui::ScmAction::Commit(
            crate::scm_commit::CommitOptions::PLAIN,
        ));
        app.run_ui_command("graphite.window.cancelClose");
        assert!(app.pending_commit_save.is_none());
        assert!(!app.unsaved_modal_visible());
        assert_eq!(app.scm_commit_message, "msg");
        assert!(app.documents.iter().any(|d| d.is_dirty()));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("teste3.cpp")).unwrap(),
            ""
        );
    }

    #[test]
    fn save_and_commit_writes_the_file_before_the_commit_runs() {
        let dir = TempDir::new();
        let mut app = dirty_file_app(&dir);
        app.perform_scm_action(crate::app::scm_ui::ScmAction::Commit(
            crate::scm_commit::CommitOptions::PLAIN,
        ));
        app.run_ui_command("graphite.window.saveAndClose");
        for _ in 0..200 {
            app.poll_saves(std::time::Instant::now());
            if !app.documents.iter().any(|d| d.is_dirty()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            std::fs::read_to_string(dir.path().join("teste3.cpp")).unwrap(),
            "int main() {}"
        );
        assert!(app.pending_commit_save.is_none());
    }
}
