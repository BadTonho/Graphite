//! State of the merge editor: which file is being merged, how the panes are laid out, what is
//! visible and which conflict is active. The text logic lives in [`crate::merge`].

use crate::file_format::{Encoding, LineEnding};
use crate::merge::{aligned_rows, MergeState, Pane, Row};
use std::path::PathBuf;

/// Encoding and line ending of the file, kept to write the result back the way it was read.
pub(crate) type MergeFormat = (Encoding, LineEnding);

/// Unchanged rows kept around a conflict when plain text is hidden.
const CONTEXT_ROWS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MergeLayout {
    /// Both inputs side by side on top, the result below.
    Mixed,
    /// Input, result and input side by side.
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BasePlacement {
    Hidden,
    /// The base is the last of the input panes.
    Top,
    /// The base sits between the two inputs.
    Center,
}

/// A rectangle in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PaneRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PaneRect {
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// Where each pane goes inside `area`. Every pane of a row has the same width; in the mixed
/// layout the inputs take the top 40% and the result the rest.
pub(crate) fn pane_rects(
    layout: MergeLayout,
    base: BasePlacement,
    area: PaneRect,
) -> Vec<(Pane, PaneRect)> {
    let columns = |panes: &[Pane], y: f32, height: f32| -> Vec<(Pane, PaneRect)> {
        let width = area.width / panes.len().max(1) as f32;
        panes
            .iter()
            .enumerate()
            .map(|(index, pane)| {
                (
                    *pane,
                    PaneRect {
                        x: area.x + width * index as f32,
                        y,
                        width,
                        height,
                    },
                )
            })
            .collect()
    };
    let mut inputs = vec![Pane::Current, Pane::Incoming];
    match base {
        BasePlacement::Hidden => {}
        BasePlacement::Top => inputs.push(Pane::Base),
        BasePlacement::Center => inputs.insert(1, Pane::Base),
    }
    match layout {
        MergeLayout::Mixed => {
            let top = (area.height * 0.4).floor();
            let mut panes = columns(&inputs, area.y, top);
            panes.extend(columns(&[Pane::Result], area.y + top, area.height - top));
            panes
        }
        MergeLayout::Column => {
            let mut order = vec![Pane::Current, Pane::Result, Pane::Incoming];
            match base {
                BasePlacement::Hidden => {}
                BasePlacement::Top => order.push(Pane::Base),
                BasePlacement::Center => order.insert(1, Pane::Base),
            }
            columns(&order, area.y, area.height)
        }
    }
}

pub(crate) struct MergeEditor {
    pub path: PathBuf,
    pub state: MergeState,
    pub layout: MergeLayout,
    pub base: BasePlacement,
    /// Hide plain text that is not next to a conflict.
    pub hide_unchanged: bool,
    /// First visible position in [`MergeEditor::visible`].
    pub scroll: usize,
    /// The conflict that block actions apply to.
    pub active: Option<usize>,
    /// Which input the compare commands use: 0 current, 1 incoming.
    pub focused_input: usize,
    pub format: MergeFormat,
    /// A close request already warned about decisions that were not written.
    pub close_armed: bool,
    ancestor: Option<String>,
    rows: Vec<Row>,
    visible: Vec<usize>,
}

impl MergeEditor {
    pub(crate) fn new(path: PathBuf, text: &str) -> Self {
        let mut editor = Self {
            path,
            state: MergeState::new(text),
            layout: MergeLayout::Mixed,
            base: BasePlacement::Hidden,
            hide_unchanged: false,
            scroll: 0,
            active: None,
            focused_input: 0,
            format: MergeFormat::default(),
            close_armed: false,
            ancestor: None,
            rows: Vec::new(),
            visible: Vec::new(),
        };
        editor.refresh();
        editor.active = editor.state.next_unresolved(None);
        editor.scroll_to_active();
        editor
    }

    /// Sets the common ancestor text (e.g. from Git index stage 1) and fills missing 2-way bases.
    pub(crate) fn set_ancestor(&mut self, ancestor: String) {
        self.state.populate_base_from_ancestor(&ancestor);
        self.ancestor = Some(ancestor);
        self.refresh();
    }

    /// Re-parses the file after it was edited. Decisions made for conflicts whose text did not
    /// change are kept.
    pub(crate) fn reload(&mut self, text: &str) {
        self.state.reload(text);
        if let Some(ancestor) = self.ancestor.as_deref() {
            self.state.populate_base_from_ancestor(ancestor);
        }
        self.refresh();
        if self.active.is_some_and(|index| index >= self.state.len()) {
            self.active = self.state.next_unresolved(None);
        }
        self.scroll = self.scroll.min(self.visible.len().saturating_sub(1));
    }

    /// Recomputes the rows after the state, the plain-text filter or the file changed.
    pub(crate) fn refresh(&mut self) {
        self.rows = aligned_rows(&self.state);
        self.visible = if self.hide_unchanged {
            let mut keep = vec![false; self.rows.len()];
            for (index, row) in self.rows.iter().enumerate() {
                if row.block.is_some() {
                    let from = index.saturating_sub(CONTEXT_ROWS);
                    let to = (index + CONTEXT_ROWS + 1).min(self.rows.len());
                    keep[from..to].iter_mut().for_each(|slot| *slot = true);
                }
            }
            (0..self.rows.len()).filter(|&index| keep[index]).collect()
        } else {
            (0..self.rows.len()).collect()
        };
    }

    pub(crate) fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Indexes into [`MergeEditor::rows`] of the rows that are shown, in order.
    pub(crate) fn visible(&self) -> &[usize] {
        &self.visible
    }

    /// Largest scroll position that still fills `window` rows.
    pub(crate) fn max_scroll(&self, window: usize) -> usize {
        self.visible
            .len()
            .saturating_sub(window.min(self.visible.len()))
    }

    pub(crate) fn scroll_by(&mut self, delta: isize, window: usize) {
        let target = self.scroll as isize + delta;
        self.scroll = target.clamp(0, self.max_scroll(window) as isize) as usize;
    }

    /// Scrolls so the first row of the active conflict is near the top.
    pub(crate) fn scroll_to_active(&mut self) {
        let Some(active) = self.active else {
            return;
        };
        let position = self.visible.iter().position(|&index| {
            let row = &self.rows[index];
            row.block == Some(active) && row.first_of_block
        });
        if let Some(position) = position {
            self.scroll = position.saturating_sub(CONTEXT_ROWS);
        }
    }

    pub(crate) fn select(&mut self, block: usize) -> bool {
        if block >= self.state.len() {
            return false;
        }
        self.active = Some(block);
        self.scroll_to_active();
        true
    }

    /// Moves to the next unresolved conflict. Returns whether there was one.
    pub(crate) fn goto_next_unresolved(&mut self) -> bool {
        match self.state.next_unresolved(self.active) {
            Some(block) => self.select(block),
            None => false,
        }
    }

    pub(crate) fn goto_previous_unresolved(&mut self) -> bool {
        match self.state.previous_unresolved(self.active) {
            Some(block) => self.select(block),
            None => false,
        }
    }

    /// The conflict shown at visible position `position`.
    pub(crate) fn block_at_visible(&self, position: usize) -> Option<usize> {
        self.visible
            .get(position)
            .and_then(|&index| self.rows[index].block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THREE: &str = "a\n<<<<<<< x\n1\n=======\n2\n>>>>>>> y\nb\nc\nd\ne\nf\ng\n<<<<<<< x\n3\n=======\n4\n>>>>>>> y\nz\n";

    fn area() -> PaneRect {
        PaneRect {
            x: 0.0,
            y: 20.0,
            width: 900.0,
            height: 500.0,
        }
    }

    #[test]
    fn mixed_layout_puts_the_inputs_on_top_and_the_result_below() {
        let panes = pane_rects(MergeLayout::Mixed, BasePlacement::Hidden, area());
        assert_eq!(panes.len(), 3);
        assert_eq!(panes[0].0, Pane::Current);
        assert_eq!(panes[1].0, Pane::Incoming);
        assert_eq!(panes[2].0, Pane::Result);
        assert_eq!(panes[0].1.width, 450.0);
        assert_eq!(panes[1].1.x, 450.0);
        assert_eq!(panes[0].1.height, 200.0);
        assert_eq!(panes[2].1.y, 220.0);
        assert_eq!(panes[2].1.width, 900.0);
        assert_eq!(panes[2].1.height, 300.0);
    }

    #[test]
    fn column_layout_puts_the_result_between_the_inputs() {
        let panes = pane_rects(MergeLayout::Column, BasePlacement::Hidden, area());
        let order: Vec<_> = panes.iter().map(|(pane, _)| *pane).collect();
        assert_eq!(order, [Pane::Current, Pane::Result, Pane::Incoming]);
        assert!(panes.iter().all(|(_, rect)| rect.height == 500.0));
    }

    #[test]
    fn the_base_pane_is_added_on_top_or_in_the_center() {
        let order = |layout, base| -> Vec<Pane> {
            pane_rects(layout, base, area())
                .iter()
                .map(|(pane, _)| *pane)
                .collect()
        };
        assert_eq!(
            order(MergeLayout::Mixed, BasePlacement::Top),
            [Pane::Current, Pane::Incoming, Pane::Base, Pane::Result]
        );
        assert_eq!(
            order(MergeLayout::Mixed, BasePlacement::Center),
            [Pane::Current, Pane::Base, Pane::Incoming, Pane::Result]
        );
        assert_eq!(
            order(MergeLayout::Column, BasePlacement::Center),
            [Pane::Current, Pane::Base, Pane::Result, Pane::Incoming]
        );
        let widths: Vec<f32> = pane_rects(MergeLayout::Mixed, BasePlacement::Top, area())
            .iter()
            .take(3)
            .map(|(_, rect)| rect.width)
            .collect();
        assert_eq!(widths, [300.0, 300.0, 300.0]);
    }

    #[test]
    fn opening_selects_and_shows_the_first_conflict() {
        let editor = MergeEditor::new("f".into(), THREE);
        assert_eq!(editor.active, Some(0));
        assert_eq!(editor.state.len(), 2);
        assert_eq!(editor.scroll, 0);
    }

    #[test]
    fn navigation_scrolls_to_the_conflict_and_wraps() {
        let mut editor = MergeEditor::new("f".into(), THREE);
        assert!(editor.goto_next_unresolved());
        assert_eq!(editor.active, Some(1));
        // Conflict 1 is row 8 (a, one conflict row, b to g, then the conflict).
        assert_eq!(editor.scroll, 6);
        assert!(editor.goto_next_unresolved());
        assert_eq!(editor.active, Some(0));
        assert!(editor.goto_previous_unresolved());
        assert_eq!(editor.active, Some(1));
        editor.state.accept_current(0);
        editor.state.accept_current(1);
        editor.refresh();
        assert!(!editor.goto_next_unresolved());
        assert!(editor.select(0));
        assert!(!editor.select(2));
    }

    #[test]
    fn hiding_plain_text_keeps_two_rows_around_each_conflict() {
        let mut editor = MergeEditor::new("f".into(), THREE);
        assert_eq!(editor.visible().len(), editor.rows().len());
        editor.hide_unchanged = true;
        editor.refresh();
        // The second conflict's context starts two rows before it; plain rows far from both
        // conflicts ("c", "d", "e", "f") are hidden.
        let shown: Vec<_> = editor
            .visible()
            .iter()
            .map(|&index| {
                editor.rows()[index]
                    .cell(Pane::Current)
                    .map(|cell| cell.text.clone())
                    .unwrap_or_default()
            })
            .collect();
        assert!(!shown.contains(&"d".to_string()));
        assert!(shown.contains(&"a".to_string()));
        assert!(shown.contains(&"z".to_string()));
        assert!(editor.block_at_visible(1).is_some());
        assert!(editor.block_at_visible(0).is_none());
    }

    #[test]
    fn scrolling_is_clamped_to_the_rows() {
        let mut editor = MergeEditor::new("f".into(), THREE);
        let window = 5;
        editor.scroll_by(-10, window);
        assert_eq!(editor.scroll, 0);
        editor.scroll_by(1000, window);
        assert_eq!(editor.scroll, editor.visible().len() - window);
        editor.scroll_by(1000, 1000);
        assert_eq!(editor.scroll, 0);
    }

    #[test]
    fn reloading_keeps_decisions_for_unchanged_conflicts_only() {
        let mut editor = MergeEditor::new("f".into(), THREE);
        editor.state.accept_incoming(0);
        editor.state.accept_current(1);
        let edited = THREE.replace("\n3\n", "\n3 edited\n");
        editor.reload(&edited);
        assert_eq!(editor.state.len(), 2);
        assert!(editor.state.resolution(0).unwrap().is_handled());
        assert!(!editor.state.resolution(1).unwrap().is_handled());
        editor.reload("no conflicts any more\n");
        assert_eq!(editor.state.len(), 0);
        assert_eq!(editor.active, None);
    }
}
