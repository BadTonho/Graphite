# Superfície da API de extensões

Extensões VSIX rodam no extension host oficial do Code OSS 1.145.2, em um processo Node separado iniciado por `packaging/code-oss/bridge.mjs`. O Graphite responde às chamadas que o host faz à thread principal, então todo efeito no workbench passa pelas verificações do próprio Graphite (confiança do workspace, arquivos somente leitura e a fronteira do workspace).

## Suportado

| Área | O que funciona |
|---|---|
| Comandos | Registrar, executar e listar comandos; comandos do Graphite podem ser executados pelas extensões. |
| Configuração | Ler valores, atualizar e remover configurações do workspace ou do usuário, escutar mudanças. |
| Storage | Storage global e do workspace da extensão. |
| Prompts | `showInputBox`, `showQuickPick` e `showInformationMessage` com botões de ação. |
| Edições | `TextEditor.edit`, `workspace.applyEdit` e documentos abertos ou exibidos no editor. |
| Provedores de linguagem | Hover, completion, formatação de documento, diagnósticos, definição, declaração, definição de tipo, implementação, referências, símbolos de documento, code actions, renomear e ajuda de assinatura. |
| Links | `window.openUri` abre links `https://` no navegador padrão; outros esquemas são recusados. `env.asExternalUri` devolve o link sem alterações. |
| Output | `window.createOutputChannel` grava em um arquivo de log. Mostrar o canal abre o log em uma view somente leitura, que se atualiza enquanto a extensão escreve. |
| Barra de status | Itens de `window.createStatusBarItem` aparecem no fim da barra de status com o texto. Clicar em um item executa o comando dele: um comando da extensão, se registrado, ou um comando do Graphite. |
| Arquivos | `workspace.findFiles` com globs de inclusão e exclusão e limite de resultados, respeitando arquivos de ignore. `workspace.findTextInFiles` (API proposta: a extensão deve listar `findTextInFiles` em `enabledApiProposals`) busca no workspace com o mecanismo de busca de projeto, com opções de regex, maiúsculas, palavra inteira e globs, e para após 5 segundos. |
| Progresso | Títulos e mensagens de `withProgress` aparecem na barra de status. |
| Terminal | `window.createTerminal` (com `name`, `shellPath`, `shellArgs`, `cwd`) abre uma aba de terminal do Graphite. `sendText`, `show`, `hide`, `dispose` e `onDidCloseTerminal` com o código de saída funcionam. |
| Webview | Um painel webview, conteúdo HTML, mensagens nos dois sentidos. |

## Contribuições declarativas

Um VSIX sem ponto de entrada Node.js (temas, snippets, pacotes de idioma) não executa código, então pode ser habilitado sem iniciar o extension host. Contribuições aplicadas:

| Contribuição | O que acontece |
|---|---|
| `commands` | Os comandos declarados aparecem na paleta antes da extensão ativar. |
| Configuração de `languages` | Os marcadores de comentário de linha e de bloco alimentam Alternar Comentário de Linha/Bloco. |
| `viewsContainers.activitybar` | Os contêineres aparecem na barra de atividades. |
| `snippets` | Os arquivos de snippets são carregados para extensões habilitadas, com escopo na linguagem contribuinte. Um snippet do usuário ou do projeto com o mesmo prefixo e escopo os substitui. |

Temas, atalhos de teclado, gramáticas e temas de ícones constam no pacote, mas ainda não são aplicados.

## Ainda não

- Provedores de linguagem além da lista acima (formatação de intervalo e ao digitar, destaques de documento, semantic tokens, code lenses, folding, selection ranges, hierarquias de chamadas e tipos, inlay hints, links e cores). Code actions que apenas executam um comando da extensão, e operações de arquivo dentro de uma renomeação, não são aplicadas.
- Pseudoterminais de propriedade da extensão, eventos de dados do terminal, tasks, depuração, controle de versão e sistemas de arquivos virtuais (`registerFileSystemProvider`).
- Vários painéis webview, `asWebviewUri` e `localResourceRoots`.
- Editores customizados e kernels de notebook.

Chamadas fora desta lista recebem o erro `Graphite does not support <actor>.<method>`, e a extensão recebe esse erro. Registros de capacidades não implementadas (`$register*`, `$unregister*`, `$emit*`) são aceitos sem erro para não poluir a barra de status. Cada chamada não suportada é registrada uma vez por sessão em `<perfil>/code-oss/unsupported-api.log`, que mostra o que uma extensão precisa e o Graphite ainda não oferece.
