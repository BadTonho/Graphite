# Settings and keyboard shortcut file commands

Graphite has no settings editor; settings are JSON files. These commands open them, creating an empty one when it does not exist yet.

| Command | File |
| --- | --- |
| Open User Settings, Open User Settings (JSON), Open Application Settings (JSON), Open Settings, Open Accessibility Settings | `settings.json` in the profile. |
| Open Workspace Settings, Open Workspace Settings (JSON), Open Folder Settings, Open Folder Settings (JSON) | `.vscode/settings.json` in the workspace folder. Existing content is never overwritten. |
| Open Keyboard Shortcuts (JSON) | `keybindings.json` in the profile. |
| Open Default Keyboard Shortcuts (JSON) | A read-only list of the built-in shortcuts, as `key`, `command` and `when` entries. Your own entries in `keybindings.json` are applied after these. |

The settings editor, remote settings and the raw default settings are not available.
