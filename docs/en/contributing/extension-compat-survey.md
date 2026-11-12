# Extension compatibility survey

`packaging/compat/survey.py` measures which `vscode.*` APIs real extensions use and compares them with what Graphite supports. It is static: it downloads each package from Open VSX, reads the manifest and scans the entry point for member accesses such as `window.createTerminal`. **Nothing from a downloaded package is executed or extracted to disk.**

```bash
python3 -I packaging/compat/survey.py LIST_FILE OUTPUT_DIR
```

`LIST_FILE` has one `namespace.name` per line; `OUTPUT_DIR` receives the packages and `report.md`. The supported list is `packaging/compat/supported_apis.txt` (keep it in sync with [the API surface](../features/extension-api.md)) and the valid API names come from `api_members.txt`, generated from `vscode.d.ts` of the pinned Code OSS commit.

## Reading the result

The scan is a heuristic. Extensions that bundle `vscode-languageclient` appear to use almost every provider because the library registers them generically; the report lists those separately. APIs the extension host answers on its own (`workspace.fs`) show up as gaps until they are added to the supported list. For exact data, run an extension and read `<profile>/code-oss/unsupported-api.log`, which records every call Graphite did not answer.

## First run (30 popular extensions, direct use)

| API | Extensions |
|---|---|
| `window.createTextEditorDecorationType` | 9 |
| `workspace.createFileSystemWatcher` | 4 (now supported) |
| `window.showOpenDialog` | 4 |
| `window.createTreeView` | 3 |
| `languages.registerCodeLensProvider` | 3 |
| `tasks.executeTask`, `tasks.fetchTasks` | 2 |
| `debug.*` | 1 to 2 |

Text decorations, code lenses, tree views and webviews need editor or sidebar rendering and are the next gaps with the broadest effect; Debug and virtual file systems affect few extensions.
