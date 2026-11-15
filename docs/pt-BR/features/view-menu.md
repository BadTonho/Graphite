# Menu Exibir e layout do ambiente

O menu **Exibir** reúne as views do ambiente de trabalho, os controles de aparência e os layouts de grupos do editor. Ele é aberto pela barra de título personalizada e aceita navegação pelo mouse e pelo teclado. O menu de contexto separado da Barra de Atividades continua disponível para controlar a visibilidade das atividades e das views contribuídas por extensões.

## Views

Explorador, Pesquisa, Controle de Código, Executar e Depurar, Extensões, Problemas, Saída, Console de Depuração e Terminal abrem suas views ou painéis correspondentes no Graphite. Chat e Navegador aparecem indisponíveis até serem implementados. A Paleta de Comandos pode ser aberta por este menu e continua sendo a forma de recuperar uma barra de menus ou Barra de Atividades oculta.

## Aparência

Os controles de aparência incluem tela cheia, Modo Zen, editor centralizado, barra de menus, barras laterais primária e secundária, barra de status, visibilidade e posição do painel, posição e tamanho da Barra de Atividades, visibilidade das abas, posição das ações do editor, minimapa, breadcrumbs, rolagem fixa, renderização de espaços em branco, caracteres de controle e zoom. As preferências ficam em `view-layout.json` no perfil ativo. Tela cheia usa o gerenciador de janelas; o Modo Zen é uma apresentação temporária e separada do ambiente.

O minimapa mostra uma visão compacta do documento; clicar nele navega pelo arquivo. Breadcrumbs mostram o caminho do arquivo ativo e a declaração que contém o cursor quando a linguagem é compatível. A rolagem fixa pode ser ligada ou desligada. Os modos de espaços em branco são Nenhum, Limites, Seleção e Todos. Painéis inferiores podem ser alinhados à esquerda, ao centro ou à direita.

## Grupos do editor

O submenu Layout do Editor permite dividir para cima, baixo, esquerda ou direita; usar layouts único, de duas ou três colunas, de duas ou três linhas, grade e layouts assimétricos; e inverter o layout. Os predefinidos mantêm as abas abertas quando os grupos são consolidados. Mover Editor para Nova Janela e Copiar Editor para Nova Janela reutilizam o workspace atual e abrem o arquivo ativo no novo processo. Um arquivo ativo com alterações pendentes é salvo antes da abertura da nova janela; ao mover, a aba original é fechada depois.

## Teclado e acessibilidade

Os submenus abrem ao lado da linha ao passar o mouse (ou clicar) e podem se encadear em qualquer nível; um submenu que não cabe à direita abre à esquerda e fica dentro da janela. Use as setas para cima e para baixo para percorrer os itens, Direita ou Enter para abrir um submenu, Esquerda para voltar e Esc para fechar o nível atual do menu. Os itens expõem os estados marcado e indisponível aos recursos de acessibilidade. Os rótulos e as descrições estão localizados em inglês e português do Brasil.

## Botões de layout na barra de título

Quatro botões ficam à esquerda dos controles da janela:

| Botão | Ação | Atalho |
|---|---|---|
| Personalizar layout... | Abre o menu Exibir embaixo do botão | — |
| Alternar barra lateral primária | Mostra ou oculta a barra lateral primária | `Ctrl+B` |
| Alternar painel | Mostra ou oculta o painel inferior | `Ctrl+J` |
| Alternar barra lateral secundária | Mostra ou oculta a barra lateral secundária | `Ctrl+Alt+B` |

Os botões de barra lateral e de painel ficam preenchidos enquanto a área estiver visível. Passe o mouse sobre um botão para ver o nome e o atalho.
