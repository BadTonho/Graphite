# Editor de mesclagem

O editor de mesclagem resolve um arquivo que tem marcadores de conflito do Git (`<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`). Ele lê os marcadores do arquivo, então funciona com qualquer número de conflitos no mesmo arquivo, e não altera o arquivo até a mesclagem ser concluída.

## Abrindo

Execute **Abrir editor de mesclagem** na paleta de comandos com o arquivo ativo. O arquivo precisa ter ao menos um conflito completo; caso contrário o editor não abre e avisa. Se o arquivo estiver aberto no editor, usa-se o texto atual dele; senão, o arquivo em disco.

Marcadores maiores que sete caracteres (a configuração de tamanho do marcador do Git) são entendidos, assim como arquivos com quebras de linha do Windows. Um conflito que nunca é fechado, ou cujos marcadores não se encaixam, é tratado como texto comum.

## Layout

Quatro painéis mostram as mesmas linhas e rolam juntos, mesmo que os dois lados de um conflito raramente tenham o mesmo número de linhas:

- **Atual** e **Recebido**: os dois lados. As linhas que diferem entre eles ficam destacadas.
- **Base**: o ancestral comum, disponível quando os marcadores trazem uma seção `|||||||` (estilo de conflito `diff3` ou `zdiff3` do Git). Sem ela o painel fica vazio e a barra de status avisa.
- **Resultado**: o texto que o arquivo terá. Um conflito ainda pendente aparece em âmbar; um já decidido aparece em verde, com a escolha escrita à direita.

| Comando | O que faz |
| --- | --- |
| Layout misto | As duas entradas em cima, o resultado embaixo (padrão). |
| Layout em colunas | Atual, resultado e recebido lado a lado. |
| Mostrar a base, no topo, no centro | Adiciona o painel da base depois das entradas ou entre elas; executar o mesmo comando de novo o oculta. |
| Mostrar ou ocultar o texto sem conflitos | Mantém só as linhas ao lado de um conflito. |
| Ir ao próximo ou ao anterior conflito pendente | Move o conflito ativo (marcado com uma barra à esquerda) e rola até ele. |

Teclas: Cima e Baixo andam entre os conflitos pendentes, Page Up e Page Down rolam, `L` troca o layout, `S` o painel da base, `H` o texto comum e Escape fecha o editor. A roda do mouse rola e um clique em um conflito o torna o ativo.

Quando o arquivo é salvo com o editor de mesclagem aberto, os painéis são carregados de novo. As decisões já tomadas são mantidas para todo conflito cujo texto não mudou.

## Decidindo um conflito

A decisão vale para o conflito ativo. Aceitar passa para o próximo conflito pendente, então um arquivo pode ser resolvido com uma tecla por conflito; alternar e reverter ficam onde estão.

| Comando | Tecla | O que faz |
| --- | --- | --- |
| Aceitar atual | `C` | O resultado fica com o lado atual. |
| Aceitar recebido | `I` | O resultado fica com o lado recebido. |
| Aceitar ambos, atual primeiro / recebido primeiro | `B` / `Shift+B` | O resultado fica com os dois lados, nessa ordem. |
| Ignorar conflito | `N` | Conta o conflito como tratado e mantém a base, ou nada quando os marcadores não trazem base. |
| Reverter conflito para pendente | `R` | Desfaz a decisão. |
| Alternar atual / recebido no conflito ativo | `1` / `2` | Adiciona ou remove um lado; o lado adicionado depois vai depois. Remover os dois torna o conflito pendente de novo. |
| Alternar entre as entradas | | Escolhe de qual entrada partem os comandos de comparação. |
| Comparar atual / recebido com a base | | Abre um diff do arquivo inteiro como aquela entrada o vê contra a base. Precisa de uma base nos marcadores. Escape volta ao editor de mesclagem. |
| Comparar a base com o resultado | | O mesmo, contra o resultado até agora. |
| Abrir o resultado no editor | `O` | Fecha o editor de mesclagem e abre o arquivo com o resultado até agora; conflitos ainda sem decisão mantêm seus marcadores, para o resto ser terminado à mão. O arquivo em disco só muda ao salvar. |

O painel de resultado não aceita digitação. Para editar à mão use **Abrir o resultado no editor**; salvar um arquivo que o Git lista como em conflito o adiciona ao índice.

## Concluindo a mesclagem

A linha de título mostra quantos conflitos ainda estão pendentes.

| Comando | Tecla | O que faz |
| --- | --- | --- |
| Aceitar tudo do atual / do recebido | | Fica com aquele lado em todos os conflitos. |
| Aceitar tudo, os dois lados | | Fica com os dois lados, atual primeiro, em todos os conflitos. |
| Redefinir o resultado | | Esquece todas as decisões; o primeiro conflito volta a ser o ativo. |
| Concluir a mesclagem | `Enter` | Exige todos os conflitos decididos. Grava o resultado no arquivo com a codificação e as quebras de linha originais e, quando o Git lista o arquivo como em conflito, o adiciona ao índice. O editor de mesclagem fecha. Se o arquivo tiver edições não salvas, ou a gravação falhar, nada muda e o editor de mesclagem continua aberto. |
| Redefinir o aviso ao fechar | | Fechar com decisões não gravadas avisa primeiro e descarta no segundo pedido; este comando faz o aviso valer de novo. Qualquer nova decisão faz o mesmo. |

Fechar sem nenhuma decisão não avisa.

## Pontos de entrada

- **Lista do controle de código-fonte:** abrir um arquivo que o Git lista como em conflito (a ação de abrir conflito, tecla `R` na lista) abre o editor de mesclagem quando o arquivo tem marcadores de conflito. Um arquivo em conflito sem marcadores, por exemplo com um dos lados apagado, continua abrindo a visão de três vias com base, nosso e deles.
- **Visão de três vias:** pressione `M` para continuar no editor de mesclagem. Se o arquivo não tiver marcadores, a visão continua e a barra de status explica o motivo.
- **Paleta de comandos:** **Abrir editor de mesclagem** para o arquivo ativo.
