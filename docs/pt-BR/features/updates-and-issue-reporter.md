# Atualizações e Relator de Problemas com Redação de Segredos

O Graphite inclui um gerenciador de atualizações baseado em GitHub Releases e um Relator de Problemas que remove credenciais automaticamente antes de gerar rascunhos de bug reports.

## Atualizações (`crates/graphite-app/src/app/updates/`)

- **Canais (`state.rs`)**: Suporta `stable` (`/releases/latest`) e `prerelease` (`/releases`), configurados em `<perfil>/updates-config.json`.
- **Comparação SemVer (`SemVer`)**: Compara `major.minor.patch` e rótulos de pré-lançamento de forma estrita antes de sinalizar atualização disponível.
- **Integridade obrigatória (`signature.rs`, `mod.rs`)**: Um pacote só é instalado se a versão publicar `<asset>.sig` (assinatura Ed25519 por uma chave de `TRUSTED_UPDATE_KEYS`) sobre `graphite-update-v1\n<tag>\n<asset>\n<sha256>\n` **e** os bytes baixados tiverem esse SHA-256 (calculado em streaming). O hash vem das notas da versão (`<hex>  <asset>`) ou do arquivo `<asset>.sha256`. Versões sem assinatura, sem hash, com chave não confiável ou com assinatura reaproveitada são rejeitadas antes de gravar. A assinatura é verificada antes de baixar o pacote.
- **Download em streaming (`transport.rs`)**: Pacotes são gravados em `<asset>.part` com progresso real, limite de tamanho, detecção de truncamento e cancelamento; só recebem o nome final após a verificação.
- **Seleção de asset**: Casa SO e arquitetura (`windows`/`linux`/`macos`, `x64`/`arm64`). Não há fallback para outra plataforma.
- **Instalador (`apply.rs`)**: Reverifica o SHA-256 do arquivo, exige que esteja em `<perfil>/updates/`, valida o cabeçalho do executável (PE/ELF/Mach-O), mantém o binário antigo como `.old` e faz rollback se o novo não iniciar. Marcador inválido é descartado, nunca repetido indefinidamente.
- **Cancelamento e polling**: Cancelar incrementa uma geração para ignorar resultados tardios, sem silenciar o poller. O poller lê o canal/`auto_check` atuais a cada ciclo e aplica backoff (10 min até 1 h) após falhas; limites de taxa do GitHub geram mensagem clara. O canal prerelease escolhe a maior versão semântica.
- **Transporte em Segundo Plano (`transport.rs`)**: Executa `GitHubReleasesTransport` em thread separada e entrega eventos via `UserEvent::Update`.

### Assinando uma versão
1. Uma vez: `cargo run -p graphite-app --example update_signer -- keygen <arquivo-chave-privada>`, mantenha a chave privada em segredo (nunca faça commit) e cole a chave pública exibida em `TRUSTED_UPDATE_KEYS`. Para rotacionar, adicione a nova chave primeiro.
2. Por asset: `cargo run -p graphite-app --example update_signer -- sign <arquivo-chave-privada> <tag> <arquivo-asset>`; publique o asset junto com `<asset>.sig` e `<asset>.sha256` gerados.
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
