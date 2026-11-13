//! The `list.*` commands: moving through the explorer tree, or through quick open while it is
//! open, and expanding, collapsing and opening items.

use super::*;
use crate::app::explorer_nav::ExplorerMove;

/// Moves the explorer selection, or runs `quick_open` when quick open has the keyboard.
fn move_in_list(app: &mut App, movement: ExplorerMove, quick_open: &str) {
    if app.quick_open.is_some() {
        app.execute_quick_open_command(quick_open);
    } else {
        app.explorer_move(movement);
    }
}

fn list_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::Up, "quickOpen.previous");
    Ok(())
}

fn list_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::Down, "quickOpen.next");
    Ok(())
}

fn list_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::Up, "quickOpen.previous");
    Ok(())
}

fn list_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::Down, "quickOpen.next");
    Ok(())
}

fn list_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::PageUp, "quickOpen.previous");
    Ok(())
}

fn list_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::PageDown, "quickOpen.next");
    Ok(())
}

fn list_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::First, "quickOpen.previous");
    Ok(())
}

fn list_7(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::First, "quickOpen.previous");
    Ok(())
}

fn list_8(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::Last, "quickOpen.next");
    Ok(())
}

fn list_9(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    move_in_list(app, ExplorerMove::Last, "quickOpen.next");
    Ok(())
}

fn list_10(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_focus_parent();
    Ok(())
}

fn list_11(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_expand();
    Ok(())
}

fn list_12(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_collapse();
    Ok(())
}

fn list_13(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_collapse_all();
    Ok(())
}

fn list_14(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_activate_selected_folder();
    Ok(())
}

fn list_15(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_activate_selected();
    Ok(())
}

fn list_16(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_activate_selected();
    Ok(())
}

fn list_17(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_scroll_by(-1);
    Ok(())
}

fn list_18(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.explorer_scroll_by(1);
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 19] = [
        ("list.focusUp", "Focus previous item", list_0),
        ("list.focusDown", "Focus next item", list_1),
        ("list.focusAnyUp", "Focus previous item (any list)", list_2),
        ("list.focusAnyDown", "Focus next item (any list)", list_3),
        ("list.focusPageUp", "Focus item a page up", list_4),
        ("list.focusPageDown", "Focus item a page down", list_5),
        ("list.focusFirst", "Focus first item", list_6),
        ("list.focusAnyFirst", "Focus first item (any list)", list_7),
        ("list.focusLast", "Focus last item", list_8),
        ("list.focusAnyLast", "Focus last item (any list)", list_9),
        ("list.focusParent", "Focus parent", list_10),
        ("list.expand", "Expand", list_11),
        ("list.collapse", "Collapse", list_12),
        ("list.collapseAll", "Collapse all", list_13),
        ("list.toggleExpand", "Toggle expand", list_14),
        ("list.select", "Select", list_15),
        (
            "list.selectAndPreserveFocus",
            "Select and preserve focus",
            list_16,
        ),
        ("list.scrollUp", "Scroll up", list_17),
        ("list.scrollDown", "Scroll down", list_18),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "List", ALL, "true", None, handler);
    }
}
