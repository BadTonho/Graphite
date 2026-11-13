# Merge editor

The merge editor resolves a file that has Git conflict markers (`<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`). It reads the markers of the file, so it works for any number of conflicts in one file, and it never changes the file until the merge is completed.

## Opening

Run **Open merge editor** from the command palette with the file active. The file must contain at least one complete conflict; otherwise the editor does not open and says so. If the file is open in the editor its current text is used, otherwise the file on disk.

Markers that are longer than seven characters (Git's conflict marker size setting) are understood, and so are files with Windows line endings. A conflict that is never closed, or whose markers do not line up, is treated as ordinary text.

## Layout

Four panes show the same rows and scroll together, even though the two sides of a conflict rarely have the same number of lines:

- **Current** and **Incoming**: the two sides. Lines that differ between them are highlighted.
- **Base**: the common ancestor, available when the markers carry a `|||||||` section (Git's `diff3` or `zdiff3` conflict style). Without it the pane stays empty and the status bar says so.
- **Result**: the text the file will have. A conflict that is still undecided is shown amber; a decided one is shown green with its choice written on the right.

| Command | What it does |
| --- | --- |
| Mixed layout | Both inputs on top, the result below (default). |
| Column layout | Current, result and incoming side by side. |
| Show base, show base at the top, show base in the center | Adds the base pane after the inputs or between them; running the same command again hides it. |
| Show or hide text without conflicts | Keeps only the rows next to a conflict. |
| Go to next or previous unhandled conflict | Moves the active conflict (marked with a bar on the left) and scrolls to it. |

Keys: Up and Down move between unresolved conflicts, Page Up and Page Down scroll, `L` switches the layout, `S` the base pane, `H` the plain text, and Escape closes the editor. The mouse wheel scrolls and a click on a conflict makes it the active one.

When the file is saved while the merge editor is open, the panes are loaded again. Decisions already made are kept for every conflict whose text did not change.

## Deciding a conflict

The decision applies to the active conflict. Accepting moves on to the next unresolved conflict, so a file can be settled with one key per conflict; toggling and reverting stay where they are.

| Command | Key | What it does |
| --- | --- | --- |
| Accept current | `C` | The result takes the current side. |
| Accept incoming | `I` | The result takes the incoming side. |
| Accept both, current first / incoming first | `B` / `Shift+B` | The result takes both sides in that order. |
| Ignore conflict | `N` | Counts the conflict as handled and keeps the base, or nothing when the markers carry no base. |
| Revert conflict to unresolved | `R` | Undoes the decision. |
| Toggle current / incoming for the active conflict | `1` / `2` | Adds or removes one side; the side added second goes second. Removing both makes the conflict unresolved again. |
| Toggle between inputs | | Chooses which input the compare commands start from. |
| Compare current / incoming with base | | Opens a diff of the whole file as that input sees it against the base. Needs a base in the markers. Escape returns to the merge editor. |
| Compare base with result | | Same, against the result so far. |
| Open result in the editor | `O` | Closes the merge editor and opens the file with the result so far; conflicts still undecided keep their markers, so the rest can be finished by hand. The file on disk changes only when saved. |

The result pane cannot be typed into. Edit by hand with **Open result in the editor**; saving the file there stages it when it has no conflict left.
