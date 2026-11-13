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
