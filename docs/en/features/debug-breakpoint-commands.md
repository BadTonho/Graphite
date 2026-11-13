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
