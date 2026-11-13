use super::command_execution::{CommandError, CommandInvocation};
use super::command_registry::{
    ArgumentKind, CommandDescriptor, CommandRegistry, Handler, TargetKind,
};
use super::context_keys::WidgetKind;
use super::*;
pub(crate) mod configuration;
mod changes;
mod debug_extra;
mod editor;
mod explorer;
mod extensions;
mod files;
mod focus;
mod groups;
mod history;
mod input;
mod layout;
mod lists;
mod misc;
mod outline_commands;
mod output_snippets;
mod parts;
mod preferences;
mod quick;
mod scm;
mod task_commands;
mod merge_action_commands;
mod merge_commands;
mod notebook_commands;
mod notebook_run_commands;
mod multi_diff_commands;
mod merge_resolution_commands;
mod timeline_commands;
mod ui;
mod widget_controls;
mod widgets;
mod workbench;
const ALL: &[TargetKind] = &[TargetKind::Global, TargetKind::View, TargetKind::Widget];
const VIEW: &[TargetKind] = &[TargetKind::View];
const WIDGET: &[TargetKind] = &[TargetKind::Widget];
fn add(
    registry: &mut CommandRegistry,
    id: &'static str,
    title: &'static str,
    category: &'static str,
    targets: &'static [TargetKind],
    enablement: &'static str,
    widget: Option<WidgetKind>,
    handler: Handler,
) {
    registry
        .register(CommandDescriptor {
            id,
            title,
            category,
            targets,
            enablement,
            widget,
            menu_when: None,
            arguments: ArgumentKind::None,
            handler,
        })
        .expect("unique built-in command");
}
pub(super) fn register(registry: &mut CommandRegistry) {
    super::debugging::register(registry);
    add(
        registry,
        "graphite.plugins.open",
        "Open Plugins",
        "Plugins",
        ALL,
        "true",
        None,
        |app, _| {
            app.open_plugins();
            Ok(())
        },
    );
    add(
        registry,
        "graphite.activityBar.default",
        "Activity Bar: Default Position",
        "Activity Bar",
        ALL,
        "true",
        None,
        workbench::activity_bar_default,
    );
    add(
        registry,
        "graphite.activityBar.top",
        "Activity Bar: Top",
        "Activity Bar",
        ALL,
        "true",
        None,
        workbench::activity_bar_top,
    );
    add(
        registry,
        "graphite.activityBar.bottom",
        "Activity Bar: Bottom",
        "Activity Bar",
        ALL,
        "true",
        None,
        workbench::activity_bar_bottom,
    );
    add(
        registry,
        "graphite.activityBar.restore",
        "Show Activity Bar",
        "Activity Bar",
        ALL,
        "true",
        None,
        workbench::activity_bar_default,
    );
    add(
        registry,
        "graphite.activityBar.hide",
        "Hide Activity Bar",
        "Activity Bar",
        ALL,
        "true",
        None,
        workbench::activity_bar_hidden,
    );
    add(
        registry,
        "graphite.activityBar.toggleSize",
        "Toggle Activity Bar Size",
        "Activity Bar",
        ALL,
        "true",
        None,
        workbench::activity_bar_toggle_size,
    );
    add(
        registry,
        "graphite.activityBar.moveSidebarRight",
        "Move Primary Side Bar Right",
        "Activity Bar",
        ALL,
        "true",
        None,
        workbench::activity_bar_move_sidebar,
    );
    add(
        registry,
        "graphite.extensions.restartHost",
        "Restart simulated extension host",
        "Developer",
        ALL,
        "extensionHostEnabled",
        None,
        |app, _| app.restart_extension_host(),
    );
    ui::register(registry);
    widget_controls::register(registry);
    editor::register(registry);
    widgets::register(registry);
    workbench::register(registry);
    scm::register(registry);
    explorer::register(registry);
    focus::register(registry);
    groups::register(registry);
    parts::register(registry);
    quick::register(registry);
    lists::register(registry);
    misc::register(registry);
    files::register(registry);
    changes::register(registry);
    history::register(registry);
    preferences::register(registry);
    task_commands::register(registry);
    debug_extra::register(registry);
    extensions::register(registry);
    output_snippets::register(registry);
    outline_commands::register(registry);
    timeline_commands::register(registry);
    merge_commands::register(registry);
    notebook_commands::register(registry);
    notebook_run_commands::register(registry);
    multi_diff_commands::register(registry);
    merge_commands::register_conflict_view(registry);
    merge_action_commands::register(registry);
    merge_resolution_commands::register(registry);
    layout::register(registry);
    configuration::register(registry);
    for (id, title, arguments, handler) in [
        (
            "type",
            "Insert text",
            ArgumentKind::Text,
            input::insert_text as Handler,
        ),
        (
            "graphite.editor.setValue",
            "Replace document text",
            ArgumentKind::Text,
            input::set_value as Handler,
        ),
        (
            "graphite.editor.setSelection",
            "Set selection",
            ArgumentKind::Selection,
            input::set_selection as Handler,
        ),
        (
            "graphite.editor.setCursor",
            "Set cursor",
            ArgumentKind::Position,
            input::set_cursor as Handler,
        ),
    ] {
        let enablement = if matches!(arguments, ArgumentKind::Text) {
            "editorWritable"
        } else {
            "documentAvailable"
        };
        registry
            .register(CommandDescriptor {
                id,
                title,
                category: "Editor",
                targets: VIEW,
                enablement,
                menu_when: None,
                widget: None,
                arguments,
                handler,
            })
            .unwrap();
    }
}
