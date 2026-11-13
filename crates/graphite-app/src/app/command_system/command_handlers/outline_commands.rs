//! Outline view commands: sorting, following the cursor, filtering, collapsing.

use super::*;

fn outline_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline_set_sort(crate::app::outline::OutlineSort::Position);
    Ok(())
}

fn outline_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline_set_sort(crate::app::outline::OutlineSort::Name);
    Ok(())
}

fn outline_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline_set_sort(crate::app::outline::OutlineSort::Kind);
    Ok(())
}

fn outline_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline_toggle_follow_cursor();
    Ok(())
}

fn outline_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline_toggle_filter_on_type();
    Ok(())
}

fn outline_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline_collapse_all();
    Ok(())
}

fn outline_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.outline_expand_all();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 7] = [
        (
            "outline.sortByPosition",
            "Sort outline by position",
            outline_0,
        ),
        ("outline.sortByName", "Sort outline by name", outline_1),
        ("outline.sortByKind", "Sort outline by kind", outline_2),
        (
            "outline.followCursor",
            "Follow cursor in outline",
            outline_3,
        ),
        ("outline.filterOnType", "Filter outline on type", outline_4),
        ("outline.collapse", "Collapse all outline items", outline_5),
        ("outline.expand", "Expand all outline items", outline_6),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Outline", ALL, "true", None, handler);
    }
}
