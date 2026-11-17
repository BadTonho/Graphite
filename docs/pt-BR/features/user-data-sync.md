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
