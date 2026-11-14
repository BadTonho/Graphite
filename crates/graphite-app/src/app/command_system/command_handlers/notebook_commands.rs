//! Notebook commands.

use super::*;
use crate::notebook::{CellKind, Mode};

const NOTEBOOK: &str = "notebookFocus";
const COMMAND_MODE: &str = "notebookCommandMode";
const EDITING: &str = "notebookEditing";

type Entry = (&'static str, &'static str, &'static str, Handler);

fn edit_cell(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.enter_edit());
    Ok(())
}

fn quit_edit(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.exit_edit());
    Ok(())
}

fn focus_next(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.focus_by(1, false));
    Ok(())
}

fn focus_previous(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.focus_by(-1, false));
    Ok(())
}

fn select_all_cells(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.select_all_cells());
    Ok(())
}

fn insert_code_above(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.notebook_insert(CellKind::Code, false);
    Ok(())
}

fn insert_code_below(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.notebook_insert(CellKind::Code, true);
    Ok(())
}

fn insert_markdown_above(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.notebook_insert(CellKind::Markdown, false);
    Ok(())
}

fn insert_markdown_below(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.notebook_insert(CellKind::Markdown, true);
    Ok(())
}

fn delete_cell(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.delete_selected();
    });
    Ok(())
}

fn move_up(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.move_selected(false);
    });
    Ok(())
}

fn move_down(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.move_selected(true);
    });
    Ok(())
}

fn split_cell(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.split_at_cursor();
        // Keep editing in the new cell.
        session.mode = Mode::Edit;
    });
    Ok(())
}

fn join_below(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.merge_selected(false);
    });
    Ok(())
}

fn join_above(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.merge_selected(true);
    });
    Ok(())
}

fn to_code(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.set_kind(CellKind::Code));
    Ok(())
}

fn to_markdown(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.set_kind(CellKind::Markdown));
    Ok(())
}

fn to_raw(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.set_kind(CellKind::Raw));
    Ok(())
}

fn copy_cells(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.copy_selected());
    Ok(())
}

fn cut_cells(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.cut_selected());
    Ok(())
}

fn paste_cells(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.paste_below();
    });
    Ok(())
}

fn collapse_input(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.set_input_collapsed(true));
    Ok(())
}

fn expand_input(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.set_input_collapsed(false));
    Ok(())
}

fn toggle_input(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.toggle_input_collapsed());
    Ok(())
}

fn collapse_output(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.set_output_collapsed(true));
    Ok(())
}

fn expand_output(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.set_output_collapsed(false));
    Ok(())
}

fn toggle_output(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.toggle_output_collapsed());
    Ok(())
}

fn undo(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.undo();
    });
    Ok(())
}

fn redo(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.redo();
    });
    Ok(())
}

fn toggle_line_numbers(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.line_numbers = !session.line_numbers);
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [Entry; 32] = [
        ("notebook.cell.edit", "Edit Cell", COMMAND_MODE, edit_cell),
        (
            "notebook.cell.quitEdit",
            "Stop Editing Cell",
            EDITING,
            quit_edit,
        ),
        (
            "notebook.focusNextEditor",
            "Focus Next Cell",
            NOTEBOOK,
            focus_next,
        ),
        (
            "notebook.focusPreviousEditor",
            "Focus Previous Cell",
            NOTEBOOK,
            focus_previous,
        ),
        (
            "graphite.notebook.selectAllCells",
            "Select All Cells",
            NOTEBOOK,
            select_all_cells,
        ),
        (
            "notebook.cell.insertCodeCellAbove",
            "Insert Code Cell Above",
            NOTEBOOK,
            insert_code_above,
        ),
        (
            "notebook.cell.insertCodeCellBelow",
            "Insert Code Cell Below",
            NOTEBOOK,
            insert_code_below,
        ),
        (
            "notebook.cell.insertMarkdownCellAbove",
            "Insert Markdown Cell Above",
            NOTEBOOK,
            insert_markdown_above,
        ),
        (
            "notebook.cell.insertMarkdownCellBelow",
            "Insert Markdown Cell Below",
            NOTEBOOK,
            insert_markdown_below,
        ),
        ("notebook.cell.delete", "Delete Cell", NOTEBOOK, delete_cell),
        ("notebook.cell.moveUp", "Move Cell Up", NOTEBOOK, move_up),
        (
            "notebook.cell.moveDown",
            "Move Cell Down",
            NOTEBOOK,
            move_down,
        ),
        ("notebook.cell.split", "Split Cell", EDITING, split_cell),
        (
            "notebook.cell.joinBelow",
            "Join With Next Cell",
            NOTEBOOK,
            join_below,
        ),
        (
            "notebook.cell.joinAbove",
            "Join With Previous Cell",
            NOTEBOOK,
            join_above,
        ),
        (
            "notebook.cell.changeToCode",
            "Change Cell to Code",
            NOTEBOOK,
            to_code,
        ),
        (
            "notebook.cell.changeToMarkdown",
            "Change Cell to Markdown",
            NOTEBOOK,
            to_markdown,
        ),
        (
            "notebook.cell.changeToRaw",
            "Change Cell to Raw",
            NOTEBOOK,
            to_raw,
        ),
        ("notebook.cell.copy", "Copy Cell", NOTEBOOK, copy_cells),
        ("notebook.cell.cut", "Cut Cell", NOTEBOOK, cut_cells),
        ("notebook.cell.paste", "Paste Cell", NOTEBOOK, paste_cells),
        (
            "notebook.cell.collapseCellInput",
            "Collapse Cell Input",
            NOTEBOOK,
            collapse_input,
        ),
        (
            "notebook.cell.expandCellInput",
            "Expand Cell Input",
            NOTEBOOK,
            expand_input,
        ),
        (
            "graphite.notebook.toggleCellInput",
            "Toggle Cell Input",
            NOTEBOOK,
            toggle_input,
        ),
        (
            "notebook.cell.collapseCellOutput",
            "Collapse Cell Output",
            NOTEBOOK,
            collapse_output,
        ),
        (
            "notebook.cell.expandCellOutput",
            "Expand Cell Output",
            NOTEBOOK,
            expand_output,
        ),
        (
            "notebook.cell.toggleOutputs",
            "Toggle Cell Outputs",
            NOTEBOOK,
            toggle_output,
        ),
        ("notebook.undo", "Undo in the Notebook", NOTEBOOK, undo),
        ("notebook.redo", "Redo in the Notebook", NOTEBOOK, redo),
        (
            "notebook.toggleLineNumbers",
            "Toggle Notebook Line Numbers",
            NOTEBOOK,
            toggle_line_numbers,
        ),
        (
            "notebook.cell.toggleLineNumbers",
            "Toggle Cell Line Numbers",
            NOTEBOOK,
            toggle_line_numbers,
        ),
        (
            "notebook.cell.nullAction",
            "Do Nothing in the Notebook",
            NOTEBOOK,
            |_, _| Ok(()),
        ),
    ];
    for (id, title, enablement, handler) in commands {
        add(
            registry, id, title, "Notebook", ALL, enablement, None, handler,
        );
    }
}
