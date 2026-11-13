# Comandos de seleção e de cursor

Comandos que agem na seleção, no cursor e nos colchetes. Os que mudam texto se desfazem em um passo e exigem um editor gravável.

## Âncora de seleção

| Comando | Atalho | O que faz |
| --- | --- | --- |
| Definir âncora de seleção | `Ctrl+K Ctrl+B` | Guarda a posição do cursor e mostra `Âncora definida em linha:coluna` na barra de status. A âncora acompanha o texto quando ele é editado. |
| Selecionar da âncora até o cursor | `Ctrl+K Ctrl+K` | Seleciona da âncora até o cursor e descarta a âncora. |
| Ir para a âncora de seleção | nenhum | Move o cursor para a âncora. |
| Cancelar âncora de seleção | nenhum | Esquece a âncora. |

Os comandos de ir e de selecionar avisam `Nenhuma âncora de seleção foi definida` quando não há âncora. A âncora não é desenhada no editor.

## Colchetes

Colchetes são `()`, `[]` e `{}`, pareados pelo texto, então colchetes dentro de strings e comentários contam.

| Comando | Atalho | O que faz |
| --- | --- | --- |
| Ir para o colchete (`editor.action.jumpToBracket`) | `Ctrl+Shift+]` | Sobre um colchete, vai para o par dele. Em outro lugar, vai para o colchete de fechamento ao redor do cursor, ou para o próximo colchete. |
| Selecionar até o colchete | nenhum | Seleciona o par ao redor do cursor, com os colchetes (ou o próximo par, se não houver nenhum ao redor). No colchete de fechamento, a seleção corre para trás. |
| Remover colchetes | nenhum | Apaga o par no cursor ou ao redor dele. |

## Mover e trocar texto

| Comando | Atalho | O que faz |
| --- | --- | --- |
| Mover texto selecionado para a esquerda / direita | nenhum | Move uma seleção de uma linha um caractere ao longo da linha; o caractere que ela ultrapassa vai para o outro lado. |
| Transpor letras | nenhum | Troca as letras antes e depois do cursor, ou as duas últimas no fim de uma linha. Nunca troca através de uma quebra de linha. |
| Substituir pelo próximo / anterior valor | `Ctrl+Shift+.` / `Ctrl+Shift+,` | Substitui a seleção, ou a palavra ou número no cursor, pelo próximo ou anterior valor. Um número anda de 1 em 1 (`9` vira `10`, e não passa de 0 para baixo); `true`/`false`, `True`/`False` e `public`/`protected`/`private` alternam. O novo texto fica selecionado. |

## Tamanho da fonte do editor

**Aumentar / Diminuir / Restaurar a fonte do editor** (`editor.action.fontZoomIn`, `fontZoomOut`, `fontZoomReset`) usam o mesmo zoom das entradas do menu Exibir: passos de 10%, até 200%.
