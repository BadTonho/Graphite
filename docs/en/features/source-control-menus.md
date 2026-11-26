# Source Control menus, commit flow and Graph

## The main button

The wide button under the commit message changes with the state of the branch:

| State | Button |
|---|---|
| There are changes, or nothing to sync | **Commit** (with an arrow, see below) |
| No changes and the branch is ahead of or behind its upstream | **Sync Changes** with the counts, such as `1↓ 2↑`: pulls, then pushes |
| A remote exists but the branch tracks nothing yet | **Publish Branch**: pushes and sets the upstream |
| There is no remote | **Publish Branch**: opens the GitHub publish dialog |

The ahead and behind counts and the upstream come from `git for-each-ref`, so they follow the last fetch. Press **Fetch** to refresh them.

## Commit

Commits run through `git commit`, so hooks (`pre-commit`, `commit-msg`), signing and the identity from the git configuration apply exactly as in the terminal. A hook that fails stops the commit, and the message stays in the box.

The arrow at the right end of the button opens: **Commit**, **Commit & Push** and **Commit & Sync**. The push or sync only runs when the commit worked.

The **⋯ → Commit** submenu has every variant:

| Entry | What it does |
|---|---|
| Commit | Commits what is staged; when nothing is staged, stages and commits everything. |
| Commit Staged | Only what is staged. Refused when nothing is staged. |
| Commit All | Stages every change (new files included), then commits. |
| ... (Amend) | The same three, replacing the last commit. A blank message keeps the old one. |
| ... (Signed Off) | The same three, adding a `Signed-off-by` line. |
| Undo Last Commit | Removes the last commit and keeps its changes staged. The first commit cannot be undone. |
| Abort Merge or Rebase | Abandons a merge, rebase, cherry-pick or revert that stopped on conflicts. |

## The ⋯ menu

The button opens a menu with submenus that open on hover or with the right arrow key. Arrow keys move, Enter chooses, Left closes a submenu and Escape closes the menu.

- **Pull**, **Push**, **Clone...** (asks for the address, then the folder to clone into; the clone opens as the workspace when it finishes), **Checkout to...**, **Fetch**
- **Commit** (above)
- **Changes**: Stage All, Unstage All, **Discard All Changes** (asks first)
- **Pull, Push**: Sync, Pull, Pull (Rebase), **Pull from...** (choose a remote branch), Push, **Push to...** (choose a remote; the branch then tracks it there), Fetch
- **Branch**: Merge, Rebase, Create, **Create From...**, **Rename**, Delete, Delete (Force), **Delete Remote Branch...**
- **Stash**: Stash, Stash (Include Untracked), Stash Staged, Apply Latest Stash, Pop Latest Stash, **Apply Stash...**, **Pop Stash...**, **Drop Stash...** and **Drop All Stashes** (asks first)
- **Tags**: Create Tag (on HEAD), **Delete Tag...**
- **Show Git Output**: opens a read-only view with the git commands that changed something (commit, push, checkout, stash...) with the time and result of each; reads such as status are left out. The last 1000 lines are kept; run it again to refresh the same tab.
- **Remote**: **Add Remote...** (asks for the URL, then the name, suggesting `origin`) and **Remove Remote...**

Entries ending in `...` open a list to choose from (the same list as the branch picker): branches, remote branches, tags, stash entries or remotes. A list with nothing to choose from does not open and says so in the status bar. **Create From...** lists branches, remote branches and tags, then asks for the new branch name.

The `Git:` commands are still listed in the command palette.

## Changes list

- The letter at the end of each row says how the file differs on that side: `M` modified, `A` added, `D` deleted, `U` untracked, `R` renamed, `C` copied, `!` conflict. A file with staged and unstaged edits shows its own letter in each list.
- Unstaged rows have a **Discard Changes** button (the arrow) beside the stage button. Discarding a tracked file restores it to the staged content; discarding an untracked file deletes it. Both ask for confirmation, and staged changes are never touched. Conflicted files are not discarded: resolve them instead.
- **⋯ → View as Tree** shows each list as folders (folders first, then files, sorted by name) and **View as List** goes back. Click a folder to open or close it; its `+` / `-` button stages or unstages every file below it. A folder is opened or closed separately in the staged and unstaged lists.
- Right-click a file for: Open File, Stage or Unstage Changes, Discard Changes, **Add to .gitignore** (an anchored entry such as `/src/a.rs`, added once) and Copy Path / Copy Relative Path.

## Graph

Right-click a commit for: **Checkout (Detached)**, **Create Branch...**, **Create Tag...** (both at that commit), **Cherry Pick**, **Revert Commit**, **Copy Commit ID** and **Copy Commit Message**. Checkout and Cherry Pick are disabled on the commit that is already checked out. A cherry-pick or revert that stops on conflicts can be finished or aborted with **Git: Continue Merge or Rebase** and **Abort Merge or Rebase**.

The Graph draws commits as circles (a ring for the checked out one) and keeps loading older commits, 100 at a time, as you scroll to its end. Hovering a commit shows a card with its message, author, date, full id and the branches and tags on it.

Commit ids passed to git are checked to be hexadecimal, and branch and tag names are validated before git runs, so nothing the user types can be read as a git option.

## Switching folders

Opening another folder (File menu, removing a folder from the workspace, or the command line) reloads the view for it: the lists, the Graph and its scroll position, the commit message, open menus and pickers are reset, and the file watcher follows the folders that are open now. A result still loading for the previous folder is discarded instead of replacing the new one.

File changes are collected for a quarter of a second and handled together, so a build or a package install that touches thousands of files refreshes the view a few times instead of once per file. Changes to paths that the folder's `.gitignore` or `.git/info/exclude` excludes, such as `node_modules`, do not refresh the view; open files there are still checked for outside changes. Only one refresh runs at a time: a change during a refresh is shown by one more refresh when it ends.

## Tooltips

Every icon button of the view (repository row, commit arrow, file rows, Changes and Graph headers) shows its name in a small card when the pointer rests on it.

## Limits

The commit message is always typed in the box under the button.
