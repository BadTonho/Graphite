//! Per-block resolution of a parsed conflict file.

use super::parse::{parse, ConflictBlock, Parsed, Segment};

/// Which sides of a conflict go into the result, and in which order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Choice {
    pub current: bool,
    pub incoming: bool,
    /// Only meaningful when both sides are taken.
    pub incoming_first: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Resolution {
    /// Not decided yet; the result still carries the conflict.
    #[default]
    Unresolved,
    Taken(Choice),
    /// Decided to keep neither side: the result takes the common ancestor, or nothing when the
    /// markers carry none.
    Ignored,
}

impl Resolution {
    pub(crate) fn is_handled(self) -> bool {
        !matches!(self, Resolution::Unresolved)
    }
}

/// Which full-file text to build from the conflict blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Current,
    Incoming,
    Base,
}

/// A file with conflicts together with what was decided for each one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MergeState {
    parsed: Parsed,
    resolutions: Vec<Resolution>,
}

fn choice(current: bool, incoming: bool, incoming_first: bool) -> Resolution {
    if current || incoming {
        Resolution::Taken(Choice {
            current,
            incoming,
            incoming_first,
        })
    } else {
        Resolution::Unresolved
    }
}

/// Drops the last line break, used when the closing marker was the last, unterminated line of the
/// file so that the merged text ends the same way.
fn without_last_break(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

impl MergeState {
    pub(crate) fn new(text: &str) -> Self {
        let parsed = parse(text);
        let resolutions = vec![Resolution::Unresolved; parsed.conflict_count()];
        Self {
            parsed,
            resolutions,
        }
    }

    /// Parses `text` again and keeps the decision of every conflict whose markers did not
    /// change, so the file can be edited while the merge is open.
    pub(crate) fn reload(&mut self, text: &str) {
        let old: Vec<(String, Resolution)> = self
            .parsed
            .conflicts()
            .zip(&self.resolutions)
            .map(|(block, resolution)| (block.raw.clone(), *resolution))
            .collect();
        let mut fresh = MergeState::new(text);
        let mut from = 0;
        let kept: Vec<(usize, Resolution)> = fresh
            .parsed
            .conflicts()
            .enumerate()
            .filter_map(|(index, block)| {
                let found = old[from..].iter().position(|(raw, _)| *raw == block.raw)?;
                from += found + 1;
                Some((index, old[from - 1].1))
            })
            .collect();
        for (index, resolution) in kept {
            fresh.resolutions[index] = resolution;
        }
        *self = fresh;
    }

    /// Populates missing `base` sections on 2-way conflict blocks from a common ancestor text
    /// (such as Git index stage 1) by matching surrounding plain-text segments in order.
    pub(crate) fn populate_base_from_ancestor(&mut self, ancestor: &str) {
        let mut cursor = 0usize;
        let len = self.parsed.segments.len();
        for index in 0..len {
            let before = if index > 0 {
                match &self.parsed.segments[index - 1] {
                    Segment::Text(text) => Some(text.clone()),
                    Segment::Conflict(_) => None,
                }
            } else {
                None
            };
            let after = if index + 1 < len {
                match &self.parsed.segments[index + 1] {
                    Segment::Text(text) => Some(text.clone()),
                    Segment::Conflict(_) => None,
                }
            } else {
                None
            };

            let Segment::Conflict(block) = &mut self.parsed.segments[index] else {
                continue;
            };
            if block.base.is_some() {
                continue;
            }
            let Some(start) = (match before.as_deref() {
                None => Some(cursor),
                Some(text) if text.is_empty() => Some(cursor),
                Some(text) => find_anchor_end(ancestor, cursor, text),
            }) else {
                continue;
            };
            let Some(end) = (match after.as_deref() {
                None if index + 1 >= len => Some(ancestor.len()),
                None => continue,
                Some(text) if text.is_empty() => Some(ancestor.len()),
                Some(text) => find_anchor_start(ancestor, start, text),
            }) else {
                continue;
            };
            if start <= end {
                block.base = Some(ancestor[start..end].to_owned());
                cursor = end;
            }
        }
    }

    pub(crate) fn parsed(&self) -> &Parsed {
        &self.parsed
    }

    pub(crate) fn len(&self) -> usize {
        self.resolutions.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.resolutions.is_empty()
    }

    pub(crate) fn block(&self, index: usize) -> Option<&ConflictBlock> {
        self.parsed.conflicts().nth(index)
    }

    pub(crate) fn resolution(&self, index: usize) -> Option<Resolution> {
        self.resolutions.get(index).copied()
    }

    pub(crate) fn unresolved_count(&self) -> usize {
        self.resolutions
            .iter()
            .filter(|resolution| !resolution.is_handled())
            .count()
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.unresolved_count() == 0
    }

    fn set(&mut self, index: usize, resolution: Resolution) -> bool {
        match self.resolutions.get_mut(index) {
            Some(slot) => {
                *slot = resolution;
                true
            }
            None => false,
        }
    }

    pub(crate) fn accept_current(&mut self, index: usize) -> bool {
        self.set(index, choice(true, false, false))
    }

    pub(crate) fn accept_incoming(&mut self, index: usize) -> bool {
        self.set(index, choice(false, true, false))
    }

    pub(crate) fn accept_both(&mut self, index: usize, incoming_first: bool) -> bool {
        self.set(index, choice(true, true, incoming_first))
    }

    pub(crate) fn ignore(&mut self, index: usize) -> bool {
        self.set(index, Resolution::Ignored)
    }

    pub(crate) fn reset(&mut self, index: usize) -> bool {
        self.set(index, Resolution::Unresolved)
    }

    pub(crate) fn reset_all(&mut self) {
        self.resolutions.fill(Resolution::Unresolved);
    }

    pub(crate) fn accept_all_current(&mut self) {
        self.resolutions.fill(choice(true, false, false));
    }

    pub(crate) fn accept_all_incoming(&mut self) {
        self.resolutions.fill(choice(false, true, false));
    }

    pub(crate) fn accept_all_both(&mut self) {
        self.resolutions.fill(choice(true, true, false));
    }

    /// Adds or removes the current side of one conflict. A side added second goes second.
    pub(crate) fn toggle_current(&mut self, index: usize) -> bool {
        let Some(existing) = self.resolution(index) else {
            return false;
        };
        let next = match existing {
            Resolution::Taken(taken) => choice(
                !taken.current,
                taken.incoming,
                taken.incoming && !taken.current,
            ),
            _ => choice(true, false, false),
        };
        self.set(index, next)
    }

    /// Adds or removes the incoming side of one conflict. A side added second goes second.
    pub(crate) fn toggle_incoming(&mut self, index: usize) -> bool {
        let Some(existing) = self.resolution(index) else {
            return false;
        };
        let next = match existing {
            Resolution::Taken(taken) => choice(
                taken.current,
                !taken.incoming,
                !(taken.current && !taken.incoming),
            ),
            _ => choice(false, true, false),
        };
        self.set(index, next)
    }

    /// Index of the next unresolved conflict after `from` (or the first one when `from` is
    /// `None`), wrapping around the end.
    pub(crate) fn next_unresolved(&self, from: Option<usize>) -> Option<usize> {
        let count = self.len();
        let start = from.map_or(0, |index| index + 1);
        (0..count)
            .map(|offset| (start + offset) % count)
            .find(|&index| !self.resolutions[index].is_handled())
    }

    /// Index of the previous unresolved conflict before `from`, wrapping around the start.
    pub(crate) fn previous_unresolved(&self, from: Option<usize>) -> Option<usize> {
        let count = self.len();
        if count == 0 {
            return None;
        }
        let start = from.unwrap_or(0) + count;
        (1..=count)
            .map(|offset| (start - offset) % count)
            .find(|&index| !self.resolutions[index].is_handled())
    }

    /// Text one conflict contributes to the result, `None` while it is unresolved.
    pub(crate) fn resolved_text(&self, index: usize) -> Option<String> {
        let block = self.block(index)?;
        let body = match self.resolution(index)? {
            Resolution::Unresolved => return None,
            Resolution::Ignored => block.base.clone().unwrap_or_default(),
            Resolution::Taken(taken) => {
                let mut parts: Vec<&str> = Vec::new();
                if taken.current {
                    parts.push(&block.current);
                }
                if taken.incoming {
                    parts.push(&block.incoming);
                }
                if taken.current && taken.incoming && taken.incoming_first {
                    parts.reverse();
                }
                parts.concat()
            }
        };
        Some(if block.end_eol.is_empty() {
            without_last_break(&body).to_owned()
        } else {
            body
        })
    }

    /// The whole file with every handled conflict replaced by its resolution. Unresolved
    /// conflicts keep their markers exactly as they were, so the text can be opened and finished
    /// by hand.
    pub(crate) fn render(&self) -> String {
        let mut out = String::new();
        let mut index = 0;
        for segment in &self.parsed.segments {
            match segment {
                Segment::Text(text) => out.push_str(text),
                Segment::Conflict(block) => {
                    match self.resolved_text(index) {
                        Some(text) => out.push_str(&text),
                        None => out.push_str(&block.raw),
                    }
                    index += 1;
                }
            }
        }
        out
    }

    /// The final merged text, only when no conflict is left undecided.
    pub(crate) fn complete(&self) -> Option<String> {
        self.is_complete().then(|| self.render())
    }

    /// The whole file as one side sees it: plain text plus that side of every conflict.
    pub(crate) fn side_text(&self, side: Side) -> String {
        let mut out = String::new();
        for segment in &self.parsed.segments {
            match segment {
                Segment::Text(text) => out.push_str(text),
                Segment::Conflict(block) => out.push_str(match side {
                    Side::Current => &block.current,
                    Side::Incoming => &block.incoming,
                    Side::Base => block.base.as_deref().unwrap_or(""),
                }),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "head\n<<<<<<< a\nc1\n=======\ni1\n>>>>>>> b\nmid\n<<<<<<< a\nc2\n||||||| o\nb2\n=======\ni2\n>>>>>>> b\ntail\n";

    #[test]
    fn nothing_is_decided_at_first_and_render_is_lossless() {
        let state = MergeState::new(TWO);
        assert_eq!(state.len(), 2);
        assert_eq!(state.unresolved_count(), 2);
        assert!(!state.is_complete());
        assert_eq!(state.complete(), None);
        assert_eq!(state.render(), TWO);
    }

    #[test]
    fn every_single_block_action_produces_its_text() {
        let mut state = MergeState::new(TWO);
        state.accept_current(0);
        state.accept_incoming(1);
        assert_eq!(state.render(), "head\nc1\nmid\ni2\ntail\n");
        state.accept_both(0, false);
        state.accept_both(1, true);
        assert_eq!(state.render(), "head\nc1\ni1\nmid\ni2\nc2\ntail\n");
        state.ignore(0);
        state.ignore(1);
        assert_eq!(state.render(), "head\nmid\nb2\ntail\n");
        state.reset(0);
        assert_eq!(state.unresolved_count(), 1);
        assert!(state.render().starts_with("head\n<<<<<<< a\nc1\n"));
        assert_eq!(state.complete(), None);
        state.reset_all();
        assert_eq!(state.render(), TWO);
    }

    #[test]
    fn accept_all_variants() {
        let mut state = MergeState::new(TWO);
        state.accept_all_current();
        assert_eq!(state.complete().unwrap(), "head\nc1\nmid\nc2\ntail\n");
        state.accept_all_incoming();
        assert_eq!(state.complete().unwrap(), "head\ni1\nmid\ni2\ntail\n");
        state.accept_all_both();
        assert_eq!(
            state.complete().unwrap(),
            "head\nc1\ni1\nmid\nc2\ni2\ntail\n"
        );
    }

    #[test]
    fn toggling_sides_remembers_the_order_they_were_added_in() {
        let mut state = MergeState::new(TWO);
        state.toggle_incoming(0);
        state.toggle_current(0);
        assert_eq!(state.resolved_text(0).unwrap(), "i1\nc1\n");
        state.toggle_incoming(0);
        assert_eq!(state.resolved_text(0).unwrap(), "c1\n");
        state.toggle_current(0);
        assert_eq!(state.resolution(0), Some(Resolution::Unresolved));
        state.toggle_current(0);
        state.toggle_incoming(0);
        assert_eq!(state.resolved_text(0).unwrap(), "c1\ni1\n");
        // Toggling over an ignored block starts from that side alone.
        state.ignore(1);
        state.toggle_incoming(1);
        assert_eq!(state.resolved_text(1).unwrap(), "i2\n");
    }

    #[test]
    fn out_of_range_blocks_are_rejected() {
        let mut state = MergeState::new(TWO);
        assert!(!state.accept_current(2));
        assert!(!state.toggle_current(9));
        assert!(!state.toggle_incoming(9));
        assert_eq!(state.resolved_text(5), None);
        assert!(MergeState::new("plain\n").is_complete());
    }

    #[test]
    fn navigation_wraps_and_skips_handled_conflicts() {
        let mut state = MergeState::new(
            "<<<<<<< a\n1\n=======\n2\n>>>>>>> b\n<<<<<<< a\n1\n=======\n2\n>>>>>>> b\n<<<<<<< a\n1\n=======\n2\n>>>>>>> b\n",
        );
        assert_eq!(state.next_unresolved(None), Some(0));
        assert_eq!(state.next_unresolved(Some(2)), Some(0));
        assert_eq!(state.previous_unresolved(Some(0)), Some(2));
        state.accept_current(1);
        assert_eq!(state.next_unresolved(Some(0)), Some(2));
        assert_eq!(state.previous_unresolved(Some(2)), Some(0));
        state.accept_current(0);
        state.accept_current(2);
        assert_eq!(state.next_unresolved(Some(0)), None);
        assert_eq!(state.previous_unresolved(None), None);
        assert_eq!(MergeState::new("x\n").next_unresolved(None), None);
    }

    #[test]
    fn crlf_and_missing_final_newline_are_preserved() {
        let mut state =
            MergeState::new("a\r\n<<<<<<< x\r\nc\r\n=======\r\ni\r\n>>>>>>> y\r\nz\r\n");
        state.accept_both(0, false);
        assert_eq!(state.render(), "a\r\nc\r\ni\r\nz\r\n");
        let mut state = MergeState::new("a\n<<<<<<< x\nc\n=======\ni\n>>>>>>> y");
        state.accept_incoming(0);
        assert_eq!(state.render(), "a\ni");
        state.accept_both(0, true);
        assert_eq!(state.render(), "a\ni\nc");
    }

    #[test]
    fn side_texts_rebuild_each_version_of_the_file() {
        let state = MergeState::new(TWO);
        assert_eq!(state.side_text(Side::Current), "head\nc1\nmid\nc2\ntail\n");
        assert_eq!(state.side_text(Side::Incoming), "head\ni1\nmid\ni2\ntail\n");
        assert_eq!(state.side_text(Side::Base), "head\nmid\nb2\ntail\n");
    }

    #[test]
    fn populate_base_from_ancestor_fills_two_way_conflicts_and_keeps_three_way() {
        let conflict = "\
head
<<<<<<< current
c1
=======
i1
>>>>>>> incoming
mid
<<<<<<< current
c2
||||||| base
explicit-base
=======
i2
>>>>>>> incoming
tail
";
        let ancestor = "head\nb1\nmid\nother-base\ntail\n";
        let mut state = MergeState::new(conflict);
        state.populate_base_from_ancestor(ancestor);
        assert_eq!(state.block(0).unwrap().base.as_deref(), Some("b1\n"));
        assert_eq!(
            state.block(1).unwrap().base.as_deref(),
            Some("explicit-base\n")
        );
        assert_eq!(
            state.side_text(Side::Base),
            "head\nb1\nmid\nexplicit-base\ntail\n"
        );
    }
}

fn find_anchor_end(haystack: &str, from: usize, needle: &str) -> Option<usize> {
    if let Some(pos) = haystack[from..].find(needle) {
        return Some(from + pos + needle.len());
    }
    let tail: String = needle
        .lines()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|line| format!("{line}\n"))
        .collect();
    if !tail.is_empty() {
        if let Some(pos) = haystack[from..].find(&tail) {
            return Some(from + pos + tail.len());
        }
    }
    None
}

fn find_anchor_start(haystack: &str, from: usize, needle: &str) -> Option<usize> {
    if let Some(pos) = haystack[from..].find(needle) {
        return Some(from + pos);
    }
    let head: String = needle
        .lines()
        .take(3)
        .map(|line| format!("{line}\n"))
        .collect();
    if !head.is_empty() {
        if let Some(pos) = haystack[from..].find(&head) {
            return Some(from + pos);
        }
    }
    None
}
