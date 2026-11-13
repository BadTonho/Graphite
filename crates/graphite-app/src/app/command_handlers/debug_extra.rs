//! Debug commands for breakpoints, the debug console and the debug view.

use super::*;

fn debug_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.go_to_breakpoint(true);
    Ok(())
}

fn debug_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.go_to_breakpoint(false);
    Ok(())
}

fn debug_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.remove_breakpoint_at_cursor()
        .map_err(CommandError::Failed)
}

fn debug_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.remove_breakpoint_at_cursor()
        .map_err(CommandError::Failed)
}

fn debug_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_breakpoints_activated();
    Ok(())
}

fn debug_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.reapply_breakpoints().map_err(CommandError::Failed)
}

fn debug_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.clear_debug_console();
    Ok(())
}

fn debug_7(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.copy_debug_console().map_err(CommandError::Failed)
}

fn debug_8(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.copy_debug_console().map_err(CommandError::Failed)
}

fn debug_9(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.evaluate_debug_input().map_err(CommandError::Failed)
}

fn debug_10(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.show_debug_view();
    Ok(())
}

fn debug_11(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.debug.collapse_variables();
    Ok(())
}

fn debug_12(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.debug.collapse_variables();
    Ok(())
}

fn debug_13(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.debug.collapse_variables();
    Ok(())
}

fn debug_14(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.debug.collapse_variables();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 15] = [
        (
            "editor.debug.action.goToNextBreakpoint",
            "Go to next breakpoint",
            debug_0,
        ),
        (
            "editor.debug.action.goToPreviousBreakpoint",
            "Go to previous breakpoint",
            debug_1,
        ),
        ("debug.removeBreakpoint", "Remove breakpoint", debug_2),
        (
            "workbench.debug.viewlet.action.removeBreakpoint",
            "Remove breakpoint on this line",
            debug_3,
        ),
        (
            "workbench.debug.viewlet.action.toggleBreakpointsActivatedAction",
            "Activate or deactivate breakpoints",
            debug_4,
        ),
        (
            "workbench.debug.viewlet.action.reapplyBreakpointsAction",
            "Reapply all breakpoints",
            debug_5,
        ),
        (
            "workbench.debug.panel.action.clearReplAction",
            "Clear debug console",
            debug_6,
        ),
        ("repl.action.copyAll", "Copy debug console", debug_7),
        (
            "workbench.debug.action.copyAll",
            "Copy all debug output",
            debug_8,
        ),
        (
            "repl.action.acceptInput",
            "Evaluate the debug console input",
            debug_9,
        ),
        ("debug.openView", "Show debug view", debug_10),
        ("variables.collapse", "Collapse variables", debug_11),
        ("watch.collapse", "Collapse watch expressions", debug_12),
        ("callStack.collapse", "Collapse call stack", debug_13),
        (
            "loadedScripts.collapse",
            "Collapse loaded scripts",
            debug_14,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Debug", ALL, "true", None, handler);
    }
}
