# Editor group and tab commands

Commands that move editors between groups and along the tab strip, create and join groups, and close editors in bulk. They are in the command palette under Workbench.

## Moving and copying editors

| Command | What it does |
| --- | --- |
| Move Editor into Next / Previous / First / Last Group | Moves the active tab to that group, in visual order, and focuses the group. A group that loses its last tab closes. If the target group already shows the same document, that tab is activated and the moved tab is dropped. Refused when there is only one group. |
| Split Editor into Next / Previous / First / Last Group | The same, but the tab stays where it was and the target gets a copy. |
| Split Editor into Left / Right / Above / Below Group | Copies the editor into the group next to the focused one in that direction, or splits a new group there when none exists. |
| New Editor Group to the Left / Right / Above / Below | Splits a new group on that side holding a copy of the active editor. Groups cannot be empty, so unlike an empty-group workflow the new group always starts with the active editor. |

## Tabs in a group

| Command | What it does |
| --- | --- |
| Move Editor Left / Right | Moves the active tab one place along the strip; it stops at the ends. Pinned tabs stay before unpinned ones. |
| Move Editor to the Start / End of the Group | Moves the active tab to the first or last place. |
| Open First / Last Editor in Group | Activates the first or last tab. |

## Joining and closing

| Command | What it does |
| --- | --- |
| Join Editor Group with Next | Moves every tab of the focused group into the next group (skipping documents it already shows) and removes the group. |
| Join All Editor Groups | Merges every group into the focused one without duplicate tabs. |
| Close Editors to the Left | Closes the tabs before the active one in the focused group. |
| Close Editors in Other Groups | Closes every tab of the groups other than the focused one. |
| Close All Editor Groups | Closes every tab. The last group stays. |
| Close Editor in All Groups | Closes the active document in every group. |
| Focus First / Last / Active Editor Group | Moves the focus to that group. |

Closing in bulk never discards unsaved changes: a document with changes that is open in only one tab stays open.
