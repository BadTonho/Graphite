# Ações do Controle de Versão

A visão de Controle de Versão tem as mesmas ações principais do VS Code:

| Onde | Botão | Ação |
|---|---|---|
| Linha do repositório | ⇅ | **Sincronizar**: faz pull e depois push. Uma branch sem upstream é publicada no `origin`. |
| Linha do repositório | ⇄ | **Criar pull request**: abre a página de comparação do GitHub para a branch atual. Precisa de um remoto `origin` no GitHub. |
| Linha do repositório | ⋯ | **Mais ações**: abre um menu com submenus, veja [Menus do Controle de Versão](source-control-menus.md). |
| Cabeçalho de alterações | ↺ | **Ver histórico**: mostra os últimos 300 commits em grafo, numa view somente leitura. |
| Cabeçalho de alterações | ▤ | **Stash**: guarda todas as alterações, inclusive arquivos novos. |
| Cabeçalho de alterações | + | Preparar tudo (já existia). |

Os comandos `Git:` estão na paleta de comandos (e os mesmos comandos estão agrupados no menu ⋯): Pull, Push, Fetch, Sincronizar, Stash, Ver histórico, Criar pull request, Mais ações e os comandos de branch abaixo.

O Git roda em segundo plano, então a janela continua respondendo durante um pull ou push. Erros do Git (por exemplo, um remoto inacessível) aparecem na barra de status.

Os botões são pequenos ícones desenhados, sem moldura; o que está sob o mouse ganha destaque, e o botão Commit mostra um visto ao lado do texto. A caixa de commit sugere o atalho e a branch em que o commit será feito. Limite: passar o mouse sobre um botão ainda não mostra dica.

## Branches, amend, merge e rebase

| Comando | O que faz |
|---|---|
| **Git: Corrigir último commit (amend)** | Substitui o último commit pelas alterações preparadas (staged). O texto digitado no campo de mensagem vira a nova mensagem (e o campo é limpo); com o campo vazio a mensagem existente é mantida. |
| **Git: Criar branch...** | Pede um nome, cria a branch no commit atual e muda para ela. Alterações não commitadas acompanham. O nome é validado com `git check-ref-format`; nomes existentes e inválidos são recusados. |
| **Git: Mesclar branch...** | Abre o seletor de branch (branches locais, exceto a atual, e depois as remotas como `origin/main`); a escolhida é mesclada na atual com um commit de merge (`--no-edit`). |
| **Git: Rebase sobre branch...** | Abre o seletor de branch; a branch atual sofre rebase sobre a escolhida. |
| **Git: Excluir branch...** | Abre o seletor com as branches locais, exceto a atual, e exclui a escolhida com `git branch -d`, que recusa uma branch sem merge completo. |
| **Git: Excluir branch (forçar)...** | O mesmo com `git branch -D`: exclui até uma branch sem merge (os commits dela só ficam acessíveis pelo reflog). |
| **Git: Abortar merge ou rebase** | Abandona um merge ou rebase parado por conflitos e restaura o estado anterior. |
| **Git: Continuar merge ou rebase** | Depois de resolver e preparar os arquivos em conflito: continua o rebase ou faz o commit do merge. |

Observações:

- Merge, rebase e continuar são recusados enquanto um documento aberto tem edições não salvas, porque reescrevem arquivos sob ele. Criar branch, amend e abortar não são.
- Um conflito não perde nada: o merge ou rebase continua em andamento, os arquivos em conflito aparecem na visão de Controle de Código-Fonte (abra-os no editor de conflitos em 3 vias) e a barra de status orienta a resolver, continuar ou abortar.
- Argumentos de branch que começam com `-` ou contêm espaços são rejeitados antes de o git rodar, então um nome de branch nunca é lido como opção do git.
- O Git roda com `GIT_EDITOR=true`, então nenhum editor abre para mensagens de merge ou rebase; a mensagem padrão é usada.
- O seletor mostra 12 entradas em volta da seleção, então listas longas rolam com as setas. O checkout também lista as branches de rastreamento remoto: escolher `origin/feature` executa `git switch --track` e cria uma `feature` local que a rastreia (o git informa se já existe uma branch local com esse nome).
- Ainda faltam: as opções `merge --squash`/`--no-ff`.

## Rebase interativo

| Comando | O que faz |
|---|---|
| **Git: Rebase interativo sobre branch...** | Escolha a base no seletor de branch. O Graphite grava o plano em `git-rebase-plan.txt` na pasta do perfil e o abre: uma linha `pick <hash> <assunto>` por commit entre a base e o `HEAD`, do mais antigo ao mais novo, sem commits de merge. |
| **Git: Executar rebase interativo** | Lê o plano do editor (não precisa estar salvo), valida e executa `git rebase -i` com ele. |
| **Git: Cancelar rebase interativo** | Esquece o plano sem executar nada. |

No plano você pode reordenar as linhas e trocar a primeira palavra por `pick` (`p`), `squash` (`s`: funde no commit anterior juntando as duas mensagens), `fixup` (`f`: funde e descarta esta mensagem) ou `drop` (`d`); apagar uma linha também remove o commit. O Graphite confere o plano antes de o git rodar: ações desconhecidas, commits desconhecidos ou repetidos, um `squash`/`fixup` sem nada antes e um plano que não mantém nenhum commit são informados com o número da linha e nada começa. Os outros documentos abertos precisam estar salvos; o próprio plano pode estar sem salvar.

O Git recebe o plano conferido por `GIT_SEQUENCE_EDITOR` (um `cp` do plano sobre a lista de tarefas, executado pelo shell do próprio git) e `GIT_EDITOR=true`, então nenhum editor abre e os commits fundidos ficam com a mensagem combinada. Um conflito deixa o rebase em andamento, como em **Git: Rebase sobre branch...**: resolva e use **Git: Continuar merge ou rebase**, ou **Git: Abortar merge ou rebase**.

Não suportado: `reword`, `edit`, `exec`, `break`, manter commits de merge (`--rebase-merges`) e editar a mensagem de um squash.

## Arquivos não salvos e a visualização de diferenças

- **Commit com arquivos não salvos.** O commit registra o que está no disco, então o texto que só existe no editor ficaria de fora. Quando documentos abertos dentro do repositório têm alterações não salvas, uma janela no meio da tela os lista e pergunta: **Salvar e commitar** (`S`), **Commitar assim mesmo** (`D`) ou **Cancelar** (`Esc`). Se o salvamento falhar, nenhum commit é feito.
- **Visualização de diferenças.** A comparação abre só na área do editor: a barra de menus, a barra de atividades, a barra lateral e a barra de status continuam visíveis e clicáveis. O cabeçalho tem um botão **×** (ou `Esc`) para fechar e um botão que alterna entre lado a lado e em linha (ou `Tab`). Clicar na barra lateral fecha a comparação. Quando os dois lados são iguais ou estão vazios, a tela avisa em vez de ficar em branco.

## Progresso e contagem de alterações

- **Indicador de trabalho.** Enquanto Sincronizar, Publicar, Commit, Push, Pull ou outra operação do Git está rodando, o botão principal troca o ícone por um anel de pontos girando e o texto por "Sincronizando...", "Publicando...", "Commitando..." ou "Trabalhando...". Clicar de novo no botão é ignorado até a operação terminar. O indicador some sozinho quando a operação responde, ou no máximo depois de dez minutos.
- **Selo de alterações.** O ícone do Controle de Código na barra de atividades mostra quantos arquivos têm alterações pendentes (em stage, fora do stage ou novos), até `99+`. Ele fica oculto quando não há nenhuma ou quando o repositório não pode ser lido.
