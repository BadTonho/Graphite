//! Timeline view commands.

use super::*;

fn timeline_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_timeline();
    Ok(())
}

fn timeline_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_timeline();
    Ok(())
}

fn timeline_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_scm_history();
    Ok(())
}

fn timeline_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_timeline_follow();
    Ok(())
}

fn timeline_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.timeline_compare_selected();
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 5] = [
        ("timeline.focus", "Focus on timeline view", timeline_0),
        ("timeline.refresh", "Refresh timeline", timeline_1),
        (
            "timeline.showWorkspaceTimeline",
            "Show timeline of the workspace",
            timeline_2,
        ),
        (
            "timeline.toggleFollowActiveEditor",
            "Follow the active editor in the timeline",
            timeline_3,
        ),
        (
            "timeline.compareSelected",
            "Compare the selected timeline entry",
            timeline_4,
        ),
    ];
    for (id, title, handler) in commands {
        add(registry, id, title, "Timeline", ALL, "true", None, handler);
    }
}
