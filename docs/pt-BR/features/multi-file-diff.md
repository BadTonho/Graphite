# Diff de vários arquivos

Uma única visão rolável com as alterações de vários arquivos do repositório, cada arquivo recolhível, com próxima e anterior alteração que passam de um arquivo para o outro.

## Abrindo

| Comando | O que compara |
| --- | --- |
| Git: abrir todas as alterações | A árvore de trabalho com o último commit. Tecla `D` na lista do controle de código-fonte. |
| Git: abrir todas as alterações preparadas | O índice com o último commit. |
| Git: abrir todas as alterações não preparadas | A árvore de trabalho com o índice, incluindo arquivos novos não rastreados. |

Arquivos cujos dois lados são iguais ficam de fora. Arquivos adicionados e apagados são marcados com `A` e `D` (modificados com `M`), e um arquivo que não é texto aparece como arquivo binário, sem linhas. Se nada mudou, ou a pasta não é um repositório, a barra de status avisa e nenhuma visão abre. Os arquivos são lidos em segundo plano, então a janela continua responsiva em repositórios grandes.

## Lendo

Cada arquivo tem um cabeçalho com o caminho e as contagens de linhas adicionadas e removidas, seguido das linhas alteradas com três linhas de contexto. Trechos de linhas sem alteração entre dois grupos de mudanças ficam recolhidos em uma linha "N linhas sem alteração". Os dois números de linha (antigo e novo) aparecem. A linha de título traz os totais da visão.

| Comando | Tecla | O que faz |
| --- | --- | --- |
| Ir à próxima / anterior alteração | Baixo / Cima | Vai ao próximo ou anterior trecho de linhas alteradas em todos os arquivos, voltando ao início no fim. Uma alteração dentro de um arquivo recolhido o expande. A alteração atual é marcada com uma barra à esquerda. |
| Recolher todos / Expandir todos | `C` / `E` | Recolhe todos os arquivos ao cabeçalho, ou expande todos. |
| Recolher ou expandir o arquivo selecionado | Espaço | Também feito com um clique no cabeçalho do arquivo. |
| Focar a lista de arquivos | Tab | Mostra uma lista dos arquivos à esquerda. Cima e Baixo escolhem um arquivo e rolam até ele, Enter o recolhe ou expande, Tab ou Escape voltam às alterações. |
| Abrir o arquivo | Enter | Abre o arquivo da alteração atual no editor naquela alteração e fecha a visão. Um arquivo apagado não pode ser aberto; a barra de status avisa. |
| Fechar o diff de vários arquivos | Escape | |

Page Up, Page Down e a roda do mouse rolam.
