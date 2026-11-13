# Snippet and output commands

| Command | What it does |
| --- | --- |
| Configure Snippets | Opens `snippets.json` in the profile, creating it when it does not exist. |
| Insert Snippet (`editor.action.insertSnippet`) | Takes `{"text": "..."}` and inserts that text at every cursor. It is meant to be called from another command or an extension; it has no picker. An empty text is refused. |
| Toggle Output | Shows the task output in the panel, or hides the panel when it already shows it. From the problems list it switches to the output. |
| Show Output | Shows the task output in the panel. |

Output channels, logs and the timeline view are not available: Graphite has one output, the task output.

## Logs

**Show Logs** and **Open Log File** list the `*.log` files in the profile (up to two folders deep, newest first), for example the unsupported-API and requested-commands logs that extensions produce. Picking one opens it. With no log files they say so.
