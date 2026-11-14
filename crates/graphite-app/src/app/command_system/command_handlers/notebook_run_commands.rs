//! Notebook commands that run cells and manage the kernel.

use super::*;
use crate::app::notebook_kernel::RunTarget;

const NOTEBOOK: &str = "notebookFocus";

type Entry = (&'static str, &'static str, Handler);

fn run(app: &mut App, target: RunTarget) -> Result<(), CommandError> {
    let sent = app
        .run_notebook_cells(target)
        .map_err(CommandError::Failed)?;
    app.status = app.tf("notebook.running", &[("count", &sent)]);
    Ok(())
}

fn execute(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    run(app, RunTarget::Selected)
}

fn execute_all(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    run(app, RunTarget::All)
}

fn execute_above(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    run(app, RunTarget::Above)
}

fn execute_below(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    run(app, RunTarget::SelectedAndBelow)
}

fn execute_and_select_below(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_and_advance(false);
    Ok(())
}

fn execute_and_insert_below(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.run_and_advance(true);
    Ok(())
}

fn cancel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.interrupt_notebook();
    Ok(())
}

fn clear_outputs(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        let (low, high) = session.selected();
        session.clear_outputs(&(low..=high).collect::<Vec<_>>());
    });
    Ok(())
}

fn clear_all_outputs(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        let count = session.cells.len();
        session.clear_outputs(&(0..count).collect::<Vec<_>>());
    });
    Ok(())
}

fn select_kernel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_kernel_picker();
    Ok(())
}

fn restart(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.restart_kernel().map_err(CommandError::Failed)
}

fn restart_and_run_all(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.restart_kernel().map_err(CommandError::Failed)?;
    run(app, RunTarget::All)
}

fn shutdown(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.shutdown_kernel();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [Entry; 13] = [
        ("notebook.cell.execute", "Run Cell", execute),
        (
            "notebook.cell.executeAndSelectBelow",
            "Run Cell and Select Below",
            execute_and_select_below,
        ),
        (
            "notebook.cell.executeAndInsertBelow",
            "Run Cell and Insert Below",
            execute_and_insert_below,
        ),
        ("notebook.execute", "Run All Cells", execute_all),
        (
            "notebook.cell.executeCellsAbove",
            "Run Cells Above",
            execute_above,
        ),
        (
            "notebook.cell.executeCellAndBelow",
            "Run Cell and Below",
            execute_below,
        ),
        ("notebook.cancelExecution", "Interrupt the Kernel", cancel),
        (
            "notebook.cell.clearOutputs",
            "Clear Cell Outputs",
            clear_outputs,
        ),
        (
            "notebook.clearAllCellsOutputs",
            "Clear All Outputs",
            clear_all_outputs,
        ),
        (
            "notebook.selectKernel",
            "Select Notebook Kernel",
            select_kernel,
        ),
        (
            "graphite.notebook.restartKernel",
            "Restart the Kernel",
            restart,
        ),
        (
            "graphite.notebook.restartKernelAndRunAll",
            "Restart the Kernel and Run All Cells",
            restart_and_run_all,
        ),
        (
            "graphite.notebook.shutdownKernel",
            "Shut Down the Kernel",
            shutdown,
        ),
    ];
    for (id, title, handler) in commands {
        add(
            registry, id, title, "Notebook", ALL, NOTEBOOK, None, handler,
        );
    }
}
