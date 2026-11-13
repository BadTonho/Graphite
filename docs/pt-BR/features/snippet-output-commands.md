# Comandos de snippets e de saída

| Comando | O que faz |
| --- | --- |
| Configurar snippets | Abre `snippets.json` no perfil, criando-o quando não existe. |
| Inserir snippet (`editor.action.insertSnippet`) | Recebe `{"text": "..."}` e insere esse texto em cada cursor. Serve para ser chamado por outro comando ou por uma extensão; não tem lista de escolha. Texto vazio é recusado. |
| Alternar saída | Mostra a saída das tarefas no painel, ou esconde o painel quando ele já a mostra. Vindo da lista de problemas, troca para a saída. |
| Mostrar saída | Mostra a saída das tarefas no painel. |

Canais de saída, logs e a vista de linha do tempo não estão disponíveis: o Graphite tem uma saída só, a das tarefas.

## Logs

**Mostrar logs** e **Abrir arquivo de log** listam os arquivos `*.log` do perfil (até duas pastas de profundidade, os mais novos primeiro), por exemplo os logs de APIs não suportadas e de comandos pedidos que as extensões geram. Escolher um o abre. Sem arquivos de log, avisam.
