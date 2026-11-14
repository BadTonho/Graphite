# Workspaces Remotos, Túneis e Sessões de Edição

O Graphite oferece descoberta e execução de hosts via OpenSSH do sistema, enquadramento WebSocket para conexões de relay/túnel, gerenciamento de portas encaminhadas e snapshots locais de Sessões de Edição.

## Visão Geral da Arquitetura

- `crates/graphite-app/src/app/remote/ssh.rs` lê hosts de `~/.ssh/config`, valida tokens contra injeção de opções SSH (`validate_ssh_token`) e delega execução ao binário `ssh` do sistema via `OpenSshTransport`.
- `crates/graphite-app/src/app/remote/tunnel.rs` implementa o handshake RFC 6455 WebSocket (`Sec-WebSocket-Accept` com SHA-1 e Base64) e codec de frames mascarados sobre `TcpStream`, transportando mensagens JSON `RelayEnvelope` para um endpoint `ws://` configurado.
- `crates/graphite-app/src/app/remote/edit_sessions.rs` persiste snapshots das alterações em `<perfil>/edit-sessions.json` e bloqueia path traversal ao restaurar buffers relativos ao workspace.
- `crates/graphite-app/src/app/remote/mod.rs` coordena o estado da conexão remota, pausa/retomada de escrita em socket, limpeza de portas encaminhadas ociosas, arquivo `remote-settings.json` e manifesto de extensões `remote-extensions.json`.

## Comandos

### Remoto
- `graphite.remote.reloadSshHosts`: Recarrega os hosts definidos em `~/.ssh/config`.
- `graphite.remote.connectSshHost`: Conecta ao host SSH selecionado ou ao primeiro host configurado.
- `workbench.action.triggerReconnect`: Aciona reconexão da sessão remota ativa.
- `workbench.action.pauseSocketWriting`: Alterna o modo de pausa de escrita no socket.
- `workbench.remote.action.closeUnusedPorts`: Fecha portas encaminhadas marcadas como ociosas.
- `workbench.action.openRemoteSettings` / `workbench.action.openRemoteSettingsFile`: Abre `remote-settings.json` no diretório de perfil.
- `workbench.extensions.actions.installLocalExtensionsInRemote`: Registra as extensões locais no manifesto `remote-extensions.json`.

### Sessões de Edição
- `workbench.action.continueOn.extensions` / `workbench.editSessions.actions.store` / `workbench.editSessions.actions.storeCurrent`: Salva buffers alterados do workspace em `edit-sessions.json`.
- `workbench.editSessions.actions.resume` / `workbench.editSessions.actions.resumeLatest`: Restaura o snapshot mais recente no workspace ativo.
- `workbench.editSessions.actions.resumeFromSerializedPayload`: Faz o parse e restaura com segurança um payload JSON de `EditSession`.
- `workbench.editSessions.actions.showEditSessions` / `workbench.editSessions.actions.showOutputChannel`: Exibe as sessões salvas e o log de atividades de Sessões de Edição.
- `workbench.editSessions.actions.delete` / `workbench.editSessions.actions.deleteAll`: Remove sessões salvas.
- `workbench.editSessions.actions.resetAuth`: Redefine o estado de autenticação associado a fluxos remotos e de sessões de edição.
