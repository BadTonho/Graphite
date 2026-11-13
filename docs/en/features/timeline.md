# Timeline

The timeline of a file lists, newest first, its saved local history versions and the Git commits that touched it (up to 30). Each commit entry shows the date, the abbreviated hash and the subject. Picking an entry opens a diff between that version of the file and the current file.

| Command | What it does |
| --- | --- |
| Focus on Timeline View | Opens the timeline picker for the active file. |
| Refresh Timeline | Builds the list again, for example after a commit. |
| Compare the Selected Timeline Entry | With the timeline open, opens the diff for the selected entry; with it closed, opens the timeline. |
| Follow the Active Editor in the Timeline | Turns following off, pinning the timeline to the current file, or back on. |
| Show Timeline of the Workspace | Shows the recent commits of the whole repository in a read-only view. |

Commit entries are copies of the file at that commit, kept under `timeline/` in the profile so they can be opened like local history versions. A file outside a Git repository, or never committed, only has its saved versions. Extension timeline sources are not supported.
