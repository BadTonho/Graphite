use super::command_execution::{CommandError, CommandRequest};
use super::*;

const TWO: &str = "top\n<<<<<<< HEAD\nours1\n=======\ntheirs1\n>>>>>>> topic\nmid\n<<<<<<< HEAD\nours2\n=======\ntheirs2\n>>>>>>> topic\nbottom\n";

fn open_text(text: &str) -> (crate::test_support::TempDir, App, PathBuf) {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("merged.txt");
    std::fs::write(&path, text).unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_merge_editor(path.clone()).unwrap();
    (dir, app, path)
}

fn complete(app: &mut App) -> Result<(), CommandError> {
    let target = app.ui_target_for_command("mergeEditor.acceptMerge");
    app.refresh_command_context();
    app.execute_command(CommandRequest {
        id: "mergeEditor.acceptMerge".into(),
        arguments: serde_json::Value::Null,
        target,
    })
    .try_result()
    .unwrap()
    .map(|_| ())
}

#[test]
fn completing_is_refused_while_conflicts_are_unresolved_and_the_file_is_untouched() {
    let (_dir, mut app, path) = open_text(TWO);
    app.run_ui_command("graphite.merge.acceptCurrent");
    let error = complete(&mut app).unwrap_err();
    assert!(app.command_error_text(&error).contains('1'));
    assert!(app.merge_editor.is_some());
    assert_eq!(std::fs::read_to_string(path).unwrap(), TWO);
}

#[test]
fn accept_all_then_complete_writes_the_result_and_closes_the_editor() {
    for (command, expected) in [
        ("merge.acceptAllInput1", "top\nours1\nmid\nours2\nbottom\n"),
        (
            "merge.acceptAllInput2",
            "top\ntheirs1\nmid\ntheirs2\nbottom\n",
        ),
        (
            "mergeEditor.acceptAllCombination",
            "top\nours1\ntheirs1\nmid\nours2\ntheirs2\nbottom\n",
        ),
    ] {
        let (_dir, mut app, path) = open_text(TWO);
        app.run_ui_command(command);
        assert!(app.merge_editor.as_ref().unwrap().state.is_complete());
        complete(&mut app).unwrap();
        assert!(app.merge_editor.is_none());
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            expected,
            "{command}"
        );
    }
}

#[test]
fn reset_forgets_every_decision() {
    let (_dir, mut app, _) = open_text(TWO);
    app.run_ui_command("merge.acceptAllInput1");
    app.run_ui_command("mergeEditor.resetResultToBaseAndAutoMerge");
    let editor = app.merge_editor.as_ref().unwrap();
    assert_eq!(editor.state.unresolved_count(), 2);
    assert_eq!(editor.active, Some(0));
    assert_eq!(editor.state.render(), TWO);
}

#[test]
fn closing_with_decisions_warns_once_and_decisions_reset_the_warning() {
    let (_dir, mut app, _) = open_text(TWO);
    // Nothing decided: closes at once.
    app.run_ui_command("graphite.merge.close");
    assert!(app.merge_editor.is_none());

    let (_dir, mut app, path) = open_text(TWO);
    app.run_ui_command("graphite.merge.acceptCurrent");
    app.run_ui_command("graphite.merge.close");
    assert!(app.merge_editor.is_some());
    assert_eq!(
        app.status,
        app.tf("merge.close_warning", &[("count", &1usize)])
    );
    // A new decision, or resetting the warning, arms it again.
    app.run_ui_command("mergeEditor.resetCloseWithConflictsChoice");
    app.run_ui_command("graphite.merge.close");
    assert!(app.merge_editor.is_some());
    app.run_ui_command("graphite.merge.close");
    assert!(app.merge_editor.is_none());
    // Discarding wrote nothing.
    assert_eq!(std::fs::read_to_string(path).unwrap(), TWO);
}

#[test]
fn the_file_keeps_its_line_endings_and_encoding() {
    let crlf = TWO.replace('\n', "\r\n");
    let (_dir, mut app, path) = open_text(&crlf);
    app.run_ui_command("merge.acceptAllInput2");
    complete(&mut app).unwrap();
    assert_eq!(
        std::fs::read(path).unwrap(),
        "top\r\ntheirs1\r\nmid\r\ntheirs2\r\nbottom\r\n".as_bytes()
    );

    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("bom.txt");
    std::fs::write(&path, [b"\xef\xbb\xbf".as_slice(), TWO.as_bytes()].concat()).unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_merge_editor(path.clone()).unwrap();
    app.run_ui_command("merge.acceptAllInput1");
    complete(&mut app).unwrap();
    assert!(std::fs::read(path)
        .unwrap()
        .starts_with(b"\xef\xbb\xbftop\n"));
}

#[test]
fn unsaved_edits_of_the_open_file_block_completion() {
    let (_dir, mut app, path) = open_text(TWO);
    app.open_file(path.clone());
    app.editor_mut().insert("x");
    app.run_ui_command("merge.acceptAllInput1");
    let error = complete(&mut app).unwrap_err();
    assert_eq!(app.command_error_text(&error), app.t("merge.error.unsaved"));
    assert!(app.merge_editor.is_some());
    assert_eq!(std::fs::read_to_string(path).unwrap(), TWO);
}

#[test]
fn completing_a_conflicted_file_stages_it() {
    let dir = crate::test_support::TempDir::new();
    gix::init(dir.path()).unwrap();
    let path = dir.path().join("merged.txt");
    std::fs::write(&path, TWO).unwrap();
    crate::scm::mark_conflicted_for_tests(dir.path(), "merged.txt", "base\n", "ours\n", "theirs\n");
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.refresh_scm_async();
    assert!(app.scm_snapshot.files.iter().any(|file| file.conflicted));
    app.open_merge_editor(path.clone()).unwrap();
    app.run_ui_command("merge.acceptAllInput2");
    complete(&mut app).unwrap();
    assert_eq!(app.status, app.t("scm.conflict.resolved_staged"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "top\ntheirs1\nmid\ntheirs2\nbottom\n"
    );
    assert!(!app.scm_snapshot.files.iter().any(|file| file.conflicted));
    assert!(crate::scm::merge_conflict(dir.path(), &path, &app.translator).is_err());
}
