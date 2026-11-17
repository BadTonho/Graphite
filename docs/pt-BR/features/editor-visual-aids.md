# Recursos visuais do editor

## Pares de colchetes

O Graphite faz o pareamento de `()`, `[]` e `{}` lendo o texto. Quando o cursor está sobre um colchete ou logo depois dele, o colchete correspondente é destacado. Os colchetes são coloridos pela profundidade de aninhamento, com três cores que se repetem a cada três níveis.

O pareamento é textual: colchetes dentro de strings e comentários também são contados. A busca para após 200.000 caracteres, então um colchete sem par dentro desse intervalo não é destacado.

## Guias de indentação

Uma linha vertical fina é desenhada em cada nível de indentação de uma linha, com base no espaço inicial. Tabs usam o tamanho de tab do documento. Linhas em branco não recebem guia.

## Juntar linhas

O comando **Juntar linhas** (`editor.action.joinLines`) junta a linha do cursor com a próxima. Com seleção, junta todas as linhas selecionadas em uma. O espaço inicial das linhas juntadas é removido e as partes são separadas por um único espaço. O comando pode ser desfeito em um único passo e respeita documentos somente leitura. Não tem atalho padrão; defina um em `keybindings.json` se quiser.

## Distinguir o arquivo do fundo

O texto de um arquivo se separa do resto da janela:

- A **coluna dos números de linha** tem um tom um pouco diferente da área do texto, com uma linha fina entre as duas.
- Uma linha fina fecha a **faixa de abas**, então as abas terminam onde o arquivo começa.
- A **linha do cursor** tem um fundo suave. Com uma seleção ele não é desenhado, porque a seleção já marca o que ela cobre.

Os tons são sutis de propósito, para o texto manter o contraste, e seguem os temas escuro e claro. No modo de alto contraste as superfícies continuam pretas e as duas linhas são desenhadas numa cor viva.
