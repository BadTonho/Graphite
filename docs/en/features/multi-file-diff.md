# Multi-file diff

One scrollable view with the changes of several files of the repository, each file collapsible, with next and previous change that cross from file to file.

## Opening

| Command | What it compares |
| --- | --- |
| Git: open all changes | The working tree against the last commit. Key `D` in the source control list. |
| Git: open all staged changes | The index against the last commit. |
| Git: open all unstaged changes | The working tree against the index, new untracked files included. |

Files whose two sides are equal are left out. Added and deleted files are tagged `A` and `D` (modified files `M`), and a file that is not text shows as a binary file with no lines. If nothing changed, or the folder is not a repository, the status bar says so and no view opens. The files are read in the background, so the window stays responsive in large repositories.

## Reading

Each file has a header with its path and its added and removed line counts, followed by its changed lines with three lines of context. Stretches of unchanged lines between two groups of changes are collapsed into a single "N unchanged lines" row. Both line numbers (old and new) are shown. The title line has the totals for the view.

| Command | Key | What it does |
| --- | --- | --- |
| Go to next / previous change | Down / Up | Moves to the next or previous run of changed lines across all files, wrapping around. A change inside a collapsed file expands it. The current change is marked with a bar on the left. |
| Collapse all files / Expand all files | `C` / `E` | Collapses every file to its header, or expands them all. |
| Collapse or expand the selected file | Space | Also done by clicking a file header. |
| Focus the file list | Tab | Shows a list of the files at the left. Up and Down choose a file and scroll to it, Enter collapses or expands it, Tab or Escape returns to the changes. |
| Open the file | Enter | Opens the file of the current change in the editor at that change and closes the view. A deleted file cannot be opened; the status bar says so. |
| Close the multi-file diff | Escape | |

Page Up, Page Down and the mouse wheel scroll.
