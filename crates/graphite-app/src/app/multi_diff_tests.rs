use super::*;

fn repo() -> (crate::test_support::TempDir, App) {
    let dir = crate::test_support::TempDir::new();
    gix::init(dir.path()).unwrap();
    let translator = graphite_i18n::Translator::english();
    for (name, text) in [("a.txt", "1\n2\n3\n"), ("b.txt", "x\n"), ("gone.txt", "bye\n")] {
        std::fs::write(dir.path().join(name), text).unwrap();
        crate::scm::stage_file(dir.path(), &dir.path().join(name), &translator).unwrap();
    }
    crate::scm::commit(dir.path(), "initial", &translator).unwrap();
    let app = App::new_with_options(dir.path().into(), None, true);
    (dir, app)
}

/// `a.txt` edited and staged, `b.txt` edited and left unstaged, `gone.txt` deleted, plus a new
/// untracked file.
fn change(dir: &crate::test_support::TempDir) {
    let translator = graphite_i18n::Translator::english();
    std::fs::write(dir.path().join("a.txt"), "1\nTWO\n3\n").unwrap();
    crate::scm::stage_file(dir.path(), &dir.path().join("a.txt"), &translator).unwrap();
    std::fs::write(dir.path().join("b.txt"), "x\ny\n").unwrap();
    std::fs::remove_file(dir.path().join("gone.txt")).unwrap();
    std::fs::write(dir.path().join("new.txt"), "fresh\n").unwrap();
}

fn files(app: &App) -> Vec<String> {
    app.multi_diff
        .as_ref()
        .unwrap()
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect()
}

#[test]
fn all_changes_open_every_changed_file() {
    let (dir, mut app) = repo();
    change(&dir);
    app.run_ui_command("graphite.scm.openAllChanges");
    let mut names = files(&app);
    names.sort();
    assert_eq!(names, ["a.txt", "b.txt", "gone.txt", "new.txt"]);
    let diff = app.multi_diff.as_ref().unwrap();
    assert!(diff.title.contains('4'));
    assert_eq!(diff.totals(), (3, 2));
    assert_eq!(app.status, app.t("multi_diff.opened"));
}

#[test]
fn staged_and_unstaged_views_split_the_changes() {
    let (dir, mut app) = repo();
    change(&dir);
    app.run_ui_command("graphite.scm.openAllStagedChanges");
    assert_eq!(files(&app), ["a.txt"]);
    app.multi_diff = None;
    app.run_ui_command("graphite.scm.openAllUnstagedChanges");
    let mut names = files(&app);
    names.sort();
    assert_eq!(names, ["b.txt", "gone.txt", "new.txt"]);
}

#[test]
fn nothing_to_show_and_not_a_repository_are_reported() {
    let (_dir, mut app) = repo();
    app.run_ui_command("graphite.scm.openAllChanges");
    assert!(app.multi_diff.is_none());
    assert_eq!(app.status, app.t("multi_diff.empty"));

    let plain = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(plain.path().into(), None, true);
    app.run_ui_command("graphite.scm.openAllChanges");
    assert!(app.multi_diff.is_none());
    assert!(app.status.starts_with(app.t("multi_diff.failed").split('{').next().unwrap()));
}

#[test]
fn commands_need_the_view_to_be_open() {
    let (_dir, mut app) = repo();
    app.run_ui_command("multiDiffEditor.goToNextChange");
    assert!(app.status.contains("multiDiffEditor.goToNextChange"));
}

#[test]
fn collapse_expand_and_navigation_commands() {
    let (dir, mut app) = repo();
    change(&dir);
    app.run_ui_command("graphite.scm.openAllChanges");
    let rows = app.multi_diff.as_ref().unwrap().rows().len();
    app.run_ui_command("multiDiffEditor.collapseAll");
    assert_eq!(app.multi_diff.as_ref().unwrap().rows().len(), 4);
    app.run_ui_command("multiDiffEditor.goToNextChange");
    // Landing on a change expands its file.
    assert!(app.multi_diff.as_ref().unwrap().rows().len() > 4);
    assert_eq!(app.multi_diff.as_ref().unwrap().current, Some(0));
    app.run_ui_command("multiDiffEditor.expandAll");
    assert_eq!(app.multi_diff.as_ref().unwrap().rows().len(), rows);
    app.run_ui_command("multiDiffEditor.goToPreviousChange");
    let diff = app.multi_diff.as_ref().unwrap();
    assert_eq!(diff.current, Some(diff.changes().len() - 1));
    app.run_ui_command("graphite.multiDiff.toggleFile");
    let selected = app.multi_diff.as_ref().unwrap().selected_file;
    assert!(app.multi_diff.as_ref().unwrap().files[selected].collapsed);
}

#[test]
fn the_file_list_takes_and_gives_back_the_keyboard() {
    let (dir, mut app) = repo();
    change(&dir);
    app.run_ui_command("graphite.scm.openAllChanges");
    app.run_ui_command("multiDiffEditor.focusFileList");
    assert!(app.multi_diff.as_ref().unwrap().list_focus);
    app.run_ui_command("multiDiffEditor.focusFileList");
    assert!(!app.multi_diff.as_ref().unwrap().list_focus);
}

#[test]
fn go_to_file_opens_the_file_at_the_change_and_closes_the_view() {
    let (dir, mut app) = repo();
    change(&dir);
    app.run_ui_command("graphite.scm.openAllStagedChanges");
    app.run_ui_command("multiDiffEditor.goToNextChange");
    app.run_ui_command("multiDiffEditor.goToFile");
    assert!(app.multi_diff.is_none());
    let path = app.documents[app.active_document()].buffer.path().unwrap().to_path_buf();
    assert!(crate::same_file_path(&path, &dir.path().join("a.txt")));
    // The change is the second line.
    assert_eq!(app.cursor_line(), 1);
}

#[test]
fn go_to_file_refuses_a_deleted_file_and_keeps_the_view() {
    let (dir, mut app) = repo();
    change(&dir);
    app.run_ui_command("graphite.scm.openAllUnstagedChanges");
    let position = files(&app).iter().position(|name| name == "gone.txt").unwrap();
    app.multi_diff.as_mut().unwrap().selected_file = position;
    app.multi_diff.as_mut().unwrap().list_focus = true;
    app.run_ui_command("multiDiffEditor.goToFile");
    assert!(app.multi_diff.is_some());
    assert!(app.status.contains("gone.txt"));
}

#[test]
fn closing_removes_the_view() {
    let (dir, mut app) = repo();
    change(&dir);
    app.run_ui_command("graphite.scm.openAllChanges");
    app.run_ui_command("graphite.multiDiff.close");
    assert!(app.multi_diff.is_none());
}
