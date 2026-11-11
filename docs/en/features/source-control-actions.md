# Source Control actions

The Source Control view has the same main actions as VS Code's:

| Where | Button | Action |
|---|---|---|
| Repository row | ⇅ | **Sync**: pulls, then pushes. A branch with no upstream is published to `origin`. |
| Repository row | ⇄ | **Create Pull Request**: opens GitHub's compare page for the current branch. Needs an `origin` remote on GitHub. |
| Repository row | ⋯ | **More actions**: opens the command palette filtered to the `Git:` commands. |
| Changes header | ↺ | **View History**: shows the last 300 commits as a graph in a read-only view. |
| Changes header | ▤ | **Stash**: stashes all changes, including new files. |
| Changes header | + | Stage all (existing). |

The `Git:` commands are also in the command palette: Pull, Push, Fetch, Sync, Stash, View History, Create Pull Request.

Git runs in the background, so the window stays responsive during a pull or push. Errors from git (for example, an unreachable remote) appear in the status bar.

Limits: the buttons use text symbols, so they may look slightly different from VS Code's icons. The ⋯ button opens the palette, not a dropdown. Hovering a button does not show a tooltip yet.
