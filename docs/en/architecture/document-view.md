# Document and view

## Ownership and identity

`Document` owns a `graphite-text::Buffer`, language, encoding, line ending, indentation and document-derived caches. The buffer owns the Rope, revision, saved-text baseline and shared undo/redo history. It has no active cursor or selection.

`DocumentStore` owns documents under `DocumentId` keys. Keys are separate from mutable document data, so replacing data cannot invalidate identity lookup. Access and removal use IDs; collection positions are not document identities. Groups own tab order. The active document always comes from the focused view.

Each `View` owns a stable `ViewId`, `SelectionState`, scroll, IME composition and smart-selection navigation state. Splitting copies selections and scroll into a new view identity and starts fresh transient input state. Focus changes do not copy presentation state into the document. Rendering, accessibility, navigation, diagnostics and LSP resolve documents by ID.

## Editing and history

`Buffer::edit(&mut SelectionState, ViewId)` returns a borrowed `Editor`. Its navigation and editing operations update the supplied view state. `EditorRead` borrows a document and selection state for queries. `App::editor_mut` creates an edit session that publishes every committed position mapping to peer views when the operation finishes, and synchronizes the resulting document revision with LSP and schema validation.

Transactions validate disjoint character ranges before applying text. `TransactionOutcome` exposes the resulting revision, character length, effective-change flag, position edits and optional history selection restoration. `apply_document_transactions` returns one outcome per target. Rejected operations leave text, selections, history and revision untouched. No-ops leave the revision and redo history unchanged. Effective transactions, undo and redo each advance the revision once.

History entries optionally record the originating `ViewId` and its before/after selection state. Undo/redo restore that view's selections if it exists and remap every other view, including the invoking view. Focus and scroll remain where the user left them. If the origin has closed, all existing views are remapped; undo does not recreate it.

Search replacement records each match as a constituent edit, so a peer cursor between matches follows the correct offset. Clipboard, IME, multicursor, completion, line commands, accessibility and quick fixes use the same transaction and mapping rules.

Dirty state compares current text and save format with the last successful save. Undoing to saved text clears text dirty state while advancing the revision. A failed save preserves the baseline. File reload uses a transaction and keeps revisions monotonic; an effective reload clears obsolete text history.

## Lifetime and workspace edits

Closing a shared view retains the document and history. Closing the last dirty view still requires the existing save/discard/cancel decision. Closed-tab history keeps the last view's selections and scroll separately from the document; reopening preserves document identity and history and assigns a new view identity.

Prepared workspace edits hold document leases. Closing the last view during a pending operation keeps its document in the store until that operation releases the lease. An effective pending edit publishes its dirty result in a background view. Other closed documents are detached into closed-tab history; the event loop also collects released leases. Discarding a prepared edit releases its leases without editing text. Reopening a file that already has a live document activates it instead of creating a duplicate.

Preparation resolves canonical local paths and UTF-16 ranges. Commit rechecks stable identities, expected revisions, paths, read-only state, write permissions and snapshots of previously closed files before changing any target. A changed, unreadable or newly opened closed-file target rejects the batch. Effective closed-file targets become dirty background documents; no-op targets create no document or view. Files are not written by workspace edits.

Atomicity covers text and undo history in memory. Disk persistence and file resource operations are outside this guarantee. Multi-document undo remains per document.

## Validation and subsequent work

The [regression coverage matrix](../contributing/testing.md) maps these invariants to tests, including combined multicursor/IME/history scenarios and rejection checks for views, subscriptions, leases and disk state.

Native tests use isolated temporary data and simulated clipboard/LSP facilities. They cover independent views, IME, multicursor, clipboard, search, completion, line commands, accessibility, history origins, no-ops, monotonic reload versions, save metadata, close/reopen, lease release and multi-document failures.

Run `cargo test --workspace` and `cargo build --workspace` on the available platform. Interactive native UI/GPU verification remains pending on Windows and Linux; running Linux tests in the same Windows development session is not required.

The [command registry](commands-context.md), [simulated extension-host boundary](services-host.md) and [asynchronous save coordinator](async-saving.md) build on these document invariants. The real Code OSS host, a pinned version, a real extension test and `.vsix` compatibility remain required future work.
