# Chat do Graphite, Chat Inline e Sessões do Agente

O Graphite inclui um assistente conversacional e agente de workspace integrado com a crate `graphite-chat` e superfícies nativas dentro do `graphite-app`.

## Provedores e Configuração

- Abra **Configurar conexões** no Chat do Graphite, pressione `F4` no chat ou use o comando de configuração na paleta. Dê um nome à conexão, selecione um provedor e informe o ID do modelo e a chave de API, quando exigida. O Graphite preenche o endereço oficial dos provedores suportados. Para **Compatível com OpenAI**, informe a URL base da API, por exemplo `http://localhost:11434/v1` para o Ollama. Endereços remotos exigem HTTPS.
- As conexões pertencem ao perfil ativo. As chaves de API são guardadas no Gerenciador de Credenciais do Windows ou no Secret Service do Linux. Se o cofre falhar ao guardar a chave, **Usar nesta sessão** a mantém em memória até fechar o aplicativo. O Graphite remove o `chat-provider.json` anterior depois de migrar a configuração e a chave salva com sucesso.
- Escolha **Usar** para trocar de conexão. Selecionar outra conexão ou modelo inicia uma conversa vazia. Conversas com a conexão original ainda válida podem ser retomadas pelo histórico. Conversas antigas, importadas ou cujo destino original mudou ficam disponíveis para leitura.
- A resposta é recebida em segundo plano. Use **Parar** ou `Esc` para cancelar uma resposta antes de trocar de conversa.

| Provedor | O que informar |
| --- | --- |
| OpenAI ou Claude | A chave de API do provedor e o ID do modelo. |
| OpenCode Console | Uma chave de API do Console para um workspace com créditos e o ID do modelo no catálogo da API. A cobrança é por uso. [OpenCode Console](https://opencode.ai/docs/console/api/inference) |
| OpenCode Go | Uma chave de API coberta por uma assinatura Go e o ID do modelo no catálogo da API do Go. [OpenCode Go](https://opencode.ai/docs/console/go/) |
| Gemini | Uma chave de API do Google AI Studio e um ID de modelo Gemini aceito pelo endpoint Chat Completions do Google. [API do Gemini](https://ai.google.dev/gemini-api/docs/openai) |
| OpenRouter | Uma chave de API do OpenRouter e um ID de modelo no formato `provedor/modelo`. [API do OpenRouter](https://openrouter.ai/docs/api/reference/overview) |

Os modelos gratuitos de chat restritos ao cliente OpenCode ficam indisponíveis no Graphite. O Graphite usa um identificador de sessão do OpenCode separado para cada conversa. A disponibilidade das ferramentas depende do modelo escolhido. [Integrações do OpenCode](https://opencode.ai/docs/console/harnesses)

## Superfícies e Navegação

- **Barra Lateral Secundária** (`Ctrl+Alt+I`, `Exibir -> Chat` ou `chat.action.focus`): Exibe o Chat do Graphite ao lado das visualizações de extensões. Abra a **Estrutura** pela paleta de comandos.
- **Chat no Editor** (`workbench.action.chat.openInEditor`): Move a conversa ativa para a área principal do editor.
- **Chat Rápido** (`Ctrl+Shift+Alt+C`, `graphite.chat.openQuickChat`): Abre um painel flutuante que pode ser promovido para a barra lateral (`workbench.action.quickchat.openInChatView`) ou fechado com `Esc`.
- **Chat Inline** (`Ctrl+I`, `inlineChat.focus` ou `inlineChat.fixDiagnostics`): Abre diretamente sobre a seleção ou diagnósticos do editor ativo para aplicar alterações (`inlineChat2.keep`) ou continuar na barra lateral (`inlineChat2.continueInChat`).

## Modos do Agente, Ferramentas e Aprovações

- Modos (`Tab` dentro do chat para alternar):
  - **Perguntar**: Modo padrão das novas conversas. A leitura ou busca de arquivos no workspace exige aprovação a cada chamada da ferramenta.
  - **Edit**: Propõe edições em arquivos acompanhadas de snapshots antes/depois para revisão.
  - **Agent**: Executa ferramentas de workspace (leitura, busca, edição de arquivos e comandos no terminal dentro da raiz do workspace) em múltiplos passos.
- **Restrição de caminhos e aprovações**: Todos os caminhos de ferramentas são validados dentro da raiz do workspace (`resolve_workspace_path`). Alterações e execuções respeitam `ToolApprovalPolicy` (`AlwaysAsk`, `AutoApproveReads`, `AutoApproveAll`), solicitando confirmação explícita quando necessário.
- **Pontos de verificação e edições**: Edições pendentes podem ser aceitas, descartadas, inspecionadas em diff (`chatEditing.openFileInDiff`) ou revertidas via pontos de verificação (`workbench.action.chat.undoEdit` / `redoEdit` / `restoreLastCheckpoint`).

No compositor, escolha **Arquivo**, **Seleção** ou **Diagnósticos** para anexar contexto do editor ativo à próxima mensagem. Cada anexo escolhido aparece marcado. O Chat Inline usa a seleção ou a linha em que o cursor está.

## Sessões, Área de Transferência e Exportação

- Sessões são persistidas por perfil com suporte a fixar, arquivar, marcar como lida, alternar orientação (`lado a lado` ou `empilhado`) e filtros (`Arquivadas`, `Execuções de Automação`, `Agendadas`).
- Blocos de código das respostas podem ser navegados (`nextCodeBlock` / `previousCodeBlock`), copiados (`copyCodeBlock`), inseridos no cursor, em novo arquivo ou célula de notebook, ou enviados ao terminal integrado (`runInTerminal`).
- Conversas e logs de depuração do agente podem ser exportados e importados em JSON em `<perfil>/chat-exports/`.

> **Nota:** A renderização visual em janela desktop das superfícies de Chat, Chat Inline e diálogo de provedor ainda não foi inspecionada interativamente em tela; o comportamento é coberto por testes automatizados unitários e de comandos.
