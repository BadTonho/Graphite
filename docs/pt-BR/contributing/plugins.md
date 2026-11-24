# Criando e instalando plugins

Compile o Graphite com `cargo build --workspace` para desenvolver plugins nativos. Plugins JavaScript/TypeScript (`"runtime": "node"`, padrão) usam Node.js 24+ e aceitam um executável personalizado por `graphite.plugins.nodePath` nas configurações do usuário. Plugins nativos compilados em Rust (`"runtime": "native"`) usam a crate `graphite-plugin-sdk` diretamente, sem exigir Node.js. Use o [pacote portátil](building.md) para executar VSIX no Code OSS.

## Criar um plugin JavaScript/TypeScript

```sh
graphite plugin create my-plugin
cd my-plugin
npm install
npm run build
graphite plugin pack .
```

Para iteração rápida local sem reempacotar a cada mudança, vincule o plugin diretamente em modo dev (a saída do console é gravada em `logs/<id>/console.log` e alterações em `graphite-plugin.json` ou no entrypoint acionam recarregamento automático):

```sh
graphite plugin dev .
# Ao terminar:
graphite plugin unlink local.my-plugin
```

O gerador cria código TypeScript, declarações locais em `graphite.d.ts`, configuração de build e um comando de exemplo. `graphite plugin pack .` grava `local.my-plugin-0.1.0.graphite-plugin` ao lado do projeto. Compile antes de empacotar. Arquivos ocultos e `target` são omitidos; extensões de chaves/bancos e links simbólicos são rejeitados.

O manifesto na raiz (`graphite-plugin.json`) suporta contribuições declarativas e permissões granulares:

```json
{
  "id": "local.my-plugin",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "node",
  "main": "dist/index.js",
  "description": "My Graphite plugin",
  "permissions": ["commands", "documents.read", "ui"],
  "activationEvents": ["onStartupFinished"],
  "commands": [{"id": "local.my-plugin.hello", "title": "Hello from my plugin"}],
  "keybindings": [{"command": "local.my-plugin.hello", "key": "ctrl+alt+h"}],
  "menus": {
    "commandPalette": [{"command": "local.my-plugin.hello"}]
  },
  "activityBar": [{
    "id": "local.my-plugin.tools",
    "title": "My Tools",
    "views": [{"id": "local.my-plugin.items", "title": "Items", "kind": "tree"}]
  }]
}
```

IDs seguem `publisher.name`, com letras ASCII minúsculas, dígitos e hífens. Versões seguem semver. IDs de comandos, contêineres e views começam com o ID completo do plugin e `.`; os registros usam os títulos declarados. Campos declarativos opcionais também incluem `themes`, `snippets` e `languages` (ID de linguagem, extensões e marcadores de comentário).

Os tokens válidos de `permissions` são: `commands`, `documents.read`, `documents.write`, `ui`, `languages`, `storage`, `workspace.read`, `workspace.write`, `terminal`, `chat` e `*`. Capacidades de sistema (`workspace.read`, `workspace.write`, `terminal`, `chat`) sempre exigem declaração explícita no manifesto.

## API v1 (JavaScript / TypeScript)

Exporte `async activate(context, graphite)` e opcionalmente `deactivate()`. O arquivo gerado `graphite.d.ts` define todo o SDK:

- `commands`: `list()`, `execute(id, args?, target?)`, `registerCommand(id, handler)`.
- `documents`: `list()`, `open(uri)`, `save(uri, reason?)`, `snapshot(uri)`, `applyEdits(edits)` (posições UTF-16 baseadas em zero e versionadas), `onDidChange(listener)` e `onWillSave(metadata, participant)`.
- `window`: `showMessage(text)`, `focusView(view)`, `showInputBox(options)`, `showQuickPick(items, options)`, `createStatusBarItem(id?)`, `createOutputChannel(name)`, `registerTreeDataProvider(viewId, provider)` e `createWebviewPanel(viewType, title, options)`.
- `storage`: `global` e `workspace` (`get`, `update`, `delete`).
- `editor`: `active()`, `setSelections(selections)`, `createDecorationType(options)`, `setDecorations(decorationType, uri, ranges)`.
- `languages`: `setDiagnostics(owner, uri, diagnostics)`, `clearDiagnostics(owner, uri)`, `registerCompletionProvider`, `registerHoverProvider`, `registerFormattingProvider`.
- `workspace`: `get()`, `configuration()`, `findFiles(options?)`, `readFile(path)`, `writeFile(path, content)` (todos confinados à raiz do workspace, bloqueando `.git/` e travessias de diretório).
- `terminal`: `create(options?)` retornando `{ id, sendText, show }`.
- `scm`: `status()` retornando o branch Git atual e arquivos alterados.
- `chat`: `registerTool(definition, handler)` registrando ferramentas invocáveis nos turnos do Chat.

Registre recursos em `context.subscriptions`; o descarte e o encerramento do host liberam automaticamente os recursos no editor.

## Criar um plugin nativo compilado em Rust (`runtime: "native"`)

Use a crate `graphite-plugin-sdk` para escrever plugins sem dependência do Node.js:

```rust
use graphite_plugin_sdk::{Plugin, PluginContext, ServiceResult};
use serde_json::json;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn activate(&self, ctx: &PluginContext) -> ServiceResult<()> {
        ctx.register_command("local.myrust.ping", "Ping", |_ctx, _args| {
            Ok(json!({ "pong": true }))
        })?;
        Ok(())
    }
}

fn main() {
    graphite_plugin_sdk::run_plugin(MyPlugin).expect("plugin failed");
}
```

Defina `"runtime": "native"` e `"main": "bin/my-plugin"` em `graphite-plugin.json` (no Windows `.exe` é resolvido automaticamente; no Unix o `graphite` aplica permissão `0o755` na extração e execução). Consulte `crates/graphite-plugin-sdk/examples/echo_plugin.rs` para um exemplo completo expondo comandos, listagem de arquivos e ferramenta de Chat.

## Instalar e gerenciar

Abra a barra lateral secundária pelo botão na barra de título ou com `Ctrl+Alt+B`. O cabeçalho inclui o Chat e uma aba para cada visualização secundária fornecida por extensões VSIX habilitadas, como Codex e Claude Code. Clique em uma aba para trocar. A faixa rola para manter a aba ativa visível. Arraste a borda voltada para o editor para redimensionar a barra. O Graphite salva a largura por perfil e restaura a visualização selecionada quando você reabre a barra na mesma sessão. O Graphite encontra as extensões instaladas pelos manifests locais. As visualizações executáveis exigem o runtime Code OSS do pacote portátil e um workspace confiável.

Use **Instalar de VSIX…** no cabeçalho de Plugins ou no menu **Mais ações de plugins** para abrir o seletor de arquivos do sistema. Ele começa na pasta do workspace e, nas próximas vezes, reabre na pasta do último VSIX selecionado na sessão e no perfil atuais. No Windows, use **Todos os arquivos** no seletor para conferir o nome real do arquivo se um pacote sumir no filtro de VSIX. O Graphite aceita arquivos com extensão `.vsix` em maiúsculas ou minúsculas. Revise o pacote e escolha **Instalar** ou **Cancelar**. **Recarregar plugins** relê a lista de instalados e repete a pesquisa no catálogo atual. Pressione **Tab** para focar as ações do cabeçalho, use **Enter** para acioná-las e as setas para navegar no menu. No terminal, `graphite plugin install <pacote>` instala um pacote local desativado para que você possa ativá-lo depois.

O menu consulta o Open VSX para encontrar novas versões dos VSIX instalados. **Atualizar todas as extensões** prepara revisões individuais. As atualizações automáticas vêm desativadas e são configuradas por perfil. Quando ativadas, o Graphite consulta ao iniciar o perfil e a cada 24 horas enquanto o editor está aberto. Use **Revisar atualizações pendentes** para aprovar ou pular cada pacote preparado. A versão instalada continua em uso até você aprovar a substituição. O menu também ativa ou desativa extensões em massa, lista extensões em execução e inicia uma bisseção. Durante a bisseção, tente reproduzir o problema em cada rodada e indique se ele persiste. O conjunto original de extensões ativas é restaurado ao fim da sessão.

Abra a barra lateral de **Plugins** com `Ctrl+Shift+X`. Ela compartilha a barra lateral esquerda com o Explorador e o Controle de código, e o editor permanece visível. Clique na barra lateral para conceder foco de teclado; clicar no editor o libera. As abas alternam entre Instalados, Open VSX e o catálogo Graphite do GitHub. Resultados Open VSX baixam o VSIX mais recente para inspeção. Clique ou pressione Enter em um repositório Graphite para abrir uma aba própria no editor com o GitHub Flavored Markdown renderizado em um WebView escuro no estilo GitHub, incluindo tabelas, blocos de código, tarefas, badges e imagens HTTPS. Imagens relativas do README são resolvidas a partir do repositório. A coluna direita lista pacotes compatíveis, informações da release, tópicos e links completos do projeto. Scripts, formulários, frames e imagens sem HTTPS são bloqueados; links HTTPS abrem no navegador do sistema. Se houver vários pacotes, escolha qual inspecionar e instalar; a confirmação atual do pacote continua sendo exibida. Repositórios sem pacote compatível em release estável mantêm o README visível e explicam por que a instalação está indisponível. As abas de detalhes podem ser alternadas e fechadas como abas do editor. As mesmas ações estão disponíveis pelo teclado:

| Tecla | Ação |
|---|---|
| I / O / G | Plugins instalados / Open VSX / catálogo Graphite no GitHub |
| L | Digitar caminho local `.graphite-plugin` ou `.vsix` |
| / | Digitar pesquisa no catálogo ativo; Enter envia |
| Enter | Inspecionar plugin instalado, baixar resultado Open VSX ou abrir/instalar o item Graphite selecionado |
| Y / Esc | Confirmar / cancelar instalação ou confiança |
| E / D | Habilitar / desabilitar o plugin selecionado |
| U | Baixar a versão Open VSX mais recente para um VSIX instalado; confirmar para substituir |
| R / X | Reiniciar plugin nativo / remover o plugin selecionado |
| C | Executar o primeiro comando registrado do plugin |

Instalar e atualizar redefinem habilitação e confiança. A confirmação permite executar código local daquela versão instalada. Falhas de ativação ou erros de permissão aparecem na barra lateral e em `logs/<id>/console.log`; use **R** para tentar novamente depois de corrigir a causa.

Selecione um VSIX instalado com ponto de entrada Node.js e pressione **E** para aprovar essa instalação. Confie no workspace, abra a paleta (`Ctrl+Shift+P` ou `F1`) e escolha o comando da extensão. Aprove a nova instalação após uma atualização. Veja as [APIs VSIX suportadas](../architecture/extension-compat.md).

## Publicar e testar

Publique extensões VSIX no Open VSX para descoberta. Para listar um plugin no catálogo Graphite, publique um repositório GitHub público, adicione o tópico `graphite-plugin` e anexe um arquivo `.graphite-plugin` ou `.vsix` à release estável mais recente. Repositórios sem release estável continuam aparecendo na busca, mas não podem ser instalados até que uma release compatível seja publicada. O Graphite também aceita pacotes locais `.vsix` e `.graphite-plugin` pelo painel Plugins.

Execute `cargo test -p graphite-plugins` para testes de pacotes, persistência e catálogo, `cargo test -p graphite-plugin-sdk --all-targets` para testes do SDK Rust e `cargo test -p graphite-app app::plugins` para testes de plugins da aplicação (Node.js 24+ é necessário para fixtures JS e `rustc` para fixtures de binários nativos). Finalize mudanças com `cargo test --workspace` e `cargo build --workspace`; os testes de marketplace usam uma fixture HTTP local e nunca acessam o Open VSX. A execução Linux e verificações interativas devem ser registradas separadamente quando indisponíveis.

Veja os [testes do pacote portátil](building.md) e a [validação da v0.5](../features/v0.5-extensions-automation.md) para as verificações do Code OSS.
