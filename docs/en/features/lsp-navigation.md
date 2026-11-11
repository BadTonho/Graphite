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
