# Vista de outline

O outline lista as declarações do arquivo ativo na barra lateral direita (alterne com **Alternar outline**). Agora é uma árvore: uma declaração mais indentada que a de cima fica aninhada nela. Cada linha mostra o número da linha e o cabeçalho da declaração. Clique numa linha para ir até ela; clique na seta à esquerda de uma linha que tem filhos para recolher ou expandir.

| Comando | O que faz |
| --- | --- |
| Ordenar o outline por posição / nome / tipo | Posição mantém a ordem do arquivo e o aninhamento. Nome ordena todas as declarações em ordem alfabética, ignorando maiúsculas. Tipo agrupa por `fn`, `struct`, `class` etc., mantendo a ordem do arquivo dentro de cada grupo. Ordenar por nome ou tipo achata a árvore. |
| Seguir o cursor no outline | Ligado por padrão: a linha da declaração em que o cursor está fica destacada. Desligar remove o destaque. |
| Filtrar o outline ao digitar | Liga uma linha de filtro no topo do outline. Depois de clicar no outline, digitar reduz as linhas às declarações que contêm o texto (ignorando maiúsculas), Backspace apaga um caractere e Esc limpa o texto. Desligar limpa o filtro. |
| Recolher / Expandir todos os itens do outline | Recolhe ou expande toda declaração que tem declarações aninhadas. |

O tipo e o nome vêm das primeiras palavras do cabeçalho da declaração, pulando modificadores como `pub`, `export` e `async`. O outline funciona para as linguagens com analisador sintático.
