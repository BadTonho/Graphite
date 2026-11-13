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
| Provedores de linguagem | Hover, completion, formatação de documento, diagnósticos, definição, declaração, definição de tipo, implementação, referências, símbolos de documento, code actions, renomear e ajuda de assinatura. Também formatação de intervalo (**Formatar seleção**; a linha do cursor quando nada está selecionado), inlay hints, destaques de documento e regiões de folding; os três últimos são pedidos automaticamente (veja abaixo). Selection ranges comandam **Expandir seleção** no lugar da expansão pela árvore sintática quando uma extensão os fornece. A formatação ao digitar roda durante a digitação quando `editor.formatOnType` está ligado. Hierarquias de chamadas e de tipos (**Show Call Hierarchy** / **Show Type Hierarchy**) listam chamadores e chamados, ou supertipos e subtipos, no seletor de locais. Links de documento são sublinhados e seguidos com Ctrl+clique. Tokens semânticos são desenhados sobre as cores da sintaxe. |
| Links | `window.openUri` abre links `https://` no navegador padrão; outros esquemas são recusados. `env.asExternalUri` devolve o link sem alterações. |
| Output | `window.createOutputChannel` grava em um arquivo de log. Mostrar o canal abre o log em uma view somente leitura, que se atualiza enquanto a extensão escreve. |
| Barra de status | Itens de `window.createStatusBarItem` aparecem no fim da barra de status com o texto. Clicar em um item executa o comando dele: um comando da extensão, se registrado, ou um comando do Graphite. |
| Arquivos | `workspace.findFiles` com globs de inclusão e exclusão e limite de resultados, respeitando arquivos de ignore. `workspace.findTextInFiles` (API proposta: a extensão deve listar `findTextInFiles` em `enabledApiProposals`) busca no workspace com o mecanismo de busca de projeto, com opções de regex, maiúsculas, palavra inteira e globs, e para após 5 segundos. |
| Progresso | Títulos e mensagens de `withProgress` aparecem na barra de status. |
| Terminal | `window.createTerminal` (com `name`, `shellPath`, `shellArgs`, `cwd`) abre uma aba de terminal do Graphite. `sendText`, `show`, `hide`, `dispose` e `onDidCloseTerminal` com o código de saída funcionam. |
| Observadores de arquivos | `workspace.createFileSystemWatcher` recebe eventos de criação, alteração e exclusão do workspace inteiro (exceto dentro de `node_modules`) e das pastas que a extensão registra fora dele. |
| Área de transferência | `env.clipboard.readText` e `writeText`. |
| `workspace.fs` | Respondido pelo próprio extension host direto no disco local; não passa pelo Graphite. |
| Decorações de texto | `window.createTextEditorDecorationType` e `TextEditor.setDecorations` desenham cores de fundo, bordas (`border`, `outline`), marcas de linha inteira e texto `after` no fim da linha, com variantes `dark` e `light`. As cores podem ser hex, `rgb()`/`rgba()` ou cores de tema comuns. Fontes, ícones de gutter, marcas da régua, texto `before` e mensagens de hover são ignorados, e as decorações ainda não são desenhadas no modo de quebra de linha. Os intervalos não são ajustados durante a edição; a extensão normalmente os define de novo. |
| Adaptadores de depuração | Uma extensão que registra um `DebugAdapterDescriptorFactory` pode fornecer o adaptador do seu tipo de depuração: descritores de executável e de servidor TCP local funcionam. Provedores de configuração, named pipes e implementações inline ainda não são suportados. Veja [Painel de depuração](debug-panel.md). |
| Webview | Um painel webview, conteúdo HTML, mensagens nos dois sentidos. |

## Contribuições declarativas

Um VSIX sem ponto de entrada Node.js (temas, snippets, pacotes de idioma) não executa código, então pode ser habilitado sem iniciar o extension host. Contribuições aplicadas:

| Contribuição | O que acontece |
|---|---|
| `commands` | Os comandos declarados aparecem na paleta antes da extensão ativar. |
| Configuração de `languages` | Os marcadores de comentário de linha e de bloco alimentam Alternar Comentário de Linha/Bloco. |
| `viewsContainers.activitybar` | Os contêineres aparecem na barra de atividades. |
| `snippets` | Os arquivos de snippets são carregados para extensões habilitadas, com escopo na linguagem contribuinte. Um snippet do usuário ou do projeto com o mesmo prefixo e escopo os substitui. |

| `themes` | Os temas de cores são importados para o perfil e selecionados com `workbench.colorTheme`, por rótulo ou id. Desabilitar a extensão os remove. |

Atalhos de teclado, gramáticas e temas de ícones constam no pacote, mas ainda não são aplicados.

## Ainda não

- Provedores de linguagem além da lista acima (code lenses e cores). Code actions que apenas executam um comando da extensão, e operações de arquivo dentro de uma renomeação, não são aplicadas.
- Pseudoterminais de propriedade da extensão, eventos de dados do terminal, tasks, depuração, controle de versão e sistemas de arquivos virtuais (`registerFileSystemProvider`).
- Vários painéis webview, `asWebviewUri` e `localResourceRoots`.
- Editores customizados e kernels de notebook.

Chamadas fora desta lista recebem o erro `Graphite does not support <actor>.<method>`, e a extensão recebe esse erro. Registros de capacidades não implementadas (`$register*`, `$unregister*`, `$emit*`) são aceitos sem erro para não poluir a barra de status. Cada chamada não suportada é registrada uma vez por sessão em `<perfil>/code-oss/unsupported-api.log`, que mostra o que uma extensão precisa e o Graphite ainda não oferece.

## Recursos pedidos automaticamente

A maioria dos provedores responde a um comando (hover, ir para definição, formatar). Cinco são pedidos sem comando:

- **Inlay hints** (`registerInlayHintsProvider`): para o documento ativo, sempre que o texto muda. São mostrados pelo mesmo recurso de dicas embutidas que os servidores de linguagem usam, e só quando nenhum servidor de linguagem atende o arquivo, para os dois nunca competirem. Um provedor que falha não é consultado de novo até o documento mudar.
- **Destaques de documento** (`registerDocumentHighlightProvider`): para o símbolo sob o cursor, depois que o cursor ficou parado por 150 ms. As ocorrências são desenhadas com a cor de destaque de palavra do editor. Os destaques somem assim que o cursor se move, e uma resposta para uma posição que o cursor já deixou é descartada.
- **Regiões de folding** (`registerFoldingRangeProvider`): para o documento ativo, sempre que o texto muda. Enquanto as regiões de um provedor descrevem o texto atual, elas substituem as regras de indentação em todos os comandos de dobra e na exibição; até a primeira resposta, ou após uma edição, vale o folding por indentação. Regiões que não cobrem duas linhas são descartadas, e um provedor que falha não é consultado de novo até o documento mudar.
- **Links de documento** (`registerDocumentLinkProvider`): para o documento ativo, sempre que o texto muda. Os links são sublinhados; Ctrl+clique em um abre links `https:` no navegador e links `file:` no editor. Links que o provedor deixa sem resolver (ainda sem URL) são informados mas não seguidos, e os sublinhados somem assim que o texto é editado, até a extensão responder de novo.
- **Tokens semânticos** (`registerDocumentSemanticTokensProvider`): para o documento ativo, sempre que o texto muda. A resposta completa do provedor (tipo, posição e tamanho de cada token) é decodificada, cada tipo de token é mapeado para a classe de sintaxe mais próxima (tipos, funções, variáveis, propriedades, palavras-chave, comentários, strings, números, operadores; outros tipos mantêm a cor da sintaxe) e desenhada sobre o realce do tree-sitter. Os tokens descrevem uma revisão do texto, então após uma edição valem as cores da sintaxe até o provedor responder de novo. Só respostas do documento inteiro são entendidas: respostas delta, provedores de intervalo e modificadores de token não são usados. O extension host envia a resposta binária ao Graphite como palavras de 32 bits, e a decodificação segue o layout do Code OSS conforme a descrição do protocolo; foi testada com respostas sintéticas, não com uma extensão de linguagem real.

Os dois só são pedidos em workspace confiável, para o documento ativo, e respostas para uma revisão desatualizada do documento são descartadas. Cada pedido tem limite de 10 segundos. Esses caminhos são cobertos por testes unitários que fazem o papel do extension host no protocolo; não foram executados com extensões de terceiros.

### Formatação ao digitar

Com `"editor.formatOnType": true` (desligado por padrão, como no VS Code), digitar um dos caracteres de gatilho que uma extensão registrou com `registerOnTypeFormattingEditProvider` pede a ela que formate em volta do cursor; as edições são aplicadas como um passo desfazível. Só contam caracteres digitados um a um (não texto colado), só com um cursor e só se o documento não mudou nesse meio tempo. Digitar Enter ainda não dispara.
