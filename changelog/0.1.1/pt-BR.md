## Novas funcionalidades

- Revise e recupere rascunhos locais de arquivos de texto não salvos e abas sem título após um encerramento inesperado.

## Correções

- Comandos `graphite plugin` que falham agora gravam uma única entrada no log de erros de severidade alta.
- Ícones SVG, bitmap e codicon contribuídos por plugins agora são renderizados na Barra de Atividades e na Barra Lateral Secundária em vez de exibir apenas a primeira letra do título do container.
- `vscode.window.tabGroups` (`ExtHostEditorTabs`) agora é inicializado e sincronizado no extension host VSIX para evitar falhas ao acessar `activeTabGroup` na ativação, a capacidade da fila do extension host foi ampliada para ativações concorrentes, e erros de ativação/execução de extensões passaram a ser gravados no log de erros de severidade média.
- Webviews de extensão agora são servidas em uma origem local `127.0.0.1` com `POST /__graphite_ipc` (evitando colisões com `window.ipc` somente leitura e bloqueios de `frame-ancestors` para iframes locais aninhados), e atualizações de recursos/intervalos de `MainThreadComments` são aceitas sem exibir erro.
- O Explorador de arquivos agora rola com a roda do mouse quando o ponteiro está sobre a barra lateral primária e exibe uma barra de rolagem quando os itens ultrapassam a área visível.
- Adicionado espaçamento horizontal entre a margem de números de linha do editor e o início do texto.
