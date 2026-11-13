# Checklist de Testes Manuais no Windows

Verificações que só foram automatizadas ou conferidas no Linux e precisam ser confirmadas manualmente no Windows.
Marque cada item e anote o resultado. Os testes automatizados (`cargo test --workspace`) devem passar antes.

## 0. Build e runtime

- [ ] `cargo build --workspace` funciona.
- [ ] `cargo test --workspace` passa.
- [ ] Gere o pacote portátil (`packaging/code-oss/portable.py`) em um caminho sem espaços; confirme que `code-oss/commit` fica ao lado de `graphite.exe`.
- [ ] Node.js 24 está no `PATH`; o app abre e mouse e teclado respondem.

## 1. Decorações de texto (`createTextEditorDecorationType`)

1. Gere a demo: `python packaging/examples/build_demo.py` (saída: `target/examples/graphite-demo.decoration-demo-0.1.0.vsix`).
2. Abra uma pasta com um arquivo de texto contendo uma linha com `TODO`, uma com `error` e uma última linha.
3. Barra de plugins (`Ctrl+Shift+X`): `L`, informe o caminho do `.vsix`, `Y`; selecione e pressione `E`.
4. Confie no workspace e execute **Graphite Demo: Decorate editor** na paleta.

Esperado:
- [ ] Primeira linha: fundo azul translúcido na largura inteira.
- [ ] Cada `TODO`: fundo amarelo com borda laranja só em volta da palavra.
- [ ] Cada linha com `error`: fundo avermelhado e `<- Graphite decoration` em vermelho depois do texto.
- [ ] Última linha: `(note added by the extension)` em verde depois do conteúdo.
- [ ] Mensagem "Decorated N TODO and M error line(s)".
- [ ] Colunas corretas com tabs, acentos e emoji na linha.
- [ ] A seleção é desenhada sobre o fundo das decorações; digitar e rolar funcionam normalmente.
- [ ] **Graphite Demo: Clear decorations** remove todas as marcas.

Limites conhecidos: não desenha com quebra de linha; os intervalos não acompanham edições até a extensão reaplicar; texto `before`, fontes e ícones de margem são ignorados.

## 2. API de terminal

- [ ] Uma extensão chamando `window.createTerminal` abre uma aba de terminal; `sendText` executa o comando (PowerShell/cmd).
- [ ] Fechar o terminal avisa a extensão; um terminal cujo processo termina informa a saída.

## 3. Observadores de arquivos e SCM

- [ ] Criar, alterar e apagar arquivos no workspace (pelo Explorer ou outro programa) chega aos listeners de `createFileSystemWatcher`.
- [ ] O app não trava nem consome CPU enquanto o Git roda (o watcher ignora o interior de `.git` e `target/`).
- [ ] O status do Git na barra lateral atualiza após commits e troca de branch.

## 4. Provedores de linguagem

- [ ] Ir para Definição / Declaração / Definição de Tipo / Implementação / Referências a partir de um provedor VSIX (menu e atalhos).
- [ ] Símbolos do documento (Ir para Símbolo), Quick Fix, Renomear e Ajuda de Assinatura funcionam com provedor VSIX quando não há LSP configurado.
- [ ] Caminhos do Windows (`C:\...`) e URIs `file:///C:/...` são convertidos corretamente nos resultados.

## 4b. Provedores de linguagem de extensões (formatação de intervalo, inlay hints, destaques)

- [ ] Uma extensão com formatador de intervalo: **Formatar seleção** formata apenas a seleção, ou a linha do cursor quando nada está selecionado.
- [ ] Uma extensão com provedor de inlay hints (em uma linguagem que nenhum servidor de linguagem atende) mostra dicas após o texto; elas se atualizam ao digitar e não piscam.
- [ ] Uma extensão com provedor de destaques de documento: parar o cursor sobre um identificador destaca as outras ocorrências; mover o cursor as remove na hora.
- [ ] A CPU fica ociosa enquanto o cursor e o documento não mudam.

## 5. Workbench

- [ ] Itens de status de extensões aparecem e o comando do clique executa.
- [ ] `env.clipboard` lê e escreve.
- [ ] Find in Files (`findTextInFiles`) devolve resultados a uma extensão.
- [ ] `<profile>/code-oss/unsupported-api.log` só é criado quando uma extensão chama uma API não suportada.

## 5b. Contribuições declarativas

- [ ] Snippets de um VSIX instalado aparecem no autocompletar; desativar a extensão os remove.
- [ ] Temas de cor de um VSIX aparecem na lista e se aplicam corretamente.

## 5c. Depuração com extensões

- [ ] Um `launch.json` com apenas `"type"` (por exemplo de uma extensão de depurador instalada que traz um `program`) inicia o adaptador; o painel mostra frames, variáveis e watches em um breakpoint.
- [ ] Uma extensão que registra um `DebugAdapterDescriptorFactory` (executável ou `DebugAdapterServer`): F5 inicia a sessão depois que o extension host sobe; a barra de status mostra "Waiting for ..." nesse meio tempo.
- [ ] `Shift+F5` enquanto espera a extensão abandona o lançamento e nada inicia depois.
- [ ] Uma fábrica que lança erro, ou devolve uma implementação inline, mostra uma mensagem legível na barra de status.
- [ ] `preLaunchTask` roda antes da sessão e uma tarefa que falha a aborta; `postDebugTask` roda uma vez depois que a sessão termina.
- [ ] Paleta de comandos: **Select Debug Configuration** e **Start Debugging: NOME** listam todas as configurações do `launch.json`.
- [ ] Com o runtime Code OSS real, execute `cargo test -p graphite-app -- --ignored official_host_creates_a_debug_adapter_from_an_extension_factory` com `GRAPHITE_TEST_CODE_OSS` definido.

## 6. Relatório

Anote falhas com: passo, resultado esperado, resultado obtido e o conteúdo de `unsupported-api.log`, se existir.
