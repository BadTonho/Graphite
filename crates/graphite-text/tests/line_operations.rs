use graphite_text::{Buffer, CaseTransform, Indentation, Selection, SelectionState, ViewId};

/// Runs `body` with an editor holding `text`.
fn with_editor(text: &str, body: impl FnOnce(&mut graphite_text::Editor<'_>)) {
    let mut document = Buffer::new();
    let mut view = SelectionState::default();
    let mut editor = document.edit(&mut view, ViewId(1));
    editor.insert(text);
    body(&mut editor);
}

fn spaces(size: usize) -> Indentation {
    Indentation {
        insert_spaces: true,
        tab_size: size,
    }
}

const TABS: Indentation = Indentation {
    insert_spaces: false,
    tab_size: 4,
};

#[test]
fn descending_sort_orders_the_selected_lines_in_one_undo_step() {
    with_editor("keep\nb\nc\na\nend", |e| {
        e.set_selection(e.line_start_char(1), e.line_start_char(3) + 1);
        e.sort_lines_ordered(true);
        assert_eq!(e.text(), "keep\nc\nb\na\nend");
        e.undo();
        assert_eq!(e.text(), "keep\nb\nc\na\nend");
    });
}

#[test]
fn a_lone_cursor_sorts_the_whole_document_and_keeps_the_final_line_break() {
    with_editor("b\r\nc\r\na\r\n", |e| {
        e.set_selection(0, 0);
        e.sort_lines_ordered(false);
        assert_eq!(e.text(), "a\r\nb\r\nc\r\n");
        e.sort_lines_ordered(true);
        assert_eq!(e.text(), "c\r\nb\r\na\r\n");
    });
}

#[test]
fn sorting_lines_that_are_already_in_order_changes_nothing() {
    with_editor("a\nb\nc", |e| {
        e.set_selection(0, 0);
        let revision = e.revision();
        e.sort_lines_ordered(false);
        assert_eq!(e.revision(), revision);
    });
}

#[test]
fn reverse_lines_flips_the_selection_and_keeps_line_breaks() {
    with_editor("one\ntwo\nthree\nfour", |e| {
        e.set_selection(e.line_start_char(0), e.line_start_char(2) + 2);
        e.reverse_lines();
        assert_eq!(e.text(), "three\ntwo\none\nfour");
        e.undo();
        assert_eq!(e.text(), "one\ntwo\nthree\nfour");
    });
}

#[test]
fn reverse_lines_on_a_cursor_reverses_the_document_without_the_trailing_empty_line() {
    with_editor("a\nb\nc\n", |e| {
        e.set_selection(0, 0);
        e.reverse_lines();
        assert_eq!(e.text(), "c\nb\na\n");
    });
}

#[test]
fn reverse_lines_moves_each_selection_end_with_its_line() {
    with_editor(
        "ab
cdef
gh",
        |e| {
            // From line 1, column 3 to the end of line 2.
            e.set_selection(e.line_start_char(1) + 3, e.text().chars().count());
            e.reverse_lines();
            assert_eq!(
                e.text(),
                "ab
gh
cdef"
            );
            // The start followed "cdef" to line 2; the end followed "gh" to line 1.
            assert_eq!(
                e.selections(),
                vec![Selection {
                    anchor: e.line_start_char(2) + 3,
                    head: e.line_start_char(1) + 2,
                }]
            );
        },
    );
}

#[test]
fn indent_moves_each_line_to_the_next_indent_stop() {
    with_editor("a\n  b\n\n\tc", |e| {
        e.set_selection(0, e.text().chars().count());
        e.indent_lines(spaces(4));
        // The empty line is skipped in a multi-line selection; the tab counts as four columns.
        assert_eq!(e.text(), "    a\n    b\n\n        c");
        e.undo();
        assert_eq!(e.text(), "a\n  b\n\n\tc");
    });
}

#[test]
fn indent_with_tabs_adds_a_tab_per_stop() {
    with_editor("a\n  b", |e| {
        e.set_selection(0, e.text().chars().count());
        e.indent_lines(TABS);
        assert_eq!(e.text(), "\ta\n\tb");
    });
}

#[test]
fn indenting_an_empty_line_puts_the_cursor_after_the_indent() {
    with_editor("x\n\ny", |e| {
        let at = e.line_start_char(1);
        e.set_selection(at, at);
        e.indent_lines(spaces(2));
        assert_eq!(e.text(), "x\n  \ny");
        assert_eq!(e.selections()[0].head, at + 2);
    });
}

#[test]
fn outdent_removes_one_level_and_ignores_lines_without_indent() {
    with_editor("    a\n      b\n\tc\nd", |e| {
        e.set_selection(0, e.text().chars().count());
        e.outdent_lines(spaces(4));
        assert_eq!(e.text(), "a\n    b\nc\nd");
    });
    with_editor("d", |e| {
        let revision = e.revision();
        e.outdent_lines(spaces(4));
        assert_eq!(e.revision(), revision);
    });
}

#[test]
fn transpose_swaps_the_characters_around_the_cursor() {
    with_editor("abc", |e| {
        e.set_selection(1, 1);
        e.transpose_characters();
        assert_eq!(e.text(), "bac");
        assert_eq!(e.selections(), vec![Selection { anchor: 2, head: 2 }]);
    });
}

#[test]
fn transpose_at_a_line_end_moves_the_last_character_to_the_next_line() {
    with_editor("ab\ncd", |e| {
        e.set_selection(2, 2);
        e.transpose_characters();
        assert_eq!(e.text(), "a\nbcd");
    });
}

#[test]
fn transpose_does_nothing_at_the_start_of_a_line_or_the_end_of_the_text() {
    with_editor("ab\ncd", |e| {
        let revision = e.revision();
        e.set_selection(3, 3);
        e.transpose_characters();
        e.set_selection(5, 5);
        e.transpose_characters();
        assert_eq!(e.revision(), revision);
    });
}

#[test]
fn case_transforms_apply_to_the_word_at_the_cursor() {
    with_editor("let helloWorld = 1;", |e| {
        e.set_selection(7, 7);
        e.transform_case(CaseTransform::Snake);
        assert_eq!(e.text(), "let hello_world = 1;");
        e.transform_case(CaseTransform::Upper);
        assert_eq!(e.text(), "let HELLO_WORLD = 1;");
        e.transform_case(CaseTransform::Pascal);
        assert_eq!(e.text(), "let HelloWorld = 1;");
    });
}

#[test]
fn case_transforms_apply_to_a_selection_and_keep_it_selected() {
    with_editor("some text here", |e| {
        e.set_selection(5, 9);
        e.transform_case(CaseTransform::Upper);
        assert_eq!(e.text(), "some TEXT here");
        assert_eq!(e.selections(), vec![Selection { anchor: 5, head: 9 }]);
    });
}

#[test]
fn two_cursors_in_one_word_change_it_once_and_a_cursor_between_words_does_nothing() {
    with_editor("ab  cd", |e| {
        e.set_selections(vec![
            Selection { anchor: 0, head: 0 },
            Selection { anchor: 1, head: 1 },
        ]);
        e.transform_case(CaseTransform::Upper);
        assert_eq!(e.text(), "AB  cd");
        let revision = e.revision();
        e.set_selection(3, 3);
        e.transform_case(CaseTransform::Upper);
        assert_eq!(e.revision(), revision);
    });
}

#[test]
fn read_only_editors_ignore_line_and_case_commands() {
    let mut document = Buffer::from_text("x.txt", "b\na");
    document.set_read_only(true);
    let mut view = SelectionState::default();
    let mut editor = document.edit(&mut view, ViewId(1));
    editor.sort_lines_ordered(false);
    editor.reverse_lines();
    editor.indent_lines(spaces(4));
    editor.transpose_characters();
    editor.transform_case(CaseTransform::Upper);
    assert_eq!(editor.text(), "b\na");
}

#[test]
fn overtype_overwrites_until_line_end_and_preserves_paste_insert() {
    with_editor("hello\r\nworld", |e| {
        e.set_cursor_line_col(0, 1);
        e.insert_overtype("a");
        e.insert_overtype("x");
        assert_eq!(e.text(), "haxlo\r\nworld");

        // At line end before CRLF, overtype inserts without eating \r\n.
        e.set_cursor_line_col(0, 5);
        e.insert_overtype("!");
        assert_eq!(e.text(), "haxlo!\r\nworld");

        // Selection is replaced normally even in overtype mode.
        e.set_selection(0, 3);
        e.insert_overtype("Z");
        assert_eq!(e.text(), "Zlo!\r\nworld");

        // Multi-cursor overtype works across cursors.
        e.set_selections(vec![
            Selection { anchor: 0, head: 0 },
            Selection { anchor: 6, head: 6 },
        ]);
        e.insert_overtype("X");
        assert_eq!(e.text(), "Xlo!\r\nXorld");
    });
}

#[test]
fn reindent_lines_and_selected_lines_follow_brackets_and_undo_together() {
    with_editor(
        "fn main() {\nlet x = 1;\n   if true {\nprintln!(\"{\");\n    }\n}\n",
        |e| {
            let changed = e.reindent_lines(spaces(4), false).unwrap();
            assert!(changed > 0);
            assert_eq!(
                e.text(),
                "fn main() {\n    let x = 1;\n    if true {\n        println!(\"{\");\n    }\n}\n"
            );
            e.undo();
            assert_eq!(
                e.text(),
                "fn main() {\nlet x = 1;\n   if true {\nprintln!(\"{\");\n    }\n}\n"
            );

            // Reindent only lines 2..=3 while still respecting outer depth from line 0.
            let start = e.rope().line_to_char(2);
            let end = e.rope().line_to_char(4);
            e.set_selection(start, end);
            e.reindent_selected_lines(spaces(4), false).unwrap();
            assert_eq!(
                e.text(),
                "fn main() {\nlet x = 1;\n    if true {\n        println!(\"{\");\n    }\n}\n"
            );
        },
    );

    with_editor("def f():\nx = 1\n", |e| {
        e.reindent_lines(spaces(4), true).unwrap();
        assert_eq!(e.text(), "def f():\n    x = 1\n");
    });
}
