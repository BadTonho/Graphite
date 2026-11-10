# Creating and installing plugins

Build Graphite with `cargo build --workspace` for native plugin development. Native plugin execution uses Node.js 24+ and accepts a custom executable through `graphite.plugins.nodePath` in user settings. Use the [portable package](building.md) for Code OSS VSIX execution.

## Create a native plugin

```sh
graphite plugin create my-plugin
cd my-plugin
npm install
npm run build
graphite plugin pack .
```

The generator creates TypeScript source, local API declarations, build configuration and an example command. The package is written beside the project directory as `local.my-plugin-0.1.0.graphite-plugin`. Existing output files are not overwritten. Compile before packing. Packages include compiled code and any dependencies in the project; no dependency installation or build occurs inside Graphite. Bundle runtime dependencies when practical, and avoid shipping development dependencies or private data. Hidden files are omitted; key/database file extensions and symlinks are rejected.

The root manifest is:

```json
{
  "id": "local.my-plugin",
  "version": "0.1.0",
  "apiVersion": 1,
  "main": "dist/index.js",
  "description": "My Graphite plugin",
  "commands": [{"id": "local.my-plugin.hello", "title": "Hello from my plugin"}]
}
```

IDs use `publisher.name` with lowercase ASCII letters, digits and hyphens. Versions use semver. Command IDs must start with the full plugin ID plus `.`; registrations must use their declared titles. Entry points are relative `.js`, `.cjs` or `.mjs` files. ES modules use `"type": "module"` in `package.json`; CommonJS can export `activate` directly.

## API v1

Export `async activate(context, graphite)` and optionally `deactivate()`. The generated `graphite.d.ts` defines the SDK:

- `commands.registerCommand(id, handler)` returns a disposable. Handlers receive JSON arguments and an `AbortSignal`, and may return JSON results asynchronously.
- `documents.list()` and `snapshot(uri)` read current in-memory text, including unsaved changes. `documents.onDidChange(listener)` receives opened, changed and closed snapshots and returns a disposable.
- `documents.applyEdits(edits)` accepts `{uri, version, edits:[{start, end, text}]}`. Positions are zero-based UTF-16 `{line, character}`; use the snapshot's version. Changes preserve editor validation and undo; saving is separate and not exposed in v1.
- `workspace.get()` and `workspace.configuration()` query the editor; `window.showMessage(text)` shows a message.

Track disposables in `context.subscriptions`; disposal is idempotent. Host termination also releases editor-side resources. Use the cancellation signal for long operations. Logs go to stderr. Plugins execute with the user's local account access: only enable trusted code.

## Install and manage

Open the **Plugins** sidebar with `Ctrl+Shift+X`. It shares the left sidebar with Explorer and Source Control, and the editor stays visible. Click the sidebar to give it keyboard focus; clicking the editor releases it. Tabs switch between Installed, Open VSX and the Graphite GitHub catalog. Open VSX results download the latest VSIX for inspection. Click or press Enter on a Graphite repository to open its own editor tab with GitHub Flavored Markdown rendered in a GitHub-style dark WebView, including tables, code blocks, task lists, badges and HTTPS images. Relative README images resolve against the repository. The right column lists compatible release packages, release information, topics and fully visible project links. Scripts, forms, frames and non-HTTPS images are blocked; HTTPS links open in the system browser. If there are multiple supported packages, choose one to inspect and install; the existing package confirmation still applies. Repositories without a compatible stable release keep their README and show why installation is unavailable. Detail tabs can be switched and closed like editor tabs. The same actions are available from the keyboard:

| Key | Action |
|---|---|
| I / O / G | Installed plugins / Open VSX / Graphite GitHub catalog |
| L | Enter a local `.graphite-plugin` or `.vsix` path |
| / | Enter search text for the active marketplace tab; Enter submits |
| Enter | Inspect an installed plugin, download an Open VSX result, or open/install the selected Graphite item |
| Y / Esc | Confirm / cancel installation or trust |
| E / D | Enable / disable the selected plugin |
| U | Download the latest Open VSX version for an installed VSIX; confirm to replace it |
| R / X | Restart a native plugin / remove the selected plugin |
| C | Run the selected plugin's first registered command |

The sidebar shows declared command IDs; assign additional commands in `keybindings.json`, for example `{"key":"ctrl+alt+h","command":"local.my-plugin.hello"}`. Installation and updates reset enablement and trust. Trust confirmation permits local code execution for that installed version. Missing Node or activation failures appear in the sidebar; use R to retry after correcting the cause.

Select an installed VSIX with a Node.js entry point and press **E** to approve that installation. Trust the workspace, then open the command palette (`Ctrl+Shift+P` or `F1`) and choose the extension command. Approve the replacement installation after an update. See [supported VSIX APIs](../architecture/extension-compat.md).

## Publish and test

Publish VSIX extensions on Open VSX for discovery. To list a plugin in the Graphite catalog, publish a public GitHub repository, add the `graphite-plugin` repository topic, and attach a `.graphite-plugin` or `.vsix` file to its latest stable release. When both formats are attached, users can choose which package to inspect. Repositories without a stable release remain searchable, but cannot be installed until a supported release asset is published. Graphite accepts local `.vsix` and `.graphite-plugin` packages through the Plugins sidebar as well.

Run `cargo test -p graphite-plugins` for package/store/catalog tests. Application plugin and upstream Emmet integration tests require Node.js 24+ and use local fixtures: `cargo test -p graphite-app app::plugins`. Complete changes with `cargo test --workspace` and `cargo build --workspace`; marketplace tests use a local HTTP fixture and never call Open VSX. Linux execution and interactive UI checks must be recorded separately when unavailable.

See [portable package tests](building.md) and [v0.5 validation](../features/v0.5-extensions-automation.md) for the Code OSS checks.
