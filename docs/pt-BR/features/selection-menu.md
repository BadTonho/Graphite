# Menu Seleção

O menu Seleção reúne ações de seleção e de linha do editor. Ele oferece selecionar tudo, expandir e reduzir a seleção, duplicar ou mover linhas, duplicar o texto selecionado, ações de múltiplos cursores, seleção de ocorrências e seleção por coluna.

Ctrl+clique adiciona um cursor por padrão. O item do menu alterna o gesto para Alt+clique e atualiza o próprio rótulo; a escolha é salva no perfil ativo. O Modo de Seleção por Coluna é uma opção separada, começa desativado e também é salvo por perfil. Arrastar com o modo ativo cria seleções alinhadas entre linhas visuais, incluindo linhas quebradas automaticamente.

Duplicar Seleção copia o texto selecionado logo depois de cada trecho e seleciona as cópias. Sem texto selecionado, duplica a linha atual. Adicionar cursores acima ou abaixo acompanha as linhas visuais e preserva a coluna quando possível. O menu e os comandos estão localizados em inglês e português brasileiro.

## Mais comandos de múltiplos cursores

| Comando | Atalho | O que faz |
| --- | --- | --- |
| Adicionar cursores até o fim / o topo (`editor.action.addCursorsToBottom` / `addCursorsToTop`) | nenhum | Adiciona um cursor em cada linha da seleção principal até a última (ou a primeira) linha, repetindo suas colunas. Uma linha mais curta que a coluna recebe o cursor no fim. |
| Alterar todas as ocorrências (`editor.action.changeAll`) | `Ctrl+F2` | Seleciona todas as ocorrências do texto selecionado, ou da palavra sob o cursor, para uma edição mudar todas. |
| Mover a última seleção para a próxima / anterior correspondência | `Ctrl+K Ctrl+D` para a próxima | Troca a última seleção (a primeira, ao voltar) pela próxima (anterior) ocorrência do seu texto; as outras seleções ficam. Dá a volta no documento. |
| Focar o próximo / anterior cursor | nenhum | Torna a seleção seguinte ou anterior a principal, e a vista rola até ela. Os cursores não se movem. A ordem vale até a próxima edição. |

Os comandos Adicionar cursor acima, Adicionar cursor abaixo e Adicionar cursores ao fim das linhas usam os ids `editor.action.insertCursorAbove`, `editor.action.insertCursorBelow` e `editor.action.insertCursorAtEndOfEachLineSelected`. Atalhos personalizados que usavam os ids antigos `graphite.editor.*` precisam ser atualizados.
