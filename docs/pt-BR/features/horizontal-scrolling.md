# Rolagem horizontal

Uma aba do editor que não quebra linhas rola para os lados quando uma linha é mais larga que a área de texto. A área de texto é a largura do grupo sem a coluna de números de linha e sem o minimapa, então ela muda sempre que o grupo é redimensionado, por exemplo quando a barra lateral secundária é arrastada para ficar mais larga ou é aberta.

- **Acompanha o cursor:** mover ou digitar além da borda direita ou esquerda rola o texto só o suficiente para trazer o cursor de volta à vista. Tabulações contam com a largura expandida.
- **Acompanha o grupo:** a rolagem nunca fica além do fim da linha visível mais longa. Quando o grupo fica mais largo (uma barra lateral fechada ou arrastada para ficar mais estreita), o texto volta à vista na hora.
- **Mouse:** um gesto lateral no touchpad, ou a roda do mouse com `Shift` pressionado, rola para os lados. Cliques e arrastos correspondem ao texto rolado.
- **Números de linha ficam parados:** a coluna de números, seus marcadores e os cabeçalhos de rolagem fixa não rolam para os lados; o texto é recortado à área à direita da coluna.
- **Quebra de linha:** com a [quebra de linha](word-wrap.md) ligada numa aba, as linhas dela quebram na área de texto e a rolagem lateral é sempre zero.

A rolagem é guardada por aba (`View::scroll_col`, em colunas). As regras ficam em `app/editing/horizontal_scroll.rs` e são cobertas pelos testes unitários desse arquivo; o desenho está em `app/editing/group_drawing.rs`.
