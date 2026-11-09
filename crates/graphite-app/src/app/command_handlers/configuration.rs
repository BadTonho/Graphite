use super::super::command_execution::{deliver_command_event, CommandResult};
use super::*;

#[derive(Debug)]
pub(crate) struct ImportedSettings {
    pub report: config::ImportReport,
    pub prefs: config::EditorPrefs,
    pub colors: HashMap<String, String>,
    pub dark: bool,
    pub snippets: Vec<config::Snippet>,
    pub warnings: Vec<String>,
    pub keymap: crate::keybindings::Keymap,
}

fn load_settings(
    root: &Path,
    cancellation: &super::super::command_execution::CancellationToken,
    translator: graphite_i18n::Translator,
) -> Result<ImportedSettings, CommandError> {
    cancellation.check()?;
    let report = config::import_vscode(&translator);
    cancellation.check()?;
    let prefs =
        config::editor_prefs_for_workspace(root, &translator).map_err(CommandError::Failed)?;
    let colors = config::imported_theme_colors(root, &translator).map_err(CommandError::Failed)?;
    let dark = config::load_dark_theme();
    let (snippets, warnings) = config::load_snippets_for_workspace(root, &translator);
    let path = config::app_config_dir().join("keybindings.json");
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "[]".into(),
        Err(error) => {
            return Err(CommandError::Failed(
                translator.format("config.keybindings", &[("error", &error)]),
            ))
        }
    };
    let mut keymap = crate::keybindings::Keymap::default();
    keymap.reload(&source, &translator).map_err(|error| {
        CommandError::Failed(translator.format("config.keybindings", &[("error", &error)]))
    })?;
    cancellation.check()?;
    Ok(ImportedSettings {
        report,
        prefs,
        colors,
        dark,
        snippets,
        warnings,
        keymap,
    })
}

pub(super) fn import(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let proxy = app
        .event_proxy
        .clone()
        .ok_or_else(|| CommandError::Failed(app.t("error.no_event_loop").into()))?;
    let root = app.tree.root().to_path_buf();
    let translator = app.translator;
    let job = app
        .command_job(false)
        .expect("registered command execution");
    std::thread::Builder::new()
        .name("graphite-import-settings".into())
        .spawn(
            move || match load_settings(&root, &job.cancellation, translator) {
                Ok(settings) => deliver_command_event(
                    &proxy,
                    Some(&job),
                    UserEvent::SettingsImported(Box::new(settings)),
                    Ok(serde_json::Value::Null),
                ),
                Err(error) => {
                    let _ = proxy.send_event(UserEvent::CommandCompleted {
                        execution: job.id,
                        event: None,
                        result: Err(error),
                    });
                }
            },
        )
        .map_err(|error| CommandError::Failed(error.to_string()))?;
    Ok(())
}
pub(super) fn snippets(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let proxy = app
        .event_proxy
        .clone()
        .ok_or_else(|| CommandError::Failed(app.t("error.no_event_loop").into()))?;
    let root = app.tree.root().to_path_buf();
    let translator = app.translator;
    let job = app
        .command_job(false)
        .expect("registered command execution");
    std::thread::Builder::new()
        .name("graphite-load-snippets".into())
        .spawn(move || {
            let result: CommandResult = job.cancellation.check().map(|_| serde_json::Value::Null);
            if let Err(error) = result {
                let _ = proxy.send_event(UserEvent::CommandCompleted {
                    execution: job.id,
                    event: None,
                    result: Err(error),
                });
                return;
            }
            let (snippets, warnings) = config::load_snippets_for_workspace(&root, &translator);
            deliver_command_event(
                &proxy,
                Some(&job),
                UserEvent::SnippetsLoaded { snippets, warnings },
                Ok(serde_json::Value::Null),
            );
        })
        .map_err(|error| CommandError::Failed(error.to_string()))?;
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    add(
        registry,
        "graphite.action.insertSnippet",
        "Insert snippet",
        "Workbench",
        VIEW,
        "true",
        None,
        snippets,
    );
    add(
        registry,
        "graphite.action.importVSCode",
        "Import vscode",
        "Workbench",
        ALL,
        "true",
        None,
        import,
    );
}
