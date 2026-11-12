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
| Language providers | Hover, completion, document formatting, diagnostics, definition, declaration, type definition, implementation, references, document symbols, code actions, rename, and signature help. |
| Links | `window.openUri` opens `https://` links in the default browser; other schemes are refused. `env.asExternalUri` returns the link unchanged. |
| Output | `window.createOutputChannel` writes to a log file. Showing a channel opens its log in a read-only view that refreshes while the extension writes. |
| Status bar | `window.createStatusBarItem` items appear at the end of the status bar with their text. Clicking an item runs its command: an extension command if registered, otherwise a Graphite command. |
| Files | `workspace.findFiles` with include and exclude globs and a result limit, honoring ignore files. `workspace.findTextInFiles` (a proposed API: the extension must list `findTextInFiles` in `enabledApiProposals`) searches the workspace with the project search engine, with regex, case, whole-word and glob options, and stops after 5 seconds. |
| Progress | `withProgress` titles and messages appear in the status bar. |
| Terminal | `window.createTerminal` (with `name`, `shellPath`, `shellArgs`, `cwd`) opens a Graphite terminal tab. `sendText`, `show`, `hide`, `dispose` and `onDidCloseTerminal` with the exit code work. |
| Webview | One webview panel, HTML content, messages both ways. |

## Declarative contributions

A VSIX without a Node.js entry point (themes, snippets, language packs) runs no code, so it can be enabled without starting the extension host. Applied contributions:

| Contribution | What happens |
|---|---|
| `commands` | Declared commands appear in the palette before the extension activates. |
| `languages` configuration | Line and block comment markers feed Toggle Line/Block Comment. |
| `viewsContainers.activitybar` | Containers appear in the activity bar. |
| `snippets` | Snippet files are loaded for enabled extensions, scoped to the contributing language. A user or project snippet with the same prefix and scope overrides them. |

Themes, keybindings, grammars and icon themes are listed in the package but not applied yet.

## Not yet

- Language providers beyond the list above (range and on-type formatting, document highlights, semantic tokens, code lenses, folding, selection ranges, call and type hierarchies, inlay hints, links and colors). Code actions that only run an extension command, and file operations inside a rename, are not applied.
- Extension-owned pseudoterminals, terminal data events, tasks, debugging, source control, and virtual file systems (`registerFileSystemProvider`).
- Several webview panels, `asWebviewUri`, and `localResourceRoots`.
- Custom editors and notebook kernels.

Requests outside this list get the error `Graphite does not support <actor>.<method>`, and the extension sees that error. Registrations of capabilities that are not implemented (`$register*`, `$unregister*`, `$emit*`) are accepted without an error so they do not clutter the status bar. Every unsupported call is recorded once per session in `<profile>/code-oss/unsupported-api.log`, which shows what an extension needs that Graphite does not provide yet.
