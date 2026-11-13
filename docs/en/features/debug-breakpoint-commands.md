# Debug breakpoint and console commands

| Command | What it does |
| --- | --- |
| Go to Next / Previous Breakpoint | Moves the cursor to the next or previous breakpoint of the active file, wrapping around. Says so when the file has none. |
| Remove Breakpoint | Removes the breakpoint on the cursor line; says so when there is none there. Updates a running debug session. |
| Activate or Deactivate Breakpoints | Turns every breakpoint off, or back on. The breakpoints are kept. |
| Reapply All Breakpoints | Sends the breakpoints of every file to the running debug session again. |
| Clear Debug Console | Empties the debug console. |
| Copy Debug Console / Copy All Debug Output | Copies the console text to the clipboard. |
| Evaluate the Debug Console Input | Evaluates what is typed in the console input. |
| Show Debug View | Shows the debug panel. |
| Collapse Variables / Watch Expressions / Call Stack / Loaded Scripts | Collapses every expanded variable and scope in the panel. |

## Renamed commands

These commands changed id. A custom shortcut in `keybindings.json` that uses the old id needs the new one; `F9` is already updated.

| Old id | New id |
| --- | --- |
| `graphite.debug.toggleBreakpoint` | `editor.debug.action.toggleBreakpoint` |
| `graphite.debug.conditionalBreakpoint` | `editor.debug.action.conditionalBreakpoint` |
| `graphite.debug.editBreakpoint` | `editor.debug.action.editBreakpoint` |
| `graphite.debug.logpoint` | `editor.debug.action.addLogPoint` |
| `graphite.debug.enableAllBreakpoints` | `workbench.debug.viewlet.action.enableAllBreakpoints` |
| `graphite.debug.disableAllBreakpoints` | `workbench.debug.viewlet.action.disableAllBreakpoints` |
| `graphite.debug.removeAllBreakpoints` | `workbench.debug.viewlet.action.removeAllBreakpoints` |

Enabling or disabling a single breakpoint, function and data breakpoints, setting a variable, and the debug hover are not available yet.

## Single breakpoints, function breakpoints, variables, hover and console filter

| Command | What it does |
| --- | --- |
| Enable or Disable Breakpoint | Turns the breakpoint on the cursor line off or back on without removing it. A disabled breakpoint stays in the list and is not sent to the debugger. |
| Toggle Breakpoint / Edit Breakpoint (`debug.toggleBreakpoint`, `debug.editBreakpoint`) | The same as the editor commands, under the ids the context menus use. |
| Add Function Breakpoint | Takes the function name from the debug console input and sends it to the debugger (`setFunctionBreakpoints`). Names already added are ignored; Activate or Deactivate Breakpoints also holds these back. |
| Set Variable Value (`debug.setVariable`) | Takes `{"text": "name=value"}` and changes a variable that is loaded in the variables list; the new value shows when the debugger answers. A rejected value is reported in the console and the old value stays. |
| Start Debugging with a Configuration (`debug.startFromConfig`) | Takes `{"text": "<configuration name>"}` and starts that launch configuration. An unknown name is refused. |
| Show Debug Hover | Evaluates the selection, or the word at the cursor, in the current frame and writes `expression = result` to the debug console. There is no popup over the code. |
| Filter / Find in the Debug Console | Shows only the console lines that contain the text in the console input; running it with an empty input shows every line again. |
