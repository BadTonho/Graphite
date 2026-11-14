//! Remote workspaces (OpenSSH), WebSocket tunnels, and Edit Sessions commands.

use super::*;
use crate::app::remote::edit_sessions::EditSessionStore;
use crate::app::remote::tunnel::{RelayEnvelope, WebSocketTunnelClient};

type Entry = (&'static str, &'static str, &'static str, Handler);

fn arg_str(inv: &CommandInvocation) -> Option<&str> {
    inv.request
        .arguments
        .as_str()
        .or_else(|| inv.request.arguments.get("text").and_then(|v| v.as_str()))
        .filter(|s| !s.trim().is_empty())
}

fn trigger_reconnect(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.trigger_remote_reconnect();
    Ok(())
}

fn pause_socket_writing(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.toggle_pause_socket_writing();
    Ok(())
}

fn close_unused_ports(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.close_unused_remote_ports();
    Ok(())
}

fn open_remote_settings(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.open_remote_settings();
    Ok(())
}

fn install_local_extensions_remote(
    app: &mut App,
    _: &CommandInvocation,
) -> Result<(), CommandError> {
    let dest = app
        .remote
        .active_host
        .as_ref()
        .map(|h| h.destination())
        .unwrap_or_else(|| "remote host".into());
    app.status = format!("Prepared local extension manifest sync for {dest}");
    Ok(())
}

fn store_edit_session(app: &mut App, inv: &CommandInvocation) -> Result<(), CommandError> {
    let name = arg_str(inv);
    app.store_current_edit_session(name);
    Ok(())
}

fn resume_edit_session(app: &mut App, inv: &CommandInvocation) -> Result<(), CommandError> {
    if let Some(id) = arg_str(inv) {
        if let Some(record) = app
            .remote
            .edit_sessions
            .sessions
            .iter()
            .find(|s| s.id == id)
            .cloned()
        {
            app.apply_edit_session_record(&record)
                .map_err(CommandError::Failed)?;
            return Ok(());
        }
    }
    app.resume_latest_edit_session()
        .map_err(CommandError::Failed)?;
    Ok(())
}

fn resume_latest_edit_session(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.resume_latest_edit_session()
        .map_err(CommandError::Failed)?;
    Ok(())
}

fn resume_from_payload(app: &mut App, inv: &CommandInvocation) -> Result<(), CommandError> {
    let payload = if let Some(raw) = arg_str(inv) {
        raw.to_owned()
    } else {
        app.editor().text()
    };
    app.resume_edit_session_from_payload(&payload)
        .map_err(CommandError::Failed)?;
    Ok(())
}

fn show_edit_sessions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.show_edit_sessions();
    Ok(())
}

fn show_output_channel(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.show_edit_session_output();
    Ok(())
}

fn delete_edit_session(app: &mut App, inv: &CommandInvocation) -> Result<(), CommandError> {
    let target_id = arg_str(inv).map(str::to_owned).or_else(|| {
        app.remote
            .edit_sessions
            .sessions
            .first()
            .map(|s| s.id.clone())
    });
    if let Some(id) = target_id {
        if app
            .remote
            .edit_sessions
            .delete_by_id(&id, &app.profile_root)
        {
            app.status = format!("Deleted edit session {id}");
        } else {
            app.status = format!("Edit session not found: {id}");
        }
    } else {
        app.status = "No edit sessions to delete".into();
    }
    Ok(())
}

fn delete_all_edit_sessions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let count = app.remote.edit_sessions.delete_all(&app.profile_root);
    app.status = format!("Deleted {count} edit session(s)");
    Ok(())
}

fn reset_edit_session_auth(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.remote.tunnel_endpoint = None;
    app.remote
        .edit_sessions
        .log("Cleared edit session relay authentication");
    app.status = "Reset edit sessions relay state".into();
    Ok(())
}

fn continue_on_extensions(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    let record = app.store_current_edit_session(Some("Continue On"));
    let json = serde_json::to_string_pretty(&record).unwrap_or_default();
    if let Some(endpoint) = app.remote.tunnel_endpoint.clone() {
        if let Ok(mut client) = WebSocketTunnelClient::connect(&endpoint) {
            let _ = client.send_envelope(&RelayEnvelope::EditSnapshot {
                session_id: record.id.clone(),
                payload_json: json.clone(),
            });
            client.close();
        }
    }
    app.remote.last_message = Some(format!("Prepared Continue On payload ({})", record.id));
    app.status = format!(
        "Stored working changes ({}) to continue on another host",
        record.id
    );
    Ok(())
}

fn reload_ssh_hosts(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    app.reload_ssh_hosts(None);
    Ok(())
}

fn connect_ssh_first_host(app: &mut App, _: &CommandInvocation) -> Result<(), CommandError> {
    if app.remote.ssh_hosts.is_empty() {
        app.reload_ssh_hosts(None);
    }
    if let Some(host) = app.remote.ssh_hosts.first().cloned() {
        app.connect_ssh_host(host);
    } else {
        app.status = "No SSH hosts configured in ~/.ssh/config".into();
    }
    Ok(())
}

pub(super) fn register(registry: &mut CommandRegistry) {
    let entries: &[Entry] = &[
        (
            "workbench.action.triggerReconnect",
            "Reconnect Remote Window",
            "Remote",
            trigger_reconnect,
        ),
        (
            "workbench.action.pauseSocketWriting",
            "Pause Remote Socket Writing",
            "Remote",
            pause_socket_writing,
        ),
        (
            "workbench.remote.action.closeUnusedPorts",
            "Close Unused Forwarded Ports",
            "Remote",
            close_unused_ports,
        ),
        (
            "workbench.action.openRemoteSettings",
            "Open Remote Settings",
            "Remote",
            open_remote_settings,
        ),
        (
            "workbench.action.openRemoteSettingsFile",
            "Open Remote Settings (JSON)",
            "Remote",
            open_remote_settings,
        ),
        (
            "workbench.extensions.actions.installLocalExtensionsInRemote",
            "Install Local Extensions in Remote",
            "Remote",
            install_local_extensions_remote,
        ),
        (
            "graphite.remote.reloadSshHosts",
            "Reload SSH Host Config",
            "Remote",
            reload_ssh_hosts,
        ),
        (
            "graphite.remote.connectSshHost",
            "Connect to First Configured SSH Host",
            "Remote",
            connect_ssh_first_host,
        ),
        // Edit Sessions
        (
            "workbench.action.continueOn.extensions",
            "Continue Working On (Store & Share Changes)",
            "Edit Sessions",
            continue_on_extensions,
        ),
        (
            "workbench.editSessions.actions.store",
            "Store Working Changes in Edit Session",
            "Edit Sessions",
            store_edit_session,
        ),
        (
            "workbench.editSessions.actions.storeCurrent",
            "Store Current Working Changes",
            "Edit Sessions",
            store_edit_session,
        ),
        (
            "workbench.editSessions.actions.resume",
            "Resume Edit Session",
            "Edit Sessions",
            resume_edit_session,
        ),
        (
            "workbench.editSessions.actions.resumeLatest",
            "Resume Latest Edit Session",
            "Edit Sessions",
            resume_latest_edit_session,
        ),
        (
            "workbench.editSessions.actions.resumeFromSerializedPayload",
            "Resume Edit Session from Payload",
            "Edit Sessions",
            resume_from_payload,
        ),
        (
            "workbench.editSessions.actions.showEditSessions",
            "Show Stored Edit Sessions",
            "Edit Sessions",
            show_edit_sessions,
        ),
        (
            "workbench.editSessions.actions.showOutputChannel",
            "Show Edit Sessions Output Log",
            "Edit Sessions",
            show_output_channel,
        ),
        (
            "workbench.editSessions.actions.delete",
            "Delete Edit Session",
            "Edit Sessions",
            delete_edit_session,
        ),
        (
            "workbench.editSessions.actions.deleteAll",
            "Delete All Edit Sessions",
            "Edit Sessions",
            delete_all_edit_sessions,
        ),
        (
            "workbench.editSessions.actions.resetAuth",
            "Reset Edit Sessions Relay State",
            "Edit Sessions",
            reset_edit_session_auth,
        ),
    ];

    for (id, title, category, handler) in entries {
        add(registry, id, title, category, ALL, "true", None, *handler);
    }

    let _ = EditSessionStore::parse_serialized_payload;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::remote::ssh::SshHostConfig;
    use crate::app::remote::ConnectionStatus;

    #[test]
    fn remote_and_edit_session_commands_round_trip_cleanly() {
        let dir = crate::test_support::TempDir::new();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "before remote edit\n").unwrap();
        let mut app = App::new_with_options(dir.path().into(), None, true);
        app.open_file(file.clone());

        app.connect_ssh_host(SshHostConfig {
            alias: "dev".into(),
            hostname: Some("127.0.0.1".into()),
            user: Some("user".into()),
            port: None,
            identity_file: None,
        });
        assert_eq!(app.remote.status, ConnectionStatus::Connected);

        app.run_ui_command("workbench.action.pauseSocketWriting");
        assert!(app.remote.socket_paused);
        assert_eq!(app.remote.status, ConnectionStatus::Paused);

        app.run_ui_command("workbench.editSessions.actions.storeCurrent");
        assert_eq!(app.remote.edit_sessions.sessions.len(), 1);

        std::fs::write(&file, "modified\n").unwrap();
        app.run_ui_command("workbench.editSessions.actions.resumeLatest");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "before remote edit\n"
        );

        app.run_ui_command("workbench.editSessions.actions.showEditSessions");
        assert!(app.editor().text().contains("Stored Edit Sessions"));

        app.run_ui_command("workbench.editSessions.actions.showOutputChannel");
        assert!(app.editor().text().contains("Resumed edit session"));

        app.run_ui_command("workbench.editSessions.actions.deleteAll");
        assert!(app.remote.edit_sessions.sessions.is_empty());
    }
}
