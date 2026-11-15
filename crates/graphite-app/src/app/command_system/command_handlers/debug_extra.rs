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

fn debug_add_data_breakpoint(
    app: &mut App,
    invocation: &CommandInvocation,
) -> Result<(), CommandError> {
    let address = invocation.request.arguments["dataId"]
        .as_str()
        .or_else(|| invocation.request.arguments["address"].as_str());
    app.add_data_breakpoint_from_input(address)
        .map_err(CommandError::Failed)
}

fn debug_remove_data_breakpoints(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.debug
        .remove_data_breakpoints()
        .map_err(|e| CommandError::Failed(e.to_string()))
}

fn debug_more_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_breakpoint_enabled()
        .map_err(CommandError::Failed)
}

fn debug_more_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_breakpoint().map_err(CommandError::Failed)
}

fn debug_more_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.execute_run_menu_command(crate::app::run_menu::RunMenuCommand::EditBreakpoint);
    Ok(())
}

fn debug_more_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.add_function_breakpoint_from_input()
        .map_err(CommandError::Failed)
}

fn debug_more_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.show_debug_hover().map_err(CommandError::Failed)
}

fn debug_more_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.filter_debug_console_from_input();
    Ok(())
}

fn debug_more_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.filter_debug_console_from_input();
    Ok(())
}

fn debug_arg_0(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let text = invocation.request.arguments["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    app.set_variable_from_text(&text)
        .map_err(CommandError::Failed)
}

fn debug_arg_1(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let text = invocation.request.arguments["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    app.start_debugging_with_config(&text)
        .map_err(CommandError::Failed)
}

/// Debug commands added after the first set: single breakpoints, function breakpoints,
/// variables, hover and console filtering.
fn register_more(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 7] = [
        (
            "debug.enableOrDisableBreakpoint",
            "Enable or disable breakpoint",
            debug_more_0,
        ),
        ("debug.toggleBreakpoint", "Toggle breakpoint", debug_more_1),
        ("debug.editBreakpoint", "Edit breakpoint", debug_more_2),
        (
            "workbench.debug.viewlet.action.addFunctionBreakpointAction",
            "Add function breakpoint",
            debug_more_3,
        ),
        (
            "editor.debug.action.showDebugHover",
            "Show debug hover",
            debug_more_4,
        ),
        (
            "repl.action.filter",
            "Filter the debug console",
            debug_more_5,
        ),
        (
            "repl.action.find",
            "Find in the debug console",
            debug_more_6,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Debug", ALL, "true", None, handler);
    }
    let with_argument: [(&'static str, &'static str, Handler); 2] = [
        ("debug.setVariable", "Set variable value", debug_arg_0),
        (
            "debug.startFromConfig",
            "Start debugging with a configuration",
            debug_arg_1,
        ),
    ];
    for (id, title, handler) in with_argument {
        registry
            .register(CommandDescriptor {
                id,
                title,
                category: "Debug",
                targets: ALL,
                enablement: "true",
                widget: None,
                menu_when: None,
                arguments: ArgumentKind::Text,
                handler,
            })
            .expect("unique built-in command");
    }
}

pub(super) fn register(registry: &mut CommandRegistry) {
    register_more(registry);
    let commands: [(&'static str, &'static str, Handler); 18] = [
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
        (
            "workbench.debug.viewlet.action.addDataBreakpointOnAddress",
            "Add data breakpoint on address",
            debug_add_data_breakpoint,
        ),
        (
            "workbench.debug.viewlet.action.editDataBreakpointOnAddress",
            "Edit data breakpoint on address",
            debug_add_data_breakpoint,
        ),
        (
            "workbench.debug.viewlet.action.removeAllDataBreakpoints",
            "Remove all data breakpoints",
            debug_remove_data_breakpoints,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Debug", ALL, "true", None, handler);
    }
}
