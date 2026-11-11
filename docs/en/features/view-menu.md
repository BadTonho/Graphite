# View menu and workspace layout

The **View** menu groups workbench views, appearance controls, and editor-group layouts. It is opened from the custom title bar and supports pointer and keyboard navigation. The separate Activity Bar context menu remains available for activity visibility and extension-contributed views.

## Views

Explorer, Search, Source Control, Run and Debug, Extensions, Problems, Output, Debug Console, and Terminal open their corresponding Graphite view or panel. Chat and Browser are shown as unavailable until Graphite implements them. The Command Palette is available from this menu and remains the recovery route for a hidden menu bar or Activity Bar.

## Appearance

Appearance controls include full screen, Zen Mode, centered editor, menu bar, primary and secondary sidebars, status bar, panel visibility and position, Activity Bar position and size, tab visibility, editor actions position, minimap, breadcrumbs, sticky scroll, whitespace rendering, control-character rendering, and zoom. Appearance preferences are stored in `view-layout.json` in the active profile. Full screen uses the window manager; Zen Mode is a separate temporary workbench presentation.

The minimap shows a compact document overview and clicking it navigates through the file. Breadcrumbs show the active file path and enclosing declaration when the language is supported. Sticky Scroll can be turned on or off. Whitespace has None, Boundary, Selection, and All modes. Bottom panels can be aligned left, center, or right.

## Editor groups

The Editor Layout submenu supports splitting up, down, left, or right; Single, two- and three-column, two- and three-row, grid, and asymmetric layouts; and Flip Layout. Presets retain open tabs when groups are consolidated. Move Editor into New Window and Copy Editor into New Window reuse the current workspace and open the active file in the new process. A dirty active file is saved before the new window is launched; moving then closes the original tab.

## Keyboard and accessibility

Use Up and Down to move through menu items, Right or Enter to open a submenu, Left to return, and Escape to close the current menu level. Menu items expose checked and unavailable states to accessibility clients. The menu labels and descriptions are localized in English and Brazilian Portuguese.

## Layout buttons in the title bar

Four buttons sit to the left of the window controls:

| Button | Action | Shortcut |
|---|---|---|
| Customize Layout... | Opens the View menu under the button | — |
| Toggle Primary Side Bar | Shows or hides the primary side bar | `Ctrl+B` |
| Toggle Panel | Shows or hides the bottom panel | `Ctrl+J` |
| Toggle Secondary Side Bar | Shows or hides the secondary side bar | `Ctrl+Alt+B` |

The side bar and panel buttons are filled while their area is visible. Hover over a button to see its name and shortcut.
