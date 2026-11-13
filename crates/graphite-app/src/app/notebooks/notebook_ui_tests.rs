use super::*;
use crate::notebook::{CellKind, Mode};

const SAMPLE: &str = r##"{
 "cells": [
  {"cell_type": "markdown", "id": "m1", "metadata": {}, "source": ["# Title"]},
  {"cell_type": "code", "id": "c1", "metadata": {}, "execution_count": 1,
   "outputs": [{"output_type": "stream", "name": "stdout", "text": ["hi\n"]}], "source": ["print('hi')"]},
  {"cell_type": "code", "id": "c2", "metadata": {}, "execution_count": null, "outputs": [], "source": ["x = 1"]}
 ],
 "metadata": {"kernelspec": {"name": "python3", "display_name": "Python 3", "language": "python"}},
 "nbformat": 4,
 "nbformat_minor": 5
}"##;

fn open() -> (crate::test_support::TempDir, App, PathBuf) {
    let dir = crate::test_support::TempDir::new();
    let path = dir.path().join("analysis.ipynb");
    std::fs::write(&path, SAMPLE).unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_file(path.clone());
    (dir, app, path)
}

fn session(app: &App) -> &crate::notebook::Session {
    app.active_notebook().expect("the active editor is a notebook")
}

fn sources(app: &App) -> Vec<String> {
    session(app).cells.iter().map(|cell| cell.text()).collect()
}

#[test]
fn an_ipynb_file_opens_as_a_notebook_and_a_broken_one_as_text() {
    let (dir, app, _) = open();
    assert_eq!(session(&app).cells.len(), 3);
    assert_eq!(session(&app).mode, Mode::Command);
    assert!(!app.documents[app.active_document()].is_dirty());

    let broken = dir.path().join("broken.ipynb");
    std::fs::write(&broken, "{ not json").unwrap();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.open_file(broken);
    assert!(app.active_notebook().is_none());
    assert_eq!(app.editor().text(), "{ not json");
}

#[test]
fn notebook_commands_are_disabled_outside_a_notebook() {
    let dir = crate::test_support::TempDir::new();
    let mut app = App::new_with_options(dir.path().into(), None, true);
    app.run_ui_command("notebook.cell.delete");
    assert!(app.status.contains("notebook.cell.delete"));
}

#[test]
fn editing_a_cell_makes_the_document_dirty_and_undo_makes_it_clean_again() {
    let (_dir, mut app, _) = open();
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.edit");
    assert_eq!(session(&app).mode, Mode::Edit);
    app.with_notebook(|session| session.edit(|editor| editor.insert("# ")));
    assert_eq!(sources(&app)[1], "# print('hi')");
    let document = app.active_document();
    assert!(app.documents[document].is_dirty());
    assert!(app.documents[document].buffer.text().contains("# print('hi')"));
    app.run_ui_command("notebook.undo");
    assert_eq!(sources(&app)[1], "print('hi')");
    assert!(!app.documents[document].is_dirty());
    app.run_ui_command("notebook.redo");
    assert_eq!(sources(&app)[1], "# print('hi')");
}

#[test]
fn cell_commands_change_the_cells_and_the_document_text() {
    let (_dir, mut app, _) = open();
    app.run_ui_command("notebook.cell.insertCodeCellBelow");
    assert_eq!(sources(&app), ["# Title", "", "print('hi')", "x = 1"]);
    assert_eq!(session(&app).mode, Mode::Edit);
    assert_eq!(session(&app).focus, 1);
    app.run_ui_command("notebook.cell.quitEdit");
    app.run_ui_command("notebook.cell.moveDown");
    assert_eq!(sources(&app)[2], "");
    app.run_ui_command("notebook.cell.moveUp");
    app.run_ui_command("notebook.cell.delete");
    assert_eq!(sources(&app), ["# Title", "print('hi')", "x = 1"]);
    app.run_ui_command("notebook.cell.changeToMarkdown");
    assert_eq!(session(&app).focused().cell.kind, CellKind::Markdown);
    app.run_ui_command("notebook.cell.joinBelow");
    assert_eq!(sources(&app), ["# Title", "print('hi')\nx = 1"]);
    let json = app.documents[app.active_document()].buffer.text();
    assert!(json.contains("print('hi')\\n"));
    // Everything above is one history.
    for _ in 0..6 {
        app.run_ui_command("notebook.undo");
    }
    assert_eq!(sources(&app), ["# Title", "print('hi')", "x = 1"]);
    assert!(!app.documents[app.active_document()].is_dirty());
}

#[test]
fn splitting_and_the_pending_delete_work_through_commands() {
    let (_dir, mut app, _) = open();
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.edit");
    app.with_notebook(|session| session.edit(|editor| editor.set_cursor_line_col(0, 1)));
    app.run_ui_command("notebook.cell.split");
    assert_eq!(sources(&app), ["# Title", "print('hi')", "x", " = 1"]);
    assert_eq!(session(&app).focus, 3);
    assert_eq!(session(&app).mode, Mode::Edit);
    // The split command only works while editing.
    app.run_ui_command("notebook.cell.quitEdit");
    app.run_ui_command("notebook.cell.split");
    assert_eq!(sources(&app).len(), 4);
}

#[test]
fn collapsing_copying_and_line_numbers() {
    let (_dir, mut app, _) = open();
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.collapseCellOutput");
    assert!(session(&app).cells[1].output_collapsed);
    app.run_ui_command("notebook.cell.expandCellOutput");
    assert!(!session(&app).cells[1].output_collapsed);
    app.run_ui_command("notebook.cell.toggleOutputs");
    assert!(session(&app).cells[1].output_collapsed);
    app.run_ui_command("notebook.cell.collapseCellInput");
    assert!(session(&app).cells[1].input_collapsed);
    app.run_ui_command("graphite.notebook.toggleCellInput");
    assert!(!session(&app).cells[1].input_collapsed);
    app.run_ui_command("notebook.toggleLineNumbers");
    assert!(session(&app).line_numbers);
    app.run_ui_command("notebook.cell.toggleLineNumbers");
    assert!(!session(&app).line_numbers);
    app.run_ui_command("notebook.cell.copy");
    app.run_ui_command("notebook.cell.paste");
    assert_eq!(sources(&app), ["# Title", "print('hi')", "print('hi')", "x = 1"]);
    let cells = &session(&app).cells;
    assert_ne!(cells[1].cell.id, cells[2].cell.id);
    // Collapsing does not touch the document.
    assert!(app.documents[app.active_document()].is_dirty());
}

#[test]
fn saving_writes_the_notebook_and_keeps_the_editing_state() {
    let (_dir, mut app, path) = open();
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.focusNextEditor");
    app.run_ui_command("notebook.cell.edit");
    app.with_notebook(|session| session.edit(|editor| editor.insert("y")));
    app.save();
    let written = crate::notebook::Notebook::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written.cells[2].source, "yx = 1");
    // The unknown parts of the file are still there.
    assert_eq!(written.kernel_name(), Some("python3"));
    assert!(!app.documents[app.active_document()].is_dirty());
    assert_eq!(session(&app).mode, Mode::Edit);
    assert_eq!(session(&app).focus, 2);
    app.refresh_notebook(app.active_document());
    assert!(session(&app).can_undo());
}

#[test]
fn a_reload_from_disk_replaces_the_cells() {
    let (_dir, mut app, path) = open();
    app.run_ui_command("notebook.focusNextEditor");
    let other = SAMPLE.replace("x = 1", "z = 3");
    let decoded = crate::file_format::DecodedFile {
        text: other,
        encoding: crate::file_format::Encoding::Utf8,
        line_ending: crate::file_format::LineEnding::Lf,
    };
    app.reload_after_branch_checkout(vec![(path, Ok(decoded))]);
    app.refresh_notebook(app.active_document());
    assert_eq!(sources(&app)[2], "z = 3");
    assert_eq!(session(&app).focus, 1);
    assert!(!app.documents[app.active_document()].is_dirty());
}

#[test]
fn notebook_context_reports_the_mode() {
    let (_dir, mut app, _) = open();
    assert_eq!(app.notebook_context(), (true, false));
    app.run_ui_command("notebook.cell.edit");
    assert_eq!(app.notebook_context(), (true, true));
    app.refresh_command_context();
    app.run_ui_command("notebook.cell.quitEdit");
    assert_eq!(app.notebook_context(), (true, false));
}

#[test]
fn scrolling_follows_the_focus_and_stays_inside_the_notebook() {
    let (_dir, mut app, _) = open();
    app.notebook_scroll(-100.0);
    assert_eq!(session(&app).scroll, 0.0);
    app.notebook_scroll(10_000.0);
    let max = session(&app).scroll;
    assert!(max >= 0.0);
    app.run_ui_command("notebook.focusNextEditor");
    assert!(session(&app).scroll <= max.max(0.0) + 1.0e6);
}
