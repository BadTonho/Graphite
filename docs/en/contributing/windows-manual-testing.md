# Windows Manual Test Checklist

Checks that were only automated or verified on Linux and must be confirmed by hand on Windows.
Mark each item and note the result. Automated tests (`cargo test --workspace`) should pass first.

## 0. Build and runtime

- [ ] `cargo build --workspace` succeeds.
- [ ] `cargo test --workspace` passes.
- [ ] Build the portable package (`packaging/code-oss/portable.py`) from a path without spaces; confirm `code-oss/commit` sits beside `graphite.exe`.
- [ ] Node.js 24 is on `PATH`; the app starts and the mouse and keyboard respond.

## 1. Text decorations (`createTextEditorDecorationType`)

1. Build the demo: `python packaging/examples/build_demo.py` (output: `target/examples/graphite-demo.decoration-demo-0.1.0.vsix`).
2. Open a folder containing a text file with a `TODO` line, a line with `error`, and a last line.
3. Plugins sidebar (`Ctrl+Shift+X`): `L`, enter the `.vsix` path, `Y`; select it and press `E`.
4. Trust the workspace, then run **Graphite Demo: Decorate editor** from the palette.

Expected:
- [ ] First line: translucent blue background across the full width.
- [ ] Each `TODO`: yellow background with an orange border around the word only.
- [ ] Each line with `error`: reddish background and red `<- Graphite decoration` after the text.
- [ ] Last line: green `(note added by the extension)` after the content.
- [ ] Status message "Decorated N TODO and M error line(s)".
- [ ] Columns are correct with tabs, accents and emoji on the line.
- [ ] Selection is drawn over decoration backgrounds; typing and scrolling work normally.
- [ ] **Graphite Demo: Clear decorations** removes every mark.

Known limits: not drawn in word-wrap mode; ranges do not follow edits until the extension reapplies them; `before` text, fonts and gutter icons are ignored.

## 2. Terminal API

- [ ] An extension calling `window.createTerminal` opens a terminal tab; `sendText` runs the command (PowerShell/cmd).
- [ ] Closing the terminal notifies the extension; a terminal whose process exits reports its exit.

## 3. File watchers and SCM

- [ ] Creating, changing and deleting files in the workspace (from Explorer or another program) reaches extension `createFileSystemWatcher` listeners.
- [ ] The app does not freeze or spin the CPU while Git runs (watcher ignores `.git` internals and `target/`).
- [ ] Git status in the sidebar updates after commits and branch switches.

## 4. Language providers

- [ ] Go to Definition / Declaration / Type Definition / Implementation / References from a VSIX provider (menu and shortcuts).
- [ ] Document symbols (Go to Symbol), Quick Fix code actions, Rename and Signature Help work with a VSIX provider when no LSP is configured.
- [ ] Windows paths (`C:\...`) and `file:///C:/...` URIs map correctly in results.

## 4b. Extension language providers (range formatting, inlay hints, highlights)

- [ ] An extension with a range formatter: **Format Selection** formats only the selection, or the cursor line when nothing is selected.
- [ ] An extension with an inlay hints provider (in a language no language server handles) shows hints after the text; they refresh after typing and do not flicker.
- [ ] An extension with a document highlight provider: resting the cursor on an identifier highlights its other occurrences; moving the cursor clears them immediately.
- [ ] The CPU stays idle while the cursor and document do not change.

## 5. Workbench

- [ ] Status bar items from extensions appear and their click command runs.
- [ ] `env.clipboard` read and write work.
- [ ] Find in Files (`findTextInFiles`) returns results from an extension.
- [ ] `<profile>/code-oss/unsupported-api.log` is created only when an extension calls an unsupported API.

## 5b. Declarative contributions

- [ ] Snippets contributed by an installed VSIX appear in completion; disabling the extension removes them.
- [ ] Color themes contributed by a VSIX appear in the theme list and apply correctly.

## 5c. Debugging with extensions

- [ ] A `launch.json` with only `"type"` (for example from an installed debugger extension that ships a `program`) starts the adapter; the panel shows frames, variables and watches at a breakpoint.
- [ ] An extension that registers a `DebugAdapterDescriptorFactory` (executable or `DebugAdapterServer`): F5 starts the session after the extension host starts; the status line says "Waiting for ..." meanwhile.
- [ ] `Shift+F5` while waiting for the extension abandons the launch and nothing starts later.
- [ ] A factory that throws, or returns an inline implementation, shows a readable status message.
- [ ] `preLaunchTask` runs before the session and a failing task aborts it; `postDebugTask` runs once after the session ends.
- [ ] Command palette: **Select Debug Configuration** and **Start Debugging: NAME** list every configuration of `launch.json`.
- [ ] With the real Code OSS runtime, run `cargo test -p graphite-app -- --ignored official_host_creates_a_debug_adapter_from_an_extension_factory` with `GRAPHITE_TEST_CODE_OSS` set.

## 6. Report

Note failures with: step, expected result, actual result, and `unsupported-api.log` contents if present.
