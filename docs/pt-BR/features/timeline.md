# Linha do tempo

A linha do tempo de um arquivo lista, do mais novo ao mais antigo, as versões salvas do histórico local e os commits do Git que mexeram nele (até 30). Cada commit mostra a data, o hash abreviado e o assunto. Escolher uma entrada abre um diff entre aquela versão do arquivo e o arquivo atual.

| Comando | O que faz |
| --- | --- |
| Focar a vista da linha do tempo | Abre a lista da linha do tempo do arquivo ativo. |
| Atualizar a linha do tempo | Monta a lista de novo, por exemplo depois de um commit. |
| Comparar a entrada selecionada da linha do tempo | Com a linha do tempo aberta, abre o diff da entrada selecionada; fechada, abre a linha do tempo. |
| Seguir o editor ativo na linha do tempo | Desliga o acompanhamento, fixando a linha do tempo no arquivo atual, ou o liga de novo. |
| Mostrar a linha do tempo do espaço de trabalho | Mostra os commits recentes do repositório inteiro numa vista somente leitura. |

As entradas de commit são cópias do arquivo naquele commit, guardadas em `timeline/` no perfil para abrirem como as versões do histórico local. Um arquivo fora de um repositório Git, ou nunca commitado, só tem as versões salvas. Fontes de linha do tempo de extensões não são suportadas.
