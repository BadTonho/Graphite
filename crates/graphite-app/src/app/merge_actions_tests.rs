use super::*;

const TWO: &str = "top\n<<<<<<< HEAD\nours1\n||||||| base\nold1\n=======\ntheirs1\n>>>>>>> topic\nmid\n<<<<<<< HEAD\nours2\n=======\ntheirs2\n>>>>>>> topic\nbottom\n";

fn open() -> (crate::test_support::TempDir, App, PathBuf) {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("merged.txt");
    std::fs::write(&path, TWO).unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_merge_editor(path.clone()).unwrap();
    (dir, app, path)
}

fn rendered(app: &App) -> String {
    app.merge_editor.as_ref().unwrap().state.render()
}

fn active(app: &App) -> Option<usize> {
    app.merge_editor.as_ref().unwrap().active
}

#[test]
fn each_decision_changes_only_the_active_conflict_and_moves_on() {
    let (_dir, mut app, _) = open();
    app.run_ui_command("graphite.merge.acceptCurrent");
    assert_eq!(
        rendered(&app),
        "top\nours1\nmid\n<<<<<<< HEAD\nours2\n=======\ntheirs2\n>>>>>>> topic\nbottom\n"
    );
    assert_eq!(active(&app), Some(1));
    app.run_ui_command("graphite.merge.acceptIncoming");
    assert_eq!(rendered(&app), "top\nours1\nmid\ntheirs2\nbottom\n");
    // Everything is decided, so the active conflict stays where it was.
    assert_eq!(active(&app), Some(1));
    assert!(app.merge_editor.as_ref().unwrap().state.is_complete());
}

#[test]
fn accept_both_orders_and_ignore_and_revert() {
    let (_dir, mut app, _) = open();
    app.run_ui_command("graphite.merge.acceptBoth");
    assert!(rendered(&app).contains("top\nours1\ntheirs1\nmid\n"));
    app.run_ui_command("merge.goToPreviousUnhandledConflict");
    app.run_ui_command("merge.goToNextUnhandledConflict");
    assert_eq!(active(&app), Some(1));
    app.run_ui_command("graphite.merge.acceptBothIncomingFirst");
    assert!(rendered(&app).contains("mid\ntheirs2\nours2\nbottom\n"));
    // Ignore uses the base when there is one, otherwise nothing.
    app.merge_editor.as_mut().unwrap().select(0);
    app.run_ui_command("graphite.merge.ignore");
    assert!(rendered(&app).starts_with("top\nold1\nmid\n"));
    app.merge_editor.as_mut().unwrap().select(1);
    app.run_ui_command("graphite.merge.ignore");
    assert!(rendered(&app).ends_with("mid\nbottom\n"));
    app.merge_editor.as_mut().unwrap().select(0);
    app.run_ui_command("graphite.merge.revert");
    assert!(rendered(&app).starts_with("top\n<<<<<<< HEAD\nours1\n"));
    assert_eq!(app.merge_editor.as_ref().unwrap().state.unresolved_count(), 1);
}

#[test]
fn toggling_inputs_builds_the_combination_one_side_at_a_time() {
    let (_dir, mut app, _) = open();
    app.run_ui_command("merge.toggleActiveConflictInput2");
    app.run_ui_command("merge.toggleActiveConflictInput1");
    assert!(rendered(&app).starts_with("top\ntheirs1\nours1\nmid\n"));
    // Toggling does not move on to the next conflict.
    assert_eq!(active(&app), Some(0));
    app.run_ui_command("merge.toggleActiveConflictInput2");
    assert!(rendered(&app).starts_with("top\nours1\nmid\n"));
    app.run_ui_command("merge.toggleActiveConflictInput1");
    assert!(rendered(&app).starts_with("top\n<<<<<<< HEAD\n"));
}

#[test]
fn actions_without_an_active_conflict_change_nothing() {
    let (_dir, mut app, _) = open();
    app.merge_editor.as_mut().unwrap().active = None;
    app.run_ui_command("graphite.merge.acceptCurrent");
    assert_eq!(rendered(&app), TWO);
    assert_eq!(app.status, app.t("merge.none_active"));
}

#[test]
fn toggle_between_inputs_alternates_the_focused_input() {
    let (_dir, mut app, _) = open();
    assert_eq!(app.merge_editor.as_ref().unwrap().focused_input, 0);
    app.run_ui_command("mergeEditor.toggleBetweenInputs");
    assert_eq!(app.merge_editor.as_ref().unwrap().focused_input, 1);
    app.run_ui_command("mergeEditor.toggleBetweenInputs");
    assert_eq!(app.merge_editor.as_ref().unwrap().focused_input, 0);
}

#[test]
fn comparing_with_the_base_opens_a_diff_of_the_whole_sides() {
    let (_dir, mut app, _) = open();
    app.run_ui_command("mergeEditor.compareInput1WithBase");
    let diff = app.diff_view.take().unwrap();
    assert_eq!(diff.left, "top\nold1\nmid\nbottom\n");
    assert_eq!(diff.right, "top\nours1\nmid\nours2\nbottom\n");
    app.run_ui_command("mergeEditor.compareInput2WithBase");
    assert_eq!(app.diff_view.take().unwrap().right, "top\ntheirs1\nmid\ntheirs2\nbottom\n");
    app.run_ui_command("graphite.merge.acceptCurrent");
    app.run_ui_command("merge.openBaseEditor");
    assert_eq!(
        app.diff_view.take().unwrap().right,
        "top\nours1\nmid\n<<<<<<< HEAD\nours2\n=======\ntheirs2\n>>>>>>> topic\nbottom\n"
    );
}

#[test]
fn comparing_needs_a_base_in_the_markers() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("two_way.txt");
    std::fs::write(&path, "<<<<<<< a\n1\n=======\n2\n>>>>>>> b\n").unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_merge_editor(path).unwrap();
    app.run_ui_command("mergeEditor.compareInput1WithBase");
    assert!(app.diff_view.is_none());
    assert_eq!(app.status, app.t("merge.no_base"));
}

#[test]
fn open_result_hands_the_text_so_far_to_a_normal_editor() {
    let (_dir, mut app, path) = open();
    app.run_ui_command("graphite.merge.acceptIncoming");
    app.run_ui_command("merge.openResult");
    assert!(app.merge_editor.is_none());
    assert_eq!(
        app.editor().text(),
        "top\ntheirs1\nmid\n<<<<<<< HEAD\nours2\n=======\ntheirs2\n>>>>>>> topic\nbottom\n"
    );
    // The file itself is untouched until it is saved.
    assert_eq!(std::fs::read_to_string(path).unwrap(), TWO);
}

#[test]
fn open_result_with_no_decision_leaves_the_document_clean() {
    let (_dir, mut app, _) = open();
    app.run_ui_command("merge.openResult");
    assert_eq!(app.editor().text(), TWO);
    assert!(!app.documents[app.active_document()].buffer.is_dirty());
}
