# Debug panel

While a debug session is active the bottom panel shows three columns. Rows are built from the debug runtime by pure functions (`app/debug_panel.rs`), so drawing, clicks and scrolling share one description of what is on screen.

## Columns

| Column | Content | Interaction |
| --- | --- | --- |
| Call stack | Stack frames of the paused thread, with the selected frame marked `▶`. Below it, **Exception breakpoints**: the filters the adapter advertises. | Click a frame to select it; its scopes, variables and watches reload. Click a filter to toggle it. |
| Variables | Scopes (the first non-expensive one, normally *Locals*, opens automatically) and their variables as an expandable tree. Below it, **Watch**. | Click a row with `▸`/`▾` to expand or collapse it; children are requested once and cached. Click the `×` of a watch to remove it. |
| Debug console | Adapter output (`stderr` in the error color) and the results of expressions you evaluate. | Scrolls from the newest line. |

Each column scrolls independently with the mouse wheel.

## Console input

The input line at the bottom of the panel (click it to focus):

- `Enter` evaluates the expression in the selected frame with the `repl` context. The input is echoed as `> expression` and the result, or the adapter error, is appended to the console.
- `Ctrl+Enter` adds the expression as a watch. Watches are evaluated in the `watch` context and re-evaluated whenever the program stops or you select another frame. A failed watch shows its error on its own row instead of the console.
- `Esc` leaves the input.

## Exception breakpoints

The adapter's `exceptionBreakpointFilters` capability populates the list, using each filter's `default` as the initial state. The enabled set is sent with `setExceptionBreakpoints` before `configurationDone` and again whenever a filter is toggled. Your choices are kept when a new session advertises the same filter ids. Adapters that advertise no filters get no request.

## Stepping

`F10`, `F11` and `Shift+F11` send `next`, `stepIn` and `stepOut` with the paused thread's `threadId`, as the protocol requires. Resuming clears the stale stack and variables so the panel never shows values from a previous stop.

## Where the debug adapter comes from

A standard VS Code `launch.json` names only the debug `type` (`"node"`, `"python"`, …). Graphite finds the adapter in the **enabled** installed extension whose `contributes.debuggers` declares that `type`:

- `program` is a path inside the extension. With a `runtime` (for example `node`), the runtime runs it: `runtime runtimeArgs… program args…`. `node` uses the runtime bundled with Graphite when there is one, otherwise `node` from `PATH`. Other runtimes must be on `PATH`. Without a `runtime`, the program is executed directly.
- `windows`, `linux` and `osx` entries override the shared fields one by one.
- `program` paths that leave the extension directory are ignored.
- An `"adapter": { "command": …, "args": […] }` in the configuration still takes precedence, for adapters not installed as extensions.
- Errors name the cause: no extension provides the type, the providing extension is disabled, or the adapter program is missing.
- Not supported yet: extensions that build their adapter in code (`registerDebugAdapterDescriptorFactory`) and declare no `program`.

Extensions installed before this feature are re-read from their `package.json` when the registry loads.

### Variables

Every string in the launch configuration (including `program`, `args`, `cwd` and `env`) expands `${workspaceFolder}`, `${workspaceFolderBasename}`, `${cwd}`, `${userHome}`, `${pathSeparator}` (and `${/}`), `${env:NAME}` (empty when unset), and, from the file open when the session starts, `${file}`, `${fileBasename}`, `${fileBasenameNoExtension}`, `${fileExtname}`, `${fileDirname}`, `${relativeFile}` and `${relativeFileDirname}`. Unknown variables, `${input:…}`, `${command:…}`, and file variables with no file open are left as written so the problem is visible. A relative `program` or `cwd` is resolved against the workspace after expansion; `cwd` defaults to the workspace.

The first configuration in `launch.json` is always the one that starts; choosing among several is not implemented yet.

## Tasks around a session

A launch configuration may name tasks from `.vscode/tasks.json`:

```json
{ "name": "Debug app", "type": "example", "request": "launch",
  "preLaunchTask": "build", "postDebugTask": "clean" }
```

- `preLaunchTask` runs first (the workspace must be trusted, as for any task). The session starts only if the task exits with code 0. A non-zero exit, a failure or a cancellation aborts the launch with a status message naming the task. A label that is not in `tasks.json` is reported before anything runs.
- `Shift+F5` while the task is still running cancels it and abandons the launch.
- `postDebugTask` runs once when the session ends: stop, the program terminating, or the adapter closing. It does not run when Graphite itself exits.
- Both fields are Graphite-side settings and are not forwarded to the debug adapter.
- Not covered yet: problem matchers do not influence whether the launch continues, and there is no "Debug Anyway" prompt after a failed task.

## Limits

- Inline values next to the code are not implemented.
- Variable values are shown on one line and are truncated by the column width; there is no value editing or hover.
- Verified by unit tests of the protocol handling, the row model and hit testing. The panel has not been checked visually in a running window.
