//! Folding commands that work on the tree of fold regions: toggling, recursive and per-level
//! folds, folding around the selection, moving between folds, ranges made by hand, and
//! comment, marker and import regions.

use super::fold_commands::region_source;
use super::folding::{self, Region};
use super::*;
use std::collections::BTreeSet;

/// The fold regions of the active document, which of them are collapsed, and the start line of
/// each selection. The commands edit `folded` and the result is stored back in the view.
struct Model {
    regions: Vec<Region>,
    folded: BTreeSet<usize>,
    lines: Vec<usize>,
}

impl Model {
    fn collapsed(&self, index: usize) -> bool {
        self.folded.contains(&self.regions[index].start)
    }

    /// Flips each listed region once.
    fn toggle(&mut self, indexes: Vec<usize>) {
        let mut seen = BTreeSet::new();
        for index in indexes {
            if seen.insert(index) {
                let start = self.regions[index].start;
                if !self.folded.remove(&start) {
                    self.folded.insert(start);
                }
            }
        }
    }

    /// Collapses or expands the region at each selection and, with `recursive`, everything in it.
    fn toggle_at_selections(&mut self, recursive: bool) {
        let mut targets = Vec::new();
        for &line in &self.lines {
            if let Some(index) = folding::region_at_line(&self.regions, line) {
                let collapse = !self.collapsed(index);
                targets.push(index);
                if recursive {
                    targets.extend(
                        folding::regions_inside(&self.regions, index)
                            .filter(|&inner| self.collapsed(inner) != collapse),
                    );
                }
            }
        }
        self.toggle(targets);
    }

    /// Sets the region at each selection and all regions inside it to `collapse`.
    fn set_recursively(&mut self, collapse: bool) {
        let mut targets = Vec::new();
        for &line in &self.lines {
            if let Some(index) = folding::region_at_line(&self.regions, line) {
                if self.collapsed(index) != collapse {
                    targets.push(index);
                }
                targets.extend(
                    folding::regions_inside(&self.regions, index)
                        .filter(|&inner| self.collapsed(inner) != collapse),
                );
            }
        }
        self.toggle(targets);
    }

    /// Folds every region of nesting `level` unless a selection is inside it.
    fn fold_level(&mut self, level: usize) {
        let targets = (0..self.regions.len())
            .filter(|&index| {
                let region = &self.regions[index];
                region.level == level
                    && !self.collapsed(index)
                    && !self.lines.iter().any(|&line| region.contains_line(line))
            })
            .collect();
        self.toggle(targets);
    }

    /// Sets every region to `collapse` except the innermost region at each selection and the
    /// regions around or inside it.
    fn set_rest(&mut self, collapse: bool) {
        let kept: Vec<Region> = self
            .lines
            .iter()
            .filter_map(|&line| folding::region_at_line(&self.regions, line))
            .map(|index| self.regions[index])
            .collect();
        let targets = (0..self.regions.len())
            .filter(|&index| {
                let region = &self.regions[index];
                kept.iter()
                    .all(|kept| !kept.within(region) && !region.within(kept))
                    && self.collapsed(index) != collapse
            })
            .collect();
        self.toggle(targets);
    }

    /// Sets to `collapse` every region whose start line is in `starts`.
    fn set_matching(&mut self, collapse: bool, starts: &BTreeSet<usize>) {
        let targets = (0..self.regions.len())
            .filter(|&index| {
                self.collapsed(index) != collapse && starts.contains(&self.regions[index].start)
            })
            .collect();
        self.toggle(targets);
    }
}

impl App {
    /// Builds the model for the active view, lets `edit` change it, and stores the folds back.
    fn edit_folds(&mut self, edit: impl FnOnce(&mut Model, &Buffer)) {
        if self.fold_unavailable_with_wrap() {
            return;
        }
        self.sync_active_folds();
        let document = self.active_document();
        let provided =
            region_source(self.editor_groups.active_view(), &self.documents[document]).cloned();
        let tab = &self.documents[document];
        let regions = folding::regions(&tab.buffer, tab.indentation.tab_size, provided.as_ref());
        let rope = tab.buffer.rope();
        let lines = self
            .editor()
            .selections()
            .iter()
            .map(|s| rope.char_to_line(s.anchor.min(s.head).min(rope.len_chars())))
            .collect();
        let mut model = Model {
            regions,
            folded: self
                .editor_groups
                .active_view()
                .folded
                .iter()
                .copied()
                .collect(),
            lines,
        };
        edit(&mut model, &tab.buffer);
        self.store_folds(model.folded, provided);
    }

    /// Writes the collapsed start lines to the view. A cursor hidden by a fold moves to the first
    /// line of that fold.
    fn store_folds(&mut self, folded: BTreeSet<usize>, provided: Option<folding::Provided>) {
        let document = self.active_document();
        let tab = &self.documents[document];
        let revision = tab.buffer.revision();
        let folded: Vec<usize> = folded.into_iter().collect();
        let hidden = folding::hidden_ranges(
            &tab.buffer,
            &folded,
            tab.indentation.tab_size,
            provided.as_ref(),
        );
        let cursor = self.cursor_line();
        let owner = folding::visible_owner(&hidden, cursor);
        let view = self.editor_groups.active_view_mut();
        view.folded = folded;
        view.folded_revision = revision;
        if owner != cursor {
            self.set_cursor_to_line(owner);
        }
    }

    pub(crate) fn toggle_fold(&mut self) {
        self.edit_folds(|model, _| model.toggle_at_selections(false));
    }

    pub(crate) fn toggle_fold_recursively(&mut self) {
        self.edit_folds(|model, _| model.toggle_at_selections(true));
    }

    pub(crate) fn fold_recursively(&mut self) {
        self.edit_folds(|model, _| model.set_recursively(true));
    }

    pub(crate) fn unfold_recursively(&mut self) {
        self.edit_folds(|model, _| model.set_recursively(false));
    }

    pub(crate) fn fold_level(&mut self, level: usize) {
        self.edit_folds(|model, _| model.fold_level(level));
    }

    pub(crate) fn fold_all_except_selection(&mut self) {
        self.edit_folds(|model, _| model.set_rest(true));
    }

    pub(crate) fn unfold_all_except_selection(&mut self) {
        self.edit_folds(|model, _| model.set_rest(false));
    }

    /// Folds every region that opens with the language's block comment token.
    pub(crate) fn fold_all_block_comments(&mut self) {
        let Some(open) = self
            .active_comment_tokens()
            .and_then(|tokens| tokens.block)
            .map(|(open, _)| open)
        else {
            return;
        };
        self.edit_folds(|model, buffer| {
            let starts = (0..model.regions.len())
                .map(|index| model.regions[index].start)
                .filter(|&start| buffer.line(start).trim_start().starts_with(&open))
                .collect();
            model.set_matching(true, &starts);
        });
    }

    /// Folds or unfolds every region that starts on a region marker line.
    pub(crate) fn set_marker_regions(&mut self, collapse: bool) {
        self.edit_folds(|model, buffer| {
            let starts = (0..model.regions.len())
                .map(|index| model.regions[index].start)
                .filter(|&start| {
                    folding::marker(&buffer.line(start)) == Some(folding::Marker::Start)
                })
                .collect();
            model.set_matching(collapse, &starts);
        });
    }

    /// Moves the cursor to the start line of a nearby fold, chosen by `pick` from the regions
    /// and the cursor line.
    fn go_to_fold(&mut self, pick: fn(&[Region], usize) -> Option<usize>) {
        let document = self.active_document();
        let provided =
            region_source(self.editor_groups.active_view(), &self.documents[document]).cloned();
        let tab = &self.documents[document];
        let regions = folding::regions(&tab.buffer, tab.indentation.tab_size, provided.as_ref());
        if let Some(line) = pick(&regions, self.cursor_line()) {
            self.set_cursor_to_line(line);
        }
    }

    pub(crate) fn go_to_next_fold(&mut self) {
        self.go_to_fold(folding::next_fold_line);
    }

    pub(crate) fn go_to_previous_fold(&mut self) {
        self.go_to_fold(folding::previous_fold_line);
    }

    pub(crate) fn go_to_parent_fold(&mut self) {
        self.go_to_fold(folding::parent_fold_line);
    }

    /// Rebuilds the regions the active view uses so they include its manual ranges.
    fn rebuild_fold_regions(&mut self) {
        let document = self.active_document();
        let tab = &self.documents[document];
        let revision = tab.buffer.revision();
        let manual = self.editor_groups.active_view().manual_folds.clone();
        let regions = if manual.is_empty() {
            None
        } else {
            let mut map: folding::Provided =
                folding::regions(&tab.buffer, tab.indentation.tab_size, tab.provided_folds())
                    .iter()
                    .map(|region| (region.start, region.end))
                    .collect();
            for (start, end, by_hand) in manual {
                if by_hand || !map.contains_key(&start) {
                    map.insert(start, end);
                }
            }
            Some((revision, map))
        };
        self.editor_groups.active_view_mut().fold_regions = regions;
    }

    /// Makes a fold region of the lines each selection covers and folds it.
    pub(crate) fn create_fold_range_from_selection(&mut self) {
        if self.fold_unavailable_with_wrap() {
            return;
        }
        self.sync_active_folds();
        let document = self.active_document();
        let rope = self.documents[document].buffer.rope();
        let mut ranges = Vec::new();
        for selection in self.editor().selections() {
            let first = selection.anchor.min(selection.head).min(rope.len_chars());
            let last = selection.anchor.max(selection.head).min(rope.len_chars());
            let start = rope.char_to_line(first);
            let mut end = rope.char_to_line(last);
            if last == rope.line_to_char(end) && end > start {
                end -= 1;
            }
            if end > start {
                ranges.push((start, end));
            }
        }
        let Some(&(cursor, _)) = ranges.last() else {
            return;
        };
        let view = self.editor_groups.active_view_mut();
        for &(start, end) in &ranges {
            view.manual_folds.retain(|&(other, _, _)| other != start);
            view.manual_folds.push((start, end, true));
            if let Err(index) = view.folded.binary_search(&start) {
                view.folded.insert(index, start);
            }
        }
        self.rebuild_fold_regions();
        self.set_cursor_to_line(cursor);
    }

    /// Removes the manual ranges at the cursor, or the ones a selection touches. A cursor that is
    /// in no manual range removes all of them.
    pub(crate) fn remove_manual_fold_ranges(&mut self) {
        self.sync_active_folds();
        let document = self.active_document();
        let rope = self.documents[document].buffer.rope();
        let selections: Vec<(usize, usize, bool)> = self
            .editor()
            .selections()
            .iter()
            .map(|s| {
                let first = s.anchor.min(s.head).min(rope.len_chars());
                let last = s.anchor.max(s.head).min(rope.len_chars());
                (
                    rope.char_to_line(first),
                    rope.char_to_line(last),
                    first == last,
                )
            })
            .collect();
        let view = self.editor_groups.active_view_mut();
        let mut remove_all = false;
        let mut doomed: BTreeSet<usize> = BTreeSet::new();
        for &(first, _, empty) in &selections {
            if !empty {
                continue;
            }
            let innermost = view
                .manual_folds
                .iter()
                .filter(|&&(start, end, by_hand)| by_hand && start <= first && first <= end)
                .min_by_key(|&&(start, end, _)| end - start);
            match innermost {
                Some(&(start, _, _)) => {
                    doomed.insert(start);
                }
                None => remove_all = true,
            }
        }
        view.manual_folds.retain(|&(start, end, by_hand)| {
            if !by_hand {
                return true;
            }
            let touched = selections
                .iter()
                .any(|&(first, last, empty)| !empty && first <= end && start <= last);
            !(remove_all || doomed.contains(&start) || touched)
        });
        self.rebuild_fold_regions();
    }

    /// Folds or unfolds the block of import lines at the top of the document.
    pub(crate) fn toggle_import_fold(&mut self) {
        if self.fold_unavailable_with_wrap() {
            return;
        }
        self.sync_active_folds();
        let document = self.active_document();
        let Some((first, last)) = folding::import_block(&self.documents[document].buffer) else {
            return;
        };
        let view = self.editor_groups.active_view_mut();
        if !view
            .manual_folds
            .iter()
            .any(|&(s, e, _)| (s, e) == (first, last))
        {
            view.manual_folds.retain(|&(_, _, by_hand)| by_hand);
            view.manual_folds.push((first, last, false));
        }
        match view.folded.binary_search(&first) {
            Ok(index) => {
                view.folded.remove(index);
            }
            Err(index) => view.folded.insert(index, first),
        }
        self.rebuild_fold_regions();
        let cursor = self.cursor_line();
        if cursor > first
            && cursor <= last
            && self.editor_groups.active_view().folded.contains(&first)
        {
            self.set_cursor_to_line(first);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NESTED: &str = "a\n  b\n    c\n    d\n  e\nf\n  g\n";

    fn model(text: &str, lines: &[usize], folded: &[usize]) -> Model {
        let buffer = Buffer::from_text("m.txt", text);
        Model {
            regions: folding::regions(&buffer, 2, None),
            folded: folded.iter().copied().collect(),
            lines: lines.to_vec(),
        }
    }

    fn starts(model: &Model) -> Vec<usize> {
        model.folded.iter().copied().collect()
    }

    #[test]
    fn regions_know_their_parent_and_level() {
        let m = model(NESTED, &[], &[]);
        let summary: Vec<_> = m
            .regions
            .iter()
            .map(|r| (r.start, r.end, r.level, r.parent))
            .collect();
        assert_eq!(
            summary,
            vec![(0, 4, 1, None), (1, 3, 2, Some(0)), (5, 6, 1, None)]
        );
    }

    #[test]
    fn toggling_flips_only_the_region_at_the_cursor() {
        let mut m = model(NESTED, &[2], &[]);
        m.toggle_at_selections(false);
        assert_eq!(starts(&m), vec![1]);
        m.toggle_at_selections(false);
        assert!(starts(&m).is_empty());
    }

    #[test]
    fn toggling_recursively_applies_the_new_state_to_everything_inside() {
        let mut m = model(NESTED, &[0], &[]);
        m.toggle_at_selections(true);
        assert_eq!(starts(&m), vec![0, 1]);
        m.toggle_at_selections(true);
        assert!(starts(&m).is_empty());
    }

    #[test]
    fn setting_recursively_folds_and_unfolds_a_region_and_its_children() {
        let mut m = model(NESTED, &[0], &[1]);
        m.set_recursively(true);
        assert_eq!(starts(&m), vec![0, 1]);
        m.set_recursively(false);
        assert!(starts(&m).is_empty());
    }

    #[test]
    fn fold_level_folds_that_depth_but_not_where_the_cursor_is() {
        let mut m = model(NESTED, &[6], &[]);
        m.fold_level(1);
        assert_eq!(starts(&m), vec![0]);
        // A cursor inside the level-2 region keeps it open.
        let mut m = model(NESTED, &[2], &[]);
        m.fold_level(2);
        assert!(starts(&m).is_empty());
        // A cursor elsewhere lets it fold.
        let mut m = model(NESTED, &[5], &[]);
        m.fold_level(2);
        assert_eq!(starts(&m), vec![1]);
    }

    #[test]
    fn folding_the_rest_spares_the_regions_around_and_inside_the_selection() {
        let mut m = model(NESTED, &[2], &[]);
        m.set_rest(true);
        // The cursor's region (line 1) and its parent (line 0) stay open; the other one folds.
        assert_eq!(starts(&m), vec![5]);
        m.set_rest(false);
        assert!(starts(&m).is_empty());
    }

    #[test]
    fn matching_regions_are_set_by_start_line() {
        let mut m = model(NESTED, &[], &[]);
        m.set_matching(true, &BTreeSet::from([5]));
        assert_eq!(starts(&m), vec![5]);
        m.set_matching(true, &BTreeSet::from([5]));
        assert_eq!(starts(&m), vec![5], "already folded regions are left alone");
        m.set_matching(false, &BTreeSet::from([5]));
        assert!(starts(&m).is_empty());
    }

    #[test]
    fn navigation_moves_between_siblings_parents_and_nearest_folds() {
        let m = model(NESTED, &[], &[]);
        let r = &m.regions;
        assert_eq!(folding::next_fold_line(r, 0), Some(5));
        assert_eq!(folding::previous_fold_line(r, 5), Some(0));
        assert_eq!(folding::next_fold_line(r, 5), None);
        assert_eq!(folding::previous_fold_line(r, 0), None);
        assert_eq!(folding::parent_fold_line(r, 2), Some(1));
        assert_eq!(folding::parent_fold_line(r, 1), Some(0));
        assert_eq!(folding::parent_fold_line(r, 0), None);
        // Off a region start: the nearest region above or below.
        assert_eq!(folding::next_fold_line(r, 4), Some(5));
        assert_eq!(folding::previous_fold_line(r, 4), Some(1));
    }

    #[test]
    fn markers_need_a_comment_leader_and_a_whole_word() {
        use folding::Marker::*;
        assert_eq!(folding::marker("// #region parts"), Some(Start));
        assert_eq!(folding::marker("  #endregion"), Some(End));
        assert_eq!(folding::marker("# region setup"), Some(Start));
        assert_eq!(folding::marker("<!-- #region x -->"), Some(Start));
        assert_eq!(folding::marker("#Region Foo"), Some(Start));
        assert_eq!(folding::marker("region without leader"), None);
        assert_eq!(folding::marker("// regional notes"), None);
    }

    #[test]
    fn marker_regions_become_fold_regions_even_without_indentation() {
        let text = "// #region one\nx\n// #region two\ny\n// #endregion\nz\n// #endregion\nw\n";
        let buffer = Buffer::from_text("m.rs", text);
        let spans: Vec<_> = folding::regions(&buffer, 4, None)
            .iter()
            .map(|r| (r.start, r.end, r.level))
            .collect();
        assert_eq!(spans, vec![(0, 6, 1), (2, 4, 2)]);
        assert_eq!(folding::region_end(&buffer, 0, 4, None), Some(6));
    }

    #[test]
    fn the_import_block_is_the_first_run_of_two_or_more_import_lines() {
        let buffer = Buffer::from_text("m.rs", "use a;\nuse b;\n\nuse c;\nfn main() {}\n");
        assert_eq!(folding::import_block(&buffer), Some((0, 3)));
        let single = Buffer::from_text("m.rs", "use a;\nfn main() {}\n");
        assert_eq!(folding::import_block(&single), None);
    }

    fn app_with(text: &str) -> (crate::test_support::TempDir, App) {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.editor_mut().insert(text);
        app.editor_mut().set_cursor_line_col(0, 0);
        (dir, app)
    }

    fn folded(app: &App) -> Vec<usize> {
        app.editor_groups.active_view().folded.clone()
    }

    #[test]
    fn commands_toggle_fold_and_move_a_hidden_cursor_to_the_fold_start() {
        let (_dir, mut app) = app_with(NESTED);
        app.editor_mut().set_cursor_line_col(2, 0);
        app.run_ui_command("editor.toggleFold");
        assert_eq!(folded(&app), vec![1]);
        assert_eq!(app.cursor_line(), 1);
        app.run_ui_command("editor.toggleFold");
        assert!(folded(&app).is_empty());
    }

    #[test]
    fn fold_level_and_recursive_commands_work_through_the_registry() {
        let (_dir, mut app) = app_with(NESTED);
        app.editor_mut().set_cursor_line_col(5, 0);
        app.run_ui_command("editor.foldLevel2");
        assert_eq!(folded(&app), vec![1]);
        app.run_ui_command("editor.unfoldAll");
        app.editor_mut().set_cursor_line_col(0, 0);
        app.run_ui_command("editor.foldRecursively");
        assert_eq!(folded(&app), vec![0, 1]);
        app.run_ui_command("editor.unfoldRecursively");
        assert!(folded(&app).is_empty());
    }

    #[test]
    fn navigation_commands_move_the_cursor() {
        let (_dir, mut app) = app_with(NESTED);
        app.run_ui_command("editor.gotoNextFold");
        assert_eq!(app.cursor_line(), 5);
        app.run_ui_command("editor.gotoPreviousFold");
        assert_eq!(app.cursor_line(), 0);
        app.editor_mut().set_cursor_line_col(3, 0);
        app.run_ui_command("editor.gotoParentFold");
        assert_eq!(app.cursor_line(), 1);
    }

    #[test]
    fn a_manual_range_is_created_folded_and_can_be_removed() {
        let (_dir, mut app) = app_with("a\nb\nc\nd\n");
        let end = app.editor().text().find('c').unwrap();
        app.editor_mut().set_selection(0, end);
        app.run_ui_command("editor.createFoldingRangeFromSelection");
        assert_eq!(folded(&app), vec![0]);
        assert_eq!(
            app.editor_groups.active_view().manual_folds,
            vec![(0, 1, true)]
        );
        // The cursor is outside the folded lines, so the fold stays through a sync.
        app.sync_active_folds();
        assert_eq!(folded(&app), vec![0]);
        app.editor_mut().set_cursor_line_col(0, 0);
        app.run_ui_command("editor.removeManualFoldingRanges");
        assert!(app.editor_groups.active_view().manual_folds.is_empty());
        assert!(app.editor_groups.active_view().fold_regions.is_none());
    }

    #[test]
    fn a_manual_range_is_dropped_when_the_text_changes() {
        let (_dir, mut app) = app_with("a\nb\nc\nd\n");
        let end = app.editor().text().find('c').unwrap();
        app.editor_mut().set_selection(0, end);
        app.run_ui_command("editor.createFoldingRangeFromSelection");
        app.editor_mut().set_cursor_line_col(3, 0);
        app.editor_mut().insert("x");
        app.sync_active_folds();
        assert!(app.editor_groups.active_view().manual_folds.is_empty());
        assert!(folded(&app).is_empty());
    }

    #[test]
    fn the_import_fold_toggles_the_import_block() {
        let (_dir, mut app) = app_with("use a;\nuse b;\nfn main() {}\n");
        app.editor_mut().set_cursor_line_col(2, 0);
        app.run_ui_command("editor.toggleImportFold");
        assert_eq!(folded(&app), vec![0]);
        app.run_ui_command("editor.toggleImportFold");
        assert!(folded(&app).is_empty());
    }

    #[test]
    fn marker_region_commands_fold_and_unfold_the_marked_regions() {
        let (_dir, mut app) = app_with("// #region r\nx\n// #endregion\ny\n");
        app.run_ui_command("editor.foldAllMarkerRegions");
        assert_eq!(folded(&app), vec![0]);
        app.run_ui_command("editor.unfoldAllMarkerRegions");
        assert!(folded(&app).is_empty());
    }
}
