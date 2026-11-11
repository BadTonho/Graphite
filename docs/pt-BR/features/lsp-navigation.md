# Navegação pelo servidor de linguagem

Estes recursos enviam requisições ao servidor de linguagem configurado para o arquivo ativo (veja `lsp.json`). Os resultados dependem do que o servidor suporta.

| Ação | Comando | Atalho padrão |
|---|---|---|
| Renomear símbolo | `editor.action.rename` | `F2` |
| Ir para referências | `editor.action.goToReferences` | `Shift + F12` |
| Ir para implementações | `editor.action.goToImplementation` | `Ctrl + F12` |
| Espiar definição | `editor.action.peekDefinition` | `Alt + F12` |
| Disparar ajuda de parâmetros | `editor.action.triggerParameterHints` | `Ctrl + Shift + Espaço` |
| Mostrar hierarquia de chamadas | `graphite.lsp.showCallHierarchy` | `Shift + Alt + H` |
| Mostrar hierarquia de tipos | `graphite.lsp.showTypeHierarchy` | nenhum (`Ctrl + Shift + H` é Substituir em Arquivos) |

## Comportamento

- **Renomear** abre um campo preenchido com o identificador sob o cursor. Enter envia a renomeação e Esc cancela. A edição do workspace retornada pelo servidor é aplicada em todos os arquivos. O Graphite não pede antes ao servidor o intervalo de renomeação (prepare-rename).
- **Referências e implementações** pulam direto quando há um único resultado. Com vários, abrem o seletor com `arquivo:linha`. Ao escolher, o cursor vai para o início da linha.
- **Espiar definição** mostra as primeiras linhas da definição no balão de hover, sem sair do arquivo.
- **Ajuda de parâmetros** mostra a assinatura ativa no balão de hover. O destaque do parâmetro ainda não é desenhado.
- **Hierarquia de chamadas** lista os chamadores (`←`) e os chamados (`→`) do símbolo sob o cursor em um único seletor. **Hierarquia de tipos** lista supertipos (`↑`) e subtipos (`↓`).
