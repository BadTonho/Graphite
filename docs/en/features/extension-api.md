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
| Language providers | Hover, completion, document formatting, and diagnostics. |
| Links | `window.openUri` opens `https://` links in the default browser; other schemes are refused. `env.asExternalUri` returns the link unchanged. |
| Output | `window.createOutputChannel` writes to a log file. Showing a channel opens its log in a read-only view that refreshes while the extension writes. |
| Status bar | `window.createStatusBarItem` items appear at the end of the status bar with their text. Clicks do not run commands yet. |
| Files | `workspace.findFiles` with include and exclude globs and a result limit, honoring ignore files. |
| Progress | `withProgress` titles and messages appear in the status bar. |
| Webview | One webview panel, HTML content, messages both ways. |

## Not yet

- Language providers for definition, references, document symbols, code actions, and signature help. Completion, hover, formatting, and diagnostics are the only ones wired.
- Terminal API, tasks, debugging, source control, and virtual file systems (`registerFileSystemProvider`).
- `workspace.findTextInFiles`.
- Several webview panels, `asWebviewUri`, and `localResourceRoots`.
- Status bar item commands.
- Custom editors and notebook kernels.

Requests outside this list get the error `Graphite does not support <actor>.<method>`, and the extension sees that error.
