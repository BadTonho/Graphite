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

Os comandos `Git:` estão na paleta de comandos (e listados pelo botão ⋯): Pull, Push, Fetch, Sincronizar, Stash, Ver histórico, Criar pull request, Mais ações e os comandos de branch abaixo.

O Git roda em segundo plano, então a janela continua respondendo durante um pull ou push. Erros do Git (por exemplo, um remoto inacessível) aparecem na barra de status.

Limites: os botões usam símbolos de texto, então podem ficar um pouco diferentes dos ícones do VS Code. O botão ⋯ abre a paleta, e não um menu suspenso. Passar o mouse sobre um botão ainda não mostra dica.

## Branches, amend, merge e rebase

| Comando | O que faz |
|---|---|
| **Git: Corrigir último commit (amend)** | Substitui o último commit pelas alterações preparadas (staged). O texto digitado no campo de mensagem vira a nova mensagem (e o campo é limpo); com o campo vazio a mensagem existente é mantida. |
| **Git: Criar branch...** | Pede um nome, cria a branch no commit atual e muda para ela. Alterações não commitadas acompanham. O nome é validado com `git check-ref-format`; nomes existentes e inválidos são recusados. |
| **Git: Mesclar branch...** | Abre o seletor de branch; a escolhida é mesclada na atual com um commit de merge (`--no-edit`). |
| **Git: Rebase sobre branch...** | Abre o seletor de branch; a branch atual sofre rebase sobre a escolhida. |
| **Git: Abortar merge ou rebase** | Abandona um merge ou rebase parado por conflitos e restaura o estado anterior. |
| **Git: Continuar merge ou rebase** | Depois de resolver e preparar os arquivos em conflito: continua o rebase ou faz o commit do merge. |

Observações:

- Merge, rebase e continuar são recusados enquanto um documento aberto tem edições não salvas, porque reescrevem arquivos sob ele. Criar branch, amend e abortar não são.
- Um conflito não perde nada: o merge ou rebase continua em andamento, os arquivos em conflito aparecem na visão de Controle de Código-Fonte (abra-os no editor de conflitos em 3 vias) e a barra de status orienta a resolver, continuar ou abortar.
- Argumentos de branch que começam com `-` ou contêm espaços são rejeitados antes de o git rodar, então um nome de branch nunca é lido como opção do git.
- O Git roda com `GIT_EDITOR=true`, então nenhum editor abre para mensagens de merge ou rebase; a mensagem padrão é usada.
- Ainda faltam: excluir branches, rebase interativo, escolher uma branch remota no seletor (só as locais são listadas) e as opções `merge --squash`/`--no-ff`.
