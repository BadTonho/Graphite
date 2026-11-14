# Activity bar and title bar

Graphite has a configurable activity bar. It switches Explorer, Search, Source
Control, Plugins and activity containers contributed by enabled extensions.
Right-click the bar to show or hide entries and open its layout options. A
custom title bar places the Graphite icon and workbench menus in the same row,
with window controls on the right. The system window title and app icon remain
set for the taskbar and window switcher. Drag the free part of the bar to move
the window, double-click it to maximize or restore, and drag its edges to resize.
When the window is narrow, menus that do not fit move into **More**.

## Built-in items

| Item | Icon | Behavior |
| --- | --- | --- |
| **Explorer** | `cod-files` | Shows the file tree and closes Source Control, project search and Plugins. |
| **Search** | `cod-search` | Opens the project search overlay (`Ctrl+Shift+F`); clicking it again closes the overlay. |
| **Source Control** | `cod-source_control` | Toggles the Git sidebar (`Ctrl+Shift+G`) and closes Plugins. |
| **Plugins** | `cod-extensions` | Toggles the Plugins sidebar (`Ctrl+Shift+X`). |

## Context menu and layout

The context menu lists the four built-in activities and activity containers
declared by enabled, workspace-trusted extensions. Hiding an entry only hides
its bar button; it does not disable or uninstall its extension. Hiding the
active entry switches to another visible built-in activity. The last visible
built-in entry cannot be hidden. The top **View** menu contains the same
visibility and layout settings; when the rail is hidden, it adds **Show Activity
Bar** so the rail can be restored without the command palette.

The top menu row contains **File**, **Edit**, **Selection**, **View**, **Go**,
**Run**, **Terminal** and **Help**. View is functional; the other menus show a
localized empty-state message until their commands are defined. Menus can be
opened with a pointer or with F10, navigated with the arrow keys, selected with
Enter/Space and dismissed with Escape.

**Activity Bar Position** supports Default (vertical left rail), Top, Bottom and
Hidden. **Activity Bar Size** supports Default and Compact. **Move Primary Side
Bar Right** places the main view column on the right while leaving the activity
rail at the left. These options and hidden entries are saved per profile. When
the bar is hidden, use **Show Activity Bar** from View or the command palette to
restore it. The context menu supports pointer selection and keyboard navigation
with the arrow keys, Enter/Space and Escape.

Default uses a 48 px rail; Compact uses a 36 px rail and 40 px items. The main
sidebar is resizable and saved per profile. Its maximum is 70% of the window
width, subject to preserving at least 320 px for the editor. Top and Bottom
reserve a horizontal strip and reflow the editor vertically. Shared geometry
keeps drawing, hit testing and resizing aligned when the rail or primary sidebar
moves.

## Extension contributions

VSIX packages can declare activity containers in
`contributes.viewsContainers.activitybar` and views in `contributes.views`.
Graphite native plugin manifests use the equivalent `activity_bar` array, with
container IDs, titles, optional icons and `tree` or `webview` view kinds. Only
enabled and workspace-trusted extension records are listed. Selecting a
contribution activates the owning extension. Tree views show their data under
the container title, and views declared with `"type": "webview"` host the page
the extension provides in an overlay below the view header (see
[Extension API](extension-api.md)).

## Accessibility and verification

The top menu and activity rail are exposed as accessibility toolbars with
localized buttons and selected states. Menu keyboard navigation and pointer
hit testing share the same item geometry.

Automated coverage checks layout modes, built-in visibility, menu actions and
plugin contribution visibility. Run `cargo test -p graphite-app --bin graphite
activity_bar` and `cargo test --workspace`. Native visual checks should include
Windows and Linux sessions where available.
