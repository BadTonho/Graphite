# User Data Sync

Graphite synchronizes user preferences and environment customization across machines using private GitHub Gists.

## Synchronized Resources

- `settings.json` (key-level 3-way JSON merge)
- `keybindings.json`
- User snippet files under `<profile>/snippets/`
- Profile definitions (`profiles.json`)
- Installed plugin and extension identifiers

## Authentication and Storage

- Uses GitHub OAuth device flow requesting `read:user gist` scope (`graphite_auth::SCOPE_GIST`).
- Stores sync state, last merged base snapshot, Gist ID, and pending conflict entries in `<profile>/user-data-sync.json`.
- Pushes and fetches a single private Gist file (`graphite-sync.json`) via `GitHubGistTransport`.

## Merge Strategy and Local Backups

- Before writing merged changes back to the profile folder, Graphite writes a timestamped snapshot backup under `<profile>/sync-backups/`.
- `settings.json` performs a 3-way merge against the last synchronized base snapshot so disjoint key changes on local and remote machines combine cleanly.
- Keys modified differently on both sides are recorded as explicit conflicts until resolved via `workbench.userDataSync.actions.acceptMerges`.
- `workbench.actions.syncData.reset` clears local sync state and deletes the remote Gist when authenticated.

## Profile menu

The profile button at the bottom of the activity bar opens a menu that now controls the sync, under the GitHub account lines:

- **Turn on settings sync...** (sync off): signs in if needed and starts the first sync.
- With sync on, a status line (`Syncing...`, `Synced 5 min ago`, `Last sync failed` or the number of conflicts to review), **Sync now** and **Turn off settings sync**.
- If the GitHub session was created without the Gist permission, turning sync on asks for it with a new sign-in that keeps the permissions already granted.

## Private repository backend

Besides the Gist, the data can be kept in a **private repository**. In the profile menu, with sync off, the two options are separate: **Turn on sync with a private Gist...** and **Turn on sync with a private repository...**. The status line shows which one is in use.

- **Where:** a repository named `graphite-sync` in the signed-in account, holding one file, `graphite-sync.json`. It is created private (with a first commit) when it does not exist. Every sync is a commit, so the repository keeps the history of the synced data.
- **Permission:** the repository needs the GitHub `repo` scope, which grants access to all of the account's private repositories (GitHub offers no narrower one for a personal token). Graphite only reads and writes the `graphite-sync` repository. If the session lacks the scope, turning the option on asks for it with a new sign-in that keeps the permissions already granted.
- **Never public:** if `graphite-sync` exists and is public, Graphite refuses to read or write it and says so; make it private or delete it.
- **Reset:** *Reset Synced Data* removes `graphite-sync.json` from the repository. The repository itself is never deleted.
- **Switching:** choosing the other backend starts afresh: the stored remote and the merge base of the previous one are forgotten, so the first sync with the new backend compares local data with whatever it already holds. State files written before this existed keep working as a Gist.
- **Failures:** a failed sync is recorded in `<app_config_dir>/logs/errors/medium.jsonl`.
