//! Borrowed editing context. Selection state belongs to a view, never to a buffer.
use super::*;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewId(pub u64);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectionState {
    pub(super) cursor: Cursor,
    pub(super) selection: Option<Selection>,
    pub(super) extra_selections: Vec<Selection>,
    /// A remembered position (the selection anchor) that follows the text through edits.
    pub(super) mark: Option<usize>,
}

impl SelectionState {
    pub fn from_selections(selections: Vec<Selection>, len: usize) -> Self {
        let mut state = Self::default();
        state.set_selections(selections, len);
        state
    }
    pub fn set_selections(&mut self, mut selections: Vec<Selection>, len: usize) {
        if selections.is_empty() {
            selections.push(Selection {
                anchor: self.cursor.char_idx,
                head: self.cursor.char_idx,
            });
        }
        for s in &mut selections {
            s.anchor = s.anchor.min(len);
            s.head = s.head.min(len);
        }
        selections.sort_by_key(|s| (s.anchor.min(s.head), s.anchor.max(s.head)));
        let mut merged: Vec<Selection> = Vec::new();
        for selection in selections {
            let start = selection.anchor.min(selection.head);
            let end = selection.anchor.max(selection.head);
            if let Some(last) = merged.last_mut() {
                let a = last.anchor.min(last.head);
                let b = last.anchor.max(last.head);
                if start < b || (start == a && end == b) || (start == b && start == end) {
                    *last = Selection {
                        anchor: a,
                        head: b.max(end),
                    };
                    continue;
                }
            }
            merged.push(selection);
        }
        let primary = merged.remove(0);
        self.cursor.char_idx = primary.head;
        self.selection = (primary.anchor != primary.head).then_some(primary);
        self.extra_selections = merged;
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }
    pub fn selections(&self) -> Vec<Selection> {
        let mut selections = vec![self.selection.unwrap_or(Selection {
            anchor: self.cursor.char_idx,
            head: self.cursor.char_idx,
        })];
        selections.extend_from_slice(&self.extra_selections);
        selections
    }
    pub fn remap(&mut self, changes: &[PositionEdit], len: usize) {
        self.mark = self.mark.map(|mark| map_position(mark, changes).min(len));
        let selections = self
            .selections()
            .into_iter()
            .map(|s| Selection {
                anchor: map_position(s.anchor, changes).min(len),
                head: map_position(s.head, changes).min(len),
            })
            .collect();
        self.set_selections(selections, len);
    }
}

#[derive(Clone, Debug)]
pub struct HistorySelection {
    pub origin: ViewId,
    pub before: SelectionState,
    pub after: SelectionState,
}

#[derive(Clone, Debug)]
pub struct TransactionOutcome {
    pub revision: u64,
    pub changed: bool,
    pub len_chars: usize,
    pub positions: Vec<PositionEdit>,
    pub selections: Option<(ViewId, SelectionState)>,
}

pub struct Editor<'a> {
    pub(super) buffer: &'a mut Buffer,
    pub(super) state: &'a mut SelectionState,
    origin: ViewId,
    outcomes: Vec<TransactionOutcome>,
}
impl Deref for Editor<'_> {
    type Target = Buffer;
    fn deref(&self) -> &Buffer {
        self.buffer
    }
}
impl DerefMut for Editor<'_> {
    fn deref_mut(&mut self) -> &mut Buffer {
        self.buffer
    }
}
impl Buffer {
    pub fn edit<'a>(&'a mut self, state: &'a mut SelectionState, origin: ViewId) -> Editor<'a> {
        Editor {
            buffer: self,
            state,
            origin,
            outcomes: Vec::new(),
        }
    }
    pub fn record_selection_history(
        &mut self,
        origin: ViewId,
        before: SelectionState,
        after: SelectionState,
    ) {
        if let Some(edit) = self.undo.last_mut() {
            edit.selections = Some(HistorySelection {
                origin,
                before,
                after,
            });
        }
    }
    /// Reload a successfully read file through the same text transaction path.
    /// Reload invalidates text history while retaining the monotonic revision.
    pub fn reload_text(&mut self, text: &str) -> TransactionOutcome {
        let revision = self.revision;
        let read_only = self.read_only;
        self.read_only = false;
        self.apply_transaction(&[TextReplacement {
            range: 0..self.len_chars(),
            text: text.into(),
        }])
        .expect("complete document replacement is valid");
        self.read_only = read_only;
        if self.revision != revision {
            self.undo.clear();
            self.redo.clear();
        }
        self.saved_text = self.rope.clone();
        self.dirty = false;
        self.outcome_since(revision)
    }
    pub fn outcome_since(&self, revision: u64) -> TransactionOutcome {
        TransactionOutcome {
            revision: self.revision,
            len_chars: self.len_chars(),
            changed: self.revision != revision,
            positions: if self.revision != revision {
                self.last_position_edits.clone()
            } else {
                Vec::new()
            },
            selections: None,
        }
    }
    pub fn undo_transaction(&mut self) -> TransactionOutcome {
        let before = self.revision;
        let selection = self.undo.last().and_then(|edit| edit.selections.clone());
        self.undo();
        let mut outcome = self.outcome_since(before);
        if outcome.changed {
            outcome.selections = selection.map(|s| (s.origin, s.before));
        }
        outcome
    }
    pub fn redo_transaction(&mut self) -> TransactionOutcome {
        let before = self.revision;
        let selection = self.redo.last().and_then(|edit| edit.selections.clone());
        self.redo();
        let mut outcome = self.outcome_since(before);
        if outcome.changed {
            outcome.selections = selection.map(|s| (s.origin, s.after));
        }
        outcome
    }
}
impl Editor<'_> {
    pub fn outcomes(&self) -> &[TransactionOutcome] {
        &self.outcomes
    }
    pub fn cursor(&self) -> Cursor {
        self.state.cursor()
    }
    pub fn selection(&self) -> Option<Selection> {
        self.state.selection()
    }
    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|s| {
            self.rope
                .slice(s.anchor.min(s.head)..s.anchor.max(s.head))
                .to_string()
        })
    }
    pub fn set_selection(&mut self, anchor: usize, head: usize) {
        self.set_selections(vec![Selection { anchor, head }]);
    }
    pub fn apply_transaction(
        &mut self,
        edits: &[TextReplacement],
    ) -> Result<Vec<usize>, TransactionError> {
        let before = self.state.clone();
        let revision = self.revision();
        let positions = self.buffer.apply_transaction(edits)?;
        if self.revision() != revision {
            self.state
                .remap(self.buffer.last_position_edits(), self.buffer.len_chars());
            self.buffer.undo.last_mut().unwrap().selections = Some(HistorySelection {
                origin: self.origin,
                before,
                after: self.state.clone(),
            });
        }
        let mut outcome = self.buffer.outcome_since(revision);
        if outcome.changed {
            outcome.selections = Some((self.origin, self.state.clone()));
            self.outcomes.push(outcome);
        }
        Ok(positions)
    }
    pub(super) fn finish_selections(&mut self) {
        let after = self.state.clone();
        if let Some(snapshot) = self
            .buffer
            .undo
            .last_mut()
            .and_then(|edit| edit.selections.as_mut())
        {
            snapshot.after = after.clone();
        }
        if let Some(outcome) = self.outcomes.last_mut() {
            outcome.selections = Some((self.origin, after));
        }
    }
    pub(super) fn replace(&mut self, start: usize, end: usize, text: &str) {
        self.apply_cursor_replacements(&[TextReplacement {
            range: start..end,
            text: text.into(),
        }]);
    }
    fn document_operation<T>(&mut self, operation: impl FnOnce(&mut Buffer) -> T) -> T {
        let before = self.state.clone();
        let revision = self.revision();
        let result = operation(self.buffer);
        if self.revision() != revision {
            self.state
                .remap(self.buffer.last_position_edits(), self.buffer.len_chars());
            self.buffer.undo.last_mut().unwrap().selections = Some(HistorySelection {
                origin: self.origin,
                before,
                after: self.state.clone(),
            });
        }
        let mut outcome = self.buffer.outcome_since(revision);
        if outcome.changed {
            outcome.selections = Some((self.origin, self.state.clone()));
            self.outcomes.push(outcome);
        }
        result
    }
    pub fn convert_indentation(&mut self, target: Indentation) -> Result<usize, TransactionError> {
        self.document_operation(|buffer| buffer.convert_indentation(target))
    }
    pub fn trim_trailing_whitespace(&mut self) -> Result<usize, TransactionError> {
        self.document_operation(Buffer::trim_trailing_whitespace)
    }
    pub fn save_with_trim(&mut self) -> Result<(), BufferError> {
        self.document_operation(Buffer::save_with_trim)
    }
    pub fn save_encoded_bytes_with_trim(&mut self, bytes: &[u8]) -> Result<(), BufferError> {
        self.document_operation(|buffer| buffer.save_encoded_bytes_with_trim(bytes))
    }
    pub fn replace_matches(
        &mut self,
        query: &str,
        replacement: &str,
        options: SearchOptions,
        scope: Option<std::ops::Range<usize>>,
    ) -> Result<usize, regex::Error> {
        self.document_operation(|buffer| buffer.replace_matches(query, replacement, options, scope))
    }
    pub fn undo(&mut self) -> TransactionOutcome {
        let outcome = self.buffer.undo_transaction();
        self.apply_history_outcome(&outcome);
        if outcome.changed {
            self.outcomes.push(outcome.clone());
        }
        outcome
    }
    pub fn redo(&mut self) -> TransactionOutcome {
        let outcome = self.buffer.redo_transaction();
        self.apply_history_outcome(&outcome);
        if outcome.changed {
            self.outcomes.push(outcome.clone());
        }
        outcome
    }
    fn apply_history_outcome(&mut self, outcome: &TransactionOutcome) {
        if !outcome.changed {
            return;
        }
        if let Some((origin, selection)) = &outcome.selections {
            if *origin == self.origin {
                *self.state = selection.clone();
                return;
            }
        }
        self.state
            .remap(&outcome.positions, self.buffer.len_chars());
    }
}

/// Read-only document and view context, with no copied selection ownership.
pub struct EditorRead<'a> {
    buffer: &'a Buffer,
    state: &'a SelectionState,
}
impl Deref for EditorRead<'_> {
    type Target = Buffer;
    fn deref(&self) -> &Buffer {
        self.buffer
    }
}
impl Buffer {
    pub fn view<'a>(&'a self, state: &'a SelectionState) -> EditorRead<'a> {
        EditorRead {
            buffer: self,
            state,
        }
    }
}
impl EditorRead<'_> {
    pub fn cursor(&self) -> Cursor {
        self.state.cursor()
    }
    pub fn selection(&self) -> Option<Selection> {
        self.state.selection()
    }
    pub fn selections(&self) -> Vec<Selection> {
        self.state.selections()
    }
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let position = self.cursor().char_idx.min(self.len_chars());
        let line = self.rope.char_to_line(position);
        (line, position - self.rope.line_to_char(line))
    }
    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|s| {
            self.rope
                .slice(s.anchor.min(s.head)..s.anchor.max(s.head))
                .to_string()
        })
    }
    pub fn selected_texts(&self) -> Vec<String> {
        self.selections()
            .iter()
            .filter(|s| s.anchor != s.head)
            .map(|s| {
                self.rope
                    .slice(s.anchor.min(s.head)..s.anchor.max(s.head))
                    .to_string()
            })
            .collect()
    }
}
