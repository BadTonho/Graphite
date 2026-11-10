# Barra de atividades

O Graphite tem uma barra de atividades na borda esquerda da janela. Ela alterna
as principais views do workbench em uma coluna de 48 px: Explorador, Controle de
código e Plugins compartilham a mesma coluna lateral de 260 px, então o editor
mantém sua posição.

## Itens

| Item | Ícone | Comportamento |
| --- | --- | --- |
| **Explorador** | `cod-files` | Mostra a árvore de arquivos e fecha o Controle de código, a pesquisa do projeto e os Plugins. |
| **Pesquisar** | `cod-search` | Abre a sobreposição de pesquisa do projeto (`Ctrl+Shift+F`); clicar novamente fecha a sobreposição. |
| **Controle de código** | `cod-source_control` | Alterna a barra lateral do Git (`Ctrl+Shift+G`) e fecha os Plugins. |
| **Plugins** | `cod-extensions` | Alterna a barra lateral de Plugins (`Ctrl+Shift+X`); um item focado a fecha e um item visível sem foco recupera o foco. |

O item ativo é marcado com uma barra de destaque de 2 px em `Theme::STATUS` e um
ícone mais claro. Passar o mouse sobre um item inativo desenha um realce sutil
na linha. O item selecionado acompanha a view visível: Plugins quando a barra
lateral está aberta, depois Pesquisar quando a sobreposição está aberta, depois
Controle de código, caso contrário Explorador.

## Layout

A geometria fica em `crates/graphite-app/src/app/activity_bar.rs`:

- `ACTIVITY_BAR_WIDTH` tem 48 px (definido em `main.rs`).
- `SIDEBAR_WIDTH` é `ACTIVITY_BAR_WIDTH + 260.0`, então o editor, o terminal, o
  Quick Open e todas as sobreposições continuam começando depois da barra
  esquerda completa.
- As linhas do Explorer, a barra lateral do Controle de código e a barra lateral
  de Plugins são deslocadas por `ACTIVITY_BAR_WIDTH`; a barra ocupa a coluna
  restante.
- Plugins reutiliza a coluna lateral e mantém o editor visível. Seus cartões,
  abas e caixa de pesquisa usam a geometria compartilhada em
  `crates/graphite-app/src/app/plugins/layout.rs`.
- A barra fica oculta no Modo Zen e é coberta por superfícies de tela cheia,
  como o visualizador de diferenças. Plugins é uma view lateral, não uma
  superfície de tela cheia.

## Foco

A barra lateral de Plugins só recebe entrada de teclado enquanto está focada.
Clicar na barra lateral concede o foco; clicar no editor ou em outra view o
libera. Quick Open, localizar, pesquisa do projeto, Controle de código, terminal,
views de diferença/conflito e diálogos de confirmação sempre têm precedência de
teclado. `pluginsVisible` e `pluginsFocus` expõem o estado para condições de
atalhos, e `editorTextFocus` é falso enquanto a barra lateral controla o teclado.

A lista da barra lateral rola com a roda do mouse enquanto o cursor está sobre
ela. A navegação por teclado mantém o cartão selecionado visível, e trocar de aba
ou atualizar uma lista reinicia o deslocamento da viewport.

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
item ativo, a alternância de Pesquisar, Controle de código e Plugins, o foco e a
geometria de rolagem da barra lateral, a barra de acessibilidade e se cada glifo
de atividade existe na fonte de ícones embutida. A verificação visual é feita
executando `target/debug/graphite.exe .` e conferindo Explorador, Controle de
código, Pesquisar e a barra lateral de Plugins nos dois temas.
