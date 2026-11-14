use super::notebook_kernel_tests::{configure, requests, spec};
use super::*;
use crate::notebook::Session;

const SAMPLE: &str = r###"{
 "cells": [
  {"cell_type": "markdown", "id": "a", "metadata": {}, "source": ["# One"]},
  {"cell_type": "code", "id": "b", "metadata": {}, "execution_count": 2, "outputs": [], "source": ["def helper():\n", "    return 1"]},
  {"cell_type": "markdown", "id": "c", "metadata": {}, "source": ["## Inner"]},
  {"cell_type": "code", "id": "d", "metadata": {}, "execution_count": null, "outputs": [], "source": ["needle = 2"]},
  {"cell_type": "markdown", "id": "e", "metadata": {}, "source": ["# Two"]},
  {"cell_type": "code", "id": "f", "metadata": {}, "execution_count": null, "outputs": [], "source": ["print(needle)"]}
 ],
 "metadata": {"kernelspec": {"name": "python3", "display_name": "Python 3", "language": "python"}},
 "nbformat": 4,
 "nbformat_minor": 5
}"###;

fn open() -> (crate::test_support::TempDir, App) {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("view.ipynb");
    std::fs::write(&path, SAMPLE).unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    configure(&mut app, vec![spec("python3", "python")]);
    app.open_file(path);
    (dir, app)
}

fn session(app: &App) -> &Session {
    app.active_notebook().expect("a notebook")
}

fn visible(app: &App) -> Vec<usize> {
    session(app)
        .hidden_cells()
        .iter()
        .enumerate()
        .filter(|(_, hidden)| !**hidden)
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn folding_hides_a_section_and_navigation_skips_it() {
    let (_dir, mut app) = open();
    app.run_ui_command("notebook.fold");
    assert_eq!(visible(&app), [0, 4, 5]);
    app.run_ui_command("notebook.focusNextEditor");
    assert_eq!(session(&app).focus, 4);
    app.run_ui_command("notebook.focusPreviousEditor");
    assert_eq!(session(&app).focus, 0);
    app.run_ui_command("notebook.unfold");
    assert_eq!(visible(&app).len(), 6);
    app.run_ui_command("notebook.unfold");
    assert_eq!(app.status, app.t("notebook.section.nothing_to_unfold"));
    app.run_ui_command("graphite.notebook.foldAll");
    assert_eq!(visible(&app), [0, 4]);
    app.run_ui_command("graphite.notebook.unfoldAll");
    assert_eq!(visible(&app).len(), 6);
    // Folding is a view setting: the file does not change.
    assert!(!app.documents[app.active_document()].is_dirty());
}

#[test]
fn a_section_runs_as_a_whole_or_one_cell_at_a_time() {
    let (_dir, mut app) = open();
    app.run_ui_command("notebook.section.runCells");
    let codes: Vec<_> = requests(0).into_iter().map(|(_, code)| code).collect();
    assert_eq!(codes, ["def helper():\n    return 1", "needle = 2"]);
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.section.runSingleCell");
    assert_eq!(requests(0).len(), 1);
}

#[test]
fn running_a_section_needs_a_section() {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("flat.ipynb");
    std::fs::write(
        &path,
        r#"{"nbformat":4,"nbformat_minor":5,"metadata":{},"cells":[{"cell_type":"code","id":"a","metadata":{},"source":"1","outputs":[],"execution_count":null}]}"#,
    )
    .unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    configure(&mut app, vec![spec("python3", "python")]);
    app.open_file(path);
    app.run_ui_command("notebook.section.runCells");
    assert_eq!(app.status, app.t("notebook.section.none"));
}

#[test]
fn the_search_list_jumps_to_a_line_and_next_and_previous_follow_the_last_search() {
    let (_dir, mut app) = open();
    app.run_ui_command("notebook.find");
    let picker = app.go_picker.as_mut().expect("the search list");
    assert_eq!(picker.kind, super::go_menu::GoPickerKind::NotebookFind);
    picker.query = "needle".into();
    app.accept_go_picker();
    assert_eq!(session(&app).focus, 3);
    assert_eq!(session(&app).find_query.as_deref(), Some("needle"));
    app.run_ui_command("notebook.findNext.fromWidget");
    assert_eq!(session(&app).focus, 5);
    app.run_ui_command("notebook.findPrevious.fromWidget");
    assert_eq!(session(&app).focus, 3);
    app.run_ui_command("notebook.hideFind");
    assert_eq!(session(&app).find_query, None);
    app.run_ui_command("notebook.findNext.fromWidget");
    assert_eq!(app.status, app.t("notebook.find.none"));
}

#[test]
fn the_outline_of_a_notebook_lists_headings_code_cells_and_symbols() {
    let (_dir, mut app) = open();
    let texts =
        |app: &App| -> Vec<String> { app.outline_rows().into_iter().map(|row| row.text).collect() };
    let all = texts(&app);
    assert!(all.contains(&"# One".to_owned()));
    assert!(all.contains(&"## Inner".to_owned()));
    assert!(all.iter().any(|text| text.starts_with("[2] def helper")));
    assert!(!all.iter().any(|text| text == "def helper()"));
    app.run_ui_command("notebook.outline.toggleCodeCellSymbols");
    assert!(texts(&app).iter().any(|text| text == "def helper()"));
    app.run_ui_command("notebook.outline.toggleCodeCells");
    assert!(!texts(&app).iter().any(|text| text.starts_with("[2]")));
    app.run_ui_command("notebook.outline.toggleShowMarkdownHeadersOnly");
    assert_eq!(texts(&app), ["# One", "## Inner", "# Two"]);
}

#[test]
fn clicking_an_outline_row_focuses_the_cell_and_opens_a_fold() {
    let (_dir, mut app) = open();
    app.run_ui_command("graphite.notebook.foldAll");
    app.run_ui_command("notebook.outline.toggleShowMarkdownHeadersOnly");
    let rows = app.outline_rows();
    let inner = rows.iter().position(|row| row.text == "## Inner").unwrap();
    app.outline_click(inner, false);
    assert_eq!(session(&app).focus, 2);
    assert!(!session(&app).hidden_cells()[2]);
}

#[test]
fn sticky_scroll_and_the_alias_commands_toggle_view_settings() {
    let (_dir, mut app) = open();
    assert!(!session(&app).sticky_scroll);
    app.run_ui_command("notebook.action.toggleNotebookStickyScroll");
    assert!(session(&app).sticky_scroll);
    app.run_ui_command("notebook.toggleLineNumbersFromEditorTitle");
    assert!(session(&app).line_numbers);
    app.run_ui_command("notebook.setProfile");
    assert_eq!(
        app.go_picker.as_ref().map(|picker| picker.kind),
        Some(super::go_menu::GoPickerKind::Kernel)
    );
}
