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
    let mut depth = 0usize;
    let mut levels = Vec::new();
    for (index, ch) in rope.chars().enumerate().take(end) {
        match bracket_kind(ch) {
            Some((_, true)) => {
                levels.push((index, (depth % 3) as u8));
                depth += 1;
            }
            Some((_, false)) => {
                depth = depth.saturating_sub(1);
                levels.push((index, (depth % 3) as u8));
            }
            None => {}
        }
    }
    levels
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
