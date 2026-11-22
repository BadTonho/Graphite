# Draft recovery

Graphite keeps a local draft of modified text files and untitled tabs. When a previous session leaves drafts, choose **Review Drafts** at startup or from the command palette. Select a draft to restore it or explicitly discard it. Choose **Later** to leave it available.

When the original still matches the version the draft was based on, Graphite opens the recovered text as unsaved changes in the original document. If the file changed, disappeared, became read-only or conflicts with open edits, Graphite opens a separate untitled tab. Use **Save** or **Save As** to write the recovered text. Auto Save waits until the recovered changes are explicitly saved or discarded.

The **Files: Draft Recovery** setting (`files.draftRecovery`) is on by default and applies to the user profile. Turning it off stops new snapshots. Existing drafts remain available for review and discard. Drafts and their contents stay in the profile's `draft-recovery/` folder, outside settings sync. Anyone with access to your user files may be able to read them.

On restart Graphite can recover the last completed snapshot. Recent edits may still have been pending when the process stopped. Draft recovery covers text only. Notebook sessions, custom editors, cursor positions and undo history remain outside this feature.
