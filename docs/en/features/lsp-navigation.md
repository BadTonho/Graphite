# Language server navigation

These features send requests to the language server configured for the active file (see `lsp.json`). Results depend on what the server supports.

| Action | Command | Default shortcut |
|---|---|---|
| Rename symbol | `editor.action.rename` | `F2` |
| Go to references | `editor.action.goToReferences` | `Shift + F12` |
| Go to implementations | `editor.action.goToImplementation` | `Ctrl + F12` |
| Peek definition | `editor.action.peekDefinition` | `Alt + F12` |
| Trigger parameter hints | `editor.action.triggerParameterHints` | `Ctrl + Shift + Space` |
| Show call hierarchy | `graphite.lsp.showCallHierarchy` | `Shift + Alt + H` |
| Show type hierarchy | `graphite.lsp.showTypeHierarchy` | none (`Ctrl + Shift + H` is Replace in Files) |

## Behavior

- **Rename** opens a prompt pre-filled with the identifier under the caret. Enter sends the rename, Escape cancels. The server's workspace edit is applied across files. Graphite does not ask the server for a prepare-rename range first.
- **References and implementations** jump directly when there is one result. With several results, they open the picker listing `file:line`. The picker jumps to the start of the line.
- **Peek definition** shows the first lines of the definition in the hover popup, without leaving the file.
- **Parameter hints** show the active signature in the hover popup. Parameter highlighting is not drawn yet.
- **Call hierarchy** lists callers (`←`) and callees (`→`) of the symbol under the caret in one picker. **Type hierarchy** lists supertypes (`↑`) and subtypes (`↓`).

## Semantic highlighting

When a language server advertises `semanticTokensProvider`, Graphite asks it for the full-document tokens (`textDocument/semanticTokens/full`) each time the document is synchronized and draws them over the tree-sitter highlighting, with the same display used for extension providers (see [Extension API](extension-api.md)). Token types map to the nearest syntax class (types, functions, variables, properties, keywords, comments, strings, numbers, operators); other types keep the syntax color and modifiers are ignored. Tokens describe one revision of the text, so after an edit the syntax colors show until the server answers again. Servers that do not advertise the capability are never asked, and delta and range requests are not used. Tokens from an extension provider take priority over the server's for the same document.
