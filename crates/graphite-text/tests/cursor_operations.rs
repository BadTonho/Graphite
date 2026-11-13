use graphite_text::{Buffer, Selection, SelectionState, ViewId};

fn with_editor(text: &str, body: impl FnOnce(&mut graphite_text::Editor<'_>)) {
    let mut document = Buffer::new();
    let mut view = SelectionState::default();
    let mut editor = document.edit(&mut view, ViewId(1));
    editor.insert(text);
    body(&mut editor);
}

fn cursor(at: usize) -> Selection {
    Selection {
        anchor: at,
        head: at,
    }
}

fn range(anchor: usize, head: usize) -> Selection {
    Selection { anchor, head }
}

#[test]
fn cursors_to_the_bottom_keep_the_column_and_stop_at_short_lines() {
    with_editor("abc\nd\nfghi", |e| {
        e.set_selection(2, 2);
        e.add_cursors_to_edge(true);
        // Line 1 ("d") is shorter than column 2, so its cursor sits at its end.
        assert_eq!(e.selections(), vec![cursor(2), cursor(5), cursor(8)]);
    });
}

#[test]
fn cursors_to_the_top_cover_every_line_above() {
    with_editor("abc\nde\nfghi", |e| {
        e.set_selection(e.line_start_char(2) + 1, e.line_start_char(2) + 1);
        e.add_cursors_to_edge(false);
        assert_eq!(e.selections(), vec![cursor(1), cursor(5), cursor(8)]);
    });
}

#[test]
fn cursors_to_the_edge_copy_the_selected_columns() {
    with_editor("abcdef\nghijkl\nmn", |e| {
        e.set_selection(1, 3);
        e.add_cursors_to_edge(true);
        assert_eq!(
            e.selections(),
            vec![range(1, 3), range(8, 10), range(15, 16)]
        );
    });
}

#[test]
fn focusing_rotates_the_primary_cursor_without_moving_any_cursor() {
    with_editor("a\nb\nc", |e| {
        e.set_selections(vec![cursor(0), cursor(2), cursor(4)]);
        e.focus_cursor(true);
        assert_eq!(e.selections(), vec![cursor(2), cursor(4), cursor(0)]);
        assert_eq!(e.cursor().char_idx, 2);
        e.focus_cursor(false);
        e.focus_cursor(false);
        assert_eq!(e.selections(), vec![cursor(4), cursor(0), cursor(2)]);
        assert_eq!(e.cursor().char_idx, 4);
    });
}

#[test]
fn focusing_with_one_cursor_does_nothing() {
    with_editor("abc", |e| {
        e.set_selection(1, 1);
        e.focus_cursor(true);
        assert_eq!(e.selections(), vec![cursor(1)]);
    });
}

#[test]
fn moving_the_last_selection_walks_through_the_matches_and_wraps() {
    with_editor("foo bar foo baz foo", |e| {
        e.set_selection(0, 3);
        e.move_selection_to_occurrence(true);
        assert_eq!(e.selections(), vec![range(8, 11)]);
        e.move_selection_to_occurrence(true);
        assert_eq!(e.selections(), vec![range(16, 19)]);
        e.move_selection_to_occurrence(true);
        assert_eq!(
            e.selections(),
            vec![range(0, 3)],
            "after the last match it wraps"
        );
    });
}

#[test]
fn moving_back_keeps_the_other_selections() {
    with_editor("foo bar foo baz foo", |e| {
        // The first selection moves back to the match before it; the second stays.
        e.set_selections(vec![range(8, 11), range(16, 19)]);
        e.move_selection_to_occurrence(false);
        assert_eq!(e.selections(), vec![range(0, 3), range(16, 19)]);
    });
}

#[test]
fn moving_a_selection_without_another_match_changes_nothing() {
    with_editor("only once", |e| {
        e.set_selection(0, 4);
        e.move_selection_to_occurrence(true);
        assert_eq!(e.selections(), vec![range(0, 4)]);
    });
}
