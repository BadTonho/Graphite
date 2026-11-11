use super::*;

#[derive(Clone)]
pub(crate) struct PaletteItem {
    pub id: String,
    pub title: String,
    pub source: String,
    pub enabled: bool,
}
#[derive(Default)]
pub(crate) struct CommandPalette {
    pub query: String,
    pub items: Vec<PaletteItem>,
    pub selected: usize,
    pub extension_prompt: Option<(String, bool)>,
}
impl App {
    pub(crate) fn open_command_palette(&mut self) {
        self.cancel_vsix_prompt();
        self.quick_open = None;
        self.local_search = None;
        self.refresh_command_context();
        let target =
            super::command_registry::CommandTarget::View(self.editor_groups.active_view().id.0);
        let context = self.context_keys.resolve_target(target);
        let mut items: Vec<_> = super::command_registry::registry()
            .iter()
            .map(|entry| PaletteItem {
                id: entry.id.into(),
                title: self
                    .translator
                    .text_or(&format!("command.{}", entry.id), entry.title)
                    .into(),
                source: "Graphite".into(),
                enabled: super::command_registry::registry().enabled(entry.id, &context),
            })
            .collect();
        items.extend(self.extension_host.commands.values().map(|command| {
            PaletteItem {
                id: command.metadata.id.clone(),
                title: command.metadata.title.clone(),
                source: "Extension".into(),
                enabled: command
                    .metadata
                    .targets
                    .contains(&graphite_core_api::TargetKind::View)
                    && crate::keybindings::Condition::parse(&command.metadata.enablement)
                        .is_ok_and(|condition| condition.evaluate(&context)),
            }
        }));
        items.extend(self.plugins.hosts.iter().flat_map(|(plugin_id, host)| {
            let context = &context;
            host.commands.values().map(move |command| PaletteItem {
                id: command.metadata.id.clone(),
                title: command.metadata.title.clone(),
                source: format!("Plugin {plugin_id}"),
                enabled: command
                    .metadata
                    .targets
                    .contains(&graphite_core_api::TargetKind::View)
                    && crate::keybindings::Condition::parse(&command.metadata.enablement)
                        .is_ok_and(|condition| condition.evaluate(&context)),
            })
        }));
        let trusted = self
            .workspace_trust
            .as_ref()
            .is_some_and(|trust| trust.is_trusted(self.tree.root()));
        if let Ok(tasks) = self.task_file() {
            items.extend(tasks.tasks.into_iter().map(|task| PaletteItem {
                id: format!("graphite.tasks.run:{}", task.label),
                title: format!("Run Task: {}", task.label),
                source: "Task".into(),
                enabled: trusted && self.task_cancel.is_none(),
            }));
        }
        items.extend(self.vsix_palette_items());
        items.sort_by(|a, b| {
            a.title
                .to_lowercase()
                .cmp(&b.title.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        self.command_palette = Some(CommandPalette {
            query: String::new(),
            items,
            selected: 0,
            extension_prompt: None,
        });
    }
    pub(crate) fn handle_command_palette(&mut self, event: &winit::event::KeyEvent) {
        let Some(palette) = &mut self.command_palette else {
            return;
        };
        match event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.command_palette = None;
                self.cancel_vsix_prompt();
            }
            Key::Named(NamedKey::ArrowDown) => {
                let count = palette
                    .items
                    .iter()
                    .filter(|item| matches(item, &palette.query))
                    .count();
                if count > 0 {
                    palette.selected = (palette.selected + 1) % count;
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                let count = palette
                    .items
                    .iter()
                    .filter(|item| matches(item, &palette.query))
                    .count();
                if count > 0 {
                    palette.selected = (palette.selected + count - 1) % count;
                }
            }
            Key::Named(NamedKey::Backspace) => {
                palette.query.pop();
                palette.selected = 0;
            }
            Key::Named(NamedKey::Enter) => self.execute_command_palette_selection(),
            _ if !self.modifiers.control_key() && !self.modifiers.alt_key() => {
                if let Some(text) = event
                    .text
                    .as_ref()
                    .filter(|text| !text.chars().any(char::is_control))
                {
                    palette.query.push_str(text);
                    palette.selected = 0;
                }
            }
            _ => {}
        }
    }
    pub(crate) fn command_palette_rows(&self) -> Vec<&PaletteItem> {
        let Some(palette) = &self.command_palette else {
            return Vec::new();
        };
        let matching: Vec<_> = palette
            .items
            .iter()
            .filter(|item| matches(item, &palette.query))
            .collect();
        let start = palette
            .selected
            .saturating_sub(7)
            .min(matching.len().saturating_sub(8));
        matching.into_iter().skip(start).take(8).collect()
    }

    pub(crate) fn execute_command_palette_selection(&mut self) {
        let Some(palette) = self.command_palette.as_ref() else {
            return;
        };
        let selected = palette
            .items
            .iter()
            .filter(|item| matches(item, &palette.query))
            .nth(palette.selected)
            .filter(|item| item.enabled || item.id.starts_with("graphite.vsix:"))
            .map(|item| item.id.clone());
        let answer = palette
            .extension_prompt
            .as_ref()
            .map(|_| (palette.query.clone(), selected.clone()));
        self.command_palette = None;
        if let Some(answer) = answer {
            self.answer_vsix_prompt(Some(answer));
            return;
        }
        if let Some(id) = selected {
            if let Some(command) = id.strip_prefix("graphite.vsix:") {
                if let Err(error) = self.run_vsix_command(command) {
                    self.status = error;
                }
            } else if let Some(label) = id.strip_prefix("graphite.tasks.run:") {
                if let Err(error) = self.run_task(label) {
                    self.status = error;
                }
            } else {
                self.run_ui_command(&id);
            }
        }
    }
}
pub(super) fn matches(item: &PaletteItem, query: &str) -> bool {
    let query = query.to_lowercase();
    query.is_empty()
        || item.title.to_lowercase().contains(&query)
        || item.id.to_lowercase().contains(&query)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_filters_builtin_commands_by_title_and_identifier() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.open_command_palette();
        assert!(!app.command_palette_rows().is_empty());
        app.command_palette.as_mut().unwrap().query = "quickOpen".into();
        assert!(app
            .command_palette_rows()
            .iter()
            .any(|item| item.title == "Quick open"));
    }

    #[test]
    fn trust_command_persists_workspace_authorization() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.workspace_trust =
            Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
        assert!(!app
            .workspace_trust
            .as_ref()
            .unwrap()
            .is_trusted(app.tree.root()));
        app.run_ui_command("graphite.security.trustWorkspace");
        assert!(app
            .workspace_trust
            .as_ref()
            .unwrap()
            .is_trusted(app.tree.root()));
        app.run_ui_command("graphite.security.revokeWorkspaceTrust");
        assert!(!app
            .workspace_trust
            .as_ref()
            .unwrap()
            .is_trusted(app.tree.root()));
    }

    #[test]
    fn revoking_workspace_trust_cancels_tasks_and_persists_restricted_state() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.workspace_trust =
            Some(crate::app::phase5::WorkspaceTrust::open(dir.path().join("trust.json")).unwrap());
        let task_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        app.task_cancel = Some(task_cancel.clone());
        app.run_ui_command("graphite.security.trustWorkspace");
        app.run_ui_command("graphite.security.revokeWorkspaceTrust");
        assert!(task_cancel.load(std::sync::atomic::Ordering::Acquire));
        assert!(!app
            .workspace_trust
            .as_ref()
            .unwrap()
            .is_trusted(app.tree.root()));
    }

    #[test]
    fn profile_commands_create_and_switch_isolated_profile_state() {
        let dir = crate::test_support::TempDir::new();
        let mut app = App::new_with_options(dir.path().to_path_buf(), None, true);
        app.profiles = Some(crate::app::phase5::Profiles::open(dir.path()).unwrap());
        app.profile_root = app.profiles.as_ref().unwrap().path("Default").unwrap();
        app.run_ui_command_args(
            "graphite.profiles.create",
            serde_json::json!({"text":"Work"}),
        );
        assert!(app.profiles.as_ref().unwrap().path("Work").is_ok());
        let work_profile = app.profiles.as_ref().unwrap().path("Work").unwrap();
        config::save_sidebar_width(&work_profile, 340.0).unwrap();
        app.run_ui_command_args(
            "graphite.profiles.switch",
            serde_json::json!({"text":"Work"}),
        );
        assert_eq!(app.profiles.as_ref().unwrap().active(), "Work");
        assert_eq!(app.profile_root, dir.path().join("profiles/Work"));
        assert_eq!(app.sidebar_width, 340.0);
        assert_eq!(
            app.plugin_store_root().unwrap(),
            dir.path().join("profiles/Work/plugins").as_path()
        );
    }
}
