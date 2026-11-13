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
- Extensions that create their adapter in code are handled by the extension host; see below.

Extensions installed before this feature are re-read from their `package.json` when the registry loads.

### Adapters created by extension code

When the extension that provides the `type` declares no `program` but has a code entry point, it normally calls `vscode.debug.registerDebugAdapterDescriptorFactory`. Graphite then:

1. Requires the workspace to be trusted and the extension to be enabled and approved in Plugins (the same rule as running any extension code). Otherwise it says so and does not start.
2. Starts the extension host if it is not running, activates the extension (activation event `onDebugResolve:<type>`) and runs its factory with the launch configuration after variable expansion.
3. Starts the session from the descriptor the factory returns:
   - `DebugAdapterExecutable` runs the command with its `args`, `options.cwd` and `options.env`.
   - `DebugAdapterServer` connects to the port. Only loopback addresses are accepted, so an extension cannot make Graphite connect to another machine.
   - Named pipe servers and `DebugAdapterInlineImplementation` (an adapter running inside the extension host) are reported as not supported yet.

The wait ends with a message if the extension fails, returns no adapter, or does not answer within 30 seconds. `Shift+F5` while waiting abandons the launch; a late answer is ignored.

### Configuration providers

The same extension-host step also runs the extension's debug configuration providers, so it applies to every debugger whose extension is enabled, approved and has code, with or without a `program`:

1. After the variables are expanded, each provider registered for the `type` runs `resolveDebugConfiguration`, then `resolveDebugConfigurationWithSubstitutedVariables`, in registration order. Each receives the configuration the previous one returned.
2. The resolved configuration replaces the one from `launch.json` field by field. Graphite's own settings (`adapter`, `preLaunchTask`, `postDebugTask`) are kept and cannot be changed by a provider. A configuration that does not parse is reported and nothing starts.
3. A provider that returns `undefined` cancels the launch (status: "Debugging cancelled by ..."); one that returns `null` also opens `launch.json`, like VS Code.
4. If the extension has a `program`, that adapter is used unless the extension's factory returns another.

An extension that is not approved is skipped when it ships a `program` (the debugger still starts, without its providers), and reported when its adapter can only be created in code.

Known limits: `provideDebugConfigurations` (the "add configuration" suggestions) and dynamic configurations are not called.

### Variables

Every string in the launch configuration (including `program`, `args`, `cwd` and `env`) expands `${workspaceFolder}`, `${workspaceFolderBasename}`, `${cwd}`, `${userHome}`, `${pathSeparator}` (and `${/}`), `${env:NAME}` (empty when unset), and, from the file open when the session starts, `${file}`, `${fileBasename}`, `${fileBasenameNoExtension}`, `${fileExtname}`, `${fileDirname}`, `${relativeFile}` and `${relativeFileDirname}`. Unknown variables, `${input:…}`, `${command:…}`, and file variables with no file open are left as written so the problem is visible. A relative `program` or `cwd` is resolved against the workspace after expansion; `cwd` defaults to the workspace.

### Choosing a configuration

`F5` and **Run → Start Debugging** start the *selected* configuration, or the first one in `launch.json` when none has been selected (or the selected one no longer exists in the file). The command palette lists, for every distinct configuration name:

- **Select Debug Configuration: NAME** — makes it the one `F5` starts, without starting it.
- **Start Debugging: NAME** — selects it and starts it (needs a trusted workspace). It is refused while another session is running or waiting for its `preLaunchTask`, so a choice is never silently ignored.

The selection lives for the current session of the window; it is not saved between launches. Duplicate names are listed once and resolve to the first match. Compound configurations are not supported.

**Start Debugging** in the Run menu no longer requires an explicit `"adapter"`: any configuration in `launch.json` enables it.

## Inline values

While the program is paused, the values of the local variables are shown at the end of the lines of the stopped function, like `total = 5, count = 2`, in a muted color.

- Only the stopped file, when it is open in the editor, is annotated, from up to 60 lines above the stop line to the stop line itself.
- A variable is shown on a line when its name appears there as a whole identifier (`count` is not found inside `counter`). Each variable appears once per line, at most four per line, and long or multi-line values are shortened to one line of 40 characters.
- The variables are those of the first non-expensive scope (normally *Locals*) of the selected frame, as already fetched for the variables column. Selecting another frame, expanding a scope or stepping updates the values.
- Values are hidden as soon as the document is edited, or when the program runs again.

This is the generic fallback that VS Code uses when the adapter gives no inline-value information: it does not understand the language, so a variable named like a word inside a string or a comment is annotated too, and the adapter's `inlineValues` provider is not used.

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

- The adapter's own inline-value provider is not used (see Inline values).
- Variable values are shown on one line and are truncated by the column width; there is no value editing or hover.
- Verified by unit tests of the protocol handling, the row model and hit testing. The panel has not been checked visually in a running window.
