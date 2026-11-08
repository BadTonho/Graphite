# Asynchronous saving

Saving is coordinated by `DocumentId`. One cycle runs per document; different documents write concurrently. Requests arriving during a cycle share a subsequent cycle, with separate completion tickets and cancellation. A document lease lasts through worker completion and reconciliation. Workers own snapshots and transport data; only the event loop changes documents, history, views and saved baselines.

## Preparation and participants

The coordinator captures the source version, text, path, encoding and line ending. Participants run sequentially: the current LSP instance first, matching simulated-host participants in resource-registration order next, and optional trailing-whitespace trimming last. Each participant has a one-second deadline, checked even without keyboard input. Failures and timeouts appear as save warnings; they do not prevent writing.

Participants edit a staged snapshot, never the live buffer. Host registrations use `RegisterSaveParticipant`, with a unique ID and optional language filter; an empty filter matches all languages. Handles belong to the session and support `Dispose`. `ParticipateInSave` carries the resource, invocation, reason and snapshot; `SaveParticipantResult` must echo the invocation and real source version. An invocation distinguishes staged text from another participant's input without inventing document versions. Parent-linked service calls permit queries only. Independent document edits still use normal transactions.

All returned UTF-16 ranges are validated together, including surrogate boundaries, ordering and overlap. Invalid, stale, disposed and old-session/instance results are ignored with a warning. If live text or format changes while awaiting a participant, its staged work is discarded, the snapshot is refreshed, and the remaining participants continue once. Participants are not automatically retried.

LSP advertises save synchronization capabilities and honors the server's `willSave`, `willSaveWaitUntil`, `save` and `includeText` options. Expiration sends `$/cancelRequest`; late responses cannot apply edits. Successful writes send `didSave` with the actual written text when requested, even if newer live text exists. Failed writes send no `didSave`.

## Writing and reconciliation

Encoding and filesystem writes happen on a worker. Existing links resolve to their targets; files are written through a same-directory temporary file, synchronized and replaced. Existing permissions and file-format behavior are preserved. An atomic commit gate allows cancellation before replacement; after the worker claims the commit, its result must be reconciled even if the requesting view, ticket or host has closed. Temporary files are removed on failure or cancellation.

After success, staged edits and trim become one undoable transaction only if the source revision and format still match and the document remains writable. Unchanged spans retain their position mappings; peer selections, origin history and focus follow normal transaction rules. No-op preparation preserves version, undo and redo.

The saved baseline always becomes the text and format actually written. Concurrent typing and format changes remain intact and dirty when they differ from that baseline. A failed write does not apply preparation or change history or the saved baseline. Cancellation never rolls back a confirmed commit.

## Commands, services and decisions

`workbench.action.files.save` and `workbench.action.files.saveAll` complete after their requested writes and return JSON `null`. Save All reports failures per document and keeps successful writes. Existing shortcuts are preserved; no default Save All shortcut is added. View context exposes `documentSaving` without disabling queued saves.

`SaveService::save(SaveRequest, CancellationToken)` accepts an in-memory document URI and returns a ticket whose JSON result is `SaveOutcome`: document identity, source version, dirty state at reconciliation and warnings. `ServiceRequest::Save` exposes this operation to the simulated host. Untitled documents without a path fail without closing; Save As is outside this change.

Saving before tab/window closure or workspace switching remains pending without blocking input. Failures, intervening edits and additional outstanding writes keep the document or window open and retain the current workspace. There is no automatic retry until typing stops. Discard cancels work before commit and waits for already claimed writes; edits made after the discard decision cancel the transition.

Workspace changes offer save/discard/cancel and preserve file tabs and focus. Discard prepares all reloads before changing any buffer, reloads dirty files from disk and closes dirty untitled views. The target workspace is prepared before discarding. LSP instances are detached only after acceptance, reaped outside the event loop and recreated for retained documents. CLI continuation and `--wait` acknowledgement are retained; cancelling a decision acknowledges handling instead of triggering another window.

The simulation protocol is version 2. User confirmations suspend ordinary RPC deadlines; `Resumed` restores a fresh 30-second deadline after the decision. Save-participant deadlines remain one second regardless of RPC suspension.

## Validation and boundaries

Tests use temporary files, controlled clocks and channels that pause workers before or after commit claim. They cover queues, independent writes, cancellation, dirty/history/format behavior, view mappings, decision barriers, CLI continuation, malformed/stale results and real simulated-host crashes. Save-participant LSP tests use owned mock channels; additional worker tests use a local Rust stdio fixture. Neither requires an external server. See the [regression coverage matrix](../contributing/testing.md).

Build the helper before targeted process tests: `cargo build -p graphite-extension-host`. Its `--save-edit`, `--save-wait`, `--save-crash`, `--save-stale`, `--save-invalid`, `--save-mutate` and `--save-check-lsp` scenarios are test fixtures, not editor options. Run `cargo test --workspace` and `cargo build --workspace` for full validation. Linux execution and interactive Windows/Linux UI/GPU verification remain unperformed in this Windows environment.

Atomicity is per document, not across Save All or filesystem resources. The real Code OSS/Node host, pinned version, real-extension test and `.vsix` compatibility remain required later work.
