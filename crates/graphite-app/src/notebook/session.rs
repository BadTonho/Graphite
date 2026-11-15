//! The state of an open notebook: its cells with one text buffer each, which cell has the focus
//! and which are selected, and one undo history that spans text edits and cell operations.

use super::model::{Cell, CellKind, Notebook};
use graphite_text::{Buffer, Editor, SelectionState, ViewId};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Keys act on cells.
    Command,
    /// Keys edit the text of the focused cell.
    Edit,
}

/// A cell together with its editing state.
pub(crate) struct CellState {
    /// Identifies the cell inside the session; unlike an index it survives moves.
    pub uid: u64,
    /// The cell as read or last exported; its `source` is only current after
    /// [`Session::export`], the buffer is the truth.
    pub cell: Cell,
    pub buffer: Buffer,
    pub selection: SelectionState,
    pub input_collapsed: bool,
    pub output_collapsed: bool,
    /// Counts changes to the outputs, so the drawn form is rebuilt only when they change.
    outputs_rev: u64,
    rendered: std::cell::RefCell<Option<((u64, usize), std::sync::Arc<Vec<super::rich::Block>>)>>,
}

impl CellState {
    pub(super) fn new(uid: u64, cell: Cell) -> Self {
        let buffer = Buffer::from_text(PathBuf::from("cell"), &cell.source);
        Self {
            uid,
            cell,
            buffer,
            selection: SelectionState::default(),
            input_collapsed: false,
            output_collapsed: false,
            outputs_rev: 0,
            rendered: Default::default(),
        }
    }

    pub(crate) fn text(&self) -> String {
        self.buffer.text()
    }

    /// Records that the outputs changed.
    pub(super) fn touch_outputs(&mut self) {
        self.outputs_rev += 1;
    }

    /// The outputs as they are drawn: text lines and decoded images. Built on first use and
    /// again after the outputs change.
    pub(crate) fn blocks(&self) -> std::sync::Arc<Vec<super::rich::Block>> {
        let key = (self.outputs_rev, self.cell.outputs.len());
        if let Some((cached, blocks)) = self.rendered.borrow().as_ref() {
            if *cached == key {
                return blocks.clone();
            }
        }
        let blocks = std::sync::Arc::new(super::rich::blocks(
            &self.cell.outputs,
            &super::rich::Limits::default(),
        ));
        *self.rendered.borrow_mut() = Some((key, blocks.clone()));
        blocks
    }

    /// The cell with its current text.
    fn snapshot(&self) -> Cell {
        let mut cell = self.cell.clone();
        cell.source = self.text();
        cell
    }
}

/// One step of the undo history. Applying an entry flips the state it describes, so the same
/// value serves for undo and redo.
enum Entry {
    /// A text edit in the cell with this uid.
    Text(u64),
    /// A cell at `index`: `None` while the cell is present (undo removes it), the removed cell
    /// while it is absent (undo puts it back).
    Insert {
        index: usize,
        cell: Option<CellState>,
    },
    Move {
        from: usize,
        to: usize,
    },
    /// The kind of a cell with what only code cells carry.
    Kind {
        uid: u64,
        kind: CellKind,
        outputs: Vec<Value>,
        execution_count: Option<u64>,
    },
    /// Steps that were one action, in the order they happened.
    Group(Vec<Entry>),
}

pub(crate) struct Session {
    base: Notebook,
    pub cells: Vec<CellState>,
    pub focus: usize,
    /// The other end of the selected range of cells; the range runs from here to `focus`.
    anchor: usize,
    pub mode: Mode,
    pub line_numbers: bool,
    /// Pixels scrolled from the top of the notebook.
    pub scroll: f32,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    clipboard: Vec<Cell>,
    next_uid: u64,
    /// `d` was pressed once; a second press deletes the selected cells.
    pub delete_pending: bool,
    /// Revision of the document buffer this session was last in step with.
    pub synced_revision: u64,
    /// The kernel this notebook runs on, with the state of its runs.
    pub(super) kernel: super::execution::KernelSlot,
    /// Set for the interactive window and the REPL.
    pub(super) interactive: Option<super::interactive::Interactive>,
    /// Headings whose sections are folded, by cell uid.
    pub(super) folded: std::collections::HashSet<u64>,
    /// Keep the heading of the section being read pinned at the top.
    pub sticky_scroll: bool,
    /// What the last search in the notebook looked for.
    pub find_query: Option<String>,
}

impl Session {
    pub(crate) fn from_notebook(notebook: Notebook) -> Self {
        let mut session = Self {
            base: notebook,
            cells: Vec::new(),
            focus: 0,
            anchor: 0,
            mode: Mode::Command,
            line_numbers: false,
            scroll: 0.0,
            undo: Vec::new(),
            redo: Vec::new(),
            clipboard: Vec::new(),
            next_uid: 1,
            delete_pending: false,
            synced_revision: 0,
            kernel: Default::default(),
            interactive: None,
            folded: Default::default(),
            sticky_scroll: false,
            find_query: None,
        };
        let cells = std::mem::take(&mut session.base.cells);
        for cell in cells {
            let uid = session.take_uid();
            session.cells.push(CellState::new(uid, cell));
        }
        if session.cells.is_empty() {
            let uid = session.take_uid();
            session
                .cells
                .push(CellState::new(uid, Cell::new(CellKind::Code, "")));
        }
        session
    }

    fn take_uid(&mut self) -> u64 {
        let uid = self.next_uid;
        self.next_uid += 1;
        uid
    }

    /// Replaces the cells with those of `notebook` (the file changed on disk), keeping the focus
    /// as close to where it was as the new cell count allows.
    pub(crate) fn reload(&mut self, notebook: Notebook) {
        let focus = self.focus;
        let line_numbers = self.line_numbers;
        let sticky_scroll = self.sticky_scroll;
        let find_query = self.find_query.take();
        let scroll = self.scroll;
        // The kernel stays; the cells it was running are gone, so its requests are dropped.
        let mut kernel = std::mem::take(&mut self.kernel);
        kernel.forget_requests();
        let interactive = self.interactive.take();
        *self = Session::from_notebook(notebook);
        self.kernel = kernel;
        // What came from the file is real history, not an unused first cell.
        self.interactive = interactive.map(|mut state| {
            state.placeholder = false;
            state
        });
        self.focus = focus.min(self.cells.len() - 1);
        self.anchor = self.focus;
        self.line_numbers = line_numbers;
        self.sticky_scroll = sticky_scroll;
        self.find_query = find_query;
        self.scroll = scroll;
    }

    pub(crate) fn metadata(&self) -> &Notebook {
        &self.base
    }

    /// Records the kernel the notebook runs on in its metadata, as Jupyter does. Returns whether
    /// the metadata changed.
    pub(crate) fn set_kernelspec(
        &mut self,
        name: &str,
        display_name: &str,
        language: &str,
    ) -> bool {
        let wanted = serde_json::json!({
            "display_name": display_name,
            "language": language,
            "name": name,
        });
        if self.base.metadata.get("kernelspec") == Some(&wanted) {
            return false;
        }
        self.base.metadata.insert("kernelspec".into(), wanted);
        true
    }

    /// The notebook with the current text of every cell.
    pub(crate) fn export(&self) -> Notebook {
        let mut notebook = self.base.clone();
        notebook.cells = self.cells.iter().map(CellState::snapshot).collect();
        notebook
    }

    pub(crate) fn to_json(&self) -> String {
        self.export().to_json()
    }

    pub(crate) fn focused(&self) -> &CellState {
        &self.cells[self.focus]
    }

    /// First and last selected cell.
    pub(crate) fn selected(&self) -> (usize, usize) {
        let last = self.cells.len() - 1;
        let (a, b) = (self.anchor.min(last), self.focus.min(last));
        (a.min(b), a.max(b))
    }

    pub(crate) fn is_selected(&self, index: usize) -> bool {
        let (low, high) = self.selected();
        (low..=high).contains(&index)
    }

    fn index_of(&self, uid: u64) -> Option<usize> {
        self.cells.iter().position(|cell| cell.uid == uid)
    }

    fn record(&mut self, entry: Entry) {
        self.undo.push(entry);
        self.redo.clear();
    }

    // Focus and selection.

    pub(crate) fn set_focus(&mut self, index: usize, extend: bool) {
        self.focus = index.min(self.cells.len() - 1);
        if !extend {
            self.anchor = self.focus;
        }
        self.delete_pending = false;
    }

    pub(crate) fn focus_by(&mut self, delta: isize, extend: bool) {
        self.focus_visible_by(delta, extend);
    }

    pub(crate) fn select_all_cells(&mut self) {
        self.anchor = 0;
        self.focus = self.cells.len() - 1;
    }

    pub(crate) fn enter_edit(&mut self) {
        self.mode = Mode::Edit;
        self.anchor = self.focus;
    }

    pub(crate) fn exit_edit(&mut self) {
        self.mode = Mode::Command;
    }

    // Text editing.

    /// Runs an edit on the focused cell and records every text change it made in the history.
    pub(crate) fn edit<R>(&mut self, action: impl FnOnce(&mut Editor<'_>) -> R) -> R {
        let uid = self.cells[self.focus].uid;
        let state = &mut self.cells[self.focus];
        let before = state.buffer.revision();
        let result = {
            let mut editor = state.buffer.edit(&mut state.selection, ViewId(uid));
            action(&mut editor)
        };
        let changes = state.buffer.revision().saturating_sub(before);
        for _ in 0..changes {
            self.record(Entry::Text(uid));
        }
        self.delete_pending = false;
        result
    }

    /// Moves the caret one line up or down. Past the first or last line it continues in the
    /// neighbouring cell. Returns whether the caret changed cell.
    pub(crate) fn move_vertical(&mut self, delta: isize, extend: bool) -> bool {
        let (line, column) = self.cells[self.focus]
            .buffer
            .view(&self.cells[self.focus].selection)
            .cursor_line_col();
        let last = self.cells[self.focus].buffer.len_lines().saturating_sub(1);
        let crossing = (delta < 0 && line == 0 && self.focus > 0)
            || (delta > 0 && line >= last && self.focus + 1 < self.cells.len());
        if !crossing {
            self.edit(|editor| editor.move_vertical(delta));
            return false;
        }
        let target = (self.focus as isize + delta.signum()) as usize;
        self.set_focus(target, extend);
        let line = if delta < 0 {
            self.cells[target].buffer.len_lines().saturating_sub(1)
        } else {
            0
        };
        self.edit(|editor| editor.set_cursor_line_col(line, column));
        true
    }

    // Cell operations.

    /// Puts a new cell at `index` and gives it the focus.
    pub(crate) fn insert_cell(&mut self, index: usize, kind: CellKind, source: &str) -> usize {
        let index = index.min(self.cells.len());
        let uid = self.take_uid();
        self.cells
            .insert(index, CellState::new(uid, Cell::new(kind, source)));
        self.record(Entry::Insert { index, cell: None });
        self.set_focus(index, false);
        index
    }

    pub(crate) fn insert_above(&mut self, kind: CellKind) -> usize {
        let (low, _) = self.selected();
        self.insert_cell(low, kind, "")
    }

    pub(crate) fn insert_below(&mut self, kind: CellKind) -> usize {
        let (_, high) = self.selected();
        self.insert_cell(high + 1, kind, "")
    }

    /// Removes the selected cells; returns them with their current text. A notebook never ends
    /// up with no cell: removing all of them leaves one empty code cell.
    pub(crate) fn delete_selected(&mut self) -> Vec<Cell> {
        let (low, high) = self.selected();
        let mut group = Vec::new();
        let mut removed = Vec::new();
        for index in (low..=high).rev() {
            let state = self.cells.remove(index);
            removed.push(state.snapshot());
            group.push(Entry::Insert {
                index,
                cell: Some(state),
            });
        }
        removed.reverse();
        if self.cells.is_empty() {
            let uid = self.take_uid();
            self.cells
                .push(CellState::new(uid, Cell::new(CellKind::Code, "")));
            group.push(Entry::Insert {
                index: 0,
                cell: None,
            });
        }
        self.record(Entry::Group(group));
        self.set_focus(low.min(self.cells.len() - 1), false);
        removed
    }

    /// Moves the selected cells one place up or down. Returns whether they moved.
    pub(crate) fn move_selected(&mut self, down: bool) -> bool {
        let (low, high) = self.selected();
        let (from, to) = if down {
            if high + 1 >= self.cells.len() {
                return false;
            }
            (high + 1, low)
        } else {
            if low == 0 {
                return false;
            }
            (low - 1, high)
        };
        let state = self.cells.remove(from);
        self.cells.insert(to, state);
        self.record(Entry::Move { from, to });
        let shift = if down { 1 } else { -1 };
        self.focus = (self.focus as isize + shift) as usize;
        self.anchor = (self.anchor as isize + shift) as usize;
        true
    }

    /// Splits the focused cell at the caret: the text after it moves to a new cell below.
    pub(crate) fn split_at_cursor(&mut self) {
        let index = self.focus;
        let (cursor, length) = {
            let state = &self.cells[index];
            (
                state.buffer.view(&state.selection).cursor().char_idx,
                state.buffer.len_chars(),
            )
        };
        let text = self.cells[index].text();
        let tail: String = text.chars().skip(cursor).collect();
        let kind = self.cells[index].cell.kind;
        let uid = self.cells[index].uid;
        let revision = self.cells[index].buffer.revision();
        if cursor < length {
            self.edit(|editor| {
                editor.set_selection(cursor, length);
                editor.backspace();
            });
        }
        // `edit` recorded the text change on its own; fold it into one action with the new cell.
        let mut group = Vec::new();
        if self.cells[index].buffer.revision() != revision {
            while matches!(self.undo.last(), Some(Entry::Text(last)) if *last == uid) {
                group.insert(0, self.undo.pop().expect("checked above"));
            }
        }
        let new_uid = self.take_uid();
        self.cells
            .insert(index + 1, CellState::new(new_uid, Cell::new(kind, &tail)));
        group.push(Entry::Insert {
            index: index + 1,
            cell: None,
        });
        self.record(Entry::Group(group));
        self.set_focus(index + 1, false);
    }

    /// Joins the selected cells into the first one. With a single cell selected it joins that
    /// cell with the one below (or above). Returns whether anything was joined.
    pub(crate) fn merge_selected(&mut self, with_above: bool) -> bool {
        let (mut low, mut high) = self.selected();
        if low == high {
            if with_above {
                if low == 0 {
                    return false;
                }
                low -= 1;
            } else {
                if high + 1 >= self.cells.len() {
                    return false;
                }
                high += 1;
            }
        }
        let target_uid = self.cells[low].uid;
        let mut group = Vec::new();
        for _ in low + 1..=high {
            let removed = self.cells.remove(low + 1);
            let addition = format!("\n{}", removed.text());
            let target = &mut self.cells[low];
            let end = target.buffer.len_chars();
            let revision = target.buffer.revision();
            {
                let mut editor = target
                    .buffer
                    .edit(&mut target.selection, ViewId(target_uid));
                editor.set_selection(end, end);
                editor.insert(&addition);
            }
            for _ in 0..target.buffer.revision() - revision {
                group.push(Entry::Text(target_uid));
            }
            group.push(Entry::Insert {
                index: low + 1,
                cell: Some(removed),
            });
        }
        self.record(Entry::Group(group));
        self.set_focus(low, false);
        true
    }

    /// Changes the kind of every selected cell.
    pub(crate) fn set_kind(&mut self, kind: CellKind) {
        let (low, high) = self.selected();
        let mut group = Vec::new();
        for state in &mut self.cells[low..=high] {
            if state.cell.kind == kind {
                continue;
            }
            group.push(Entry::Kind {
                uid: state.uid,
                kind: state.cell.kind,
                outputs: state.cell.outputs.clone(),
                execution_count: state.cell.execution_count,
            });
            state.cell.set_kind(kind);
            state.touch_outputs();
        }
        if !group.is_empty() {
            self.record(Entry::Group(group));
        }
    }

    // Cell clipboard.

    pub(crate) fn copy_selected(&mut self) {
        let (low, high) = self.selected();
        self.clipboard = self.cells[low..=high]
            .iter()
            .map(CellState::snapshot)
            .collect();
    }

    pub(crate) fn cut_selected(&mut self) {
        self.clipboard = self.delete_selected();
    }

    /// Pastes the copied cells below the selection, with new ids. Returns how many.
    pub(crate) fn paste_below(&mut self) -> usize {
        let copied = self.clipboard.clone();
        let (_, high) = self.selected();
        let mut group = Vec::new();
        for (offset, mut cell) in copied.into_iter().enumerate() {
            cell.id = Some(super::model::new_cell_id());
            let index = high + 1 + offset;
            let uid = self.take_uid();
            self.cells.insert(index, CellState::new(uid, cell));
            group.push(Entry::Insert { index, cell: None });
        }
        let count = group.len();
        if count > 0 {
            self.record(Entry::Group(group));
            self.set_focus(high + count, false);
            self.anchor = high + 1;
        }
        count
    }

    // Collapsing.

    pub(crate) fn set_input_collapsed(&mut self, collapse: bool) {
        let (low, high) = self.selected();
        for state in &mut self.cells[low..=high] {
            state.input_collapsed = collapse;
        }
    }

    pub(crate) fn set_output_collapsed(&mut self, collapse: bool) {
        let (low, high) = self.selected();
        for state in &mut self.cells[low..=high] {
            state.output_collapsed = collapse;
        }
    }

    pub(crate) fn toggle_input_collapsed(&mut self) {
        let collapse = !self.cells[self.focus].input_collapsed;
        self.set_input_collapsed(collapse);
    }

    pub(crate) fn toggle_output_collapsed(&mut self) {
        let collapse = !self.cells[self.focus].output_collapsed;
        self.set_output_collapsed(collapse);
    }

    pub(crate) fn format_cell_at(&mut self, index: usize, tab_size: usize) -> bool {
        let Some(state) = self.cells.get_mut(index) else {
            return false;
        };
        let current = state.text();
        let formatted = format_cell_text(&current, state.cell.kind, tab_size);
        if formatted == current {
            return false;
        }
        let uid = state.uid;
        let before = state.buffer.revision();
        {
            let mut editor = state.buffer.edit(&mut state.selection, ViewId(uid));
            editor.set_selection(0, current.len());
            editor.insert(&formatted);
        }
        let changes = state.buffer.revision().saturating_sub(before);
        for _ in 0..changes {
            self.record(Entry::Text(uid));
        }
        self.delete_pending = false;
        true
    }

    pub(crate) fn format_all_cells(&mut self, tab_size: usize) -> usize {
        let mut changed = 0;
        for index in 0..self.cells.len() {
            if self.format_cell_at(index, tab_size) {
                changed += 1;
            }
        }
        changed
    }

    // History.

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some(entry) = self.undo.pop() else {
            return false;
        };
        let flipped = self.flip(entry, true);
        self.redo.push(flipped);
        self.after_history();
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(entry) = self.redo.pop() else {
            return false;
        };
        let flipped = self.flip(entry, false);
        self.undo.push(flipped);
        self.after_history();
        true
    }

    fn after_history(&mut self) {
        self.focus = self.focus.min(self.cells.len().saturating_sub(1));
        self.anchor = self.focus;
        self.delete_pending = false;
    }

    /// Applies an entry in one direction and returns the entry that applies it back.
    fn flip(&mut self, entry: Entry, undoing: bool) -> Entry {
        match entry {
            Entry::Text(uid) => {
                if let Some(index) = self.index_of(uid) {
                    let state = &mut self.cells[index];
                    if undoing {
                        state.buffer.undo_transaction();
                    } else {
                        state.buffer.redo_transaction();
                    }
                    self.focus = index;
                }
                Entry::Text(uid)
            }
            Entry::Insert { index, cell: None } => {
                let index = index.min(self.cells.len().saturating_sub(1));
                let removed = self.cells.remove(index);
                self.focus = index;
                Entry::Insert {
                    index,
                    cell: Some(removed),
                }
            }
            Entry::Insert {
                index,
                cell: Some(state),
            } => {
                let index = index.min(self.cells.len());
                self.cells.insert(index, state);
                self.focus = index;
                Entry::Insert { index, cell: None }
            }
            Entry::Move { from, to } => {
                let state = self.cells.remove(to);
                self.cells.insert(from, state);
                self.focus = from;
                Entry::Move { from: to, to: from }
            }
            Entry::Kind {
                uid,
                kind,
                outputs,
                execution_count,
            } => match self.index_of(uid) {
                Some(index) => {
                    let cell = &mut self.cells[index].cell;
                    let current = Entry::Kind {
                        uid,
                        kind: cell.kind,
                        outputs: std::mem::replace(&mut cell.outputs, outputs),
                        execution_count: std::mem::replace(
                            &mut cell.execution_count,
                            execution_count,
                        ),
                    };
                    cell.kind = kind;
                    self.cells[index].touch_outputs();
                    self.focus = index;
                    current
                }
                None => Entry::Kind {
                    uid,
                    kind,
                    outputs,
                    execution_count,
                },
            },
            Entry::Group(members) => {
                let mut flipped = Vec::with_capacity(members.len());
                if undoing {
                    for member in members.into_iter().rev() {
                        flipped.push(self.flip(member, true));
                    }
                    flipped.reverse();
                } else {
                    for member in members {
                        flipped.push(self.flip(member, false));
                    }
                }
                Entry::Group(flipped)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(sources: &[&str]) -> Session {
        let mut notebook = Notebook::new();
        notebook.cells = sources
            .iter()
            .map(|source| Cell::new(CellKind::Code, source))
            .collect();
        Session::from_notebook(notebook)
    }

    fn texts(session: &Session) -> Vec<String> {
        session.cells.iter().map(CellState::text).collect()
    }

    fn type_text(session: &mut Session, text: &str) {
        session.enter_edit();
        session.edit(|editor| editor.insert(text));
    }

    #[test]
    fn opening_gives_every_cell_a_buffer_and_never_zero_cells() {
        let session = make(&["a", "b"]);
        assert_eq!(texts(&session), ["a", "b"]);
        assert_eq!(session.mode, Mode::Command);
        let mut empty = Notebook::new();
        empty.cells.clear();
        assert_eq!(Session::from_notebook(empty).cells.len(), 1);
    }

    #[test]
    fn export_carries_the_edited_text() {
        let mut session = make(&["a"]);
        type_text(&mut session, "bc");
        let notebook = Notebook::parse(&session.to_json()).unwrap();
        assert_eq!(notebook.cells[0].source, "bca");
    }

    #[test]
    fn focus_and_selection_ranges() {
        let mut session = make(&["a", "b", "c", "d"]);
        session.focus_by(1, false);
        assert_eq!(session.selected(), (1, 1));
        session.focus_by(2, true);
        assert_eq!(session.selected(), (1, 3));
        assert!(session.is_selected(2) && !session.is_selected(0));
        session.focus_by(-10, true);
        assert_eq!(session.selected(), (0, 1));
        session.focus_by(10, false);
        assert_eq!(session.focus, 3);
        session.select_all_cells();
        assert_eq!(session.selected(), (0, 3));
    }

    #[test]
    fn insert_above_and_below_focus_the_new_cell() {
        let mut session = make(&["a", "b"]);
        assert_eq!(session.insert_below(CellKind::Markdown), 1);
        assert_eq!(session.cells[1].cell.kind, CellKind::Markdown);
        assert_eq!(session.focus, 1);
        assert_eq!(session.insert_above(CellKind::Code), 1);
        assert_eq!(session.cells.len(), 4);
        assert_eq!(texts(&session), ["a", "", "", "b"]);
        let ids: std::collections::HashSet<_> = session
            .cells
            .iter()
            .map(|state| state.cell.id.clone())
            .collect();
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn delete_removes_the_selection_and_keeps_one_cell() {
        let mut session = make(&["a", "b", "c"]);
        session.focus_by(1, false);
        session.focus_by(1, true);
        let removed = session.delete_selected();
        assert_eq!(
            removed
                .iter()
                .map(|cell| cell.source.as_str())
                .collect::<Vec<_>>(),
            ["b", "c"]
        );
        assert_eq!(texts(&session), ["a"]);
        assert_eq!(session.focus, 0);
        session.delete_selected();
        assert_eq!(texts(&session), [""]);
        assert_eq!(session.cells.len(), 1);
    }

    #[test]
    fn undo_and_redo_cover_cell_operations_in_order() {
        let mut session = make(&["a", "b", "c"]);
        session.focus_by(1, false);
        session.delete_selected();
        session.insert_below(CellKind::Raw);
        session.move_selected(false);
        let after = texts(&session);
        let kinds: Vec<_> = session.cells.iter().map(|state| state.cell.kind).collect();
        assert!(session.undo());
        assert!(session.undo());
        assert_eq!(texts(&session), ["a", "c"]);
        assert!(session.undo());
        assert_eq!(texts(&session), ["a", "b", "c"]);
        assert!(!session.undo());
        assert!(session.redo() && session.redo() && session.redo());
        assert_eq!(texts(&session), after);
        assert_eq!(
            session
                .cells
                .iter()
                .map(|state| state.cell.kind)
                .collect::<Vec<_>>(),
            kinds
        );
        assert!(!session.redo());
    }

    #[test]
    fn a_new_action_clears_the_redo_history() {
        let mut session = make(&["a"]);
        session.insert_below(CellKind::Code);
        session.undo();
        assert!(session.can_redo());
        session.insert_below(CellKind::Markdown);
        assert!(!session.can_redo());
    }

    #[test]
    fn text_edits_and_cell_operations_share_one_history() {
        let mut session = make(&["a"]);
        type_text(&mut session, "x");
        session.exit_edit();
        session.insert_below(CellKind::Code);
        type_text(&mut session, "y");
        assert_eq!(texts(&session), ["xa", "y"]);
        session.undo();
        assert_eq!(texts(&session), ["xa", ""]);
        session.undo();
        assert_eq!(texts(&session), ["xa"]);
        session.undo();
        assert_eq!(texts(&session), ["a"]);
        session.redo();
        session.redo();
        session.redo();
        assert_eq!(texts(&session), ["xa", "y"]);
    }

    #[test]
    fn moving_cells_moves_the_selection_with_them() {
        let mut session = make(&["a", "b", "c", "d"]);
        session.focus_by(1, false);
        session.focus_by(1, true);
        assert!(session.move_selected(true));
        assert_eq!(texts(&session), ["a", "d", "b", "c"]);
        assert_eq!(session.selected(), (2, 3));
        assert!(!session.move_selected(true));
        assert!(session.move_selected(false));
        assert!(session.move_selected(false));
        assert_eq!(texts(&session), ["b", "c", "a", "d"]);
        assert_eq!(session.selected(), (0, 1));
        assert!(!session.move_selected(false));
        session.undo();
        assert_eq!(texts(&session), ["a", "b", "c", "d"]);
    }

    #[test]
    fn split_moves_the_text_after_the_caret_into_a_new_cell() {
        let mut session = make(&["hello world"]);
        session.enter_edit();
        session.edit(|editor| editor.set_cursor_line_col(0, 5));
        session.split_at_cursor();
        assert_eq!(texts(&session), ["hello", " world"]);
        assert_eq!(session.focus, 1);
        session.undo();
        assert_eq!(texts(&session), ["hello world"]);
        session.redo();
        assert_eq!(texts(&session), ["hello", " world"]);
        // Splitting at the end leaves an empty cell below.
        session.set_focus(0, false);
        session.edit(|editor| editor.set_cursor_line_col(0, 5));
        session.split_at_cursor();
        assert_eq!(texts(&session), ["hello", "", " world"]);
    }

    #[test]
    fn merge_joins_with_the_next_the_previous_or_the_whole_selection() {
        let mut session = make(&["a", "b", "c"]);
        assert!(session.merge_selected(false));
        assert_eq!(texts(&session), ["a\nb", "c"]);
        session.set_focus(1, false);
        assert!(session.merge_selected(true));
        assert_eq!(texts(&session), ["a\nb\nc"]);
        assert!(!session.merge_selected(false));
        assert!(!session.merge_selected(true));
        session.undo();
        session.undo();
        assert_eq!(texts(&session), ["a", "b", "c"]);
        session.select_all_cells();
        assert!(session.merge_selected(false));
        assert_eq!(texts(&session), ["a\nb\nc"]);
        session.undo();
        assert_eq!(texts(&session), ["a", "b", "c"]);
        session.redo();
        assert_eq!(texts(&session), ["a\nb\nc"]);
    }

    #[test]
    fn changing_the_kind_drops_outputs_and_undo_restores_them() {
        let mut session = make(&["a"]);
        session.cells[0].cell.outputs = vec![serde_json::json!({"output_type": "stream"})];
        session.cells[0].cell.execution_count = Some(4);
        session.set_kind(CellKind::Markdown);
        assert!(session.cells[0].cell.outputs.is_empty());
        assert_eq!(session.cells[0].cell.execution_count, None);
        session.undo();
        assert_eq!(session.cells[0].cell.kind, CellKind::Code);
        assert_eq!(session.cells[0].cell.outputs.len(), 1);
        assert_eq!(session.cells[0].cell.execution_count, Some(4));
        session.redo();
        assert_eq!(session.cells[0].cell.kind, CellKind::Markdown);
        // Setting the kind it already has records nothing.
        let depth = session.undo.len();
        session.set_kind(CellKind::Markdown);
        assert_eq!(session.undo.len(), depth);
    }

    #[test]
    fn copy_cut_and_paste_work_on_cells_with_new_ids() {
        let mut session = make(&["a", "b", "c"]);
        session.focus_by(1, false);
        session.copy_selected();
        session.set_focus(2, false);
        assert_eq!(session.paste_below(), 1);
        assert_eq!(texts(&session), ["a", "b", "c", "b"]);
        assert_eq!(session.focus, 3);
        assert_ne!(session.cells[1].cell.id, session.cells[3].cell.id);
        session.set_focus(0, false);
        session.focus_by(1, true);
        session.cut_selected();
        assert_eq!(texts(&session), ["c", "b"]);
        session.set_focus(1, false);
        assert_eq!(session.paste_below(), 2);
        assert_eq!(texts(&session), ["c", "b", "a", "b"]);
        assert_eq!(session.selected(), (2, 3));
        session.undo();
        assert_eq!(texts(&session), ["c", "b"]);
        session.undo();
        assert_eq!(texts(&session), ["a", "b", "c", "b"]);
        let mut empty = make(&["x"]);
        assert_eq!(empty.paste_below(), 0);
    }

    #[test]
    fn moving_the_caret_up_or_down_crosses_into_the_next_cell() {
        let mut session = make(&["one\ntwo", "three"]);
        session.enter_edit();
        session.edit(|editor| editor.set_cursor_line_col(1, 2));
        assert!(session.move_vertical(1, false));
        assert_eq!(session.focus, 1);
        let (line, column) = session.cells[1]
            .buffer
            .view(&session.cells[1].selection)
            .cursor_line_col();
        assert_eq!((line, column), (0, 2));
        assert!(session.move_vertical(-1, false));
        assert_eq!(session.focus, 0);
        let (line, column) = session.cells[0]
            .buffer
            .view(&session.cells[0].selection)
            .cursor_line_col();
        assert_eq!((line, column), (1, 2));
        // Inside a cell the caret just moves.
        assert!(!session.move_vertical(-1, false));
        assert_eq!(session.focus, 0);
        // The first and last cells have nowhere to go.
        session.edit(|editor| editor.set_cursor_line_col(0, 0));
        assert!(!session.move_vertical(-1, false));
    }

    #[test]
    fn collapsing_applies_to_the_selection() {
        let mut session = make(&["a", "b", "c"]);
        session.focus_by(1, true);
        session.toggle_input_collapsed();
        assert!(session.cells[0].input_collapsed && session.cells[1].input_collapsed);
        assert!(!session.cells[2].input_collapsed);
        session.toggle_input_collapsed();
        assert!(!session.cells[0].input_collapsed);
        session.toggle_output_collapsed();
        assert!(session.cells[1].output_collapsed);
    }

    #[test]
    fn reload_keeps_the_focus_and_view_settings() {
        let mut session = make(&["a", "b", "c"]);
        session.set_focus(2, false);
        session.line_numbers = true;
        let mut smaller = Notebook::new();
        smaller.cells = vec![
            Cell::new(CellKind::Code, "x"),
            Cell::new(CellKind::Code, "y"),
        ];
        session.reload(smaller);
        assert_eq!(texts(&session), ["x", "y"]);
        assert_eq!(session.focus, 1);
        assert!(session.line_numbers);
        assert!(!session.can_undo());
    }

    #[test]
    fn metadata_and_unknown_fields_survive_editing() {
        let notebook = Notebook::parse(
            r#"{"nbformat":4,"nbformat_minor":5,"metadata":{"kernelspec":{"name":"python3"}},"future":1,"cells":[{"cell_type":"code","id":"a","source":"x","metadata":{"k":1},"outputs":[],"execution_count":null,"unknown":true}]}"#,
        )
        .unwrap();
        let mut session = Session::from_notebook(notebook);
        type_text(&mut session, "y");
        let written: Value = serde_json::from_str(&session.to_json()).unwrap();
        assert_eq!(written["future"], 1);
        assert_eq!(written["metadata"]["kernelspec"]["name"], "python3");
        assert_eq!(written["cells"][0]["unknown"], true);
        assert_eq!(written["cells"][0]["source"], serde_json::json!(["yx"]));
        assert_eq!(session.metadata().kernel_name(), Some("python3"));
    }

    #[test]
    fn format_cell_and_notebook_trim_trailing_whitespace_and_support_undo() {
        let mut session = make(&["\tx = 1   \r\n\treturn x  \n\n", "# Title   \n"]);
        assert!(session.format_cell_at(0, 4));
        assert_eq!(texts(&session)[0], "    x = 1\n    return x");
        assert!(session.undo());
        assert_eq!(texts(&session)[0], "\tx = 1   \r\n\treturn x  \n\n");

        let changed = session.format_all_cells(2);
        assert_eq!(changed, 2);
        assert_eq!(texts(&session), ["  x = 1\n  return x", "# Title"]);
    }
}

fn format_cell_text(text: &str, kind: CellKind, tab_size: usize) -> String {
    let indent = " ".repeat(tab_size.max(1));
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = normalized
        .split('\n')
        .map(|line| {
            let trimmed = line.trim_end_matches([' ', '\t']);
            if kind == CellKind::Code {
                let leading_tabs = trimmed.bytes().take_while(|&b| b == b'\t').count();
                if leading_tabs > 0 {
                    return format!("{}{}", indent.repeat(leading_tabs), &trimmed[leading_tabs..]);
                }
            }
            trimmed.to_owned()
        })
        .collect();
    while lines.len() > 1 && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}
