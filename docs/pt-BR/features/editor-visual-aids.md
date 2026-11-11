# Recursos visuais do editor

## Pares de colchetes

O Graphite faz o pareamento de `()`, `[]` e `{}` lendo o texto. Quando o cursor está sobre um colchete ou logo depois dele, o colchete correspondente é destacado. Os colchetes são coloridos pela profundidade de aninhamento, com três cores que se repetem a cada três níveis.

O pareamento é textual: colchetes dentro de strings e comentários também são contados. A busca para após 200.000 caracteres, então um colchete sem par dentro desse intervalo não é destacado.

## Guias de indentação

Uma linha vertical fina é desenhada em cada nível de indentação de uma linha, com base no espaço inicial. Tabs usam o tamanho de tab do documento. Linhas em branco não recebem guia.

## Juntar linhas

O comando **Juntar linhas** (`editor.action.joinLines`) junta a linha do cursor com a próxima. Com seleção, junta todas as linhas selecionadas em uma. O espaço inicial das linhas juntadas é removido e as partes são separadas por um único espaço. O comando pode ser desfeito em um único passo e respeita documentos somente leitura. Não tem atalho padrão; defina um em `keybindings.json` se quiser.
