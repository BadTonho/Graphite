# Menus do Controle de Versão, fluxo de commit e Grafo

## O botão principal

O botão largo sob a mensagem de commit muda conforme o estado da branch:

| Estado | Botão |
|---|---|
| Há alterações, ou nada a sincronizar | **Commit** (com uma seta, veja abaixo) |
| Sem alterações e a branch à frente ou atrás do upstream | **Sincronizar Alterações** com as contagens, como `1↓ 2↑`: faz pull e depois push |
| Existe um remoto, mas a branch ainda não rastreia nada | **Publicar Branch**: faz push e define o upstream |
| Não há remoto | **Publicar Branch**: abre o diálogo de publicação no GitHub |

As contagens de à frente e atrás e o upstream vêm de `git for-each-ref`, então seguem o último fetch. Use **Fetch** para atualizá-las.

## Commit

Os commits rodam por `git commit`, então hooks (`pre-commit`, `commit-msg`), assinatura e a identidade da configuração do git valem exatamente como no terminal. Um hook que falha interrompe o commit, e a mensagem continua na caixa.

A seta na ponta direita do botão abre: **Commit**, **Commit e Push** e **Commit e Sincronizar**. O push ou a sincronização só rodam se o commit der certo.

O submenu **⋯ → Commit** tem todas as variantes:

| Entrada | O que faz |
|---|---|
| Commit | Faz commit do que está preparado (staged); se nada estiver preparado, prepara e faz commit de tudo. |
| Commit dos Preparados | Só o que está preparado. Recusado se nada estiver preparado. |
| Commit de Tudo | Prepara todas as alterações (arquivos novos incluídos) e faz o commit. |
| ... (Amend) | Os mesmos três, substituindo o último commit. Mensagem em branco mantém a antiga. |
| ... (Com Signed-off-by) | Os mesmos três, adicionando uma linha `Signed-off-by`. |
| Desfazer Último Commit | Remove o último commit e mantém as alterações preparadas. O primeiro commit não pode ser desfeito. |
| Abortar Merge ou Rebase | Abandona um merge, rebase, cherry-pick ou revert parado por conflitos. |

## O menu ⋯

O botão abre um menu com submenus que abrem ao passar o mouse ou com a seta para a direita. As setas movem, Enter escolhe, Esquerda fecha um submenu e Escape fecha o menu.

- **Pull**, **Push**, **Checkout para...**, **Fetch**
- **Commit** (acima)
- **Alterações**: Preparar Todas, Remover Todas do Stage, **Descartar Todas as Alterações** (pergunta antes)
- **Pull, Push**: Sincronizar, Pull, Pull (Rebase), **Pull de...** (escolha uma branch remota), Push, **Push para...** (escolha um remoto; a branch passa a rastreá-lo), Fetch
- **Branch**: Merge, Rebase, Criar, **Criar a Partir de...**, **Renomear**, Excluir, Excluir (Forçado), **Excluir Branch Remota...**
- **Stash**: Stash, Stash (Incluir Não Rastreados), Stash dos Preparados, Aplicar Último Stash, Aplicar e Remover Último Stash, **Aplicar Stash...**, **Aplicar e Remover Stash...**, **Descartar Stash...** e **Descartar Todos os Stashes** (pergunta antes)
- **Tags**: Criar Tag (no HEAD), **Excluir Tag...**
- **Remoto**: **Adicionar Remoto...** (pede a URL e depois o nome, sugerindo `origin`) e **Remover Remoto...**

As entradas terminadas em `...` abrem uma lista para escolher (a mesma lista do seletor de branch): branches, branches remotas, tags, entradas de stash ou remotos. Uma lista sem nada para escolher não abre e avisa na barra de status. **Criar a Partir de...** lista branches, branches remotas e tags e depois pede o nome da nova branch.

Os comandos `Git:` continuam listados na paleta de comandos.

## Lista de alterações

- A letra no fim de cada linha diz como o arquivo difere naquele lado: `M` modificado, `A` adicionado, `D` excluído, `U` não rastreado, `R` renomeado, `C` copiado, `!` conflito. Um arquivo com edições preparadas e não preparadas mostra a própria letra em cada lista.
- As linhas não preparadas têm um botão **Descartar Alterações** (a seta) ao lado do botão de preparar. Descartar um arquivo rastreado o restaura ao conteúdo preparado; descartar um não rastreado o exclui. Ambos pedem confirmação, e as alterações preparadas nunca são tocadas. Arquivos em conflito não são descartados: resolva-os.
- **⋯ → Exibir como Árvore** mostra cada lista em pastas (pastas primeiro, depois arquivos, em ordem alfabética) e **Exibir como Lista** volta ao normal. Clique numa pasta para abri-la ou fechá-la; o botão `+` / `-` dela prepara ou remove do stage todos os arquivos abaixo. Uma pasta abre e fecha separadamente nas listas de preparados e não preparados.
- Clique com o botão direito num arquivo para: Abrir Arquivo, Preparar ou Remover Alterações do Stage, Descartar Alterações, **Adicionar ao .gitignore** (uma entrada ancorada como `/src/a.rs`, adicionada uma só vez) e Copiar Caminho / Copiar Caminho Relativo.

## Grafo

Clique com o botão direito num commit para: **Checkout (Detached)**, **Criar Branch...**, **Criar Tag...** (ambas nesse commit), **Cherry Pick**, **Reverter Commit**, **Copiar ID do Commit** e **Copiar Mensagem do Commit**. Checkout e Cherry Pick ficam desabilitados no commit que já está em uso. Um cherry-pick ou revert parado por conflitos pode ser concluído ou abortado com **Git: Continuar merge ou rebase** e **Abortar merge ou rebase**.

O Grafo desenha os commits como círculos (um anel para o commit em uso) e continua carregando commits mais antigos, 100 por vez, ao rolar até o fim. Passar o mouse sobre um commit mostra um cartão com mensagem, autor, data, id completo e as branches e tags nele.

Os ids de commit passados ao git são conferidos como hexadecimais, e nomes de branch e tag são validados antes de o git rodar, então nada digitado pode ser lido como opção do git.

## Dicas

Todo botão de ícone da visão (linha do repositório, seta do commit, linhas de arquivo, cabeçalhos de Alterações e do Grafo) mostra o nome num pequeno cartão quando o mouse para sobre ele.

## Limites

**Clone** ainda não existe.
