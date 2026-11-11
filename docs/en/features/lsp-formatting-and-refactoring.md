# Language server formatting and refactoring

These features ask the language server configured for the active file (see `lsp.json`) to format code or list refactorings. Results depend on what the server supports.

| Action | Command | Default shortcut |
|---|---|---|
| Format document | `editor.action.formatDocument` | none |
| Format selection | `editor.action.formatSelection` | `Ctrl + K Ctrl + F` |
| Refactor… | `editor.action.refactor` | `Ctrl + Shift + R` |

## Settings

Set these in `settings.json` (user or workspace `.vscode/settings.json`):

| Setting | Default | Effect |
|---|---|---|
| `editor.formatOnSave` | `false` | Formats the file with the language server before it is written. |
| `editor.formatOnPaste` | `false` | Formats the pasted text after a paste with a single cursor. |

Formatting uses the editor's `editor.tabSize` and `editor.insertSpaces` values.

## Behavior

- **Format document** tries the active VSIX formatter first. If none matches the file, it asks the language server (`textDocument/formatting`).
- **Format selection** sends the selected range. With no selection, it formats the line under the caret.
- **Format on save** runs after the server's `willSaveWaitUntil` edits and before the file is written. It waits at most one second. If an earlier save participant changed the text, formatting is skipped for that save and a warning is shown, because the server does not know those edits.
- **Format on paste** formats the range of the pasted text. Pastes with several cursors are not formatted.
- **Stale results are dropped.** If the document changes while the server is working, the formatting edits are not applied and the status bar says so.
- **Refactor…** lists the server's `refactor` code actions (for example extract, inline, or rewrite) for the selection, or the caret's line, in the quick-fix popup. Use Up/Down to choose and Enter to apply.
- **Auto-import** comes from the server as a quick fix (`Ctrl + .`) on the unresolved name, using the same popup and workspace-edit path as other quick fixes.

## Not yet available

- Format on type (`textDocument/onTypeFormatting`).
- Semantic tokens (`textDocument/semanticTokens`).
- TextMate grammars. Highlighting uses tree-sitter only.
