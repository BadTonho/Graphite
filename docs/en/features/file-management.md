# File management and workspace

## Explorer

- **Select** an entry by clicking it. The selection is the target of the keys below. Clicking the editor moves keyboard focus back to the editor.
- **Rename** (`F2`) opens a prompt with the current name. A name with folders moves the entry, for example `src/lib/old.rs`. Absolute names and `..` are refused, so the entry stays in the workspace.
- **Move by drag and drop**: press an entry and release it on a folder row to move it into that folder. Releasing on a file moves the entry into that file's folder. A folder cannot be moved into itself, and existing names are never overwritten.
- **Delete** (`Delete`) asks for confirmation, then removes the file or folder. Deletion is permanent; Graphite does not use the Recycle Bin.
- Open documents follow renames and moves. An entry with unsaved edits cannot be moved or deleted until the edits are saved or discarded.
- **File nesting**: companion files are listed indented under their parent, for example `app.js` and `app.d.ts` under `app.ts`, or `package-lock.json` under `package.json`. The rules are built in (see `src/file_nesting.rs`) and always on.

## External changes

When the file watcher reports a change in the workspace, open files are compared with the disk:

- A clean document that changed on disk is reloaded, and the status bar reports how many files were reloaded.
- A document with unsaved edits keeps those edits, and the status bar reports the conflict. Save or discard to resolve it.
- Files written in the last second are checked again after they settle, so a save that is still being written is not read half-finished.

## Project search

The search panel (`Ctrl + Shift + F`) has these options:

| Option | Key |
|---|---|
| Regular expression | `Alt + R` |
| Match case | `Alt + C` |
| Whole word | `Alt + W` |
| Show include/exclude fields | `Ctrl + Shift + J` |

With the fields shown, `Tab` moves between the query, the replacement (in replace mode), **files to include**, and **files to exclude**. Glob lists are comma-separated, for example `src/**, *.rs` and `target/**`. Include globs limit the search to matching files; exclude globs skip matching files. An invalid regex or glob is reported in the status bar and clears the results.

Replace all in regex mode inserts the replacement text literally; capture references such as `$1` are not expanded yet.

## Auto save

`files.autoSave` in `settings.json` selects when dirty documents are saved:

| Value | When |
|---|---|
| `off` | Never. |
| `afterDelay` | After `files.autoSaveDelay` milliseconds without edits (default `1000`). |
| `onFocusChange` | When focus leaves a document for another one. |
| `onWindowChange` | When the Graphite window loses focus. |

If `files.autoSave` is not set, the File menu toggle decides: on means `afterDelay`, off means `off`. Documents without a file path, and read-only documents, are never saved automatically.

## Local history

Each save records the file's text in the profile (`history/` folder), unless it is identical to the latest version. Up to 50 versions are kept per file, and the oldest are removed first.

Run **Open local history** from the command palette to list the versions of the active file, newest first, with their date (UTC) and line count. Selecting one opens it read-only in a new tab, so you can copy text back. Isolated sessions do not write history.

Local history does not replace Git: it records what was saved from the editor, not commits.
