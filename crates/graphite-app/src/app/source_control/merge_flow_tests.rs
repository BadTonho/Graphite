use super::merge_editor::{BasePlacement, MergeLayout};
use super::*;

const CONFLICTED: &str =
    "top\n<<<<<<< HEAD\nours\n||||||| base\nold\n=======\ntheirs\n>>>>>>> topic\nbottom\n";

fn app_with(text: &str) -> (crate::test_support::TempDir, App, PathBuf) {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("merged.txt");
    std::fs::write(&path, text).unwrap();
    let app = App::new_with_options(dir.path().into(), None, true);
    (dir, app, path)
}

#[test]
fn opening_reads_the_conflicts_from_disk_and_sets_the_context_key() {
    let (_dir, mut app, path) = app_with(CONFLICTED);
    assert!(app.merge_editor.is_none());
    app.open_merge_editor(path).unwrap();
    let editor = app.merge_editor.as_ref().unwrap();
    assert_eq!(editor.state.len(), 1);
    assert_eq!(editor.active, Some(0));
    app.refresh_command_context();
    assert!(app.status.contains('1'));
}

#[test]
fn a_file_without_markers_is_refused_and_nothing_opens() {
    let (_dir, mut app, path) = app_with("plain\n");
    assert!(app.open_merge_editor(path).is_err());
    assert!(app.merge_editor.is_none());
    assert!(app
        .open_merge_editor(PathBuf::from("does-not-exist.txt"))
        .is_err());
}

#[test]
fn merge_commands_are_disabled_until_the_editor_is_open() {
    let (_dir, mut app, path) = app_with(CONFLICTED);
    app.run_ui_command("merge.columnLayout");
    assert!(app.status.contains("merge.columnLayout"));
    app.open_merge_editor(path).unwrap();
    app.run_ui_command("merge.columnLayout");
    assert_eq!(
        app.merge_editor.as_ref().unwrap().layout,
        MergeLayout::Column
    );
    app.run_ui_command("merge.mixedLayout");
    assert_eq!(
        app.merge_editor.as_ref().unwrap().layout,
        MergeLayout::Mixed
    );
}

#[test]
fn the_base_pane_toggles_between_top_center_and_hidden() {
    let (_dir, mut app, path) = app_with(CONFLICTED);
    app.open_merge_editor(path).unwrap();
    let base = |app: &App| app.merge_editor.as_ref().unwrap().base;
    app.run_ui_command("merge.showBase");
    assert_eq!(base(&app), BasePlacement::Top);
    app.run_ui_command("merge.showBase");
    assert_eq!(base(&app), BasePlacement::Hidden);
    app.run_ui_command("merge.showBaseCenter");
    assert_eq!(base(&app), BasePlacement::Center);
    app.run_ui_command("merge.showBaseTop");
    assert_eq!(base(&app), BasePlacement::Top);
}

#[test]
fn hiding_plain_text_and_closing_work_through_commands() {
    let (_dir, mut app, path) = app_with(CONFLICTED);
    app.open_merge_editor(path).unwrap();
    app.run_ui_command("merge.showNonConflictingChanges");
    assert!(app.merge_editor.as_ref().unwrap().hide_unchanged);
    app.run_ui_command("merge.showNonConflictingChanges");
    assert!(!app.merge_editor.as_ref().unwrap().hide_unchanged);
    app.run_ui_command("graphite.merge.close");
    assert!(app.merge_editor.is_none());
}

#[test]
fn navigation_reports_when_no_conflict_is_left() {
    let (_dir, mut app, path) = app_with(CONFLICTED);
    app.open_merge_editor(path).unwrap();
    app.merge_editor.as_mut().unwrap().state.accept_current(0);
    app.run_ui_command("merge.goToNextUnhandledConflict");
    assert_eq!(app.status, app.t("merge.none_left"));
}

#[test]
fn saving_the_file_reloads_the_panes() {
    let (_dir, mut app, path) = app_with(CONFLICTED);
    app.open_merge_editor(path.clone()).unwrap();
    std::fs::write(&path, "resolved by hand\n").unwrap();
    app.reload_merge_editor_for(&path);
    assert_eq!(app.merge_editor.as_ref().unwrap().state.len(), 0);
    // Another file does not touch the open editor.
    let other = path.with_file_name("other.txt");
    std::fs::write(&other, CONFLICTED).unwrap();
    app.reload_merge_editor_for(&other);
    assert_eq!(app.merge_editor.as_ref().unwrap().state.len(), 0);
}

#[test]
fn the_palette_command_opens_the_active_file() {
    let (_dir, mut app, path) = app_with(CONFLICTED);
    app.open_file(path);
    app.run_ui_command("graphite.merge.open");
    assert!(app.merge_editor.is_some());
}

#[test]
fn opening_a_two_way_git_conflict_populates_base_from_index_stage_one() {
    let dir = crate::test_support::TempDir::new();
    let root = dir.path();
    let run_git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run_git(&["init", "-b", "main"]);
    run_git(&["config", "user.email", "test@example.com"]);
    run_git(&["config", "user.name", "Test User"]);

    let file = root.join("conflict.txt");
    std::fs::write(&file, "head\nbase line\ntail\n").unwrap();
    run_git(&["add", "conflict.txt"]);
    run_git(&["commit", "-m", "base"]);

    run_git(&["checkout", "-b", "incoming"]);
    std::fs::write(&file, "head\nincoming line\ntail\n").unwrap();
    run_git(&["commit", "-am", "incoming"]);

    run_git(&["checkout", "main"]);
    std::fs::write(&file, "head\ncurrent line\ntail\n").unwrap();
    run_git(&["commit", "-am", "current"]);

    // Force 2-way conflict markers (default merge.conflictStyle = merge).
    let merge = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "merge.conflictStyle=merge", "merge", "incoming"])
        .output()
        .unwrap();
    assert!(!merge.status.success());

    let mut app = App::new_with_options(root.to_path_buf(), None, true);
    app.open_merge_editor(file).unwrap();
    let editor = app.merge_editor.as_ref().unwrap();
    assert_eq!(
        editor.state.block(0).unwrap().base.as_deref(),
        Some("base line\n")
    );
}
