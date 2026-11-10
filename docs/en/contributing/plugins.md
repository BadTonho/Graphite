# Creating and installing plugins

Build Graphite with `cargo build --workspace`. Native plugin execution requires system Node.js 24 or newer; VSIX inspection does not. Set `graphite.plugins.nodePath` in your user `settings.json` to select a different Node executable. Windows and Linux use the same API and package format.

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

Open the **Plugins** sidebar with `Ctrl+Shift+X`. It shares the left sidebar with Explorer and Source Control, and the editor stays visible. Click the sidebar to give it keyboard focus; clicking the editor releases it. Tabs switch between Installed and GitHub, and the search box accepts GitHub queries. Arrow keys select cards; Escape returns or closes. The same actions are available from the keyboard:

| Key | Action |
|---|---|
| I / G | Installed plugins / GitHub catalog |
| L | Enter a local `.graphite-plugin` or `.vsix` path |
| / | Enter GitHub search text; Enter submits |
| P / N | Previous / next catalog page |
| Enter | Inspect an installed plugin, or show release assets and download the selected asset |
| Y / Esc | Confirm / cancel installation or trust |
| E / D | Enable / disable the selected native plugin |
| R / X | Restart / remove the selected plugin |
| C | Run the selected plugin's first registered command |

The sidebar shows declared command IDs; assign additional commands in `keybindings.json`, for example `{"key":"ctrl+alt+h","command":"local.my-plugin.hello"}`. Installation and updates reset enablement and trust. Trust confirmation permits local code execution for that installed version. Missing Node or activation failures appear in the sidebar; use R to retry after correcting the cause.

VSIX packages show identity, version, engines, entry and contribution categories. Their status is **Imported — VS Code execution unavailable**. They cannot be enabled, and their themes, commands and code do not execute.

## Publish and test

Add the GitHub topic `graphite-plugin` and attach `.graphite-plugin` or `.vsix` assets to a stable release. Graphite lists all supported assets so users can choose. It never clones/builds the repository. Updates are installed manually through the catalog or a local file. Public discovery uses no token and reports network/rate-limit errors.

Run `cargo test -p graphite-plugins` for package/store/catalog tests. Application plugin tests require Node.js 24+ and use local fixtures: `cargo test -p graphite-app app::plugins`. Complete changes with `cargo test --workspace` and `cargo build --workspace`; tests do not call the real GitHub API. Linux execution and interactive UI checks must be recorded separately when unavailable.

Initial validation on Windows passed workspace tests, workspace build, Rust formatting and Node host syntax checking. Interactive Windows UI/GPU verification could not run because the Computer Use native pipe was unavailable (`os error 2`) on both connection attempts. Linux execution was not performed in this Windows environment. Catalog behavior was verified against a local HTTP fixture, not the live GitHub service.
