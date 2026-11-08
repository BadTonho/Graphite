use super::command_execution::{CommandError, CommandInvocation};
use super::command_registry::{
    ArgumentKind, CommandDescriptor, CommandRegistry, Handler, TargetKind,
};
use super::context_keys::WidgetKind;
use super::*;
pub(crate) mod configuration;
mod editor;
mod input;
mod scm;
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
        "shoppe.extensions.restartHost",
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
    configuration::register(registry);
    for (id, title, arguments, handler) in [
        (
            "type",
            "Insert text",
            ArgumentKind::Text,
            input::insert_text as Handler,
        ),
        (
            "shoppe.editor.setValue",
            "Replace document text",
            ArgumentKind::Text,
            input::set_value as Handler,
        ),
        (
            "shoppe.editor.setSelection",
            "Set selection",
            ArgumentKind::Selection,
            input::set_selection as Handler,
        ),
        (
            "shoppe.editor.setCursor",
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
