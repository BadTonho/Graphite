# Inventário de paridade com o VS Code

O objetivo é um Graphite igual ao VS Code, com interfaces e painéis. Este inventário compara os comandos registrados no Code - OSS (`microsoft/vscode`, licença MIT) com os comandos do Graphite, para planejar o trabalho área por área.

## Como foi medido

- Fonte: clone raso de `microsoft/vscode` de 2026-11-13, varrendo `src/vs/editor`, `src/vs/workbench/contrib`, `src/vs/workbench/browser` e `src/vs/platform/actions`.
- Um comando conta quando é registrado com `registerAction2`, `EditorAction`, `registerCommand` ou similar com id literal.
- Do lado do Graphite, vale a lista de chaves `command.*` em `locales/en/commands.json`.

Resultado: **949** comandos no VS Code, **47** já no Graphite com o mesmo id, **902** faltando.

Limites da contagem:

- Ids montados a partir de constantes não são encontrados, então o total real do VS Code é maior.
- O Graphite pode ter o comando com outro id (por exemplo, `graphite.editor.addCursorsToLineEnds` faz o que `editor.action.insertCursorAtEndOfEachLineSelected` faz), então parte dos "faltando" são só nomes diferentes.
- O mesmo id pode ter comportamento diferente.

A lista completa, agrupada por área, fica no checklist local (`Itens/comandos-vscode-faltando.md`, fora do commit).

## Comandos faltando por área

| Área | Faltando | Tipo |
| --- | ---: | --- |
| Shell do workbench: `parts` e `actions` (layout, barras laterais, painel, grupos de editor, zoom) | 214 | Interface |
| Chat, chat inline, agentes, sessões | ~145 | Precisa de serviço de IA |
| Notebooks, janela interativa, REPL | ~76 | Subsistema grande |
| Depuração | 46 | Interface + DAP |
| Vista de extensões | 40 | Interface |
| Controle de código-fonte | 28 | Interface + Git |
| Editor de merge | 26 | Interface grande |
| Dobramento | 15 | Lógica de editor |
| Preferências, tarefas | 30 | Interface |
| Operações de linhas | 13 | Lógica de editor |
| Histórico local, linha do tempo | 18 | Interface |
| Arquivos, saída, outline | 25 | Interface |
| Multi-cursor, ir ao símbolo, sticky scroll, âncoras, cursor | ~30 | Lógica de editor |
| Sync de dados, sessões de edição, remoto, atualização, issue | ~45 | Precisa de serviço |

## Ordem proposta

1. **Núcleo do editor** (cerca de 90 comandos): operações de linhas, multi-cursor, dobramento, ir ao símbolo, sticky scroll, operações de cursor e colchetes, zoom da fonte. É lógica de texto pura, então porta com fidelidade e testa bem.
2. **Shell do workbench** (cerca de 214): partes e ações de layout. É o que faz a janela parecer o VS Code.
3. **Ferramentas do dia a dia**: controle de código-fonte, arquivos, histórico local, linha do tempo, outline, saída, preferências, tarefas, testes, depuração, vista de extensões, snippets, temas.
4. **Editor de merge e diff de vários arquivos.**
5. **Notebooks e janela interativa**: um subsistema à parte.
6. **Recursos que dependem de serviço**: chat, sync, remoto, atualização. Precisam de backend ou conta, então cada um exige uma decisão sobre a que se conectam.

Cada etapa lê o código do VS Code para entender o comportamento, reescreve em Rust, adiciona testes e textos EN/PT-BR, e faz commit por grupo de comandos.
