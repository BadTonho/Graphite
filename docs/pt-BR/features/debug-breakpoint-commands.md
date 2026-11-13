# Comandos de pontos de interrupção e do console de depuração

| Comando | O que faz |
| --- | --- |
| Ir para o próximo / anterior ponto de interrupção | Move o cursor para o próximo ou anterior ponto de interrupção do arquivo ativo, dando a volta. Avisa quando o arquivo não tem nenhum. |
| Remover ponto de interrupção | Remove o ponto de interrupção da linha do cursor; avisa quando não há nenhum ali. Atualiza uma sessão de depuração em andamento. |
| Ativar ou desativar os pontos de interrupção | Desliga todos os pontos de interrupção, ou os religa. Eles são mantidos. |
| Reaplicar todos os pontos de interrupção | Envia os pontos de interrupção de todos os arquivos para a sessão de depuração em andamento de novo. |
| Limpar o console de depuração | Esvazia o console de depuração. |
| Copiar o console de depuração / Copiar toda a saída da depuração | Copia o texto do console para a área de transferência. |
| Avaliar a entrada do console de depuração | Avalia o que está digitado na entrada do console. |
| Mostrar a vista de depuração | Mostra o painel de depuração. |
| Recolher variáveis / expressões monitoradas / pilha de chamadas / scripts carregados | Recolhe todas as variáveis e escopos expandidos no painel. |

## Comandos renomeados

Estes comandos mudaram de id. Um atalho personalizado em `keybindings.json` que usa o id antigo precisa do novo; o `F9` já foi atualizado.

| Id antigo | Id novo |
| --- | --- |
| `graphite.debug.toggleBreakpoint` | `editor.debug.action.toggleBreakpoint` |
| `graphite.debug.conditionalBreakpoint` | `editor.debug.action.conditionalBreakpoint` |
| `graphite.debug.editBreakpoint` | `editor.debug.action.editBreakpoint` |
| `graphite.debug.logpoint` | `editor.debug.action.addLogPoint` |
| `graphite.debug.enableAllBreakpoints` | `workbench.debug.viewlet.action.enableAllBreakpoints` |
| `graphite.debug.disableAllBreakpoints` | `workbench.debug.viewlet.action.disableAllBreakpoints` |
| `graphite.debug.removeAllBreakpoints` | `workbench.debug.viewlet.action.removeAllBreakpoints` |

Ativar ou desativar um ponto de interrupção isolado, pontos de interrupção de função e de dados, alterar uma variável e o hover de depuração ainda não estão disponíveis.
