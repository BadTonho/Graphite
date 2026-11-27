# Layout da janela

A janela é feita de quatro blocos que se encaixam sem se sobrepor: a barra lateral primária, a coluna do editor (os grupos do editor com o painel embaixo) e a barra lateral secundária. A barra de atividades fica ao lado deles. Mudar o tamanho de um bloco faz os vizinhos acompanharem.

## Modo IA

O Chat e o progresso da tarefa ocupam a área principal. Durante a revisão de uma proposta, o diff usa cerca de dois terços de uma janela larga e o Chat ocupa o terço restante. Em uma janela estreita, use as abas **Chat** e **Revisão**. O terminal integrado abre sob demanda abaixo da área principal.

## A coluna do editor

Os grupos do editor e o painel dividem uma só coluna. As bordas esquerda e direita dela são definidas uma única vez (`editor_column` em `app/window/layout.rs`) e todo mundo lê esses valores: os grupos do editor, o painel (terminal, problemas, console de depuração e semelhantes), a contagem de colunas do terminal e a rotina principal de desenho. O painel nunca passa por baixo de uma barra lateral, em qualquer lado em que a barra primária esteja, no modo zen e com o painel posicionado à direita do editor. Um painel alinhado fica alinhado dentro da coluna.

## Tamanhos

- Cada barra lateral tem uma largura mínima, e a coluna do editor mantém pelo menos 320 px.
- O bloco que você está arrastando ganha o espaço e o vizinho cede. Alargar a barra secundária estreita a primária, até o mínimo dela; alargar a primária estreita a secundária, até o mínimo dela. Nenhuma das duas leva o editor abaixo do mínimo.
- Os tamanhos que você definiu são mantidos: quando o outro bloco encolhe de novo, o que cedeu volta ao que tinha.

## Divisórias

Há três divisórias arrastáveis: barra primária | coluna do editor, grupos do editor | painel e coluna do editor | barra secundária. Cada uma acende na cor de destaque enquanto o ponteiro está sobre ela ou ela está sendo arrastada, e um duplo clique devolve o bloco ao tamanho padrão (a barra secundária também esquece a largura salva). A barra secundária tem uma borda fina no lado que encara o editor, como os outros blocos.

As larguras das duas barras laterais são salvas por perfil.

## Abas do painel

Quando o painel fica estreito demais para todas as abas (Problemas, Saída, Console de Depuração, Terminal, Portas, Comentários), as abas que não cabem são escondidas e um botão "…" aparece depois da última. A aba ativa sempre continua visível, e o botão "…" abre um menu com as abas escondidas; escolher uma a mostra. As abas voltam conforme o painel alarga.
