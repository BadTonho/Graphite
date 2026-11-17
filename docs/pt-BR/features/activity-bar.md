# Barra de atividades e barra de título

O Graphite tem uma barra de atividades configurável. Ela alterna Explorador,
Pesquisar, Controle de código, Plugins e contêineres de atividade contribuídos
por extensões habilitadas. Clique com o botão direito na barra para mostrar ou
ocultar itens e abrir as opções de layout. A barra de título personalizada
coloca o ícone do Graphite e os menus na mesma faixa, com os controles da janela
à direita. O título e o ícone do sistema continuam definidos para a barra de
tarefas e o alternador de janelas. Arraste a área livre para mover a janela,
clique duas vezes nela para maximizar ou restaurar e arraste as bordas para
redimensionar. Quando a janela for estreita, os menus que não couberem ficam em
**Mais**.

## Itens nativos

| Item | Ícone | Comportamento |
| --- | --- | --- |
| **Explorador** | `cod-files` | Mostra a árvore de arquivos e fecha Controle de código, pesquisa do projeto e Plugins. |
| **Pesquisar** | `cod-search` | Abre a pesquisa do projeto (`Ctrl+Shift+F`); clicar novamente fecha a pesquisa. |
| **Controle de código** | `cod-source_control` | Alterna o painel Git (`Ctrl+Shift+G`) e fecha Plugins. |
| **Plugins** | `cod-extensions` | Alterna o painel de Plugins (`Ctrl+Shift+X`). |

## Menu de contexto e layout

O menu de contexto lista as quatro atividades nativas e os contêineres declarados
por extensões habilitadas e confiáveis no workspace. Ocultar um item remove
somente seu botão da barra; não desabilita nem desinstala a extensão. Ao ocultar
o item ativo, o Graphite muda para outra atividade nativa visível. O último item
nativo visível não pode ser ocultado. O menu superior **Exibir** contém as
mesmas opções de visibilidade e layout; quando a barra estiver oculta, ele
mostra **Mostrar barra de atividades** para restaurá-la sem depender da paleta.

A faixa superior contém **Arquivo**, **Editar**, **Seleção**, **Exibir**, **Ir**,
**Executar**, **Terminal** e **Ajuda**. Exibir já é funcional; os demais menus
mostram uma mensagem localizada de estado vazio até que seus comandos sejam
definidos. É possível abrir os menus com o ponteiro ou F10, navegar pelas setas,
selecionar com Enter/Espaço e fechar com Esc.

**Activity Bar Position** oferece Default (barra vertical à esquerda), Top,
Bottom e Hidden. **Activity Bar Size** oferece Default e Compact. **Move Primary
Side Bar Right** coloca a coluna principal à direita e mantém a barra de
atividades à esquerda. Essas opções e os itens ocultos são salvos por perfil.
Quando a barra estiver oculta, use **Show Activity Bar** na paleta de comandos
para restaurá-la. O menu de contexto aceita navegação por ponteiro e pelas
setas, Enter/Espaço e Esc.

Default usa uma barra de 48 px; Compact usa uma barra de 36 px e itens de 40 px.
A barra lateral principal pode ser redimensionada e é salva por perfil. Seu
limite máximo é 70% da largura da janela, preservando ao menos 320 px para o
editor. Top e Bottom reservam uma faixa horizontal e reorganizam o editor na
vertical. A geometria compartilhada mantém desenho, cliques e redimensionamento
alinhados quando a barra ou a coluna principal muda de posição.

## Contribuições de extensões

Pacotes VSIX podem declarar contêineres em
`contributes.viewsContainers.activitybar` e views em `contributes.views`.
Manifestos de plugins nativos Graphite usam a lista equivalente `activity_bar`,
com IDs, títulos, ícones opcionais e tipos `tree` ou `webview`. Só são listados
registros habilitados e confiáveis no workspace. Selecionar uma contribuição
ativa a extensão dona. Views de árvore mostram seus dados sob o título do
contêiner, e views declaradas com `"type": "webview"` hospedam a página fornecida
pela extensão em uma sobreposição abaixo do cabeçalho da view (veja
[API de extensões](extension-api.md)).

## Acessibilidade e verificação

A barra superior e a barra de atividades são expostas como toolbars de
acessibilidade, com botões localizados e estado selecionado. A navegação por
teclado e a detecção de cliques usam a mesma geometria.

Os testes cobrem modos de layout, visibilidade nativa, ações do menu e
visibilidade de contribuições de plugins. Execute `cargo test -p graphite-app
--bin graphite activity_bar` e `cargo test --workspace`. A verificação visual
nativa deve incluir Windows e Linux quando disponíveis.

## Barra lateral de plugins: uma coluna de listas

A barra lateral de Plugins não tem botões de aba. Abaixo da caixa de pesquisa há uma barra para cada lista (**Instalados**, **Open VSX**, **Graphite**), cada uma com uma seta, o título e um selo com o número de itens, entre linhas finas. A lista aberta mostra os cartões dos plugins logo abaixo da sua barra, e as outras barras ficam empilhadas abaixo dos cartões.

- Clique numa barra fechada para abrir aquela lista (os atalhos de tecla `i`, `o` e `g` fazem o mesmo).
- Clique na barra aberta para recolher os cartões; as outras barras sobem para baixo dela. Cada lista lembra se está recolhida.
- Uma linha fina separa um cartão de plugin do próximo.
- Enquanto os detalhes de um plugin ou o aviso de confiança aparecem, as barras abaixo da aberta ficam ocultas para dar mais espaço aos detalhes.
