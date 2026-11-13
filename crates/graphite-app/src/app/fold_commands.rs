//! Editor state and commands for code folding.
//!
//! Folds are stored per view as the start lines of collapsed regions. A document revision
//! change clears them, because the stored lines may no longer describe the same blocks.

use super::editor_groups::View;
use super::folding;
use super::*;

/// Folds that apply to `view` for a document at `revision`. Folds made against an
/// earlier revision are stale and are treated as absent until a command refreshes them.
pub(crate) fn effective_folds(view: &View, revision: u64) -> &[usize] {
    folds_for_revision(&view.folded, view.folded_revision, revision)
}

fn folds_for_revision(folded: &[usize], folded_revision: u64, revision: u64) -> &[usize] {
    if folded_revision == revision {
        folded
    } else {
        &[]
    }
}

impl App {
    /// Refreshes the active view's folds before drawing: drops stale folds and reveals
    /// the cursor line when a fold hides it.
    pub(crate) fn sync_active_folds(&mut self) {
        let document = self.active_document();
        let cursor = self.editor().cursor().char_idx;
        let buffer = &self.documents[document].buffer;
        let revision = buffer.revision();
        let tab_size = self.documents[document].indentation.tab_size;
        let provided = self.documents[document].provided_folds();
        let cursor_line = buffer.rope().char_to_line(cursor.min(buffer.len_chars()));
        let view = self.editor_groups.active_view_mut();
        if view.folded_revision != revision {
            view.folded.clear();
            view.folded_revision = revision;
        }
        view.folded.retain(|&start| {
            !folding::region_end(buffer, start, tab_size, provided)
                .is_some_and(|end| start < cursor_line && cursor_line <= end)
        });
    }

    /// Editor line shown on the `row`-th visible row of the active view.
    pub(crate) fn editor_line_at_row(&self, row: usize) -> usize {
        let document = self.active_document();
        let tab = &self.documents[document];
        let view = self.editor_groups.active_view();
        let last = tab.buffer.len_lines().saturating_sub(1);
        let hidden = folding::hidden_ranges(
            &tab.buffer,
            effective_folds(view, tab.buffer.revision()),
            tab.indentation.tab_size,
            tab.provided_folds(),
        );
        folding::visible_lines(&tab.buffer, &hidden, view.scroll_line, row + 1)
            .last()
            .copied()
            .unwrap_or(last)
            .min(last)
    }

    fn cursor_line(&self) -> usize {
        let document = self.active_document();
        let buffer = &self.documents[document].buffer;
        let cursor = self.editor().cursor().char_idx;
        buffer.rope().char_to_line(cursor.min(buffer.len_chars()))
    }

    fn set_cursor_to_line(&mut self, line: usize) {
        self.editor_mut().set_cursor_line_col(line, 0);
    }

    fn fold_unavailable_with_wrap(&mut self) -> bool {
        if self.word_wrap {
            self.status = self.t("fold.unavailable_wrap").to_owned();
            true
        } else {
            false
        }
    }

    /// Folds the innermost region around the cursor. When that region is already
    /// folded, folds the enclosing region instead. The cursor moves to the fold's first line.
    pub(crate) fn fold_at_cursor(&mut self) {
        if self.fold_unavailable_with_wrap() {
            return;
        }
        self.sync_active_folds();
        let document = self.active_document();
        let tab_size = self.documents[document].indentation.tab_size;
        let revision = self.documents[document].buffer.revision();
        let mut line = self.cursor_line();
        loop {
            let Some((start, _)) = folding::innermost_region(
                &self.documents[document].buffer,
                line,
                tab_size,
                self.documents[document].provided_folds(),
            ) else {
                self.status = self.t("fold.no_region").to_owned();
                return;
            };
            let view = self.editor_groups.active_view();
            if let Err(index) = view.folded.binary_search(&start) {
                self.set_cursor_to_line(start);
                let view = self.editor_groups.active_view_mut();
                view.folded.insert(index, start);
                view.folded_revision = revision;
                return;
            }
            if start == 0 {
                self.status = self.t("fold.no_region").to_owned();
                return;
            }
            line = start - 1;
        }
    }

    /// Unfolds the fold that starts on the cursor line, or else the innermost fold
    /// hiding the cursor line.
    pub(crate) fn unfold_at_cursor(&mut self) {
        if self.fold_unavailable_with_wrap() {
            return;
        }
        self.sync_active_folds();
        let document = self.active_document();
        let tab_size = self.documents[document].indentation.tab_size;
        let line = self.cursor_line();
        let provided = self.documents[document].provided_folds();
        let buffer = &self.documents[document].buffer;
        let view = self.editor_groups.active_view_mut();
        let target = view
            .folded
            .iter()
            .copied()
            .filter(|&start| {
                start == line
                    || folding::region_end(buffer, start, tab_size, provided)
                        .is_some_and(|end| start < line && line <= end)
            })
            .max();
        if let Some(start) = target {
            view.folded.retain(|&folded| folded != start);
        }
    }

    /// Folds every top-level region of the document.
    pub(crate) fn fold_all(&mut self) {
        if self.fold_unavailable_with_wrap() {
            return;
        }
        self.sync_active_folds();
        let document = self.active_document();
        let tab = &self.documents[document];
        let tab_size = tab.indentation.tab_size;
        let revision = tab.buffer.revision();
        let mut starts = Vec::new();
        let mut line = 0;
        while line < tab.buffer.len_lines() {
            match folding::region_end(&tab.buffer, line, tab_size, tab.provided_folds()) {
                Some(end) => {
                    starts.push(line);
                    line = end + 1;
                }
                None => line += 1,
            }
        }
        let cursor_line = self.cursor_line();
        let owner = folding::visible_owner(
            &folding::hidden_ranges(
                &self.documents[document].buffer,
                &starts,
                tab_size,
                self.documents[document].provided_folds(),
            ),
            cursor_line,
        );
        if owner != cursor_line {
            self.set_cursor_to_line(owner);
        }
        let view = self.editor_groups.active_view_mut();
        view.folded = starts;
        view.folded_revision = revision;
    }

    /// Unfolds every region of the active view.
    pub(crate) fn unfold_all(&mut self) {
        if self.fold_unavailable_with_wrap() {
            return;
        }
        let revision = self.documents[self.active_document()].buffer.revision();
        let view = self.editor_groups.active_view_mut();
        view.folded.clear();
        view.folded_revision = revision;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str =
        "fn main() {\n    let a = 1;\n    if a > 0 {\n        println!(\"x\");\n    }\n}\n";

    fn app_with(text: &str) -> (crate::test_support::TempDir, App) {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.editor_mut().insert(text);
        app.editor_mut().set_cursor_line_col(0, 0);
        (dir, app)
    }

    fn line_of_cursor(app: &App) -> usize {
        let index = app.editor().cursor().char_idx;
        app.editor()
            .text()
            .chars()
            .take(index)
            .filter(|&ch| ch == '\n')
            .count()
    }

    fn cursor_to(app: &mut App, line: usize) {
        app.editor_mut().set_cursor_line_col(line, 0);
    }

    #[test]
    fn stale_folds_from_an_earlier_revision_are_ignored() {
        assert_eq!(folds_for_revision(&[0, 2], 4, 4), &[0, 2]);
        assert!(folds_for_revision(&[0, 2], 4, 5).is_empty());
    }

    #[test]
    fn fold_hides_the_body_and_moves_the_cursor_to_the_fold_start() {
        let (_dir, mut app) = app_with(SAMPLE);
        cursor_to(&mut app, 1);
        app.run_ui_command("editor.fold");
        assert_eq!(app.editor_groups.active_view().folded, vec![0]);
        assert_eq!(line_of_cursor(&app), 0);
        assert_eq!(app.editor_line_at_row(1), 5);
    }

    #[test]
    fn folding_an_already_folded_region_folds_the_enclosing_one() {
        let (_dir, mut app) = app_with(SAMPLE);
        cursor_to(&mut app, 3);
        app.run_ui_command("editor.fold");
        assert_eq!(app.editor_groups.active_view().folded, vec![2]);
        app.run_ui_command("editor.fold");
        assert_eq!(app.editor_groups.active_view().folded, vec![0, 2]);
    }

    #[test]
    fn unfold_removes_the_fold_at_the_cursor_line() {
        let (_dir, mut app) = app_with(SAMPLE);
        cursor_to(&mut app, 0);
        app.run_ui_command("editor.fold");
        app.run_ui_command("editor.unfold");
        assert!(app.editor_groups.active_view().folded.is_empty());
    }

    #[test]
    fn fold_all_keeps_only_top_level_regions_and_moves_a_hidden_cursor_to_its_owner() {
        let (_dir, mut app) = app_with(SAMPLE);
        cursor_to(&mut app, 3);
        app.run_ui_command("editor.foldAll");
        assert_eq!(app.editor_groups.active_view().folded, vec![0]);
        assert_eq!(line_of_cursor(&app), 0);
    }

    #[test]
    fn unfold_all_clears_every_fold() {
        let (_dir, mut app) = app_with(SAMPLE);
        app.run_ui_command("editor.foldAll");
        app.run_ui_command("editor.unfoldAll");
        assert!(app.editor_groups.active_view().folded.is_empty());
    }

    #[test]
    fn moving_the_cursor_into_a_folded_body_reveals_it() {
        let (_dir, mut app) = app_with(SAMPLE);
        cursor_to(&mut app, 0);
        app.run_ui_command("editor.fold");
        cursor_to(&mut app, 3);
        app.sync_active_folds();
        assert!(app.editor_groups.active_view().folded.is_empty());
    }

    #[test]
    fn editing_the_document_clears_folds_made_before_the_edit() {
        let (_dir, mut app) = app_with(SAMPLE);
        cursor_to(&mut app, 0);
        app.run_ui_command("editor.fold");
        cursor_to(&mut app, 5);
        app.editor_mut().insert("// note\n");
        app.sync_active_folds();
        assert!(app.editor_groups.active_view().folded.is_empty());
    }

    #[test]
    fn folding_without_a_region_reports_it_and_changes_nothing() {
        let (_dir, mut app) = app_with("plain\ntext\n");
        app.run_ui_command("editor.fold");
        assert!(app.editor_groups.active_view().folded.is_empty());
        assert_eq!(app.status, app.t("fold.no_region"));
    }

    #[test]
    fn folding_is_refused_while_word_wrap_is_on() {
        let (_dir, mut app) = app_with(SAMPLE);
        app.word_wrap = true;
        cursor_to(&mut app, 0);
        app.run_ui_command("editor.fold");
        assert!(app.editor_groups.active_view().folded.is_empty());
        assert_eq!(app.status, app.t("fold.unavailable_wrap"));
    }
}
