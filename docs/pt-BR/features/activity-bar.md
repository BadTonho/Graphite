# Barra de atividades

O Graphite tem uma barra de atividades na borda esquerda da janela. Ela
alterna as principais views do workbench sem reduzir o painel do Explorer: o
Explorer mantém sua largura de 260 px e a barra adiciona uma coluna de 48 px
ao lado.

## Itens

| Item | Ícone | Comportamento |
| --- | --- | --- |
| **Explorador** | `cod-files` | Mostra a árvore de arquivos e fecha o Controle de código e a pesquisa do projeto. |
| **Pesquisar** | `cod-search` | Abre a sobreposição de pesquisa do projeto (`Ctrl+Shift+F`); clicar novamente fecha a sobreposição. |
| **Controle de código** | `cod-source_control` | Alterna a barra lateral do Git (`Ctrl+Shift+G`). |
| **Plugins** | `cod-extensions` | Abre o painel de Plugins em tela cheia (`Ctrl+Shift+X`). |

O item ativo é marcado com uma barra de destaque de 2 px em `Theme::STATUS` e um
ícone mais claro. Passar o mouse sobre um item inativo desenha um realce sutil
na linha. O item selecionado acompanha a view visível: Plugins quando o painel
está aberto, depois Pesquisar quando a sobreposição está aberta, depois Controle
de código, caso contrário Explorador.

## Layout

A geometria fica em `crates/graphite-app/src/app/activity_bar.rs`:

- `ACTIVITY_BAR_WIDTH` tem 48 px (definido em `main.rs`).
- `SIDEBAR_WIDTH` é `ACTIVITY_BAR_WIDTH + 260.0`, então o editor, o terminal, o
  Quick Open e todas as sobreposições continuam começando depois da barra
  esquerda completa.
- As linhas do Explorer e a barra lateral do Controle de código são deslocadas
  por `ACTIVITY_BAR_WIDTH`; a barra ocupa a coluna restante.
- A barra fica oculta no Modo Zen e é coberta por superfícies de tela cheia,
  como o visualizador de diferenças e o painel de Plugins.

O fundo usa `activityBar.background` quando um tema importado define essa cor e
recorre a `#333333` no tema escuro e `#e8e8e8` no tema claro.

## Acessibilidade

A barra é exposta como uma `Toolbar` de acessibilidade com um `Button` por item.
Cada botão informa seu rótulo localizado e o estado selecionado e suporta a ação
`Click`, permitindo alternar views com leitores de tela sem usar o mouse.

## Adicionando um item

1. Adicione uma variante a `Activity` e a `Activity::ALL` em
   `crates/graphite-app/src/app/activity_bar.rs`.
2. Retorne o glifo em `Activity::glyph` e a chave do rótulo em
   `Activity::label_key`.
3. Adicione o codepoint do Nerd Fonts em `AVAILABLE_GLYPHS` em
   `crates/graphite-render/src/icons.rs` e recompile o subset com
   `python crates/graphite-render/assets/generate-icons.py --source SymbolsNerdFont-Regular.ttf`.
4. Adicione o rótulo em `locales/en/app.json` e `locales/pt-BR/app.json`.
5. Trate a nova variante em `App::activate_activity`.
6. Execute `cargo test -p graphite-app --bin graphite activity_bar` e
   `cargo test --workspace`.

## Verificação

A cobertura automatizada verifica o teste de posição dos slots, a prioridade do
item ativo, a alternância de Pesquisar e Controle de código, a barra de
acessibilidade e se cada glifo de atividade existe na fonte de ícones embutida.
A verificação visual é feita executando `target/debug/graphite.exe .` e
conferindo Explorador, Controle de código, Pesquisar e Plugins nos dois temas.
