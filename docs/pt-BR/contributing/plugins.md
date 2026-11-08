# Criando e instalando plugins

Compile o Graphite com `cargo build --workspace`. Plugins nativos exigem Node.js 24 ou superior no sistema; a inspeção de VSIX não exige Node. Configure `graphite.plugins.nodePath` no `settings.json` do usuário para selecionar outro executável. Windows e Linux usam a mesma API e formato de pacote.

## Criar um plugin nativo

```sh
graphite plugin create my-plugin
cd my-plugin
npm install
npm run build
graphite plugin pack .
```

O gerador cria código TypeScript, declarações locais da API, configuração de build e comando de exemplo. O pacote é gravado ao lado do projeto como `local.my-plugin-0.1.0.graphite-plugin`. Arquivos de saída existentes não são sobrescritos. Compile antes de empacotar. Pacotes incluem código compilado e dependências presentes no projeto; o Graphite não instala dependências nem compila. Agrupe dependências de runtime quando possível e evite distribuir dependências de desenvolvimento ou dados privados. Arquivos ocultos são omitidos; extensões de chaves/bancos e links simbólicos são rejeitados.

O manifesto na raiz é:

```json
{
  "id": "local.my-plugin",
  "version": "0.1.0",
  "apiVersion": 1,
  "main": "dist/index.js",
  "description": "My Graphite plugin",
  "commands": [{"id": "local.my-plugin.hello", "title": "Hello from my plugin"}]
}
```

IDs seguem `publisher.name`, com letras ASCII minúsculas, dígitos e hífens. Versões seguem semver. IDs de comandos começam com o ID completo do plugin e `.`; os registros usam seus títulos declarados. Entradas são caminhos relativos `.js`, `.cjs` ou `.mjs`. Módulos ES usam `"type": "module"` no `package.json`; CommonJS pode exportar `activate` diretamente.

## API v1

Exporte `async activate(context, graphite)` e opcionalmente `deactivate()`. O arquivo gerado `graphite.d.ts` define o SDK:

- `commands.registerCommand(id, handler)` retorna um recurso descartável. Handlers recebem argumentos JSON e `AbortSignal` e podem retornar resultados JSON de forma assíncrona.
- `documents.list()` e `snapshot(uri)` leem o texto atual em memória, inclusive alterações não salvas. `documents.onDidChange(listener)` recebe snapshots de abertura, mudança e fechamento e retorna um recurso descartável.
- `documents.applyEdits(edits)` aceita `{uri, version, edits:[{start, end, text}]}`. Posições são `{line, character}` baseadas em zero e UTF-16; use a versão do snapshot. Mudanças preservam validação e undo; salvamento é separado e não é exposto na v1.
- `workspace.get()` e `workspace.configuration()` consultam o editor; `window.showMessage(text)` exibe mensagem.

Registre recursos em `context.subscriptions`; o descarte é idempotente. O encerramento do host também libera recursos no editor. Use o sinal de cancelamento em operações longas. Logs vão para stderr. Plugins executam com acesso da conta local do usuário: habilite somente código confiável.

## Instalar e gerenciar

Abra **Plugins** com `Ctrl+Shift+X`. As setas selecionam linhas; Escape retorna ou fecha. Ações do cabeçalho são clicáveis e têm equivalentes no teclado:

| Tecla | Ação |
|---|---|
| I / G | Plugins instalados / catálogo GitHub |
| L | Digitar caminho local `.graphite-plugin` ou `.vsix` |
| / | Digitar pesquisa no GitHub; Enter envia |
| P / N | Página anterior / seguinte do catálogo |
| Enter | Inspecionar plugin instalado ou mostrar assets da release e baixar o selecionado |
| Y / Esc | Confirmar / cancelar instalação ou confiança |
| E / D | Habilitar / desabilitar o plugin nativo selecionado |
| R / X | Reiniciar / remover o plugin selecionado |
| C | Executar o primeiro comando registrado do plugin |

O painel mostra os IDs declarados; associe outros comandos no `keybindings.json`, por exemplo `{"key":"ctrl+alt+h","command":"local.my-plugin.hello"}`. Instalar e atualizar redefine habilitação e confiança. A confirmação permite executar código local daquela versão instalada. Node ausente e falhas de ativação aparecem no painel; use R para tentar novamente depois de corrigir a causa.

Pacotes VSIX mostram identidade, versão, engines, entrada e categorias de contribuições. Seu estado é **Imported — VS Code execution unavailable** (Importado — execução VS Code indisponível). Eles não podem ser habilitados e seus temas, comandos e código não executam.

## Publicar e testar

Adicione o tópico GitHub `graphite-plugin` e anexe assets `.graphite-plugin` ou `.vsix` a uma release estável. O Graphite lista todos os assets suportados para escolha. Ele nunca clona/compila o repositório. Atualizações são instaladas manualmente pelo catálogo ou arquivo local. A descoberta pública não usa token e informa falhas de rede/limite de requisições.

Execute `cargo test -p graphite-plugins` para testes de pacotes, persistência e catálogo. Os testes da aplicação exigem Node.js 24+ e usam exemplos locais: `cargo test -p graphite-app app::plugins`. Finalize mudanças com `cargo test --workspace` e `cargo build --workspace`; testes não acessam a API GitHub real. A execução Linux e verificações interativas devem ser registradas separadamente quando indisponíveis.

A validação inicial no Windows passou nos testes e build do workspace, formatação Rust e verificação de sintaxe do host Node. A verificação interativa de UI/GPU no Windows não pôde executar porque o canal nativo do Computer Use estava indisponível (`os error 2`) nas duas tentativas de conexão. A execução Linux não foi realizada neste ambiente Windows. O catálogo foi validado com servidor HTTP local, sem acessar o serviço GitHub real.
