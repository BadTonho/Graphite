//! The undo history of a buffer, kept within a fixed budget.

use std::collections::VecDeque;

/// Most steps kept for undo.
pub(crate) const MAX_UNDO_STEPS: usize = 10_000;
/// Most text, in bytes, the undo steps may hold together.
pub(crate) const MAX_UNDO_BYTES: usize = 64 * 1024 * 1024;

/// Undo steps, newest last. When a new step goes over [`MAX_UNDO_STEPS`] or [`MAX_UNDO_BYTES`],
/// the oldest steps are forgotten, so a long session or huge replacements cannot grow memory
/// without limit. The newest step is always kept.
#[derive(Debug)]
pub(crate) struct UndoStack<T> {
    steps: VecDeque<T>,
    bytes: usize,
    size_of: fn(&T) -> usize,
}

impl<T> UndoStack<T> {
    pub(crate) fn new(size_of: fn(&T) -> usize) -> Self {
        Self {
            steps: VecDeque::new(),
            bytes: 0,
            size_of,
        }
    }

    pub(crate) fn push(&mut self, step: T) {
        self.bytes += (self.size_of)(&step);
        self.steps.push_back(step);
        while self.steps.len() > 1
            && (self.steps.len() > MAX_UNDO_STEPS || self.bytes > MAX_UNDO_BYTES)
        {
            if let Some(oldest) = self.steps.pop_front() {
                self.bytes -= (self.size_of)(&oldest);
            }
        }
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        let step = self.steps.pop_back()?;
        self.bytes -= (self.size_of)(&step);
        Some(step)
    }

    pub(crate) fn last(&self) -> Option<&T> {
        self.steps.back()
    }

    /// The newest step, for changes that leave its size as measured when it was pushed (its
    /// selections, for example); the byte count would otherwise go wrong.
    pub(crate) fn last_mut(&mut self) -> Option<&mut T> {
        self.steps.back_mut()
    }

    pub(crate) fn clear(&mut self) {
        self.steps.clear();
        self.bytes = 0;
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.steps.len()
    }

    #[cfg(test)]
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack() -> UndoStack<String> {
        UndoStack::new(String::len)
    }

    #[test]
    fn the_oldest_steps_are_forgotten_past_the_step_limit() {
        let mut history = stack();
        for index in 0..MAX_UNDO_STEPS + 25 {
            history.push(index.to_string());
        }
        assert_eq!(history.len(), MAX_UNDO_STEPS);
        assert_eq!(history.last().map(String::as_str), Some("10024"));
        // The oldest kept step is the 26th pushed.
        let mut oldest = None;
        while let Some(step) = history.pop() {
            oldest = Some(step);
        }
        assert_eq!(oldest.as_deref(), Some("25"));
        assert_eq!(history.bytes(), 0);
    }

    #[test]
    fn large_steps_are_forgotten_past_the_byte_budget_but_the_newest_stays() {
        let mut history = stack();
        let quarter = "x".repeat(MAX_UNDO_BYTES / 4);
        for _ in 0..6 {
            history.push(quarter.clone());
        }
        assert_eq!(history.len(), 4);
        assert!(history.bytes() <= MAX_UNDO_BYTES);

        // One step over the whole budget is still undoable.
        history.push("y".repeat(MAX_UNDO_BYTES + 1));
        assert_eq!(history.len(), 1);
        assert_eq!(history.last().map(String::len), Some(MAX_UNDO_BYTES + 1));
    }

    #[test]
    fn popping_and_clearing_keep_the_byte_count_right() {
        let mut history = stack();
        history.push("abc".into());
        history.push("de".into());
        assert_eq!(history.bytes(), 5);
        assert_eq!(history.pop().as_deref(), Some("de"));
        assert_eq!(history.bytes(), 3);
        history.clear();
        assert!(history.is_empty());
        assert_eq!(history.bytes(), 0);
    }
}
