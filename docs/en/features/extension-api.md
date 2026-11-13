# Extension API surface

VSIX extensions run in the official Code OSS 1.145.2 extension host, in a separate Node process started by `packaging/code-oss/bridge.mjs`. Graphite answers the host's calls to the main thread, so every workbench effect goes through Graphite's own checks (workspace trust, read-only files, and the workspace boundary).

## Supported

| Area | What works |
|---|---|
| Commands | Register, execute, and list commands; Graphite commands can be run by extensions. |
| Configuration | Read values, update and remove workspace or user settings, listen for changes. |
| Storage | Extension global and workspace storage. |
| Prompts | `showInputBox`, `showQuickPick`, and `showInformationMessage` with action buttons. |
| Edits | `TextEditor.edit`, `workspace.applyEdit`, and documents opened or shown in the editor. |
| Language providers | Hover, completion, document formatting, diagnostics, definition, declaration, type definition, implementation, references, document symbols, code actions, rename, and signature help. Also range formatting (**Format Selection**, the cursor line when nothing is selected), inlay hints, document highlights and folding ranges; the last three are requested automatically (see below). Selection ranges drive **Expand Selection** in place of the syntax-tree expansion when an extension provides them. On-type formatting runs while typing when `editor.formatOnType` is on. Call and type hierarchies (**Show Call Hierarchy** / **Show Type Hierarchy**) list callers and callees, or super and subtypes, in the locations picker. Document links are underlined and followed with Ctrl+click. |
| Links | `window.openUri` opens `https://` links in the default browser; other schemes are refused. `env.asExternalUri` returns the link unchanged. |
| Output | `window.createOutputChannel` writes to a log file. Showing a channel opens its log in a read-only view that refreshes while the extension writes. |
| Status bar | `window.createStatusBarItem` items appear at the end of the status bar with their text. Clicking an item runs its command: an extension command if registered, otherwise a Graphite command. |
| Files | `workspace.findFiles` with include and exclude globs and a result limit, honoring ignore files. `workspace.findTextInFiles` (a proposed API: the extension must list `findTextInFiles` in `enabledApiProposals`) searches the workspace with the project search engine, with regex, case, whole-word and glob options, and stops after 5 seconds. |
| Progress | `withProgress` titles and messages appear in the status bar. |
| Terminal | `window.createTerminal` (with `name`, `shellPath`, `shellArgs`, `cwd`) opens a Graphite terminal tab. `sendText`, `show`, `hide`, `dispose` and `onDidCloseTerminal` with the exit code work. |
| File watchers | `workspace.createFileSystemWatcher` receives create, change and delete events for the whole workspace (except inside `node_modules`) and for folders the extension registers outside it. |
| Clipboard | `env.clipboard.readText` and `writeText`. |
| `workspace.fs` | Answered by the extension host directly on the local disk; it does not go through Graphite. |
| Text decorations | `window.createTextEditorDecorationType` and `TextEditor.setDecorations` draw background colors, borders (`border`, `outline`), whole-line marks and `after` text at the end of a line, with `dark` and `light` variants. Colors can be hex, `rgb()`/`rgba()` or common theme colors. Fonts, gutter icons, overview ruler marks, `before` text and hover messages are ignored, and decorations are not drawn in word-wrap mode yet. Ranges are not adjusted while the document is edited; the extension normally sets them again. |
| Debug adapters | An extension that registers a `DebugAdapterDescriptorFactory` can supply the adapter for its debug type: executable and local TCP server descriptors work. Configuration providers, named pipes and inline implementations are not supported yet. See [Debug panel](debug-panel.md). |
| Webview | One webview panel, HTML content, messages both ways. |

## Declarative contributions

A VSIX without a Node.js entry point (themes, snippets, language packs) runs no code, so it can be enabled without starting the extension host. Applied contributions:

| Contribution | What happens |
|---|---|
| `commands` | Declared commands appear in the palette before the extension activates. |
| `languages` configuration | Line and block comment markers feed Toggle Line/Block Comment. |
| `viewsContainers.activitybar` | Containers appear in the activity bar. |
| `snippets` | Snippet files are loaded for enabled extensions, scoped to the contributing language. A user or project snippet with the same prefix and scope overrides them. |

| `themes` | Color themes are imported into the profile and selected with `workbench.colorTheme`, by label or id. Disabling the extension removes them. |

Keybindings, grammars and icon themes are listed in the package but not applied yet.

## Not yet

- Language providers beyond the list above (semantic tokens, code lenses and colors). Code actions that only run an extension command, and file operations inside a rename, are not applied.
- Extension-owned pseudoterminals, terminal data events, tasks, debugging, source control, and virtual file systems (`registerFileSystemProvider`).
- Several webview panels, `asWebviewUri`, and `localResourceRoots`.
- Custom editors and notebook kernels.

Requests outside this list get the error `Graphite does not support <actor>.<method>`, and the extension sees that error. Registrations of capabilities that are not implemented (`$register*`, `$unregister*`, `$emit*`) are accepted without an error so they do not clutter the status bar. Every unsupported call is recorded once per session in `<profile>/code-oss/unsupported-api.log`, which shows what an extension needs that Graphite does not provide yet.

## Features requested automatically

Most providers answer a command (hover, go to definition, format). Four are asked for without one:

- **Inlay hints** (`registerInlayHintsProvider`): for the active document, whenever its text changes. They are shown by the same inline-hint display that language servers use, and only when no language server serves that file, so the two never compete. A provider that fails is not asked again until the document changes.
- **Document highlights** (`registerDocumentHighlightProvider`): for the symbol at the cursor, once the cursor has rested for 150 ms. Occurrences are drawn with the editor's word-highlight color. Highlights are removed as soon as the cursor moves, and an answer for a position the cursor has left is dropped.
- **Folding ranges** (`registerFoldingRangeProvider`): for the active document, whenever its text changes. While a provider's regions describe the current text, they replace the indentation rules for every fold command and for the fold display; until the first answer, or after an edit, indentation folding applies. Regions that do not span two lines are dropped, and a failing provider is not asked again until the document changes.
- **Document links** (`registerDocumentLinkProvider`): for the active document, whenever its text changes. Links are underlined; Ctrl+click on one opens `https:` links in the browser and `file:` links in the editor. Links the provider leaves unresolved (no URL yet) are reported but not followed, and the underlines disappear as soon as the text is edited, until the extension answers again.

Both are requested only in a trusted workspace, for the active document, and answers for an outdated document revision are discarded. Each request has a 10-second limit. These paths are covered by unit tests that play the extension host's side of the protocol; they have not been run against third-party extensions.

### On-type formatting

With `"editor.formatOnType": true` (off by default, like VS Code), typing one of the trigger characters an extension registered with `registerOnTypeFormattingEditProvider` asks it to format around the cursor; the edits are applied as one undoable step. Only single typed characters count (not pasted text), only with one cursor, and only if the document did not change meanwhile. Typing Enter does not trigger it yet.
