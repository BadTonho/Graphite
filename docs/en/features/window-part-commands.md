# Window part commands

Commands for the parts of the window: the side bars, the panel and the keyboard focus. They are in the command palette.

| Command | What it does |
| --- | --- |
| Close Primary Side Bar / Close Panel / Close Secondary Side Bar | Hides that part. The choice is saved with the other view preferences. |
| Focus into Primary Side Bar | Shows the side bar if it is hidden and moves the keyboard focus into it. |
| Focus into Panel | Shows the panel with the terminal and moves the keyboard focus into it. |
| Focus Next Part / Focus Previous Part | Moves the focus between the editor, the terminal and the side bar, the same cycle as `F6` and `Shift+F6`, and says where it went. |
| Next / Previous Primary Side Bar View | Switches between Explorer, Search, Source Control and Extensions, in that order, wrapping around. Views hidden from the activity bar are skipped, and a hidden side bar is shown first. |
| Toggle Full Screen | Same as the View menu entry. |
| Toggle Centered Layout | Same as Toggle Centered Editor. |
| Customize Layout | Opens the layout customization popup. |
| About | Opens the About dialog. |
| Navigate Back / Previous in Navigation Locations, Navigate Forward in Navigation Locations | The same history navigation as Navigate Back and Navigate Forward, under their other command ids. |

The activity bar, the status bar and the panel's own views (problems, output, debug console) have no keyboard focus of their own yet, so commands that would focus them are not available.
