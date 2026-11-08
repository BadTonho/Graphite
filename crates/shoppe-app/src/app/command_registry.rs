//! Command descriptions are independent of keybindings and menu visibility.
use super::command_execution::{CommandError, CommandInvocation};
use super::*;
pub(crate) use graphite_core_api::{CommandTarget, TargetKind};

pub(crate) type Handler = fn(&mut App, &CommandInvocation) -> Result<(), CommandError>;

#[derive(Clone)]
pub(crate) struct CommandDescriptor {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub targets: &'static [TargetKind],
    pub enablement: &'static str,
    pub menu_when: Option<&'static str>,
    pub arguments: ArgumentKind,
    pub widget: Option<super::context_keys::WidgetKind>,
    pub handler: Handler,
}
#[derive(Clone, Copy)]
pub(crate) enum ArgumentKind {
    None,
    Text,
    Index,
    Selection,
    Position,
}

#[derive(Default)]
pub(crate) struct CommandRegistry {
    entries: HashMap<&'static str, CommandDescriptor>,
}
impl CommandRegistry {
    pub fn register(&mut self, command: CommandDescriptor) -> Result<(), String> {
        if self.entries.contains_key(command.id) {
            return Err(format!("Duplicate command: {}", command.id));
        }
        if command.title.is_empty() || command.category.is_empty() {
            return Err("Command metadata must be nonempty".into());
        }
        crate::keybindings::Condition::parse(command.enablement)?;
        if let Some(condition) = command.menu_when {
            crate::keybindings::Condition::parse(condition)?;
        }
        self.entries.insert(command.id, command);
        Ok(())
    }
    pub fn get(&self, id: &str) -> Option<&CommandDescriptor> {
        self.entries.get(id)
    }
    #[allow(dead_code)] // Metadata API for menu/palette consumers.
    pub fn iter(&self) -> impl Iterator<Item = &CommandDescriptor> {
        self.entries.values()
    }
    pub fn enabled(&self, id: &str, context: &HashMap<String, String>) -> bool {
        self.get(id).is_some_and(|entry| {
            crate::keybindings::Condition::parse(entry.enablement)
                .unwrap()
                .evaluate(context)
        })
    }
    #[allow(dead_code)] // Visibility stays independent from execution enablement.
    pub fn visible(&self, id: &str, context: &HashMap<String, String>) -> bool {
        self.get(id).is_some_and(|entry| {
            entry.menu_when.is_none_or(|condition| {
                crate::keybindings::Condition::parse(condition)
                    .unwrap()
                    .evaluate(context)
            })
        })
    }
}
pub(crate) fn registry() -> &'static CommandRegistry {
    static REGISTRY: OnceLock<CommandRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = CommandRegistry::default();
        super::command_handlers::register(&mut registry);
        #[cfg(test)]
        super::command_tests::register_test_commands(&mut registry);
        registry
    })
}
