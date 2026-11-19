//! Commands for the extensions side bar: enabling and disabling, searching and installing.

use super::*;

fn ext_0(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.plugins_enable_all();
    Ok(())
}

fn ext_1(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.plugins_disable_all();
    Ok(())
}

fn ext_2(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.plugins_search_open_vsx("");
    Ok(())
}

fn ext_3(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.plugins_search_open_vsx("");
    Ok(())
}

fn ext_4(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.plugins_search_open_vsx("");
    Ok(())
}

fn ext_5(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.plugins_search_language_extensions();
    Ok(())
}

fn ext_6(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.plugins_search_language_extensions();
    Ok(())
}

fn ext_7(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.plugins_search_language_extensions();
    Ok(())
}

fn ext_show_builtin(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.plugins_show_installed();
    Ok(())
}

fn ext_open_folder(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let Some(root) = app.plugins.store.as_ref().map(|store| store.root.clone()) else {
        return Err(CommandError::Disabled(
            "No extensions folder configured".into(),
        ));
    };
    let _ = std::fs::create_dir_all(&root);
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(&root).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(&root).spawn();
    app.status = root.to_string_lossy().into_owned();
    Ok(())
}

fn ext_check_updates(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_plugins();
    app.plugins_check_updates();
    Ok(())
}

fn ext_update_all(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_plugins();
    app.plugins_update_all();
    Ok(())
}

fn ext_arg_0(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let text = invocation.request.arguments["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    app.plugins_search_open_vsx(&text);
    Ok(())
}

fn ext_arg_1(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let text = invocation.request.arguments["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    app.plugins_install_from_open_vsx(&text)
        .map_err(CommandError::Failed)
}

fn ext_arg_2(app: &mut App, invocation: &CommandInvocation) -> Result<(), CommandError> {
    let text = invocation.request.arguments["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    app.plugins_uninstall(&text).map_err(CommandError::Failed)
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let commands: [(&'static str, &'static str, Handler); 12] = [
        (
            "workbench.extensions.action.enableAll",
            "Enable all extensions",
            ext_0,
        ),
        (
            "workbench.extensions.action.disableAll",
            "Disable all installed extensions",
            ext_1,
        ),
        (
            "workbench.extensions.action.showPopularExtensions",
            "Show popular extensions",
            ext_2,
        ),
        (
            "workbench.extensions.action.showRecommendedExtensions",
            "Show recommended extensions",
            ext_3,
        ),
        (
            "workbench.extensions.action.recentlyPublishedExtensions",
            "Show recently published extensions",
            ext_4,
        ),
        (
            "workbench.action.showLanguageExtensions",
            "Search language extensions",
            ext_5,
        ),
        (
            "workbench.extensions.action.showExtensionsForLanguage",
            "Search extensions for the active language",
            ext_6,
        ),
        (
            "workbench.extensions.action.showLanguageExtensions",
            "Search language extensions for this file",
            ext_7,
        ),
        (
            "workbench.extensions.action.showBuiltInExtensions",
            "Show built-in extensions",
            ext_show_builtin,
        ),
        (
            "workbench.extensions.action.openExtensionsFolder",
            "Open extensions folder",
            ext_open_folder,
        ),
        (
            "workbench.extensions.action.checkForUpdates",
            "Check for extension updates",
            ext_check_updates,
        ),
        (
            "workbench.extensions.action.updateAll",
            "Update all extensions",
            ext_update_all,
        ),
    ];
    for (id, title, handler) in commands {
        add(
            registry,
            id,
            title,
            "Extensions",
            ALL,
            "true",
            None,
            handler,
        );
    }
    // These take the search text or extension id as their argument.
    let with_argument: [(&'static str, &'static str, Handler); 3] = [
        (
            "workbench.extensions.search",
            "Search extensions",
            ext_arg_0,
        ),
        (
            "workbench.extensions.installExtension",
            "Install extension",
            ext_arg_1,
        ),
        (
            "workbench.extensions.uninstallExtension",
            "Uninstall extension",
            ext_arg_2,
        ),
    ];
    for (id, title, handler) in with_argument {
        registry
            .register(CommandDescriptor {
                id,
                title,
                category: "Extensions",
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
