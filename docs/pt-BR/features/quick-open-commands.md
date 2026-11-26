# Comandos de abrir rápido e de histórico de editores

O abrir rápido de arquivos classifica os arquivos de todas as raízes do workspace, deixando de fora `.git` e o que os arquivos de ignorar excluem. Na primeira vez que abre, o Graphite lista os arquivos numa thread em segundo plano e guarda a lista na memória; até lá, cada consulta percorre as pastas. Depois, a lista só é refeita quando um arquivo é criado ou removido fora dos caminhos excluídos pelo `.gitignore` da raiz, então digitar uma consulta não lê mais o disco.

| Comando | O que faz |
| --- | --- |
| Abrir rápido o editor anterior do histórico (e Abrir rápido o editor anterior) | Abre uma lista com os editores abertos, o mais recente primeiro, com o anterior selecionado. Enter troca para ele. |
| Abrir rápido o editor menos usado recentemente | A mesma lista, do mais antigo ao mais novo, com o mais antigo selecionado. |
| Variantes "no grupo" | Listam só os editores do grupo focado. |
| Aceitar o item selecionado do abrir rápido | Aceita o item selecionado do abrir rápido ou da lista aberta. |
| Abrir o item selecionado do abrir rápido em aba permanente | Como aceitar, mas um arquivo do abrir rápido abre como aba permanente em vez de aba de pré-visualização. |
| Fechar o abrir rápido | Fecha o abrir rápido, as listas e a paleta de comandos. |
| Focar o abrir rápido | Abre o abrir rápido de arquivos quando nenhum widget está aberto; um widget aberto já está com o teclado. |
| Abrir rápido com modos | Abre o abrir rápido de arquivos. |
| Abrir o próximo / anterior editor usado recentemente (e no grupo) | Trocam de editor pela ordem de uso recente, igual aos comandos existentes Próximo / Anterior editor usado. |

Cada documento aparece uma vez, mesmo aberto em vários grupos. Editores sem arquivo não aparecem. A lista mostra nome do arquivo e pasta; não oferece os prefixos de modo de outros editores (`>`, `@`, `:`).
