//! Notebook commands for folding, searching, the outline and the other views.

use super::*;

const NOTEBOOK: &str = "notebookFocus";

type Entry = (&'static str, &'static str, &'static str, Handler);

fn fold(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let mut folded = false;
    app.with_notebook(|session| folded = session.fold_at_focus());
    if !folded {
        app.status = app.t("notebook.section.nothing_to_fold").into();
    }
    Ok(())
}

fn unfold(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let mut unfolded = false;
    app.with_notebook(|session| unfolded = session.unfold_at_focus());
    if !unfolded {
        app.status = app.t("notebook.section.nothing_to_unfold").into();
    }
    Ok(())
}

fn fold_all(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.fold_all());
    Ok(())
}

fn unfold_all(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.unfold_all());
    Ok(())
}

fn run_section(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let sent = app
        .run_notebook_section(false)
        .map_err(CommandError::Failed)?;
    app.status = app.tf("notebook.running", &[("count", &sent)]);
    Ok(())
}

fn run_single(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let sent = app
        .run_notebook_section(true)
        .map_err(CommandError::Failed)?;
    app.status = app.tf("notebook.running", &[("count", &sent)]);
    Ok(())
}

fn toggle_sticky(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.sticky_scroll = !session.sticky_scroll);
    Ok(())
}

fn find(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_notebook_find();
    Ok(())
}

fn find_next(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.notebook_find_step(true);
    Ok(())
}

fn find_previous(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.notebook_find_step(false);
    Ok(())
}

fn hide_find(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.notebook_hide_find();
    Ok(())
}

fn set_profile(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_kernel_picker();
    Ok(())
}

fn toggle_line_numbers(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.line_numbers = !session.line_numbers);
    Ok(())
}

fn outline_code_cells(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline.notebook_code_cells = !app.outline.notebook_code_cells;
    Ok(())
}

fn outline_symbols(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline.notebook_symbols = !app.outline.notebook_symbols;
    Ok(())
}

fn outline_headers_only(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline.notebook_headers_only = !app.outline.notebook_headers_only;
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [Entry; 16] = [
        ("notebook.fold", "Fold the Section", NOTEBOOK, fold),
        ("notebook.unfold", "Unfold the Section", NOTEBOOK, unfold),
        (
            "notebook.section.foldSection",
            "Fold Section",
            NOTEBOOK,
            fold,
        ),
        (
            "notebook.section.expandSection",
            "Expand Section",
            NOTEBOOK,
            unfold,
        ),
        (
            "graphite.notebook.foldAll",
            "Fold All Sections",
            NOTEBOOK,
            fold_all,
        ),
        (
            "graphite.notebook.unfoldAll",
            "Unfold All Sections",
            NOTEBOOK,
            unfold_all,
        ),
        (
            "notebook.section.runCells",
            "Run the Cells of the Section",
            NOTEBOOK,
            run_section,
        ),
        (
            "notebook.section.runSingleCell",
            "Run Only the Cell",
            NOTEBOOK,
            run_single,
        ),
        (
            "notebook.action.toggleNotebookStickyScroll",
            "Toggle Notebook Sticky Scroll",
            NOTEBOOK,
            toggle_sticky,
        ),
        ("notebook.find", "Find in the Notebook", NOTEBOOK, find),
        (
            "notebook.findNext.fromWidget",
            "Find Next in the Notebook",
            NOTEBOOK,
            find_next,
        ),
        (
            "notebook.findPrevious.fromWidget",
            "Find Previous in the Notebook",
            NOTEBOOK,
            find_previous,
        ),
        (
            "notebook.hideFind",
            "Hide the Notebook Search",
            NOTEBOOK,
            hide_find,
        ),
        (
            "notebook.setProfile",
            "Set the Notebook Kernel Profile",
            NOTEBOOK,
            set_profile,
        ),
        (
            "notebook.toggleLineNumbersFromEditorTitle",
            "Toggle Line Numbers from the Editor Title",
            NOTEBOOK,
            toggle_line_numbers,
        ),
        (
            "notebook.outline.toggleCodeCells",
            "Outline: Show Code Cells",
            "true",
            outline_code_cells,
        ),
    ];
    for (id, title, enablement, handler) in commands {
        add(
            registry, id, title, "Notebook", ALL, enablement, None, handler,
        );
    }
    let outline: [Entry; 2] = [
        (
            "notebook.outline.toggleCodeCellSymbols",
            "Outline: Show Code Cell Symbols",
            "true",
            outline_symbols,
        ),
        (
            "notebook.outline.toggleShowMarkdownHeadersOnly",
            "Outline: Show Markdown Headers Only",
            "true",
            outline_headers_only,
        ),
    ];
    for (id, title, enablement, handler) in outline {
        add(
            registry, id, title, "Notebook", ALL, enablement, None, handler,
        );
    }
}
