# File menu and workspaces

The title bar's **File** menu provides local file, editor, window, profile, and workspace actions. It keeps the same popup geometry for pointer input and keyboard navigation. Press **F10** to open the menu, use the arrow keys to move, **Enter** to activate, and **Escape** to close or return from a submenu.

## Files and editors

- **New Text File** creates another untitled editor tab. **New File**, **Open File**, and **Open Folder** use the native file picker.
- **Save**, **Save As**, **Save All**, and **Revert File** operate on the current editor through Graphite's document model.
- **Auto Save** is off by default. When enabled, a dirty named file is saved after one second without edits. The choice is stored in the active profile's `files.json`.
- Closing an editor or window uses Graphite's existing save, discard, and cancel flow.

## Workspaces

Graphite accepts standard `.code-workspace` files with a `folders` array. Folder paths can be absolute or relative to the workspace file. Use **Add Folder to Workspace**, **Remove Folder from Workspace** (removes the active file's root, or the last root if no file is active), **Save Workspace As**, or **Duplicate Workspace** to manage multi-root workspaces. Explorer, quick open, and text search include all roots. A workspace switch opens a centered dialog that lists the documents with unsaved changes and asks how to handle them before replacing the current roots: **Save all** (`S`), **Discard** (`D`) or **Cancel** (`Esc`), also available as buttons. The same dialog appears when closing the window or reverting a file with unsaved changes. Recent workspace paths are stored in the active profile.

## Preferences and profiles

**Preferences** opens the profile's settings, keybindings, snippets, extension view, task configuration, and theme choices. Settings and keybindings are editable JSON files. Tasks open `.vscode/tasks.json`; when absent, Graphite creates an empty VS Code-compatible task file. **New Window with Profile** starts a separate Graphite process using that profile without changing the active profile in the current window.

Share and online synchronization are not included in this local menu.

Submenus open beside the row that owns them, as soon as the pointer is over it, and can nest: **Profiles** and **Themes** open to the side of **Preferences** without replacing it. Left or Escape closes one level at a time; a submenu that does not fit on the right opens on the left.
