//! Bracket pairing for `()`, `[]` and `{}`.
//!
//! Pairing is purely textual: brackets inside strings and comments are counted as well.
//! Scans are capped so that a cursor on a huge file stays responsive.

use ropey::Rope;

/// Maximum number of characters scanned while searching for a matching bracket.
const MAX_SCAN: usize = 200_000;

/// Returns the opposite bracket and whether `ch` opens a pair, if `ch` is a bracket.
fn bracket_kind(ch: char) -> Option<(char, bool)> {
    match ch {
        '(' => Some((')', true)),
        '[' => Some((']', true)),
        '{' => Some(('}', true)),
        ')' => Some(('(', false)),
        ']' => Some(('[', false)),
        '}' => Some(('{', false)),
        _ => None,
    }
}

/// Finds the bracket that pairs with the bracket at `cursor` or just before it.
///
/// Returns the character indices of the bracket at the cursor and of its match.
pub fn matching_bracket(rope: &Rope, cursor: usize) -> Option<(usize, usize)> {
    let len = rope.len_chars();
    for index in [cursor, cursor.saturating_sub(1)] {
        if index >= len {
            continue;
        }
        let Some((other, is_open)) = bracket_kind(rope.char(index)) else {
            continue;
        };
        let found = if is_open {
            scan_forward(rope, index, rope.char(index), other)
        } else {
            scan_backward(rope, index, other, rope.char(index))
        };
        if let Some(found) = found {
            return Some((index, found));
        }
    }
    None
}

fn scan_forward(rope: &Rope, start: usize, opener: char, closer: char) -> Option<usize> {
    let end = rope.len_chars().min(start.saturating_add(MAX_SCAN));
    let mut depth = 0usize;
    for index in start..end {
        let ch = rope.char(index);
        if ch == opener {
            depth += 1;
        } else if ch == closer {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn scan_backward(rope: &Rope, start: usize, opener: char, closer: char) -> Option<usize> {
    let lower = start.saturating_sub(MAX_SCAN);
    let mut depth = 0usize;
    for index in (lower..=start).rev() {
        let ch = rope.char(index);
        if ch == closer {
            depth += 1;
        } else if ch == opener {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

/// The innermost bracket pair around `offset`, as the indices of its opening and closing bracket.
pub fn enclosing_brackets(rope: &Rope, offset: usize) -> Option<(usize, usize)> {
    let offset = offset.min(rope.len_chars());
    let mut open_depth = [0usize; 3];
    for index in (offset.saturating_sub(MAX_SCAN)..offset).rev() {
        let ch = rope.char(index);
        let Some((other, is_open)) = bracket_kind(ch) else {
            continue;
        };
        let kind = match ch {
            '(' | ')' => 0,
            '[' | ']' => 1,
            _ => 2,
        };
        if !is_open {
            open_depth[kind] += 1;
        } else if open_depth[kind] > 0 {
            open_depth[kind] -= 1;
        } else {
            return scan_forward(rope, index, ch, other).map(|close| (index, close));
        }
    }
    None
}

/// Index of the first bracket at or after `offset`.
pub fn next_bracket(rope: &Rope, offset: usize) -> Option<usize> {
    let end = rope.len_chars().min(offset.saturating_add(MAX_SCAN));
    (offset..end).find(|&index| bracket_kind(rope.char(index)).is_some())
}

/// Nesting level (cycling through 0..3) of every bracket before `end`, from the document start.
///
/// Each closing bracket gets the level of its opening bracket.
pub fn bracket_levels(rope: &Rope, end: usize) -> Vec<(usize, u8)> {
    let mut levels = Vec::new();
    scan_levels(rope, 0..end.min(rope.len_chars()), 0, Some(&mut levels));
    levels
}

/// Walks the brackets of `chars` starting at `depth`, recording their levels in `levels` when
/// given, and returns the depth after the range. A closing bracket never goes below zero.
fn scan_levels(
    rope: &Rope,
    chars: std::ops::Range<usize>,
    mut depth: usize,
    mut levels: Option<&mut Vec<(usize, u8)>>,
) -> usize {
    if chars.start >= chars.end {
        return depth;
    }
    for (offset, ch) in rope
        .chars_at(chars.start)
        .take(chars.end - chars.start)
        .enumerate()
    {
        let index = chars.start + offset;
        match bracket_kind(ch) {
            Some((_, true)) => {
                if let Some(levels) = levels.as_deref_mut() {
                    levels.push((index, (depth % 3) as u8));
                }
                depth += 1;
            }
            Some((_, false)) => {
                depth = depth.saturating_sub(1);
                if let Some(levels) = levels.as_deref_mut() {
                    levels.push((index, (depth % 3) as u8));
                }
            }
            None => {}
        }
    }
    depth
}

/// Lines between two remembered depths.
const DEPTH_STRIDE: usize = 256;

/// The bracket depth at every [`DEPTH_STRIDE`]th line of a buffer, so the levels of the visible
/// lines need a scan from the nearest remembered line above them instead of from the start of
/// the document. Depths after an edited line are forgotten and found again when needed.
#[derive(Debug, Default, Clone)]
pub struct BracketDepths {
    revision: Option<u64>,
    /// `depths[k]` is the depth at the start of line `k * DEPTH_STRIDE`.
    depths: Vec<usize>,
}

impl BracketDepths {
    /// The levels of the brackets from the start of line `first_line` up to char `end`, the
    /// same as [`bracket_levels`] gives for them.
    pub fn levels(
        &mut self,
        buffer: &crate::Buffer,
        first_line: usize,
        end: usize,
    ) -> Vec<(usize, u8)> {
        self.forget_changed(buffer);
        let rope = buffer.rope();
        let first_line = first_line.min(rope.len_lines().saturating_sub(1));
        let checkpoint = first_line / DEPTH_STRIDE;
        if self.depths.is_empty() {
            self.depths.push(0);
        }
        while self.depths.len() <= checkpoint {
            let known = self.depths.len() - 1;
            let from = rope.line_to_char(known * DEPTH_STRIDE);
            let to = rope.line_to_char(((known + 1) * DEPTH_STRIDE).min(rope.len_lines()));
            let depth = scan_levels(rope, from..to, self.depths[known], None);
            self.depths.push(depth);
        }
        let start = rope.line_to_char(first_line);
        let depth = scan_levels(
            rope,
            rope.line_to_char(checkpoint * DEPTH_STRIDE)..start,
            self.depths[checkpoint],
            None,
        );
        let mut levels = Vec::new();
        scan_levels(
            rope,
            start..end.min(rope.len_chars()).max(start),
            depth,
            Some(&mut levels),
        );
        levels
    }

    /// Drops the depths an edit since the last call may have changed.
    fn forget_changed(&mut self, buffer: &crate::Buffer) {
        let current = buffer.revision();
        if self.revision == Some(current) {
            return;
        }
        let changes = self
            .revision
            .and_then(|revision| buffer.changes_since(revision));
        match changes {
            Some(changes) => {
                if let Some(row) = changes.iter().map(|change| change.start_row).min() {
                    // Depth `k` covers the text before line `k * DEPTH_STRIDE`; an edit on that
                    // line or later leaves it intact.
                    self.depths.truncate(row / DEPTH_STRIDE + 1);
                }
            }
            None => self.depths.clear(),
        }
        self.revision = Some(current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_pairs_at_and_before_the_cursor() {
        let rope = Rope::from_str("f(a, [b, {c}])");
        assert_eq!(matching_bracket(&rope, 1), Some((1, 13)));
        assert_eq!(matching_bracket(&rope, 14), Some((13, 1)));
        assert_eq!(matching_bracket(&rope, 10), Some((9, 11)));
        assert_eq!(matching_bracket(&Rope::from_str("(open"), 0), None);
        assert_eq!(matching_bracket(&Rope::from_str("plain"), 2), None);
    }

    fn buffer(text: &str) -> crate::Buffer {
        crate::Buffer::from_text("fixture.rs", text)
    }

    #[test]
    fn remembered_depths_give_the_same_levels_as_a_scan_from_the_start() {
        let text = "fn f() { (a[b]) }\n{\n".repeat(700);
        let document = buffer(&text);
        let mut depths = BracketDepths::default();
        for first_line in [0, 1, 255, 256, 600, 1_399] {
            let end = document.rope().line_to_char((first_line + 40).min(1_400));
            let start = document.rope().line_to_char(first_line);
            let expected: Vec<_> = bracket_levels(document.rope(), end)
                .into_iter()
                .filter(|(index, _)| *index >= start)
                .collect();
            assert_eq!(
                depths.levels(&document, first_line, end),
                expected,
                "{first_line}"
            );
        }
    }

    #[test]
    fn depths_after_an_edit_are_found_again() {
        let mut document = buffer(&"{\n".repeat(1_000));
        let mut depths = BracketDepths::default();
        let end = document.rope().len_chars();
        let before = depths.levels(&document, 900, end);
        assert_eq!(before[0], (900 * 2, (900 % 3) as u8));

        // Close a bracket on line 10: every later level drops by one.
        let mut view = crate::SelectionState::default();
        {
            let mut editor = document.edit(&mut view, crate::ViewId(1));
            editor.set_selection(20, 20);
            editor.insert("}");
        }
        let end = document.rope().len_chars();
        let start = document.rope().line_to_char(900);
        let expected: Vec<_> = bracket_levels(document.rope(), end)
            .into_iter()
            .filter(|(index, _)| *index >= start)
            .collect();
        assert_eq!(depths.levels(&document, 900, end), expected);
        assert_eq!(expected[0].1, (899 % 3) as u8);
    }

    #[test]
    fn levels_follow_nesting_and_cycle_after_three() {
        let rope = Rope::from_str("(()[])");
        assert_eq!(
            bracket_levels(&rope, rope.len_chars()),
            vec![(0, 0), (1, 1), (2, 1), (3, 1), (4, 1), (5, 0)]
        );
        assert_eq!(
            bracket_levels(&Rope::from_str("(((("), 4),
            vec![(0, 0), (1, 1), (2, 2), (3, 0)]
        );
    }
}
