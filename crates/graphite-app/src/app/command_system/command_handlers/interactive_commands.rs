//! Interactive window and REPL commands.

use super::*;
use crate::notebook::InteractiveKind;

const WINDOW: &str = "interactiveWindow";
const INPUT: &str = "interactiveInputFocus";

type Entry = (&'static str, &'static str, &'static str, Handler);

fn open_window(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_interactive(InteractiveKind::Window, None);
    Ok(())
}

fn open_repl(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_interactive(InteractiveKind::Repl, None);
    Ok(())
}

fn execute(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.interactive_execute().map_err(CommandError::Failed)
}

fn execute_selection(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let (code, language) = app.text_to_send();
    app.send_to_interactive(&code, language)
        .map_err(CommandError::Failed)
}

fn focus_input(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.focus_input(true));
    Ok(())
}

fn focus_history(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.focus_input(false));
    Ok(())
}

fn focus_last_executed(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.focus_input(false);
        let last = session.cells.len().saturating_sub(1);
        session.set_focus(last, false);
    });
    Ok(())
}

fn clear_input(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| session.clear_input());
    Ok(())
}

fn history_previous(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.history_previous();
    });
    Ok(())
}

fn history_next(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.with_notebook(|session| {
        session.history_next();
    });
    Ok(())
}

fn scroll_to_top(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.notebook_scroll(-1.0e6);
    Ok(())
}

fn scroll_to_bottom(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.notebook_scroll(1.0e6);
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [Entry; 15] = [
        (
            "interactive.open",
            "Open Interactive Window",
            "true",
            open_window,
        ),
        ("graphite.repl.open", "Open REPL", "true", open_repl),
        (
            "interactive.execute",
            "Run Input in the Interactive Window",
            INPUT,
            execute,
        ),
        ("repl.execute", "Run Input in the REPL", INPUT, execute),
        (
            "graphite.interactive.executeSelection",
            "Run Selection in the Interactive Window",
            "documentAvailable",
            execute_selection,
        ),
        (
            "interactive.input.focus",
            "Focus Interactive Input",
            WINDOW,
            focus_input,
        ),
        ("repl.input.focus", "Focus REPL Input", WINDOW, focus_input),
        (
            "list.find.replInputFocus",
            "Focus REPL Input from the List",
            WINDOW,
            focus_input,
        ),
        (
            "interactive.input.clear",
            "Clear Interactive Input",
            WINDOW,
            clear_input,
        ),
        (
            "interactive.history.focus",
            "Focus the Interactive History",
            WINDOW,
            focus_history,
        ),
        (
            "repl.focusLastItemExecuted",
            "Focus the Last Item Executed",
            WINDOW,
            focus_last_executed,
        ),
        (
            "interactive.history.previous",
            "Previous Input in the History",
            INPUT,
            history_previous,
        ),
        (
            "interactive.history.next",
            "Next Input in the History",
            INPUT,
            history_next,
        ),
        (
            "interactive.scrollToTop",
            "Scroll Interactive Window to Top",
            WINDOW,
            scroll_to_top,
        ),
        (
            "interactive.scrollToBottom",
            "Scroll Interactive Window to Bottom",
            WINDOW,
            scroll_to_bottom,
        ),
    ];
    for (id, title, enablement, handler) in commands {
        add(
            registry,
            id,
            title,
            "Interactive",
            ALL,
            enablement,
            None,
            handler,
        );
    }
}
