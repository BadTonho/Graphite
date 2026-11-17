//! Tokenizing text with a TextMate grammar, one line at a time. The state at the end of a line
//! (the rules still open) starts the next line, which is what lets a block comment or string
//! span lines and what makes re-tokenizing after an edit stop where the state matches again.

use super::grammar::{CaptureMap, Groups, Include, Kind, Registry, RuleRef};
use super::scopes::class_for_scopes;
use crate::{Class, Span};
use std::collections::{HashMap, HashSet};

/// A `begin`/`end` (or `begin`/`while`) rule that is open.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Frame {
    rule: usize,
    /// The end pattern with the groups of the begin match filled in.
    end: Option<String>,
}

/// The rules open at some point of a document.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct State {
    frames: Vec<Frame>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Candidate {
    Rule(usize),
    End,
}

/// Painting that later paints override: one class per byte of the line.
struct Canvas {
    classes: Vec<Class>,
}

impl Canvas {
    fn new(len: usize) -> Self {
        Self {
            classes: vec![Class::Default; len],
        }
    }

    fn paint(&mut self, start: usize, end: usize, class: Class) {
        let end = end.min(self.classes.len());
        if start < end {
            self.classes[start..end].fill(class);
        }
    }

    fn spans(&self) -> Vec<Span> {
        let mut spans: Vec<Span> = Vec::new();
        let mut start = 0;
        while start < self.classes.len() {
            let class = self.classes[start];
            let mut end = start + 1;
            while end < self.classes.len() && self.classes[end] == class {
                end += 1;
            }
            if class != Class::Default {
                spans.push(Span {
                    range: start..end,
                    class,
                });
            }
            start = end;
        }
        spans
    }
}

/// Nested capture patterns tokenize at most this deep.
const MAX_NESTING: usize = 6;
/// The most matches taken on one line, against runaway grammars.
const MAX_STEPS: usize = 4_000;

impl Registry {
    /// Tokenizes `line` (without its line terminator) starting in `state`. `first` is true for
    /// the first line of the document, where `\G` also matches at the start. Returns the spans
    /// and the state the next line starts in.
    pub fn tokenize_line(
        &self,
        base: usize,
        line: &str,
        state: &State,
        first: bool,
    ) -> (Vec<Span>, State) {
        let text = format!("{line}\n");
        let mut canvas = Canvas::new(line.len());
        let mut frames = state.frames.clone();
        let mut pos = 0;
        let mut anchor = first.then_some(0);
        // `while` rules stop applying on the first line their condition fails.
        let mut checked = 0;
        while checked < frames.len() {
            let rule = &self.rules[frames[checked].rule];
            if let Kind::BeginWhile {
                condition,
                while_captures,
                ..
            } = &rule.kind
            {
                match condition.find(&text, 0, Some(0)) {
                    Some(groups) => {
                        let scopes = self.scopes(&[], &frames[..=checked], true);
                        self.paint_match(
                            &mut canvas,
                            &text,
                            &groups,
                            &scopes,
                            None,
                            while_captures,
                            base,
                            0,
                        );
                        pos = pos.max(groups[0].map_or(0, |(_, end)| end));
                        anchor = Some(pos);
                    }
                    None => {
                        frames.truncate(checked);
                        break;
                    }
                }
            }
            checked += 1;
        }
        self.scan(
            base,
            &text,
            pos,
            text.len(),
            &mut frames,
            &[],
            anchor,
            &mut canvas,
            0,
        );
        (canvas.spans(), State { frames })
    }

    /// The scopes of a token inside `frames`: `prefix`, then the name of every open rule and the
    /// content name of every open rule but (unless `content_of_top`) the innermost.
    fn scopes<'a>(
        &'a self,
        prefix: &[&'a str],
        frames: &[Frame],
        content_of_top: bool,
    ) -> Vec<&'a str> {
        let mut scopes: Vec<&'a str> = prefix.to_vec();
        for (index, frame) in frames.iter().enumerate() {
            let rule = &self.rules[frame.rule];
            if let Some(name) = rule.name.as_deref() {
                scopes.push(name);
            }
            if content_of_top || index + 1 < frames.len() {
                if let Some(name) = rule.content_name.as_deref() {
                    scopes.push(name);
                }
            }
        }
        scopes
    }

    fn candidates(&self, base: usize, frames: &[Frame]) -> Vec<Candidate> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        match frames.last() {
            None => self.expand_root(base, base, &mut out, &mut seen),
            Some(frame) => {
                let rule = &self.rules[frame.rule];
                match &rule.kind {
                    Kind::BeginEnd {
                        patterns,
                        apply_end_last,
                        ..
                    } => {
                        if !apply_end_last {
                            out.push(Candidate::End);
                        }
                        self.expand(patterns, rule.grammar, base, &mut out, &mut seen);
                        if *apply_end_last {
                            out.push(Candidate::End);
                        }
                    }
                    Kind::BeginWhile { patterns, .. } | Kind::Container(patterns) => {
                        self.expand(patterns, rule.grammar, base, &mut out, &mut seen);
                    }
                    Kind::Match { .. } => {}
                }
            }
        }
        out
    }

    fn expand_root(
        &self,
        grammar: usize,
        base: usize,
        out: &mut Vec<Candidate>,
        seen: &mut HashSet<(bool, usize)>,
    ) {
        if seen.insert((true, grammar)) {
            self.expand(&self.grammars[grammar].root, grammar, base, out, seen);
        }
    }

    fn expand_rule(
        &self,
        id: usize,
        base: usize,
        out: &mut Vec<Candidate>,
        seen: &mut HashSet<(bool, usize)>,
    ) {
        if !seen.insert((false, id)) {
            return;
        }
        let rule = &self.rules[id];
        match &rule.kind {
            Kind::Container(patterns) => self.expand(patterns, rule.grammar, base, out, seen),
            _ => out.push(Candidate::Rule(id)),
        }
    }

    fn expand(
        &self,
        refs: &[RuleRef],
        grammar: usize,
        base: usize,
        out: &mut Vec<Candidate>,
        seen: &mut HashSet<(bool, usize)>,
    ) {
        for reference in refs {
            match reference {
                RuleRef::Rule(id) => self.expand_rule(*id, base, out, seen),
                RuleRef::Include(Include::This) => self.expand_root(grammar, base, out, seen),
                RuleRef::Include(Include::Base) => self.expand_root(base, base, out, seen),
                RuleRef::Include(Include::Repository(name)) => {
                    if let Some(id) = self.grammars[grammar].repository.get(name) {
                        self.expand_rule(*id, base, out, seen);
                    }
                }
                RuleRef::Include(Include::External { scope, name }) => {
                    let Some(target) = self.index_of(scope) else {
                        continue;
                    };
                    match name {
                        None => self.expand_root(target, base, out, seen),
                        Some(name) => {
                            if let Some(id) = self.grammars[target].repository.get(name) {
                                self.expand_rule(*id, base, out, seen);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Takes matches from `from` until none is left before `limit`, painting `canvas`.
    #[allow(clippy::too_many_arguments)]
    fn scan<'a>(
        &'a self,
        base: usize,
        text: &str,
        from: usize,
        limit: usize,
        frames: &mut Vec<Frame>,
        prefix: &[&'a str],
        anchor: Option<usize>,
        canvas: &mut Canvas,
        depth: usize,
    ) {
        let end_of_text = limit.min(text.len());
        let slice = &text[..end_of_text];
        let mut pos = from;
        let mut anchor = anchor;
        let mut cache: HashMap<usize, Option<Groups>> = HashMap::new();
        let mut stalled = 0;
        for _ in 0..MAX_STEPS {
            if pos >= end_of_text {
                break;
            }
            let candidates = self.candidates(base, frames);
            let mut best: Option<(Candidate, Groups)> = None;
            for candidate in candidates {
                let hit = match candidate {
                    Candidate::End => {
                        let Some(end) = frames.last().and_then(|frame| frame.end.as_deref()) else {
                            continue;
                        };
                        self.end_pattern(end).find(slice, pos, anchor)
                    }
                    Candidate::Rule(id) => {
                        let pattern = self.begin_pattern(id);
                        match (pattern.anchored, cache.get(&id)) {
                            (false, Some(cached))
                                if cached
                                    .as_ref()
                                    .is_none_or(|g| g[0].is_some_and(|(s, _)| s >= pos)) =>
                            {
                                cached.clone()
                            }
                            _ => {
                                let found = pattern.find(slice, pos, anchor);
                                if !pattern.anchored {
                                    cache.insert(id, found.clone());
                                }
                                found
                            }
                        }
                    }
                };
                let Some(groups) = hit else {
                    continue;
                };
                let Some((start, _)) = groups[0] else {
                    continue;
                };
                if best
                    .as_ref()
                    .is_none_or(|(_, g)| start < g[0].map_or(usize::MAX, |(s, _)| s))
                {
                    best = Some((candidate, groups));
                }
            }
            let Some((candidate, groups)) = best else {
                break;
            };
            let (start, end) = groups[0].unwrap_or((pos, pos));
            let plain = class_for_scopes(&self.scopes(prefix, frames, true));
            canvas.paint(pos, start, plain);
            match candidate {
                Candidate::End => {
                    let rule = &self.rules[frames.last().map_or(0, |frame| frame.rule)];
                    if let Kind::BeginEnd { end_captures, .. } = &rule.kind {
                        let scopes = self.scopes(prefix, frames, false);
                        self.paint_match(
                            canvas,
                            slice,
                            &groups,
                            &scopes,
                            None,
                            end_captures,
                            base,
                            depth,
                        );
                    }
                    frames.pop();
                }
                Candidate::Rule(id) => {
                    let rule = &self.rules[id];
                    let scopes = self.scopes(prefix, frames, true);
                    match &rule.kind {
                        Kind::Match { captures, .. } => {
                            self.paint_match(
                                canvas,
                                slice,
                                &groups,
                                &scopes,
                                rule.name.as_deref(),
                                captures,
                                base,
                                depth,
                            );
                        }
                        Kind::BeginEnd {
                            end,
                            begin_captures,
                            ..
                        } => {
                            self.paint_match(
                                canvas,
                                slice,
                                &groups,
                                &scopes,
                                rule.name.as_deref(),
                                begin_captures,
                                base,
                                depth,
                            );
                            frames.push(Frame {
                                rule: id,
                                end: Some(end.resolve(slice, &groups)),
                            });
                        }
                        Kind::BeginWhile { begin_captures, .. } => {
                            self.paint_match(
                                canvas,
                                slice,
                                &groups,
                                &scopes,
                                rule.name.as_deref(),
                                begin_captures,
                                base,
                                depth,
                            );
                            frames.push(Frame {
                                rule: id,
                                end: None,
                            });
                        }
                        Kind::Container(_) => {}
                    }
                }
            }
            anchor = Some(end);
            if end > pos {
                pos = end;
                stalled = 0;
            } else {
                // An empty match makes no progress: after a few, step over a character.
                stalled += 1;
                if stalled > 2
                    || matches!(candidate, Candidate::Rule(id) if matches!(self.rules[id].kind, Kind::Match { .. }))
                {
                    pos = next_boundary(slice, pos);
                    stalled = 0;
                }
            }
        }
        let plain = class_for_scopes(&self.scopes(prefix, frames, true));
        canvas.paint(pos.min(end_of_text), end_of_text, plain);
    }

    /// Paints a match: the rule's `name` over all of it, and the capture groups over their text,
    /// outer groups first so inner ones override. Groups with their own `patterns` are
    /// tokenized by them.
    #[allow(clippy::too_many_arguments)]
    fn paint_match<'a>(
        &'a self,
        canvas: &mut Canvas,
        text: &str,
        groups: &Groups,
        scopes: &[&'a str],
        name: Option<&'a str>,
        captures: &'a CaptureMap,
        base: usize,
        depth: usize,
    ) {
        let Some((start, end)) = groups[0] else {
            return;
        };
        let mut base_scopes: Vec<&'a str> = scopes.to_vec();
        if let Some(name) = name {
            base_scopes.push(name);
        }
        let mut items: Vec<(usize, usize, usize)> = captures
            .keys()
            .filter(|index| **index > 0)
            .filter_map(|index| {
                groups
                    .get(*index)
                    .copied()
                    .flatten()
                    .filter(|(a, b)| a < b && *a >= start && *b <= end)
                    .map(|(a, b)| (*index, a, b))
            })
            .collect();
        items.sort_by_key(|(index, a, b)| (*a, std::cmp::Reverse(*b), *index));
        let mut bounds: Vec<usize> = vec![start, end];
        for (_, a, b) in &items {
            bounds.push(*a);
            bounds.push(*b);
        }
        bounds.sort_unstable();
        bounds.dedup();
        for pair in bounds.windows(2) {
            let (x, y) = (pair[0], pair[1]);
            let mut scopes_here = base_scopes.clone();
            for (index, a, b) in &items {
                if *a <= x && y <= *b {
                    if let Some(name) = captures[index].name.as_deref() {
                        scopes_here.push(name);
                    }
                }
            }
            canvas.paint(x, y, class_for_scopes(&scopes_here));
        }
        if depth >= MAX_NESTING {
            return;
        }
        for (index, a, b) in &items {
            let Some(container) = captures[index].container else {
                continue;
            };
            let mut nested_prefix = base_scopes.clone();
            for (outer, oa, ob) in &items {
                if *oa <= *a && *b <= *ob {
                    if let Some(name) = captures[outer].name.as_deref() {
                        nested_prefix.push(name);
                    }
                }
            }
            let mut frames = vec![Frame {
                rule: container,
                end: None,
            }];
            self.scan(
                base,
                text,
                *a,
                *b,
                &mut frames,
                &nested_prefix,
                Some(*a),
                canvas,
                depth + 1,
            );
        }
    }
}

/// The next character boundary after `pos`.
fn next_boundary(text: &str, pos: usize) -> usize {
    let mut next = pos + 1;
    while next < text.len() && !text.is_char_boundary(next) {
        next += 1;
    }
    next
}
