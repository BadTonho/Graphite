# Extension commands

Commands that work with the extensions side bar. They are in the command palette under Extensions, except the ones that take an argument, which are meant to be called from another command or an extension with `{"text": "..."}`.

| Command | What it does |
| --- | --- |
| Enable All Extensions | Enables every disabled extension that was already trusted at its installed version. Extensions that still need the trust prompt stay disabled, and the side bar says so. |
| Disable All Installed Extensions | Stops and disables every enabled extension. |
| Show Popular / Recommended / Recently Published Extensions | Opens the extensions side bar on the Open VSX tab. The tab lists Open VSX results for an empty search, which is Open VSX's own default ordering. |
| Search Language Extensions (and the same for this file / the active language) | Opens the Open VSX tab searching for the language of the active file. |
| Search Extensions (`workbench.extensions.search`) | Opens the Open VSX tab and searches for the text given. |
| Install Extension (`workbench.extensions.installExtension`) | Takes a `publisher.name` id, downloads the latest version from Open VSX and shows it for approval before installing, the same preview as selecting it in the list. An id that does not look like `publisher.name` is refused. |
| Uninstall Extension (`workbench.extensions.uninstallExtension`) | Takes an installed extension id, stops it and removes it. An unknown id is refused. |

Not available: workspace-only enable and disable, update all, checking for updates, recommendations, and the commands about debugging the extension host.
