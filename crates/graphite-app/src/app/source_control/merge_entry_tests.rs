use super::*;

const MARKED: &str = "top\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> topic\nbottom\n";

fn conflicted_repo(text: &str) -> (crate::test_support::TempDir, App, PathBuf) {
    let dir = crate::test_support::TempDir::new();
    crate::test_support::init_git_repo(dir.path());
    let path = dir.path().join("merged.txt");
    std::fs::write(&path, text).unwrap();
    crate::scm::mark_conflicted_for_tests(dir.path(), "merged.txt", "base\n", "ours\n", "theirs\n");
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.refresh_scm_async();
    app.scm.selected = app
        .scm
        .snapshot
        .files
        .iter()
        .position(|file| file.conflicted)
        .unwrap();
    (dir, app, path)
}

#[test]
fn a_conflicted_file_in_the_source_control_list_opens_the_merge_editor() {
    let (_dir, mut app, path) = conflicted_repo(MARKED);
    app.run_ui_command("graphite.scm.openConflict");
    let editor = app.merge_editor.as_ref().expect("merge editor opened");
    assert!(crate::same_file_path(&editor.path, &path));
    assert_eq!(editor.state.len(), 1);
    assert!(app.conflict_view.is_none());
}

#[test]
fn a_conflicted_file_without_markers_keeps_the_three_way_view() {
    let (_dir, mut app, _) = conflicted_repo("no markers here\n");
    app.run_ui_command("graphite.scm.openConflict");
    assert!(app.merge_editor.is_none());
}

#[test]
fn the_conflict_view_hands_over_to_the_merge_editor() {
    let (_dir, mut app, path) = conflicted_repo(MARKED);
    app.conflict_view = Some(super::scm_ui::ConflictView {
        path: path.clone(),
        sides: [
            Some("base\n".into()),
            Some("ours\n".into()),
            Some("theirs\n".into()),
        ],
    });
    app.run_ui_command("graphite.conflict.openMergeEditor");
    assert!(app.conflict_view.is_none());
    assert!(app.merge_editor.is_some());
}

#[test]
fn the_conflict_view_stays_when_the_file_has_no_markers() {
    let (_dir, mut app, path) = conflicted_repo("plain\n");
    app.conflict_view = Some(super::scm_ui::ConflictView {
        path,
        sides: [None, Some("ours\n".into()), Some("theirs\n".into())],
    });
    app.run_ui_command("graphite.conflict.openMergeEditor");
    assert!(app.conflict_view.is_some());
    assert!(app.merge_editor.is_none());
    assert_eq!(app.status, app.t("merge.error.no_conflicts"));
}

#[test]
fn the_conflict_command_needs_the_conflict_view() {
    let (_dir, mut app, _) = conflicted_repo(MARKED);
    app.run_ui_command("graphite.conflict.openMergeEditor");
    assert!(app.merge_editor.is_none());
}
