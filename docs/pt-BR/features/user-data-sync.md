# Sincronização de Dados do Usuário

O Graphite sincroniza preferências e customizações do usuário entre máquinas utilizando Gists privados do GitHub.

## Recursos Sincronizados

- `settings.json` (mesclagem JSON de 3 vias por chave)
- `keybindings.json`
- Arquivos de snippets em `<perfil>/snippets/`
- Definições de perfis (`profiles.json`)
- Lista de plugins e extensões instalados

## Autenticação e Armazenamento

- Utiliza o fluxo de dispositivo OAuth do GitHub solicitando o escopo `read:user gist` (`graphite_auth::SCOPE_GIST`).
- Armazena o estado de sincronização, snapshot base, ID do Gist e conflitos pendentes em `<perfil>/user-data-sync.json`.
- Lê e grava um arquivo em Gist privado (`graphite-sync.json`) por meio de `GitHubGistTransport`.

## Estratégia de Mesclagem e Backups Locais

- Antes de gravar alterações mescladas na pasta do perfil, o Graphite salva um backup com carimbo de tempo em `<perfil>/sync-backups/`.
- O `settings.json` usa mesclagem de 3 vias em nível de chave contra o último snapshot base sincronizado, combinando alterações independentes sem perda.
- Chaves alteradas de forma divergente são registradas como conflitos manuais até resolução via `workbench.userDataSync.actions.acceptMerges`.
- `workbench.actions.syncData.reset` redefine os metadados locais e remove o Gist remoto quando autenticado.

## Menu do perfil

O botão do perfil, na parte de baixo da barra de atividades, abre um menu que agora controla a sincronização, abaixo das linhas da conta do GitHub:

- **Ativar sincronização...** (sincronização desligada): entra na conta se for preciso e faz a primeira sincronização.
- Com a sincronização ligada, uma linha de estado (`Sincronizando...`, `Sincronizado há 5 min`, `A última sincronização falhou` ou o número de conflitos para revisar), **Sincronizar agora** e **Desativar sincronização**.
- Se a sessão do GitHub foi criada sem a permissão de Gist, ativar a sincronização pede essa permissão com um novo login, mantendo as permissões já concedidas.

## Backend de repositório privado

Além do Gist, os dados podem ficar em um **repositório privado**. No menu do perfil, com a sincronização desligada, as duas opções são separadas: **Ativar sincronização com Gist privado...** e **Ativar sincronização com repositório privado...**. A linha de estado mostra qual está em uso.

- **Onde:** um repositório chamado `graphite-sync` na conta logada, com um único arquivo, `graphite-sync.json`. Ele é criado privado (com um primeiro commit) quando não existe. Cada sincronização é um commit, então o repositório guarda o histórico dos dados sincronizados.
- **Permissão:** o repositório precisa do escopo `repo` do GitHub, que dá acesso a todos os repositórios privados da conta (o GitHub não oferece um escopo mais estreito para um token pessoal). O Graphite só lê e grava o repositório `graphite-sync`. Se a sessão não tiver o escopo, ativar a opção pede esse escopo com um novo login que mantém as permissões já concedidas.
- **Nunca público:** se `graphite-sync` existir e for público, o Graphite se recusa a ler ou gravar nele e avisa; torne-o privado ou apague-o.
- **Reset:** *Reset Synced Data* remove o `graphite-sync.json` do repositório. O repositório em si nunca é apagado.
- **Troca:** escolher o outro backend recomeça do zero: o destino guardado e a base de mesclagem do anterior são esquecidos, então a primeira sincronização com o novo backend compara os dados locais com o que ele já tiver. Arquivos de estado de antes desta mudança continuam funcionando como Gist.
- **Falhas:** uma sincronização que falha fica registrada em `<app_config_dir>/logs/errors/medium.jsonl`.
