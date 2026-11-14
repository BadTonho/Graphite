# Compatibilidade com extensões do VS Code

O Graphite inicia o host de extensões oficial do Code OSS **1.145.2** (commit `c4453f8ea430795f941c1b2ff6c9321135a74a02`) em um processo separado do Node.js 24.22.1. Os pacotes portáteis Windows e Linux x86-64 incluem o runtime e os avisos de licença. O build comum do Cargo permite inspecionar e instalar VSIX sem preparar o runtime.

## Runtime e autorização

O processo do Graphite troca mensagens com limites de tamanho por canais privados do processo filho. O adaptador Node usa `PersistentProtocol` e `RPCProtocol` do upstream por um socket local ou pipe nomeado por sessão. Ele faz o handshake oficial e converte as chamadas. O Rust controla as operações do editor, o registro, as versões dos documentos, os diagnósticos, as caixas de diálogo e o ciclo de vida.

O painel Plugins instala os pacotes VSIX desabilitados. A instalação aprovada, a confiança do workspace e o perfil ativo determinam quais pacotes o host recebe. Comandos declarados aparecem na paleta antes da ativação, e eventos de inicialização ou ciclo de vida (`*`, `onStartupFinished`, `workspaceContains:`, `onLanguage:`, `onView:`), assim como o primeiro comando elegível da paleta ou atalho de teclado, ativam o host de extensões. Comandos registrados, provedores, Tree Views, Webviews, tarefas, ferramentas LM, diagnósticos e solicitações pendentes pertencem à sessão. Revogar a confiança, trocar de workspace ou perfil ou alterar instalações habilitadas encerra a sessão. O processo roda com as permissões do usuário, então aprove apenas código em que você confia.

## Matriz de APIs e contribuições VSIX suportadas

| Namespace / Contribuição | Fluxo suportado |
|---|---|
| `commands` e `contributes.commands` | Ativar comandos declarados sob demanda (`Ctrl+Shift+P` / `F1`), registrar/remover handlers, executar comandos registrados de extensões e despachar comandos do Graphite via `$executeCommand`. |
| `activationEvents` | Disparar `*` e `onStartupFinished` quando o host estiver pronto, avaliar checagens `workspaceContains:` dentro da raiz do workspace e enviar ativações `onLanguage:` / `onView:`. |
| `contributes.menus` e `contributes.keybindings` | Projetar entradas de menu VSIX (`commandPalette`, `editor/context` e `view/item/context`) nas superfícies de menu de plugins do Graphite e vincular atalhos de `contributes.keybindings` ao keymap ativo quando aprovados e confiáveis. |
| `workspace` | Ler pastas e documentos, observar eventos de abertura/alteração/salvamento/fechamento, receber eventos de file watchers, buscar arquivos e texto no workspace, aplicar edições individuais e em lote com desfazer, persistir armazenamento isolado por extensão (`MainThreadStorage`) e atualizar configurações por perfil e workspace. |
| `window` e `env` | Ler o editor ativo e seleções, abrir/mostrar documentos, aplicar decorações de editor (`MainThreadEditorTabsAndDecorations`), exibir mensagens, progresso, caixas de entrada e seleções rápidas (Esc cancela), gerenciar canais de saída e itens da barra de status, ler/escrever na área de transferência (`MainThreadClipboard`) e validar URIs externas `https://` via `MainThreadWindow.$openUri`. |
| `languages` | Registrar provedores de completion, hover, formatação de documento, formatação por caractere e por intervalo, definição, declaração, definição de tipo, implementação, referências, símbolos do documento e do workspace, code action, code lens, document highlight, document link, rename, signature help, semantic tokens, inlay hints, folding range, selection range e call/type hierarchy, além de Diagnósticos. |
| `window.createTreeView` | Registrar provedores em `MainThreadTreeViews`, buscar filhos sob demanda (`$getChildren`), acompanhar expansão/seleção, aplicar atualizações/mensagens e revelar itens na barra lateral do Explorer/Tree. |
| `window.createWebviewPanel` | Criar painéis Webview (`MainThreadWebviewPanels` / `MainThreadWebviews`), atualizar HTML/título/opções, revelar painéis, trocar mensagens bidirecionais `postMessage`, descartar painéis com limpeza e reescrever URIs `vscode-resource` estritamente confinadas às `localResourceRoots` aprovadas (ou raízes da extensão/workspace). |
| `terminal` e `Pseudoterminal` | Abrir shells do sistema com `cwd` confinado ao workspace e pseudoterminais customizados controlados pela extensão (`isExtensionCustomPtyTerminal`), encaminhando saída (`$sendProcessData`), títulos, dimensões/ready, entrada do teclado (`$acceptTerminalProcessInput`) e códigos de saída (`$sendProcessExit`). |
| `tasks` | Registrar provedores em `MainThreadTask`, tratar `$tryCreateTaskId`, executar tarefas de processo/shell com `cwd` confinado ao workspace ou tarefas `CustomExecution` conduzidas por `ExtHostTask`, encerrar tarefas ativas e publicar o código de saída. |
| `debug` e `testing` | Registrar fábricas/sessões de depuração (`MainThreadDebugService`) e controladores, itens, perfis e execuções de teste (`MainThreadTesting`). |
| `lm` (`LanguageModelTools`) | Registrar ferramentas VSIX para chat (`MainThreadLanguageModelTools`), expô-las à lista de ferramentas do chat nativo do Graphite e despachar chamadas `$invokeTool` quando o workspace for confiável. |

Métodos de atores `MainThread*` não suportados falham explicitamente com erro de operação indisponível e são registrados para inspeção da sessão. Temas de cores, temas de ícones de arquivo e snippets de pacotes VSIX são aplicados sem iniciar o extension host; o Open VSX é o registro público usado pelo Graphite.

Veja [builds portáteis](../contributing/building.md), [instalação de plugins](../contributing/plugins.md) e [validação da v0.5](../features/v0.5-extensions-automation.md).
