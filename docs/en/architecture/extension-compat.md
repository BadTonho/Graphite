# VS Code extension compatibility

Graphite launches the official Code OSS **1.145.2** extension host (commit `c4453f8ea430795f941c1b2ff6c9321135a74a02`) in a separate Node.js 24.22.1 process. The portable Windows and Linux x86-64 packages carry the runtime and license notices. The regular Cargo build supports VSIX inspection and installation without provisioning the runtime.

## Runtime and authorization

The Graphite process exchanges bounded messages over private child-process pipes. The Node adapter uses upstream `PersistentProtocol` and `RPCProtocol` over a per-session local socket or named pipe. It performs the upstream initialization handshake and marshals calls. Rust owns the workbench operations, registry, document versions, diagnostics, prompts and lifecycle.

The Plugins sidebar installs VSIX packages disabled. The approved installation, trusted workspace and active profile determine which VSIX packages are visible to the host. Declared commands appear in the palette before activation, and startup or lifecycle events (`*`, `onStartupFinished`, `workspaceContains:`, `onLanguage:`, `onView:`) as well as the first eligible palette command or keybinding activate the extension host. Registered commands, providers, tree views, webviews, tasks, LM tools, diagnostics and outstanding requests are scoped to the session. Revoking trust, switching workspace or profile, or changing enabled installations ends that session. The process runs with the user's permissions, so approve only code you trust.

## Supported VSIX API and contribution matrix

| Namespace / Contribution | Supported workflow |
|---|---|
| `commands` & `contributes.commands` | Activate declared commands on demand (`Ctrl+Shift+P` / `F1`), register/unregister handlers, execute registered extension commands, and dispatch Graphite commands via `$executeCommand`. |
| `activationEvents` | Fire `*` and `onStartupFinished` when ready, evaluate `workspaceContains:` checks inside the workspace root, and dispatch `onLanguage:` / `onView:` activations. |
| `contributes.menus` & `contributes.keybindings` | Project VSIX `commandPalette`, `editor/context` and `view/item/context` menu entries into Graphite's plugin menu surfaces, and bind `contributes.keybindings` chords into the active keymap when approved and trusted. |
| `workspace` | Read workspace folders and documents, observe open/change/save/close events, receive file watcher events, search workspace files/text, apply single-editor and workspace bulk edits with undo, persist scoped extension storage (`MainThreadStorage`), and update profile/workspace configuration. |
| `window` & `env` | Read active editor state and selections, open/show text documents, apply editor decorations (`MainThreadEditorTabsAndDecorations`), display messages, progress, input boxes and quick picks (Esc cancels), manage output channels and status bar items, read/write clipboard text (`MainThreadClipboard`), and validate external `https://` URIs via `MainThreadWindow.$openUri`. |
| `languages` | Register completion, hover, document formatting, on-type formatting, range formatting, definition, declaration, type definition, implementation, reference, document symbol, workspace symbol, code action, code lens, document highlight, document link, rename, signature help, semantic tokens, inlay hints, folding range, selection range and call/type hierarchy providers, plus Diagnostics. |
| `window.createTreeView` | Register `MainThreadTreeViews` providers, fetch hierarchical children on demand (`$getChildren`), track expand/collapse/selection state, handle refresh/message updates, and reveal tree items inside the Explorer/Tree sidebar. |
| `window.createWebviewPanel` | Create hosted Webview panels (`MainThreadWebviewPanels` / `MainThreadWebviews`), update HTML/title/options, reveal panels, deliver bidirectional `postMessage` payloads, dispose panels cleanly, and rewrite `vscode-resource` URIs strictly confined to approved `localResourceRoots` (or extension/workspace roots). |
| `terminal` & `Pseudoterminal` | Spawn OS shells with confined workspace `cwd` as well as extension-owned custom pseudoterminals (`isExtensionCustomPtyTerminal`), forwarding output (`$sendProcessData`), titles, dimensions/ready states, user keystrokes (`$acceptTerminalProcessInput`), and exit codes (`$sendProcessExit`). |
| `tasks` | Register `MainThreadTask` providers, handle `$tryCreateTaskId`, execute process/shell tasks with workspace-confined `cwd` or `CustomExecution` tasks driven by `ExtHostTask`, terminate running tasks, and publish exit status. |
| `debug` & `testing` | Register `DebugAdapterDescriptorFactory` / sessions through `MainThreadDebugService` and test controllers, test items, profiles and run updates through `MainThreadTesting`. |
| `lm` (`LanguageModelTools`) | Register VSIX chat tools (`MainThreadLanguageModelTools`), expose them to Graphite's built-in chat tool list, and dispatch `$invokeTool` calls when the workspace is trusted. |

Unsupported `MainThread*` actor methods fail explicitly with unsupported-operation errors and are recorded for session inspection. Themes and snippets are also inspectable from VSIX packages; Open VSX is the public registry used by Graphite.

See [portable builds](../contributing/building.md), [plugin installation](../contributing/plugins.md) and [v0.5 validation](../features/v0.5-extensions-automation.md).
