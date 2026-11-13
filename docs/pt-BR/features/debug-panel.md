# Painel de depuração

Com uma sessão de depuração ativa, o painel inferior mostra três colunas. As linhas são montadas a partir do runtime de depuração por funções puras (`app/debug_panel.rs`), então o desenho, os cliques e a rolagem compartilham a mesma descrição do que está na tela.

## Colunas

| Coluna | Conteúdo | Interação |
| --- | --- | --- |
| Pilha de chamadas | Frames da thread pausada, com o frame selecionado marcado por `▶`. Abaixo, **Exception breakpoints**: os filtros anunciados pelo adaptador. | Clique em um frame para selecioná-lo; escopos, variáveis e watches são recarregados. Clique em um filtro para ligá-lo ou desligá-lo. |
| Variáveis | Escopos (o primeiro não caro, normalmente *Locals*, abre sozinho) e suas variáveis como árvore expansível. Abaixo, **Watch**. | Clique em uma linha com `▸`/`▾` para expandir ou recolher; os filhos são pedidos uma vez e guardados. Clique no `×` de um watch para removê-lo. |
| Console de depuração | Saída do adaptador (`stderr` na cor de erro) e resultados das expressões avaliadas. | Rola a partir da linha mais recente. |

Cada coluna rola de forma independente com a roda do mouse.

## Entrada do console

A linha de entrada no fim do painel (clique nela para focar):

- `Enter` avalia a expressão no frame selecionado com o contexto `repl`. A entrada aparece como `> expressão` e o resultado, ou o erro do adaptador, é acrescentado ao console.
- `Ctrl+Enter` adiciona a expressão como watch. Os watches são avaliados no contexto `watch` e reavaliados sempre que o programa para ou você seleciona outro frame. Um watch que falha mostra o erro na própria linha, não no console.
- `Esc` sai da entrada.

## Exception breakpoints

A capacidade `exceptionBreakpointFilters` do adaptador preenche a lista, usando o `default` de cada filtro como estado inicial. O conjunto ativo é enviado com `setExceptionBreakpoints` antes de `configurationDone` e de novo a cada alternância. As escolhas são mantidas quando uma nova sessão anuncia os mesmos ids de filtro. Adaptadores que não anunciam filtros não recebem a requisição.

## Passos

`F10`, `F11` e `Shift+F11` enviam `next`, `stepIn` e `stepOut` com o `threadId` da thread pausada, como o protocolo exige. Retomar a execução limpa a pilha e as variáveis antigas, para o painel nunca mostrar valores de uma parada anterior.

## De onde vem o adaptador de depuração

Um `launch.json` padrão do VS Code cita só o `type` de depuração (`"node"`, `"python"`, …). O Graphite procura o adaptador na extensão instalada e **habilitada** cujo `contributes.debuggers` declara esse `type`:

- `program` é um caminho dentro da extensão. Com um `runtime` (por exemplo `node`), o runtime o executa: `runtime runtimeArgs… program args…`. `node` usa o runtime que acompanha o Graphite quando existe, senão o `node` do `PATH`. Outros runtimes precisam estar no `PATH`. Sem `runtime`, o programa é executado diretamente.
- As entradas `windows`, `linux` e `osx` substituem os campos compartilhados um a um.
- Caminhos de `program` que saem da pasta da extensão são ignorados.
- Um `"adapter": { "command": …, "args": […] }` na configuração continua tendo prioridade, para adaptadores que não são instalados como extensões.
- Os erros indicam a causa: nenhuma extensão fornece o tipo, a extensão que o fornece está desabilitada, ou o programa do adaptador não existe.
- Ainda não suportado: extensões que criam o adaptador em código (`registerDebugAdapterDescriptorFactory`) e não declaram `program`.

Extensões instaladas antes desta funcionalidade são relidas do `package.json` quando o registro é carregado.

### Variáveis

Toda string da configuração (incluindo `program`, `args`, `cwd` e `env`) expande `${workspaceFolder}`, `${workspaceFolderBasename}`, `${cwd}`, `${userHome}`, `${pathSeparator}` (e `${/}`), `${env:NOME}` (vazio se não definida) e, a partir do arquivo aberto quando a sessão começa, `${file}`, `${fileBasename}`, `${fileBasenameNoExtension}`, `${fileExtname}`, `${fileDirname}`, `${relativeFile}` e `${relativeFileDirname}`. Variáveis desconhecidas, `${input:…}`, `${command:…}` e variáveis de arquivo sem arquivo aberto ficam como estão escritas, para o problema ficar visível. Um `program` ou `cwd` relativo é resolvido contra o workspace depois da expansão; `cwd` assume o workspace por padrão.

### Escolher a configuração

`F5` e **Executar → Iniciar depuração** iniciam a configuração *selecionada*, ou a primeira do `launch.json` quando nenhuma foi selecionada (ou a selecionada não existe mais no arquivo). A paleta de comandos lista, para cada nome de configuração distinto:

- **Select Debug Configuration: NOME** — torna essa a configuração que o `F5` inicia, sem iniciá-la.
- **Start Debugging: NOME** — seleciona e inicia (exige workspace confiável). É recusado enquanto outra sessão roda ou espera o `preLaunchTask`, para que a escolha nunca seja ignorada em silêncio.

A seleção vale durante a sessão da janela; não é salva entre execuções. Nomes duplicados aparecem uma vez e usam a primeira ocorrência. Configurações compostas (`compounds`) não são suportadas.

**Iniciar depuração** no menu Executar não exige mais um `"adapter"` explícito: qualquer configuração do `launch.json` o habilita.

## Tarefas ao redor da sessão

Uma configuração de lançamento pode citar tarefas de `.vscode/tasks.json`:

```json
{ "name": "Debug app", "type": "example", "request": "launch",
  "preLaunchTask": "build", "postDebugTask": "clean" }
```

- `preLaunchTask` roda primeiro (o workspace precisa ser confiável, como em qualquer tarefa). A sessão só começa se a tarefa terminar com código 0. Um código diferente de zero, uma falha ou um cancelamento abortam o lançamento, com uma mensagem na barra de status que cita a tarefa. Um rótulo que não existe em `tasks.json` é informado antes de qualquer execução.
- `Shift+F5` enquanto a tarefa ainda roda cancela a tarefa e abandona o lançamento.
- `postDebugTask` roda uma vez quando a sessão termina: parada, fim do programa ou fechamento do adaptador. Não roda quando o próprio Graphite é fechado.
- Os dois campos são configurações do Graphite e não são repassados ao adaptador de depuração.
- Ainda não coberto: os problem matchers não influenciam se o lançamento continua, e não há o aviso "Depurar mesmo assim" após uma tarefa que falhou.

## Limites

- Valores inline ao lado do código não estão implementados.
- Os valores das variáveis aparecem em uma linha e são cortados pela largura da coluna; não há edição de valor nem hover.
- Verificado por testes unitários do tratamento do protocolo, do modelo de linhas e da detecção de cliques. O painel não foi conferido visualmente em uma janela em execução.
