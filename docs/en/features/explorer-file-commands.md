# Explorer and file commands

| Command | What it does |
| --- | --- |
| New File... / New Folder... (from the explorer) | Asks for a name and creates the entry inside the selected folder, next to the selected file, or in the workspace folder when nothing is selected. A name with folders (`a/b/c.txt`) creates them too. The new file opens in the editor. Names that are empty, absolute, climb out of the folder (`..`) or already exist are refused with a message. |
| Refresh Explorer | Reads the folders from disk again, so files created outside Graphite show up. |
| Collapse Folders in Explorer | Collapses every folder. |
| Open File and Keep Focus in Explorer | Opens the selected file as a preview tab and leaves the keyboard focus in the explorer. |
| Copy Path of Active File | Copies the full path of the active editor's file. |
| New Untitled Text File | Opens a new empty tab (also from the tab bar command). |
| Close All Editors | Closes every tab, keeping those with unsaved changes. |
| Toggle Vertical/Horizontal Editor Layout | Flips the direction in which the editor groups are laid out. |

There is no cut and paste in the explorer yet, so the commands that cancel a cut are not available.
