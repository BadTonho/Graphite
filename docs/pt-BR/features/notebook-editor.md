# Editor de notebook

Arquivos terminados em `.ipynb` (notebooks Jupyter de formato 4) abrem como notebook: uma lista vertical de células, cada uma com seu próprio editor. Um arquivo que não é um notebook válido abre como texto comum.

## O que é preservado

Tudo o que está no arquivo é mantido ao salvar, inclusive campos que este programa não entende (no notebook, nos metadados, nas células e nas saídas). O arquivo é escrito como o Jupyter escreve: chaves em ordem, indentação de um espaço, o código dividido em linhas, quebra de linha no fim. Um notebook formatado de outro jeito aparece como não modificado ao abrir e só é reescrito nesse formato quando você salva. A partir do formato 4.5 toda célula tem um id único; células sem id ganham um.

## Células

Cada célula mostra um cabeçalho (`[3]` para uma célula de código que rodou como a terceira execução, `[ ]` para uma que não rodou, `markdown` ou `raw`), sua entrada e, nas células de código, as saídas como texto (fluxo, resultados e erros, sem as cores do terminal). A célula com o foco tem uma barra à esquerda: azul no modo de comando, verde durante a edição. As células selecionadas têm o fundo colorido.

Há dois modos, como no Jupyter.

### Modo de comando (as teclas agem sobre células)

| Tecla | Comando |
| --- | --- |
| Enter | Editar a célula. |
| Cima / Baixo, `K` / `J` | Focar a célula anterior ou a próxima. Com Shift a seleção se estende a várias células. |
| Home / End | Primeira ou última célula. |
| `A` / `B` | Inserir uma célula de código acima ou abaixo e editá-la. |
| `D` `D` | Excluir as células selecionadas (aperte `D` duas vezes). Excluir todas deixa uma célula de código vazia. |
| `M` / `Y` / `R` | Mudar a célula para markdown, código ou raw. Sair do tipo código descarta as saídas e a contagem de execução. |
| Shift+`M` | Juntar as células selecionadas, ou a célula com a próxima. |
| Ctrl+Shift+Baixo | Mover as células selecionadas para baixo (e o comando equivalente para cima). |
| `X` / `C` / `V` | Recortar, copiar e colar células. As células coladas ganham ids novos. |
| `O` | Recolher ou expandir as saídas. |
| `H` | Recolher ou expandir a entrada. |
| `L` | Mostrar ou ocultar os números de linha. |
| Ctrl+`A` | Selecionar todas as células. |

### Modo de edição (as teclas editam o texto da célula)

Digitar, Enter, Tab (insere espaços), Backspace, Delete, as setas, Home e End (com Shift para selecionar), Ctrl+`A` (selecionar o texto da célula), Ctrl+`C`, Ctrl+`X` e Ctrl+`V` funcionam como no editor de texto. Cima na primeira linha e Baixo na última passam para a célula vizinha na mesma coluna. Escape volta ao modo de comando. Shift+Enter ou Ctrl+Enter saem da célula e focam a próxima. Ctrl+Shift+`-` divide a célula no cursor. Um clique na entrada de uma célula começa a edição naquele ponto; um clique em outro lugar da célula a seleciona.

## Desfazer

Ctrl+`Z` e Ctrl+`Y` (ou Ctrl+Shift+`Z`) desfazem e refazem nos dois modos, em um único histórico para o texto digitado em qualquer célula e para as operações com células (inserir, excluir, mover, dividir, juntar, mudar de tipo). Desfazer até o estado salvo deixa o documento sem modificações.

## Salvar e recarregar

O notebook é um documento comum: aparece como modificado na aba, salva com os comandos de sempre e é recarregado quando o arquivo muda no disco e não tem edições não salvas.

Todos os comandos também estão na paleta de comandos, na categoria **Notebook**.
