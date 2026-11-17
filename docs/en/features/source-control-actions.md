# Source Control actions

The Source Control view has the same main actions as VS Code's:

| Where | Button | Action |
|---|---|---|
| Repository row | ⇅ | **Sync**: pulls, then pushes. A branch with no upstream is published to `origin`. |
| Repository row | ⇄ | **Create Pull Request**: opens GitHub's compare page for the current branch. Needs an `origin` remote on GitHub. |
| Repository row | ⋯ | **More actions**: opens a menu of submenus, see [Source Control menus](source-control-menus.md). |
| Changes header | ↺ | **View History**: shows the last 300 commits as a graph in a read-only view. |
| Changes header | ▤ | **Stash**: stashes all changes, including new files. |
| Changes header | + | Stage all (existing). |

The `Git:` commands are in the command palette (and the same commands are grouped in the ⋯ menu): Pull, Push, Fetch, Sync, Stash, View History, Create Pull Request, More Actions, and the branch commands below.

Git runs in the background, so the window stays responsive during a pull or push. Errors from git (for example, an unreachable remote) appear in the status bar.

The buttons are small drawn icons with no frame; the one under the pointer gets a highlight, and the Commit button shows a check mark next to its label. The commit box suggests the shortcut and the branch it commits on. Limit: hovering a button does not show a tooltip yet.

## Branches, amend, merge and rebase

| Command | What it does |
|---|---|
| **Git: Amend Last Commit** | Replaces the last commit with the staged changes. Text typed in the commit message field becomes the new message (and the field is cleared); an empty field keeps the existing message. |
| **Git: Create Branch...** | Asks for a name, then creates the branch at the current commit and switches to it. Uncommitted changes come along. The name is checked with `git check-ref-format`; existing and invalid names are refused. |
| **Git: Merge Branch...** | Opens the branch picker (local branches except the current one, then remote-tracking branches such as `origin/main`); the chosen branch is merged into the current one with a merge commit (`--no-edit`). |
| **Git: Rebase onto Branch...** | Opens the branch picker; the current branch is rebased onto the chosen one. |
| **Git: Delete Branch...** | Opens the picker with the local branches except the checked out one and deletes the chosen branch with `git branch -d`, which refuses a branch that is not fully merged. |
| **Git: Delete Branch (Force)...** | The same with `git branch -D`: deletes even an unmerged branch (its commits stay reachable only through the reflog). |
| **Git: Abort Merge or Rebase** | Abandons a merge or rebase that stopped on conflicts and restores the previous state. |
| **Git: Continue Merge or Rebase** | After resolving and staging the conflicting files: continues the rebase, or commits the merge. |

Notes:

- Merge, rebase and continue are refused while an open document has unsaved edits, because they rewrite files under it. Create Branch, Amend and Abort are not.
- A conflict is not an error that loses anything: the merge or rebase stays in progress, the conflicting files appear in the Source Control view (open them in the 3-way conflict editor), and the status line says to resolve, continue or abort.
- Branch arguments that start with `-` or contain whitespace are rejected before git runs, so a branch name can never be read as a git option.
- Git runs with `GIT_EDITOR=true`, so no editor opens for merge or rebase messages; the default message is used.
- The picker shows 12 entries around the selection, so long lists scroll with the arrow keys. Checkout also lists the remote-tracking branches: choosing `origin/feature` runs `git switch --track` and creates a local `feature` that tracks it (git reports it if a local branch with that name already exists).
- Still missing: the `merge --squash`/`--no-ff` options.

## Interactive rebase

| Command | What it does |
|---|---|
| **Git: Interactive Rebase onto Branch...** | Choose the base in the branch picker. Graphite writes the plan to `git-rebase-plan.txt` in the profile folder and opens it: one `pick <hash> <subject>` line per commit between the base and `HEAD`, oldest first, merges left out. |
| **Git: Run Interactive Rebase** | Reads the plan from the editor (it does not need to be saved), checks it and runs `git rebase -i` with it. |
| **Git: Cancel Interactive Rebase** | Forgets the plan without running anything. |

In the plan you can reorder lines and change the first word to `pick` (`p`), `squash` (`s`: meld into the previous commit, joining both messages), `fixup` (`f`: meld and drop this message) or `drop` (`d`); deleting a line also drops that commit. Graphite checks the plan before git runs: unknown actions, unknown or repeated commits, a `squash`/`fixup` with nothing before it, and a plan that keeps no commit are reported with their line number and nothing starts. Other open documents must be saved first; the plan itself may be unsaved.

Git receives the checked plan through `GIT_SEQUENCE_EDITOR` (a `cp` of the plan over its todo list, run by git's own shell) and `GIT_EDITOR=true`, so no editor opens and squashed commits keep the joined message. A conflict leaves the rebase in progress, exactly as for **Git: Rebase onto Branch...**: resolve it, then **Git: Continue Merge or Rebase**, or **Git: Abort Merge or Rebase**.

Not supported: `reword`, `edit`, `exec`, `break`, keeping merge commits (`--rebase-merges`) and editing the message of a squash.

## Unsaved files and the diff view

- **Commit with unsaved files.** A commit records what is on disk, so text that is only in the editor would be left out. When open documents inside the repository have unsaved changes, a dialog in the middle of the window lists them and asks: **Save and commit** (`S`), **Commit anyway** (`D`) or **Cancel** (`Esc`). If saving fails, no commit is made.
- **Diff view.** The comparison opens in the editor area only: the menu bar, activity bar, sidebar and status bar stay visible and clickable. The header has a **×** button (also `Esc`) to close it and a button that switches between side by side and inline (also `Tab`). Clicking in the sidebar closes the comparison. When both sides are identical or empty, the view says so instead of staying blank.

## Progress and change count

- **Working indicator.** While Sync, Publish, Commit, Push, Pull or another Git operation runs, the main button replaces its icon with a spinning ring of dots and its label with "Syncing...", "Publishing...", "Committing..." or "Working...". Clicking the button again is ignored until the operation ends. The indicator clears itself when the operation reports back, or after ten minutes at most.
- **Change badge.** The Source Control icon in the activity bar shows how many files have pending changes (staged, unstaged or untracked), up to `99+`. It is hidden when there are none or when the repository cannot be read.
