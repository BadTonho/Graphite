use super::*;
use graphite_text::Indentation;

fn app_with(text: &str) -> (App, crate::test_support::TempDir) {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.editor_mut().insert(text);
    (app, dir)
}

#[test]
fn case_commands_change_the_word_at_the_cursor() {
    let cases = [
        ("editor.action.transformToUppercase", "FOO_BAR baz"),
        ("editor.action.transformToLowercase", "foo_bar baz"),
        ("editor.action.transformToSnakecase", "foo_bar baz"),
        ("editor.action.transformToCamelcase", "fooBar baz"),
        ("editor.action.transformToPascalcase", "FooBar baz"),
        ("editor.action.transformToKebabcase", "foo-bar baz"),
        ("editor.action.transformToTitlecase", "Foo_Bar baz"),
    ];
    for (command, expected) in cases {
        let (mut app, _dir) = app_with("Foo_Bar baz");
        app.editor_mut().set_selection(2, 2);
        app.run_ui_command(command);
        assert_eq!(app.editor().text(), expected, "{command}");
        if expected != "Foo_Bar baz" {
            app.run_ui_command("undo");
            assert_eq!(app.editor().text(), "Foo_Bar baz", "undo {command}");
        }
    }
}

#[test]
fn line_commands_run_from_the_command_registry() {
    let (mut app, _dir) = app_with("b\nc\na");
    app.editor_mut().set_selection(0, 0);
    app.run_ui_command("editor.action.sortLinesDescending");
    assert_eq!(app.editor().text(), "c\nb\na");
    app.run_ui_command("editor.action.reverseLines");
    assert_eq!(app.editor().text(), "a\nb\nc");

    app.editor_mut().set_selection(1, 1);
    app.run_ui_command("editor.action.transpose");
    // The cursor is at the end of "a", so it swaps with the line break.
    assert_eq!(app.editor().text(), "\nab\nc");
}

#[test]
fn indent_commands_follow_the_document_indentation_and_default_keys() {
    let (mut app, _dir) = app_with("a\nb");
    let document = app.command_document();
    app.documents[document].indentation = Indentation {
        insert_spaces: true,
        tab_size: 2,
    };
    app.editor_mut().set_selection(0, 3);
    assert!(app.handle_keybinding_stroke("ctrl+]"));
    assert_eq!(app.editor().text(), "  a\n  b");
    assert!(app.handle_keybinding_stroke("ctrl+["));
    assert_eq!(app.editor().text(), "a\nb");

    app.documents[document].indentation = Indentation {
        insert_spaces: false,
        tab_size: 4,
    };
    app.run_ui_command("editor.action.indentLines");
    assert_eq!(app.editor().text(), "\ta\n\tb");
}
