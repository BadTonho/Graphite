# Terminal menu

The title bar's **Terminal** menu groups the integrated terminal and task commands. Keyboard navigation (arrow keys, Enter, Escape) uses the same rows, enabled states, and accessibility nodes as pointer input.

## Terminal sessions

**New Terminal** opens a session. **Split Terminal** shows two terminals side by side and is enabled once a terminal exists. **New Terminal Window** is listed but disabled until terminals can open in a separate window.

## Running code

**Run Active File** types the quoted path of the active file into the focused terminal, creating a session if none exists. On Windows the input uses PowerShell's `&` call operator. On other systems the shell receives the quoted path, so the file must be executable (or start with a shebang) to run. Untitled documents have no path and disable this row.

**Run Selected Text** sends the selected text to the focused terminal, one line at a time. It is disabled when nothing is selected.

**Run Build Task** runs the default build task from `.vscode/tasks.json`, the same action as **Ctrl+Shift+B**. **Run Task** opens the command palette filtered to the tasks in that file. Tasks run only in trusted workspaces.

## Task control

These rows are enabled only while a task is running:

- **Show Running Tasks** reveals the terminal panel.
- **Restart Running Task** cancels the task and starts it again after it stops.
- **Terminate Task** stops the task.

## Task configuration

**Configure Tasks** and **Configure Default Build Task** open `.vscode/tasks.json`, creating it with an empty task list when it is missing. The default build task is currently set by editing that file; there is no picker yet.
