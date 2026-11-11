# File menu and workspaces

The title bar's **File** menu provides local file, editor, window, profile, and workspace actions. It keeps the same popup geometry for pointer input and keyboard navigation. Press **F10** to open the menu, use the arrow keys to move, **Enter** to activate, and **Escape** to close or return from a submenu.

## Files and editors

- **New Text File** creates another untitled editor tab. **New File**, **Open File**, and **Open Folder** use the native file picker.
- **Save**, **Save As**, **Save All**, and **Revert File** operate on the current editor through Graphite's document model.
- **Auto Save** is off by default. When enabled, a dirty named file is saved after one second without edits. The choice is stored in the active profile's `files.json`.
- Closing an editor or window uses Graphite's existing save, discard, and cancel flow.

## Workspaces

Graphite accepts standard `.code-workspace` files with a `folders` array. Folder paths can be absolute or relative to the workspace file. Use **Add Folder to Workspace**, **Remove Folder from Workspace** (removes the active file's root, or the last root if no file is active), **Save Workspace As**, or **Duplicate Workspace** to manage multi-root workspaces. Explorer, quick open, and text search include all roots. A workspace switch asks how to handle unsaved documents before replacing the current roots. Recent workspace paths are stored in the active profile.

## Preferences and profiles

**Preferences** opens the profile's settings, keybindings, snippets, extension view, task configuration, and theme choices. Settings and keybindings are editable JSON files. Tasks open `.vscode/tasks.json`; when absent, Graphite creates an empty VS Code-compatible task file. **New Window with Profile** starts a separate Graphite process using that profile without changing the active profile in the current window.

Share and online synchronization are not included in this local menu.
