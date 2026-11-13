use graphite_text::{next_value, Buffer, Selection, SelectionState, ViewId};

fn with_editor(text: &str, body: impl FnOnce(&mut graphite_text::Editor<'_>)) {
    let mut document = Buffer::new();
    let mut view = SelectionState::default();
    let mut editor = document.edit(&mut view, ViewId(1));
    editor.insert(text);
    body(&mut editor);
}

fn sel(anchor: usize, head: usize) -> Selection {
    Selection { anchor, head }
}

#[test]
fn numbers_step_by_one_in_their_last_decimal_place() {
    assert_eq!(next_value("5", true).as_deref(), Some("6"));
    assert_eq!(next_value("5", false).as_deref(), Some("4"));
    assert_eq!(next_value("1.5", true).as_deref(), Some("2.5"));
    assert_eq!(next_value("1.5", false).as_deref(), Some("0.5"));
    assert_eq!(next_value("1.50", true).as_deref(), Some("2.5"));
    assert_eq!(next_value("9", true).as_deref(), Some("10"));
    assert_eq!(next_value("0", false), None, "numbers do not go negative");
    assert_eq!(next_value("+5", true), None);
}

#[test]
fn words_cycle_through_their_value_set() {
    assert_eq!(next_value("true", true).as_deref(), Some("false"));
    assert_eq!(next_value("true", false).as_deref(), Some("false"));
    assert_eq!(next_value("False", true).as_deref(), Some("True"));
    assert_eq!(next_value("public", true).as_deref(), Some("protected"));
    assert_eq!(next_value("public", false).as_deref(), Some("private"));
    assert_eq!(next_value("private", true).as_deref(), Some("public"));
    assert_eq!(next_value("maybe", true), None);
}

#[test]
fn the_anchor_follows_edits_and_selects_back_to_the_cursor() {
    with_editor("hello world", |e| {
        e.set_selection(6, 6);
        e.set_selection_anchor();
        e.set_selection(0, 0);
        e.insert("XX");
        assert_eq!(
            e.selection_anchor(),
            Some(8),
            "the anchor moved with the text"
        );
        e.set_selection(e.len_chars(), e.len_chars());
        assert!(e.go_to_selection_anchor());
        assert_eq!(e.selections(), vec![sel(8, 8)]);
        e.set_selection(11, 11);
        assert!(e.select_from_selection_anchor());
        assert_eq!(e.selections(), vec![sel(8, 11)]);
        assert_eq!(e.selection_anchor(), None);
        assert!(!e.go_to_selection_anchor());
        assert!(!e.select_from_selection_anchor());
    });
}

#[test]
fn clearing_the_anchor_forgets_it() {
    with_editor("abc", |e| {
        e.set_selection_anchor();
        e.clear_selection_anchor();
        assert_eq!(e.selection_anchor(), None);
    });
}

#[test]
fn jumping_to_a_bracket_goes_to_its_pair_the_closing_bracket_or_the_next_one() {
    // 0 f, 1 (, 2 a, 3 ",", 4 " ", 5 (, 6 b, 7 ), 8 )
    with_editor("f(a, (b))", |e| {
        e.set_selection(1, 1);
        e.jump_to_bracket();
        assert_eq!(
            e.selections(),
            vec![sel(8, 8)],
            "from an opening bracket to its pair"
        );
        e.jump_to_bracket();
        assert_eq!(e.selections(), vec![sel(1, 1)], "and back");
        e.set_selection(3, 3);
        e.jump_to_bracket();
        assert_eq!(
            e.selections(),
            vec![sel(8, 8)],
            "inside: the closing bracket around"
        );
        e.set_selection(0, 0);
        e.jump_to_bracket();
        assert_eq!(e.selections(), vec![sel(1, 1)], "outside: the next bracket");
    });
    with_editor("no brackets", |e| {
        e.set_selection(3, 3);
        e.jump_to_bracket();
        assert_eq!(e.selections(), vec![sel(3, 3)]);
    });
}

#[test]
fn selecting_to_a_bracket_covers_the_pair_or_its_contents() {
    with_editor("f(a, (b))", |e| {
        e.set_selection(3, 3);
        e.select_to_bracket(true);
        assert_eq!(e.selections(), vec![sel(1, 9)]);
        e.set_selection(3, 3);
        e.select_to_bracket(false);
        assert_eq!(e.selections(), vec![sel(2, 8)]);
        e.set_selection(0, 0);
        e.select_to_bracket(true);
        assert_eq!(e.selections(), vec![sel(1, 9)], "outside: the next pair");
        // On the closing bracket the selection runs backwards.
        e.set_selection(8, 8);
        e.select_to_bracket(true);
        assert_eq!(e.selections(), vec![sel(9, 1)]);
        assert_eq!(e.cursor().char_idx, 1);
    });
}

#[test]
fn removing_brackets_deletes_the_pair_around_the_cursor_in_one_undo_step() {
    with_editor("f(a, (b))", |e| {
        e.set_selection(6, 6);
        e.remove_brackets();
        assert_eq!(e.text(), "f(a, b)");
        e.undo();
        assert_eq!(e.text(), "f(a, (b))");
    });
}

#[test]
fn removing_brackets_does_nothing_outside_any_pair() {
    with_editor("plain (text)", |e| {
        e.set_selection(2, 2);
        let revision = e.revision();
        e.remove_brackets();
        assert_eq!(e.revision(), revision);
    });
}

#[test]
fn selected_text_moves_left_and_right_over_one_character() {
    with_editor("abcd", |e| {
        e.set_selection(1, 3);
        e.move_selected_text(true);
        assert_eq!(e.text(), "bcad");
        assert_eq!(e.selections(), vec![sel(0, 2)]);
        e.move_selected_text(false);
        assert_eq!(e.text(), "abcd");
        assert_eq!(e.selections(), vec![sel(1, 3)]);
    });
}

#[test]
fn selected_text_stops_at_the_ends_of_its_line() {
    with_editor("ab\ncd", |e| {
        e.set_selection(0, 2);
        e.move_selected_text(true);
        assert_eq!(e.text(), "ab\ncd", "already at the line start");
        e.move_selected_text(false);
        assert_eq!(e.text(), "ab\ncd", "already at the line end");
        e.set_selection(1, 4);
        e.move_selected_text(true);
        assert_eq!(
            e.text(),
            "ab\ncd",
            "a selection over two lines does not move"
        );
        e.set_selection(1, 1);
        let revision = e.revision();
        e.move_selected_text(true);
        assert_eq!(e.revision(), revision, "a cursor has nothing to move");
    });
}

#[test]
fn letters_around_the_cursor_swap() {
    with_editor("abc", |e| {
        e.set_selection(1, 1);
        e.transpose_letters();
        assert_eq!(e.text(), "bac");
        assert_eq!(e.selections(), vec![sel(2, 2)]);
    });
    with_editor("abc", |e| {
        e.set_selection(3, 3);
        e.transpose_letters();
        assert_eq!(e.text(), "acb", "at the end of a line the last two swap");
    });
}

#[test]
fn letters_do_not_swap_across_a_line_break_or_at_a_line_start() {
    with_editor("ab\ncd", |e| {
        let revision = e.revision();
        e.set_selection(0, 0);
        e.transpose_letters();
        e.set_selection(3, 3);
        e.transpose_letters();
        assert_eq!(e.revision(), revision);
    });
}

#[test]
fn replacing_a_value_selects_the_new_text() {
    with_editor("x = 5;", |e| {
        e.set_selection(4, 4);
        e.replace_with_next_value(true);
        assert_eq!(e.text(), "x = 6;");
        assert_eq!(e.selections(), vec![sel(4, 5)]);
        e.replace_with_next_value(false);
        assert_eq!(e.text(), "x = 5;");
    });
    with_editor("flag = true", |e| {
        e.set_selection(8, 8);
        e.replace_with_next_value(true);
        assert_eq!(e.text(), "flag = false");
        assert_eq!(e.selections(), vec![sel(7, 12)]);
    });
}

#[test]
fn replacing_uses_the_selection_and_skips_unknown_values() {
    with_editor("a 1.5 b", |e| {
        e.set_selection(2, 5);
        e.replace_with_next_value(true);
        assert_eq!(e.text(), "a 2.5 b");
    });
    with_editor("nothing here", |e| {
        e.set_selection(2, 2);
        let revision = e.revision();
        e.replace_with_next_value(true);
        assert_eq!(e.revision(), revision);
    });
}
