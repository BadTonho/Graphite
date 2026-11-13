# Local history commands

Graphite saves a copy of a file in the profile each time it is saved (up to 50 versions per file, skipping saves that change nothing). These commands work with those saved versions of the active file.

| Command | What it does |
| --- | --- |
| Find Local History Entry to Open | Lists the versions, newest first, with their date and size. Picking one opens it read-only. |
| Find Local History Entry to Restore | Lists the versions; picking one replaces the text of the file with it. The replacement is one edit, so Undo brings your text back. |
| Compare Local History Entry with the File | Lists the versions; picking one opens a diff between it and the file. |
| Compare File with the Latest Local History Entry | Opens a diff between the file and its most recent saved version. |
| Create Local History Entry | Saves a version now. Says so when the file has not changed since the latest version. |
| Delete Local History Entry | Lists the versions; picking one deletes it. |
| Delete All Local History of the File | Deletes every saved version of the file. |
| Restore the Open Local History Entry | With a saved version open in the editor, puts its text into the original file. |

Entries are named by their date, so renaming an entry, selecting one entry to compare with another, and revealing the folder in the system file manager are not available. History is only kept for profiles; a session without a profile saves nothing.
