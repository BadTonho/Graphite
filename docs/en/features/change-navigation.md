# Change navigation and commit box commands

| Command | What it does |
| --- | --- |
| Go to Next Change / Go to Previous Change (also `Next Change`, `Previous Change`, and the diff gutter ids) | Moves the cursor to the first line of the next or previous changed block of the active file, as shown by the colored marks in the gutter. Consecutive changed lines count as one block. At the last (or first) block it wraps to the other end and says so in the status bar. A file without changes says so and stays put. |
| Commit Staged Changes (`scm.acceptInput`) | Commits the staged changes with the message in the commit box. An empty message is refused. |
| Clear Commit Message | Empties the commit box. |
| Open Repository in Terminal | Opens a new terminal in the repository folder of the active file (the workspace folder when the file is outside any repository). |

The changed lines come from the last source control refresh, so they follow what the gutter shows.
