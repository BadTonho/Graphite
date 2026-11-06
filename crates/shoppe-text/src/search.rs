use super::*;
use regex::{Regex, RegexBuilder};
use std::ops::Range;

#[derive(Debug, Default, Clone, Copy)]
pub struct SearchOptions {
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub preserve_case: bool,
}

/// A match expressed as character offsets, suitable for Buffer selections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub range: Range<usize>,
}

fn pattern(query: &str, options: SearchOptions) -> Result<Regex, regex::Error> {
    let pattern = if options.regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!options.case_sensitive)
        .build()
}

fn word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn whole_word(text: &str, start: usize, end: usize) -> bool {
    !text[..start].chars().next_back().is_some_and(word)
        && !text[end..].chars().next().is_some_and(word)
}

fn recase(source: &str, replacement: &str) -> String {
    let letters: Vec<char> = source.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return replacement.to_owned();
    }
    if letters.iter().all(|c| c.is_uppercase()) {
        return replacement.to_uppercase();
    }
    if letters.iter().all(|c| c.is_lowercase()) {
        return replacement.to_lowercase();
    }
    if letters[0].is_uppercase() && letters[1..].iter().all(|c| c.is_lowercase()) {
        let mut chars = replacement.chars();
        return chars
            .next()
            .map(|c| format!("{}{}", c.to_uppercase(), chars.as_str().to_lowercase()))
            .unwrap_or_default();
    }
    replacement.to_owned()
}

impl Buffer {
    /// Replace the selected match if it still matches the query in its original context.
    pub fn replace_selected_match(
        &mut self,
        query: &str,
        replacement: &str,
        options: SearchOptions,
        scope: Option<Range<usize>>,
    ) -> Result<bool, regex::Error> {
        if query.is_empty() || self.read_only {
            return Ok(false);
        }
        let Some(selection) = self.selection else {
            return Ok(false);
        };
        let selected = selection.anchor.min(selection.head)..selection.anchor.max(selection.head);
        let bounds = scope.unwrap_or(0..self.len_chars());
        let start = bounds.start.min(self.len_chars());
        let end = bounds.end.min(self.len_chars()).max(start);
        if selected.start < start || selected.end > end {
            return Ok(false);
        }
        let regex = pattern(query, options)?;
        let text = self.text();
        let first = self.rope.char_to_byte(start);
        let last = self.rope.char_to_byte(end);
        let target_start = self.rope.char_to_byte(selected.start);
        let target_end = self.rope.char_to_byte(selected.end);
        for captures in regex.captures_iter(&text[first..last]) {
            let m = captures.get(0).expect("capture zero always exists");
            if first + m.start() != target_start || first + m.end() != target_end {
                continue;
            }
            if options.whole_word && !whole_word(&text, target_start, target_end) {
                continue;
            }
            let mut value = String::new();
            if options.regex {
                captures.expand(replacement, &mut value);
            } else {
                value.push_str(replacement);
            }
            if options.preserve_case {
                value = recase(m.as_str(), &value);
            }
            self.replace(selected.start, selected.end, &value);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn find_matches(
        &self,
        query: &str,
        options: SearchOptions,
        scope: Option<Range<usize>>,
    ) -> Result<Vec<SearchMatch>, regex::Error> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let regex = pattern(query, options)?;
        let bounds = scope.unwrap_or(0..self.len_chars());
        let start = bounds.start.min(self.len_chars());
        let end = bounds.end.min(self.len_chars()).max(start);
        let text = self.text();
        let first = self.rope.char_to_byte(start);
        let last = self.rope.char_to_byte(end);
        Ok(regex
            .find_iter(&text[first..last])
            .filter_map(|m| {
                let a = first + m.start();
                let b = first + m.end();
                (!options.whole_word || whole_word(&text, a, b)).then(|| SearchMatch {
                    range: self.rope.byte_to_char(a)..self.rope.byte_to_char(b),
                })
            })
            .collect())
    }

    /// Replace all matches in one undoable edit. Regex replacements support capture groups.
    pub fn replace_matches(
        &mut self,
        query: &str,
        replacement: &str,
        options: SearchOptions,
        scope: Option<Range<usize>>,
    ) -> Result<usize, regex::Error> {
        if query.is_empty() || self.read_only {
            return Ok(0);
        }
        let regex = pattern(query, options)?;
        let bounds = scope.unwrap_or(0..self.len_chars());
        let start = bounds.start.min(self.len_chars());
        let end = bounds.end.min(self.len_chars()).max(start);
        let text = self.text();
        let first = self.rope.char_to_byte(start);
        let last = self.rope.char_to_byte(end);
        let scoped = &text[first..last];
        let mut result = String::new();
        let mut previous = 0;
        let mut count = 0;
        for captures in regex.captures_iter(scoped) {
            let m = captures.get(0).expect("capture zero always exists");
            if options.whole_word && !whole_word(&text, first + m.start(), first + m.end()) {
                continue;
            }
            result.push_str(&scoped[previous..m.start()]);
            let mut value = String::new();
            if options.regex {
                captures.expand(replacement, &mut value);
            } else {
                value.push_str(replacement);
            }
            if options.preserve_case {
                value = recase(m.as_str(), &value);
            }
            result.push_str(&value);
            previous = m.end();
            count += 1;
        }
        if count > 0 {
            result.push_str(&scoped[previous..]);
            self.replace(start, end, &result);
        }
        Ok(count)
    }
}
