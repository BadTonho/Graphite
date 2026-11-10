# Criando e instalando plugins

Compile o Graphite com `cargo build --workspace` para desenvolver plugins nativos. Plugins nativos usam Node.js 24+ e aceitam um executável personalizado por `graphite.plugins.nodePath` nas configurações do usuário. Use o [pacote portátil](building.md) para executar VSIX no Code OSS.

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

A barra lateral mostra os IDs declarados; associe outros comandos no `keybindings.json`, por exemplo `{"key":"ctrl+alt+h","command":"local.my-plugin.hello"}`. Instalar e atualizar redefine habilitação e confiança. A confirmação permite executar código local daquela versão instalada. Node ausente e falhas de ativação aparecem na barra lateral; use R para tentar novamente depois de corrigir a causa.

Selecione um VSIX instalado com ponto de entrada Node.js e pressione **E** para aprovar essa instalação. Confie no workspace, abra a paleta (`Ctrl+Shift+P` ou `F1`) e escolha o comando da extensão. Aprove a nova instalação após uma atualização. Veja as [APIs VSIX suportadas](../architecture/extension-compat.md).

## Publicar e testar

Publique extensões VSIX no Open VSX para descoberta. Para listar um plugin no catálogo Graphite, publique um repositório GitHub público, adicione o tópico `graphite-plugin` e anexe um arquivo `.graphite-plugin` ou `.vsix` à release estável mais recente. Quando houver ambos os formatos ou mais de um pacote, a pessoa escolhe qual inspecionar. Repositórios sem release estável continuam aparecendo na busca, mas não podem ser instalados até que uma release compatível seja publicada. O Graphite também aceita pacotes locais `.vsix` e `.graphite-plugin` pelo painel Plugins.

Execute `cargo test -p graphite-plugins` para testes de pacotes, persistência e catálogo. Os testes da aplicação para plugins e Emmet upstream exigem Node.js 24+ e usam fixtures locais: `cargo test -p graphite-app app::plugins`. Finalize mudanças com `cargo test --workspace` e `cargo build --workspace`; os testes de marketplace usam uma fixture HTTP local e nunca acessam o Open VSX. A execução Linux e verificações interativas devem ser registradas separadamente quando indisponíveis.

Veja os [testes do pacote portátil](building.md) e a [validação da v0.5](../features/v0.5-extensions-automation.md) para as verificações do Code OSS.
