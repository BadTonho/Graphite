# Task commands

Tasks are defined in `.vscode/tasks.json` of the workspace and need the workspace to be trusted. These commands complement Run Build Task and Terminate Task.

| Command | What it does |
| --- | --- |
| Run Task... | Opens the command palette filtered to the workspace tasks. |
| Run Test Task / Run Clean Task / Run Rebuild Task | Runs the default task of that group: the task whose `group` is that name, or the one marked `isDefault`. Not available when the file has none. |
| Rerun Last Task | Runs again the task that ran last in this session. |
| Show Running Tasks | Shows the panel when a task is running; not available otherwise. |
| Restart Running Task | Stops the running task and starts it again. |
| Terminate Task | Stops the running task. |
| Show Task Log | Shows the panel with the output of the last task. |
| Toggle Problems in the Panel | Switches the panel between the task output and the problems list. |
| Configure Task Runner, Open Workspace Tasks, Configure Default Build Task, Configure Default Test Task | Open `.vscode/tasks.json`, creating it when it does not exist. |

Tasks of the user (outside a workspace) and the choice of which task is the default are not handled by a dialog; edit the `group` of the task in the file.
