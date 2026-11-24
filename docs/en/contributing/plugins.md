# Creating and installing plugins

Build Graphite with `cargo build --workspace` for native plugin development. JavaScript/TypeScript plugins (`"runtime": "node"`, default) use Node.js 24+ and accept a custom executable through `graphite.plugins.nodePath` in user settings. Compiled native Rust plugins (`"runtime": "native"`) use the `graphite-plugin-sdk` crate directly without requiring Node.js. Use the [portable package](building.md) for Code OSS VSIX execution.

## Create a JavaScript/TypeScript plugin

```sh
graphite plugin create my-plugin
cd my-plugin
npm install
npm run build
graphite plugin pack .
```

For fast local iteration without packing on every edit, link your plugin directly in development mode (console output is saved to `logs/<id>/console.log`, and changes to `graphite-plugin.json` or the entrypoint trigger hot reload):

```sh
graphite plugin dev .
# When done testing:
graphite plugin unlink local.my-plugin
```

The generator creates TypeScript source, local `graphite.d.ts` API declarations, build configuration and an example command. `graphite plugin pack .` writes `local.my-plugin-0.1.0.graphite-plugin` beside the project directory. Compile before packing. Hidden files and `target` are omitted; key/database file extensions and symlinks are rejected.

The root manifest (`graphite-plugin.json`) supports declarative contributions and granular permissions:

```json
{
  "id": "local.my-plugin",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "node",
  "main": "dist/index.js",
  "description": "My Graphite plugin",
  "permissions": ["commands", "documents.read", "ui"],
  "activationEvents": ["onStartupFinished"],
  "commands": [{"id": "local.my-plugin.hello", "title": "Hello from my plugin"}],
  "keybindings": [{"command": "local.my-plugin.hello", "key": "ctrl+alt+h"}],
  "menus": {
    "commandPalette": [{"command": "local.my-plugin.hello"}]
  },
  "activityBar": [{
    "id": "local.my-plugin.tools",
    "title": "My Tools",
    "views": [{"id": "local.my-plugin.items", "title": "Items", "kind": "tree"}]
  }]
}
```

IDs use `publisher.name` with lowercase ASCII letters, digits and hyphens. Versions use semver. Command, container and view IDs must start with the full plugin ID plus `.`; registrations must use their declared titles. Optional declarative fields also include `themes`, `snippets` and `languages` (language ID, extensions, line/block comment markers).

Valid `permissions` tokens are: `commands`, `documents.read`, `documents.write`, `ui`, `languages`, `storage`, `workspace.read`, `workspace.write`, `terminal`, `chat`, and `*`. System capabilities (`workspace.read`, `workspace.write`, `terminal`, `chat`) always require explicit permission in the manifest.

## API v1 (JavaScript / TypeScript)

Export `async activate(context, graphite)` and optionally `deactivate()`. The generated `graphite.d.ts` defines the full SDK:

- `commands`: `list()`, `execute(id, args?, target?)`, `registerCommand(id, handler)`.
- `documents`: `list()`, `open(uri)`, `save(uri, reason?)`, `snapshot(uri)`, `applyEdits(edits)` (zero-based UTF-16 positions, versioned), `onDidChange(listener)` and `onWillSave(metadata, participant)`.
- `window`: `showMessage(text)`, `focusView(view)`, `showInputBox(options)`, `showQuickPick(items, options)`, `createStatusBarItem(id?)`, `createOutputChannel(name)`, `registerTreeDataProvider(viewId, provider)` and `createWebviewPanel(viewType, title, options)`.
- `storage`: `global` and `workspace` (`get`, `update`, `delete`).
- `editor`: `active()`, `setSelections(selections)`, `createDecorationType(options)`, `setDecorations(decorationType, uri, ranges)`.
- `languages`: `setDiagnostics(owner, uri, diagnostics)`, `clearDiagnostics(owner, uri)`, `registerCompletionProvider`, `registerHoverProvider`, `registerFormattingProvider`.
- `workspace`: `get()`, `configuration()`, `findFiles(options?)`, `readFile(path)`, `writeFile(path, content)` (all confined to the workspace root, blocking `.git/` and path escapes).
- `terminal`: `create(options?)` returning `{ id, sendText, show }`.
- `scm`: `status()` returning current Git branch and changed files.
- `chat`: `registerTool(definition, handler)` registering tools callable during Graphite Chat turns.

Track disposables in `context.subscriptions`; disposal and host shutdown release editor-side resources automatically.

## Create a compiled Rust plugin (`runtime: "native"`)

Use the `graphite-plugin-sdk` crate to write plugins with zero Node.js dependency:

```rust
use graphite_plugin_sdk::{Plugin, PluginContext, ServiceResult};
use serde_json::json;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn activate(&self, ctx: &PluginContext) -> ServiceResult<()> {
        ctx.register_command("local.myrust.ping", "Ping", |_ctx, _args| {
            Ok(json!({ "pong": true }))
        })?;
        Ok(())
    }
}

fn main() {
    graphite_plugin_sdk::run_plugin(MyPlugin).expect("plugin failed");
}
```

Set `"runtime": "native"` and `"main": "bin/my-plugin"` in `graphite-plugin.json` (on Windows `.exe` is resolved automatically; on Unix `graphite` sets `0o755` on extraction and launch). See `crates/graphite-plugin-sdk/examples/echo_plugin.rs` for a complete example exposing commands, workspace file listing and a Chat tool.

## Install and manage

Open the secondary sidebar with the title bar button or `Ctrl+Alt+B`. The header includes Graphite Chat and one tab for each secondary view contributed by enabled VSIX extensions, such as Codex and Claude Code. Click a tab to switch. The tab strip scrolls to keep the active tab visible. Drag the edge facing the editor to resize the sidebar. Graphite saves the width per profile and restores the selected view when you reopen the sidebar during the same session. Graphite discovers installed extensions from their local manifests. Executable views require the Code OSS runtime from a portable build and a trusted workspace.

Use **Install from VSIX…** in the Plugins header or its **More plugin actions** menu to open the system file picker. It starts in the workspace folder, then reopens in the folder of the last VSIX selected in the current session and profile. On Windows, use **All files** in the picker to check the actual filename if a package is missing from the VSIX filter. Graphite accepts files with a `.vsix` extension regardless of capitalization. Review the package and choose **Install** or **Cancel**. **Refresh plugins** reloads the installed list and repeats the current catalog search. Press **Tab** to focus the header actions, then use **Enter** to activate them and the arrow keys to navigate the menu. From a terminal, `graphite plugin install <package>` installs a local package disabled so you can enable it later.

The menu checks installed VSIX extensions for newer Open VSX versions. **Update all extensions** prepares individual reviews. Automatic updates are off by default and set per profile. When enabled, Graphite checks at profile startup and every 24 hours while the editor is open. Use **Review pending updates** to approve or skip each prepared package. The installed version remains in use until you approve the replacement. The menu also enables or disables extensions in bulk, lists running extensions, and starts a bisection. During a bisection, reproduce the problem during each trial and report whether it persists. The original enabled set is restored when the session ends.

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

Installation and updates reset enablement and trust. Trust confirmation permits local code execution for that installed version. Activation failures or permission errors appear in the sidebar and `logs/<id>/console.log`; use **R** to retry after correcting the cause.

Select an installed VSIX with a Node.js entry point and press **E** to approve that installation. Trust the workspace, then open the command palette (`Ctrl+Shift+P` or `F1`) and choose the extension command. Approve the replacement installation after an update. See [supported VSIX APIs](../architecture/extension-compat.md).

## Publish and test

Publish VSIX extensions on Open VSX for discovery. To list a plugin in the Graphite catalog, publish a public GitHub repository, add the `graphite-plugin` repository topic, and attach a `.graphite-plugin` or `.vsix` file to its latest stable release. Repositories without a stable release remain searchable, but cannot be installed until a supported release asset is published. Graphite accepts local `.vsix` and `.graphite-plugin` packages through the Plugins sidebar as well.

Run `cargo test -p graphite-plugins` for package/store/catalog tests, `cargo test -p graphite-plugin-sdk --all-targets` for Rust SDK tests, and `cargo test -p graphite-app app::plugins` for application plugin tests (Node.js 24+ required for JS fixtures; `rustc` required for native binary test fixtures). Complete changes with `cargo test --workspace` and `cargo build --workspace`; marketplace tests use a local HTTP fixture and never call Open VSX. Linux execution and interactive UI checks must be recorded separately when unavailable.

See [portable package tests](building.md) and [v0.5 validation](../features/v0.5-extensions-automation.md) for the Code OSS checks.
