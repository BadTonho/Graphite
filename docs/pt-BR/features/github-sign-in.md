# Login com GitHub

Cada perfil pode ser conectado a uma conta do GitHub pelo menu de perfil (o botão de perfil na barra de atividades).

## Para usuários

1. Abra o menu de perfil e escolha **Entrar com GitHub**.
2. O Graphite abre o github.com no seu navegador e mostra um código curto na barra de status, por exemplo `WDJB-MJHT`.
3. Digite o código na página do GitHub e autorize o Graphite. O Graphite espera a autorização e então mostra **Conectado como @seu-login**.

Depois disso, o menu mostra sua conta e **Sair da conta GitHub**. Sair remove o login salvo neste computador; não revoga a autorização no GitHub. Para revogar, abra suas configurações do GitHub em *Aplicativos → Aplicativos OAuth autorizados*.

O Graphite pede ao GitHub apenas o escopo `read:user`, que dá acesso ao perfil público (login e nome). Ele não lê repositórios nem dados privados.

### Onde o login fica guardado

O token de acesso é salvo em `github-session.json` na pasta do perfil. No Linux e no macOS, o arquivo só pode ser lido pela sua conta de usuário. No Windows, ele é protegido pelas permissões da pasta de perfil do usuário. Cada perfil tem seu próprio login.

## Para quem distribui o Graphite

O Graphite já traz seu próprio OAuth App do GitHub, então os usuários não precisam configurar nada. O App está registrado com o **Device Flow** ativado, e o client ID público (`Ov23lieI1U1RTbC3aaHz`) é compilado em todo build. O device flow nunca usa client secret, então nenhum segredo fica no código.

Para usar outro OAuth App (por exemplo, em um fork), crie um em *GitHub → Settings → Developer settings → OAuth Apps*, ative **Enable Device Flow** e compile com o client ID dele:

```
GRAPHITE_GITHUB_CLIENT_ID=<seu client id> cargo build --release -p graphite-app
```

A variável de ambiente substitui o ID embutido naquele build. Nunca coloque um client secret no Graphite.

## Limites

- O login precisa de acesso à internet para github.com e api.github.com.
- O código expira em cerca de 15 minutos. Se expirar, escolha o login de novo.
- O login vale só para o menu de perfil; outros recursos ainda não usam a conta para chamar o GitHub.
