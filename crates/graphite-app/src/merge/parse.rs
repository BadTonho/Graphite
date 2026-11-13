//! Parsing of conflict markers (`<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`).

/// One conflict: the text of each side and where its markers were.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConflictBlock {
    /// Text of the current side, every line with its own line ending.
    pub current: String,
    /// Text of the incoming side.
    pub incoming: String,
    /// Text of the common ancestor, present only when the markers carry a `|||||||` section.
    pub base: Option<String>,
    pub current_label: String,
    pub incoming_label: String,
    pub base_label: Option<String>,
    /// Line ending of the closing marker line; empty when the file ends right after it.
    pub end_eol: String,
    /// 0-based line of the opening marker in the parsed text.
    pub start_line: usize,
    /// The conflict as it was written, markers included.
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Segment {
    Text(String),
    Conflict(ConflictBlock),
}

/// A file split into plain text and conflict blocks, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub segments: Vec<Segment>,
}

impl Parsed {
    pub(crate) fn conflicts(&self) -> impl Iterator<Item = &ConflictBlock> {
        self.segments.iter().filter_map(|segment| match segment {
            Segment::Conflict(block) => Some(block),
            Segment::Text(_) => None,
        })
    }

    pub(crate) fn conflict_count(&self) -> usize {
        self.conflicts().count()
    }
}

/// What kind of marker a line is, with the length of its run of marker characters.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Marker {
    Start(usize),
    Base(usize),
    Middle(usize),
    End(usize),
}

fn split_eol(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix("\r\n") {
        (body, &line[body.len()..])
    } else if let Some(body) = line.strip_suffix('\n') {
        (body, &line[body.len()..])
    } else {
        (line, "")
    }
}

/// Classifies a line without its ending. A marker is a run of at least seven identical marker
/// characters; the three that carry a label need a space after the run, `=======` must stand
/// alone. Longer runs are accepted because Git can be told to use a different marker size.
fn marker_of(body: &str) -> Option<(Marker, &str)> {
    let first = body.chars().next()?;
    if !matches!(first, '<' | '|' | '=' | '>') {
        return None;
    }
    let run = body.chars().take_while(|&c| c == first).count();
    if run < 7 {
        return None;
    }
    let rest = &body[run..];
    match first {
        '=' => rest.trim().is_empty().then_some((Marker::Middle(run), "")),
        _ => {
            let label = if rest.is_empty() {
                ""
            } else if let Some(label) = rest.strip_prefix(' ') {
                label
            } else {
                return None;
            };
            let marker = match first {
                '<' => Marker::Start(run),
                '|' => Marker::Base(run),
                _ => Marker::End(run),
            };
            Some((marker, label.trim_end()))
        }
    }
}

#[derive(PartialEq, Eq)]
enum Part {
    Current,
    Base,
    Incoming,
}

struct Open {
    block: ConflictBlock,
    run: usize,
    part: Part,
}

/// Splits `text` into plain text and conflict blocks. A conflict that never closes, or whose
/// markers do not line up, is kept as plain text, so a half-written file is never damaged.
pub(crate) fn parse(text: &str) -> Parsed {
    let mut segments: Vec<Segment> = Vec::new();
    let mut plain = String::new();
    let mut open: Option<Open> = None;

    for (index, line) in text.split_inclusive('\n').enumerate() {
        let (body, eol) = split_eol(line);
        let marker = marker_of(body);
        // A new start marker inside an open conflict means the first one was never closed.
        if matches!(marker, Some((Marker::Start(_), _))) {
            if let Some(abandoned) = open.take() {
                plain.push_str(&abandoned.block.raw);
            }
        }
        let Some(current) = open.as_mut() else {
            if let Some((Marker::Start(run), label)) = marker {
                open = Some(Open {
                    block: ConflictBlock {
                        current: String::new(),
                        incoming: String::new(),
                        base: None,
                        current_label: label.to_owned(),
                        incoming_label: String::new(),
                        base_label: None,
                        end_eol: String::new(),
                        start_line: index,
                        raw: line.to_owned(),
                    },
                    run,
                    part: Part::Current,
                });
            } else {
                plain.push_str(line);
            }
            continue;
        };
        current.block.raw.push_str(line);
        match marker {
            Some((Marker::Base(run), label)) if run == current.run && current.part == Part::Current => {
                current.block.base = Some(String::new());
                current.block.base_label = Some(label.to_owned());
                current.part = Part::Base;
            }
            Some((Marker::Middle(run), _)) if run == current.run && current.part != Part::Incoming => {
                current.part = Part::Incoming;
            }
            Some((Marker::End(run), label)) if run == current.run && current.part == Part::Incoming => {
                if let Some(mut finished) = open.take() {
                    finished.block.incoming_label = label.to_owned();
                    finished.block.end_eol = eol.to_owned();
                    if !plain.is_empty() {
                        segments.push(Segment::Text(std::mem::take(&mut plain)));
                    }
                    segments.push(Segment::Conflict(finished.block));
                }
            }
            _ => match current.part {
                Part::Current => current.block.current.push_str(line),
                Part::Base => {
                    if let Some(base) = current.block.base.as_mut() {
                        base.push_str(line);
                    }
                }
                Part::Incoming => current.block.incoming.push_str(line),
            },
        }
    }
    if let Some(unfinished) = open {
        plain.push_str(&unfinished.block.raw);
    }
    if !plain.is_empty() {
        segments.push(Segment::Text(plain));
    }
    Parsed { segments }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(parsed: &Parsed, index: usize) -> &ConflictBlock {
        parsed.conflicts().nth(index).unwrap()
    }

    #[test]
    fn parses_two_way_conflict_with_labels() {
        let text = "a\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\nz\n";
        let parsed = parse(text);
        assert_eq!(parsed.segments.len(), 3);
        let conflict = block(&parsed, 0);
        assert_eq!(conflict.current, "ours\n");
        assert_eq!(conflict.incoming, "theirs\n");
        assert_eq!(conflict.base, None);
        assert_eq!(conflict.current_label, "HEAD");
        assert_eq!(conflict.incoming_label, "feature");
        assert_eq!(conflict.start_line, 1);
        assert_eq!(
            conflict.raw,
            "<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n"
        );
        assert_eq!(parsed.segments[0], Segment::Text("a\n".into()));
        assert_eq!(parsed.segments[2], Segment::Text("z\n".into()));
    }

    #[test]
    fn parses_base_section() {
        let text = "<<<<<<< a\none\n||||||| merged common ancestors\nbase\n=======\ntwo\n>>>>>>> b\n";
        let parsed = parse(text);
        let conflict = block(&parsed, 0);
        assert_eq!(conflict.base.as_deref(), Some("base\n"));
        assert_eq!(
            conflict.base_label.as_deref(),
            Some("merged common ancestors")
        );
        assert_eq!(conflict.current, "one\n");
    }

    #[test]
    fn several_conflicts_keep_their_order_and_the_text_between() {
        let text = "<<<<<<< a\n1\n=======\n2\n>>>>>>> b\nmid\n<<<<<<< a\n3\n=======\n4\n>>>>>>> b\n";
        let parsed = parse(text);
        assert_eq!(parsed.conflict_count(), 2);
        assert_eq!(parsed.segments[1], Segment::Text("mid\n".into()));
        assert_eq!(block(&parsed, 1).start_line, 6);
    }

    #[test]
    fn crlf_lines_are_kept_exactly() {
        let text = "x\r\n<<<<<<< a\r\nours\r\n=======\r\ntheirs\r\n>>>>>>> b\r\ny\r\n";
        let parsed = parse(text);
        let conflict = block(&parsed, 0);
        assert_eq!(conflict.current, "ours\r\n");
        assert_eq!(conflict.incoming, "theirs\r\n");
        assert_eq!(conflict.end_eol, "\r\n");
    }

    #[test]
    fn longer_markers_must_match_in_length() {
        let text = "<<<<<<<<<< a\nours\n==========\ntheirs\n>>>>>>>>>> b\n";
        assert_eq!(parse(text).conflict_count(), 1);
        // A seven-character separator inside a ten-character conflict is ordinary text.
        let text = "<<<<<<<<<< a\nours\n=======\nstill ours\n==========\ntheirs\n>>>>>>>>>> b\n";
        let parsed = parse(text);
        assert_eq!(block(&parsed, 0).current, "ours\n=======\nstill ours\n");
    }

    #[test]
    fn marker_lookalikes_are_plain_text() {
        for text in [
            "<<<<<< six\n",
            "<<<<<<<nospace\n=======\n>>>>>>> x\n",
            "======= trailing\nword\n",
            "text <<<<<<< inline\n",
            "=======x\n",
        ] {
            assert_eq!(parse(text).conflict_count(), 0, "{text:?}");
        }
        // A separator may carry trailing blanks.
        assert_eq!(
            parse("<<<<<<< a\n1\n=======  \n2\n>>>>>>> b\n").conflict_count(),
            1
        );
    }

    #[test]
    fn unterminated_or_misordered_conflicts_stay_text_and_round_trip() {
        for text in [
            "<<<<<<< a\nours\n=======\ntheirs\n",
            "<<<<<<< a\nours\n",
            "=======\n>>>>>>> b\n",
            "<<<<<<< a\n>>>>>>> b\n",
            "<<<<<<< a\nx\n>>>>>>> b\n",
        ] {
            let parsed = parse(text);
            assert_eq!(parsed.conflict_count(), 0, "{text:?}");
            assert_eq!(parsed.segments, vec![Segment::Text(text.into())]);
        }
    }

    #[test]
    fn a_restarted_conflict_keeps_the_abandoned_one_as_text() {
        let text = "<<<<<<< a\nlost\n<<<<<<< b\nours\n=======\ntheirs\n>>>>>>> c\n";
        let parsed = parse(text);
        assert_eq!(parsed.conflict_count(), 1);
        assert_eq!(
            parsed.segments[0],
            Segment::Text("<<<<<<< a\nlost\n".into())
        );
        assert_eq!(block(&parsed, 0).current, "ours\n");
    }

    #[test]
    fn closing_marker_without_a_final_newline_is_recorded() {
        let parsed = parse("<<<<<<< a\n1\n=======\n2\n>>>>>>> b");
        assert_eq!(block(&parsed, 0).end_eol, "");
    }

    #[test]
    fn empty_sides_are_allowed() {
        let parsed = parse("<<<<<<< a\n=======\nnew\n>>>>>>> b\n");
        assert_eq!(block(&parsed, 0).current, "");
        assert_eq!(block(&parsed, 0).incoming, "new\n");
    }

    #[test]
    fn text_without_conflicts_is_one_segment_and_empty_text_has_none() {
        assert_eq!(parse("plain\n").segments, vec![Segment::Text("plain\n".into())]);
        assert!(parse("").segments.is_empty());
    }
}
