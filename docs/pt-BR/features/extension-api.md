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
| Provedores de linguagem | Hover, completion, formatação de documento e diagnósticos. |
| Links | `window.openUri` abre links `https://` no navegador padrão; outros esquemas são recusados. `env.asExternalUri` devolve o link sem alterações. |
| Output | `window.createOutputChannel` grava em um arquivo de log. Mostrar o canal abre o log em uma view somente leitura, que se atualiza enquanto a extensão escreve. |
| Barra de status | Itens de `window.createStatusBarItem` aparecem no fim da barra de status com o texto. Cliques ainda não executam comandos. |
| Arquivos | `workspace.findFiles` com globs de inclusão e exclusão e limite de resultados, respeitando arquivos de ignore. |
| Progresso | Títulos e mensagens de `withProgress` aparecem na barra de status. |
| Webview | Um painel webview, conteúdo HTML, mensagens nos dois sentidos. |

## Ainda não

- Provedores de linguagem para definição, referências, símbolos de documento, code actions e ajuda de assinatura. Apenas completion, hover, formatação e diagnósticos estão ligados.
- API de terminal, tasks, depuração, controle de versão e sistemas de arquivos virtuais (`registerFileSystemProvider`).
- `workspace.findTextInFiles`.
- Vários painéis webview, `asWebviewUri` e `localResourceRoots`.
- Comandos nos itens da barra de status.
- Editores customizados e kernels de notebook.

Chamadas fora desta lista recebem o erro `Graphite does not support <actor>.<method>`, e a extensão recebe esse erro.
