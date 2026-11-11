# Foco entre regiões com F6 e alto contraste

## Mover o foco com F6

`F6` move o foco do teclado pelas regiões principais da janela, nesta ordem:

1. O editor.
2. O terminal, quando o painel está visível.
3. O explorador, quando a barra lateral principal está visível.

Regiões ocultas são puladas. `Shift + F6` percorre no sentido contrário. A barra de status informa para onde o foco foi. Clicar em uma região também move o foco para ela.

Antes, `F6` alternava entre grupos de editores. A navegação entre grupos continua disponível pela paleta de comandos (`Focar o próximo grupo`) e pelos atalhos próprios.

## Alto contraste

Execute **Alternar alto contraste** na paleta de comandos. A opção é salva no perfil (`high-contrast.json`) e restaurada ao iniciar.

O alto contraste usa a paleta escura:

- Superfícies escuras viram preto puro e o texto claro vira branco puro.
- Cores intermediárias (cores de sintaxe, texto apagado) são mantidas.
- A linha ativa (seleção atual) vira azul saturado para continuar visível.

Limites: algumas áreas usam cores fixas que ainda não passam por essa transformação, como o fundo do terminal. A paleta não foi conferida visualmente em todas as telas.

## Fora do escopo desta página

O suporte a leitores de tela (pela árvore de acessibilidade) e o comportamento de métodos de entrada (IME) precisam de verificação manual com tecnologia assistiva real e entrada real. Eles não são verificados por testes automatizados.
