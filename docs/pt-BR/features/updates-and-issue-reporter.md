# Atualizações e Relator de Problemas com Redação de Segredos

O Graphite inclui um gerenciador de atualizações baseado em GitHub Releases e um Relator de Problemas que remove credenciais automaticamente antes de gerar rascunhos de bug reports.

## Atualizações (`crates/graphite-app/src/app/updates/`)

- **Canais (`state.rs`)**: Suporta `stable` (`/releases/latest`) e `prerelease` (`/releases`), configurados em `<perfil>/updates-config.json`.
- **Comparação SemVer (`SemVer`)**: Compara `major.minor.patch` e rótulos de pré-lançamento de forma estrita antes de sinalizar atualização disponível.
- **Verificação SHA-256**: Valida pacotes baixados via `ring::digest::SHA256` contra o hash declarado nas notas da versão antes de gravar em `<perfil>/updates/<tag>/`.
- **Transporte em Segundo Plano (`transport.rs`)**: Executa `GitHubReleasesTransport` em thread separada e entrega eventos via `UserEvent::Update`.

### Comandos de Atualização
- `update.check` / `update.checkForUpdate`: Verifica novas versões disponíveis.
- `update.downloadNow` / `update.downloadUpdate`: Baixa e verifica a integridade SHA-256 do pacote da plataforma.
- `update.install` / `update.installUpdate` / `update.restart` / `update.restartToUpdate`: Prepara e agenda a instalação (`pending-install.json`) para a próxima reinicialização.
- `update.cancelling`: Cancela o fluxo de atualização em andamento.
- `update.checking` / `update.downloading` / `update.updating` / `update.showUpdateInfo`: Exibe o resumo de versão e status de atualização.
- `update.showUpdateReleaseNotes`: Abre as notas da versão atual ou disponível.

## Relator de Problemas e Solução de Problemas (`crates/graphite-app/src/app/issue_reporter/`)

- **Redação de Segredos (`redact.rs`)**: Substitui automaticamente tokens do GitHub, chaves de API (`sk-`, `sk-ant-`, `AIza`, tokens Slack), cabeçalhos `Authorization`, chaves JSON/env, blocos PEM privados e caminhos do diretório do usuário por `[REDACTED]` / `~`.
- **Sessão de Solução de Problemas**: Registra o estado da sessão e metadados de ambiente (versão, SO, arquitetura, contagem de extensões instaladas, alias remoto) sem coletar credenciais.

### Comandos de Relato e Diagnóstico
- `workbench.action.openIssueReporter`: Gera um rascunho Markdown com segredos redigidos e a URL pré-preenchida para abrir issue no GitHub.
- `workbench.action.troubleshootIssue.start` / `workbench.action.troubleshootIssue.stop`: Inicia e encerra uma sessão guiada de diagnóstico.
- `workbench.action.chat.reportIssueForBug`: Abre o relator de problemas pré-preenchido para falhas no chat/agente.
