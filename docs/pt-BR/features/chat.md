# Chat, Chat Inline e Sessões do Agente

O Graphite inclui um assistente conversacional e agente de workspace integrado com a crate `graphite-chat` e superfícies nativas dentro do `graphite-app`.

## Provedores e Configuração

- Transportes compatíveis: API Messages da Anthropic (`anthropic`) e endpoints compatíveis com OpenAI Chat Completions (`openai-compatible`).
- Streaming em segundo plano: Eventos Server-Sent Events (SSE) processados em thread dedicada para manter o editor responsivo.
- Armazenamento: A configuração do provedor e a chave de API são salvas localmente em `chat-provider.json` na pasta do perfil ativo (`workbench.action.openLanguageModelsJson`).
- Diálogo de configuração: Aberto automaticamente ao enviar um prompt sem credenciais válidas, ou pressionando `F4` com foco no campo de entrada do Chat.

## Superfícies e Navegação

- **Barra Lateral Secundária** (`Ctrl+Alt+I`, `Exibir -> Chat` ou `chat.action.focus`): Exibe o histórico de turnos, barra de aprovação de ferramentas, edições pendentes, lista recolhível de Sessões do Agente e compositor.
- **Chat no Editor** (`workbench.action.chat.openInEditor`): Move a conversa ativa para a área principal do editor.
- **Chat Rápido** (`Ctrl+Shift+Alt+C`, `graphite.chat.openQuickChat`): Abre um painel flutuante que pode ser promovido para a barra lateral (`workbench.action.quickchat.openInChatView`) ou fechado com `Esc`.
- **Chat Inline** (`Ctrl+I`, `inlineChat.focus` ou `inlineChat.fixDiagnostics`): Abre diretamente sobre a seleção ou diagnósticos do editor ativo para aplicar alterações (`inlineChat2.keep`) ou continuar na barra lateral (`inlineChat2.continueInChat`).

## Modos do Agente, Ferramentas e Aprovações

- Modos (`Ctrl+M` dentro do Chat para alternar):
  - **Ask**: Perguntas e respostas com leitura de contexto e busca somente leitura no workspace.
  - **Edit**: Propõe edições em arquivos acompanhadas de snapshots antes/depois para revisão.
  - **Agent**: Executa ferramentas de workspace (leitura, busca, edição de arquivos e comandos no terminal dentro da raiz do workspace) em múltiplos passos.
- **Restrição de caminhos e aprovações**: Todos os caminhos de ferramentas são validados dentro da raiz do workspace (`resolve_workspace_path`). Alterações e execuções respeitam `ToolApprovalPolicy` (`AlwaysAsk`, `AutoApproveReads`, `AutoApproveAll`), solicitando confirmação explícita quando necessário.
- **Pontos de verificação e edições**: Edições pendentes podem ser aceitas, descartadas, inspecionadas em diff (`chatEditing.openFileInDiff`) ou revertidas via pontos de verificação (`workbench.action.chat.undoEdit` / `redoEdit` / `restoreLastCheckpoint`).

## Sessões, Área de Transferência e Exportação

- Sessões são persistidas por perfil com suporte a fixar, arquivar, marcar como lida, alternar orientação (`lado a lado` ou `empilhado`) e filtros (`Arquivadas`, `Execuções de Automação`, `Agendadas`).
- Blocos de código das respostas podem ser navegados (`nextCodeBlock` / `previousCodeBlock`), copiados (`copyCodeBlock`), inseridos no cursor, em novo arquivo ou célula de notebook, ou enviados ao terminal integrado (`runInTerminal`).
- Conversas e logs de depuração do agente podem ser exportados e importados em JSON em `<perfil>/chat-exports/`.

> **Nota:** A renderização visual em janela desktop das superfícies de Chat, Chat Inline e diálogo de provedor ainda não foi inspecionada interativamente em tela; o comportamento é coberto por testes automatizados unitários e de comandos.
