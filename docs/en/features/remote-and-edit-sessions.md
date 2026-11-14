# Remote Workspaces, Tunnels, and Edit Sessions

Graphite supports OpenSSH host discovery and command execution, WebSocket tunnel framing for relay connections, forwarded port lifecycle management, and local Edit Sessions snapshots.

## Architecture Overview

- `crates/graphite-app/src/app/remote/ssh.rs` parses `~/.ssh/config` hosts, validates tokens against SSH option injection (`validate_ssh_token`), and delegates execution to the system `ssh` binary via `OpenSshTransport`.
- `crates/graphite-app/src/app/remote/tunnel.rs` implements an RFC 6455 WebSocket handshake (`Sec-WebSocket-Accept` with SHA-1 and Base64) and masked client frame codec over `TcpStream`, carrying JSON `RelayEnvelope` messages to a configured `ws://` relay endpoint.
- `crates/graphite-app/src/app/remote/edit_sessions.rs` persists working-copy snapshots in `<profile>/edit-sessions.json` and enforces relative path traversal protection when restoring modified buffers into a workspace.
- `crates/graphite-app/src/app/remote/mod.rs` coordinates active connection state, socket write backpressure pause/resume, forwarded port cleanup, remote settings files (`remote-settings.json`), and local-to-remote extension manifests (`remote-extensions.json`).

## Commands

### Remote
- `graphite.remote.reloadSshHosts`: Reloads host entries from `~/.ssh/config`.
- `graphite.remote.connectSshHost`: Connects to the configured or first discovered SSH host.
- `workbench.action.triggerReconnect`: Marks the active session reconnected.
- `workbench.action.pauseSocketWriting`: Toggles socket write pause mode.
- `workbench.remote.action.closeUnusedPorts`: Closes idle forwarded ports.
- `workbench.action.openRemoteSettings` / `workbench.action.openRemoteSettingsFile`: Opens `remote-settings.json` in the profile configuration directory.
- `workbench.extensions.actions.installLocalExtensionsInRemote`: Records installed local extensions into `remote-extensions.json`.

### Edit Sessions
- `workbench.action.continueOn.extensions` / `workbench.editSessions.actions.store` / `workbench.editSessions.actions.storeCurrent`: Saves open dirty workspace buffers into `edit-sessions.json`.
- `workbench.editSessions.actions.resume` / `workbench.editSessions.actions.resumeLatest`: Restores the latest snapshot into the active workspace.
- `workbench.editSessions.actions.resumeFromSerializedPayload`: Parses and restores a JSON `EditSession` payload safely.
- `workbench.editSessions.actions.showEditSessions` / `workbench.editSessions.actions.showOutputChannel`: Displays stored sessions and the Edit Sessions activity log.
- `workbench.editSessions.actions.delete` / `workbench.editSessions.actions.deleteAll`: Removes stored edit sessions.
- `workbench.editSessions.actions.resetAuth`: Resets authentication state associated with remote and edit session flows.
