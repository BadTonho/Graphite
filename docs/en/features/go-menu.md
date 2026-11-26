# Go menu

The Go menu provides navigation through editor tabs and groups, files, symbols, lines, and diagnostics. File search uses the workspace quick opener. Editor and workspace symbol pickers use the declarations recognized by Graphite's syntax parser; workspace results include every workspace root and use unsaved text from open files. Files larger than 2 MiB, such as minified bundles, are skipped. The files are read on several threads, and closing or reopening the picker stops a search still running.

Go to Line/Column accepts a one-based line or `line:column`. The Switch Editor submenu cycles open views in visual order or recent-use order, with group-only variants. Switch Group focuses one of the first five groups, the next or previous group, or the nearest group in a requested direction.

Back and Forward follow editor navigation history. Go to Definition is available when the active file has a configured language server. Navigation actions without a Graphite or language-server implementation remain visible but disabled.
