# Formatação e refatoração pelo servidor de linguagem

Estes recursos pedem ao servidor de linguagem configurado para o arquivo ativo (veja `lsp.json`) que formate o código ou liste refatorações. Os resultados dependem do que o servidor suporta.

| Ação | Comando | Atalho padrão |
|---|---|---|
| Formatar documento | `editor.action.formatDocument` | nenhum |
| Formatar seleção | `editor.action.formatSelection` | `Ctrl + K Ctrl + F` |
| Refatorar… | `editor.action.refactor` | `Ctrl + Shift + R` |

## Configurações

Defina estas opções em `settings.json` (do usuário ou em `.vscode/settings.json` do workspace):

| Configuração | Padrão | Efeito |
|---|---|---|
| `editor.formatOnSave` | `false` | Formata o arquivo com o servidor de linguagem antes de gravá-lo. |
| `editor.formatOnPaste` | `false` | Formata o texto colado após uma colagem com um único cursor. |

A formatação usa os valores de `editor.tabSize` e `editor.insertSpaces`.

## Comportamento

- **Formatar documento** tenta primeiro o formatador VSIX ativo. Se nenhum corresponder ao arquivo, pede ao servidor de linguagem (`textDocument/formatting`).
- **Formatar seleção** envia o intervalo selecionado. Sem seleção, formata a linha do cursor.
- **Formatar ao salvar** roda depois das edições `willSaveWaitUntil` do servidor e antes de o arquivo ser gravado. Espera no máximo um segundo. Se um participante anterior do salvamento alterou o texto, a formatação é ignorada nesse salvamento e um aviso é exibido, pois o servidor não conhece essas edições.
- **Formatar ao colar** formata o intervalo do texto colado. Colagens com vários cursores não são formatadas.
- **Resultados obsoletos são descartados.** Se o documento mudar enquanto o servidor trabalha, as edições de formatação não são aplicadas e a barra de status informa isso.
- **Refatorar…** lista as ações de código `refactor` do servidor (por exemplo extrair, inline ou reescrever) para a seleção, ou para a linha do cursor, no popup de correção rápida. Use Cima/Baixo para escolher e Enter para aplicar.
- **Importação automática** vem do servidor como correção rápida (`Ctrl + .`) sobre o nome não resolvido, usando o mesmo popup e o mesmo caminho de edição de workspace das demais correções rápidas.

## Ainda não disponível

- Formatar ao digitar (`textDocument/onTypeFormatting`).
- Tokens semânticos (`textDocument/semanticTokens`).
- Gramáticas TextMate. O realce usa apenas tree-sitter.
