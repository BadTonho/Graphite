# Ações do Controle de Versão

A visão de Controle de Versão tem as mesmas ações principais do VS Code:

| Onde | Botão | Ação |
|---|---|---|
| Linha do repositório | ⇅ | **Sincronizar**: faz pull e depois push. Uma branch sem upstream é publicada no `origin`. |
| Linha do repositório | ⇄ | **Criar pull request**: abre a página de comparação do GitHub para a branch atual. Precisa de um remoto `origin` no GitHub. |
| Linha do repositório | ⋯ | **Mais ações**: abre a paleta de comandos filtrada pelos comandos `Git:`. |
| Cabeçalho de alterações | ↺ | **Ver histórico**: mostra os últimos 300 commits em grafo, numa view somente leitura. |
| Cabeçalho de alterações | ▤ | **Stash**: guarda todas as alterações, inclusive arquivos novos. |
| Cabeçalho de alterações | + | Preparar tudo (já existia). |

Os comandos `Git:` também estão na paleta de comandos: Pull, Push, Fetch, Sincronizar, Stash, Ver histórico, Criar pull request.

O Git roda em segundo plano, então a janela continua respondendo durante um pull ou push. Erros do Git (por exemplo, um remoto inacessível) aparecem na barra de status.

Limites: os botões usam símbolos de texto, então podem ficar um pouco diferentes dos ícones do VS Code. O botão ⋯ abre a paleta, e não um menu suspenso. Passar o mouse sobre um botão ainda não mostra dica.
