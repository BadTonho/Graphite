# Editor Core Contracts

This document proposes three contracts: **document/view**, **commands and context**, and the **extension boundary**. The target behavior and acceptance criteria are design decisions for future implementation, not claims about current capabilities. The type examples are illustrative, not final Rust APIs. Formalizing these contracts does not require implementing the entire VS Code architecture at once.

## 1. Document and view

### Current state

`Tab` in `crates/graphite-app/src/main.rs` combines document and presentation state. `Buffer` in `crates/graphite-text/src/lib.rs` owns the active selections and text history. `View` in `crates/graphite-app/src/app/editor_groups.rs` stores per-group selections and scroll; snapshots and restores synchronize the active view with the buffer, and position edits remap other views. The existing `app_views_share_text_and_history_with_independent_selections` test exercises independent selections, shared text and undo. The risk is that mutation paths must invoke this synchronization consistently, rather than ownership enforcing it.

`Buffer::apply_transaction` already validates disjoint replacements, and effective replacements, undo and redo update its revision in `crates/graphite-text/src/edit.rs`. These are useful foundations, not proof that every future document operation satisfies the proposed contract.

`graphite-text` also provides pre-validation and an in-memory multi-buffer transaction with optional expected revisions. It does not yet integrate with the editor's workspace edits or provide disk atomicity; the migration below remains necessary.

The quick-fix path pre-validates text edits, rejects unsupported resource operations and invalid UTF-16 positions, and checks supplied LSP document versions. It still writes closed files separately, so its workspace edits are not yet atomic across disk writes and open documents.

### Target contract

- A document has a stable identity, text, URI or untitled identity, language, monotonically increasing version, saved state and undo history. One document can have multiple views; a view owns its selections, scroll, focus and other presentation state. Closing a view keeps the document alive while another view refers to it.
- A text transaction is one logical change, potentially containing multiple non-overlapping replacements against the same revision. An effective transaction increments that document's version exactly once; a no-op does not. Undo and redo that change text each produce a new version. All views observe the same text and version, and their selections are remapped through the change, including undo and redo.
- Dirty state reflects whether current content and relevant save metadata match the last successful save, not merely whether version numbers match: undo can restore saved content while advancing the version. A failed save does not update the saved baseline.
- An edit with an expected version is rejected if the document no longer has that version. For a multi-document text edit, validate all identities, expected versions, permissions and ranges before committing; either every text change commits or none does. Each changed document advances once and records an undoable transaction. This initial atomicity guarantee excludes file operations and persistence; those must not be advertised as part of the atomic operation.
- LSP synchronization uses document identity and version rather than tab identity. The active view's post-edit selection is defined by the operation; other views are remapped from their previous positions.

### Acceptance criteria

1. Two views of one document retain independent selections and scroll across focus changes, edits, undo and redo; both see the same text.
2. Effective, no-op, undo and redo transactions follow the version rules; undoing to saved content clears dirty state without decreasing the version.
3. A stale version, invalid range or read-only target rejects an entire multi-document text edit without changing any document, version or history.
4. Closing one view preserves the document and its history while another view remains.

### Incremental migration

Introduce stable document and view identities, then move selection ownership from `Buffer` to views and pass selection state explicitly to editing operations. Route all text changes through document transactions and centralize view remapping. Migrate existing callers and tests incrementally; retain adapters for the current `Tab` structure until no longer needed.

## 2. Commands and context

### Current state

`execute_custom_command` in `crates/graphite-app/src/app/commands.rs` dispatches through a large match and mutates the app directly. The keybinding handler constructs context values locally; `crates/graphite-app/src/keybindings/when.rs` provides expression parsing for keybinding resolution. The quick-fix code handles `CodeActionOrCommand` in `crates/graphite-app/src/app/editor_commands.rs`, but a unified command registry and general asynchronous command contract are not established by these paths.

### Target contract

- A registry associates a stable command ID with metadata, optional enablement and a handler. Use VS Code-compatible IDs where a corresponding command has compatible semantics. Arguments and results crossing a process boundary are serializable; handlers receive an explicit service context instead of unrestricted app ownership.
- One execution entry point accepts an ID, arguments, target context and cancellation token, and asynchronously returns a result or a typed error. Cancellation is cooperative and must not imply rollback of effects already committed. Unknown, disabled, failed and cancelled commands are reported without crashing or blocking the UI.
- A global context supplies shared keys; each target view or widget has a scoped context. Resolve a key from the target scope first, then the global scope. Focus and explicit targets determine which scope is evaluated; invocations without a target use the global scope.
- Keybinding `when` expressions control binding eligibility; menu conditions control visibility; command enablement controls whether execution is permitted. They share context evaluation but remain distinct conditions. The registry checks enablement centrally even for programmatic calls.
- Keybindings, menus, palette, extensions and tests enter through the registry. Adapt LSP commands according to their protocol: commands intended for a language server use its `workspace/executeCommand` request; only client-owned commands route to the local registry when appropriate. Text changes use document transactions and undo.

### Acceptance criteria

1. A command invoked by a keybinding and programmatically reaches the same handler and observes the intended target scope; a local key overrides the global key.
2. A hidden menu item or ineligible binding does not itself disable a command; a disabled command is rejected even when invoked directly.
3. Unknown IDs, handler errors and cancellation are reported, and a pending asynchronous handler does not freeze the UI. Server-directed LSP commands remain server-directed.
4. A command that edits text produces the same version, view-mapping and undo behavior as another document transaction.

### Incremental migration

Add the registry and scoped context service, extract a small set of existing commands into handlers, and route calls through the registry with a temporary legacy fallback. Migrate remaining commands and remove the fallback; use registry metadata for a future command palette.

## 3. Extension and protocol boundary

### Current state

`docs/en/architecture/extension-compat.md` describes a planned Code OSS extension host in a separate Node process and staged compatibility. The LSP client in `crates/graphite-app/src/lsp.rs` and its UI integration already use worker messages and event-loop wakeups. This is a useful concurrency example, but does not establish a shared versioned extension protocol or a reusable extension-host dispatcher.

### Target contract

- Define reusable internal document, command, workspace/configuration and window services, along with explicit messages, identity, versions, cancellation and lifecycle. Pin the Code OSS release or commit targeted by the extension bridge; add further services as compatibility layers require them.
- Keep protocol adapters distinct: LSP and the extension host have different message semantics, directions, transports and lifecycles. They may call the same internal services, but need not share a transport or dispatcher. A protocol crate and a supervised plugin-host crate are proposed boundaries, subject to the selected host's actual protocol.
- Extensions and workers never mutate UI state directly. Deliver results to the owning event loop; do not block the UI waiting for RPC. Track requests, cancellation, deadlines, backpressure, host shutdown and restart, and release host-owned registrations and pending requests on exit.
- Define URI and filesystem abstractions with local files first. Versioned document changes and text edits pass through the document contract; translate positions and protocol-specific guarantees at each boundary. Resource operations and disk persistence are outside the initial multi-document text atomicity guarantee.
- Save participants run asynchronously in a bounded sequence. Associate each participant's returned edits with the document version it observed and reject them if that version is stale. On failure, timeout or stale edits, report the participant failure and continue saving the current content; never block the UI. A successful save updates the baseline to the content actually written: edits made while the write is pending remain dirty. Deliver protocol-appropriate before- and after-save events, including `willSave`, `willSaveWaitUntil` and `didSave` where supported.

### Acceptance criteria

1. Versioned LSP changes and extension edits reach the same document invariants through their respective adapters; a stale edit is rejected without modifying text.
2. A save participant returning after intervening typing cannot overwrite it; timeout and failure are reported, the current content can still be saved, and the UI remains responsive.
3. Typing during an outstanding save leaves the newer content dirty after the earlier snapshot is written; a failed write leaves the previous saved baseline intact.
4. Host termination fails pending requests and releases its registrations without terminating the editor; LSP and host lifecycles remain independent.

### Incremental migration

Specify document, command, workspace/configuration and window service interfaces first. Adapt existing LSP flows as acceptance cases for the shared services, retaining LSP-specific dispatch and wire behavior. Introduce the host supervisor and its versioned bridge separately, then add compatibility layers incrementally.

The real Code OSS host remains a required later step: pin a release or commit, integrate the actual Node host, and validate it with a real extension before claiming compatibility.

## How the contracts interact

Typing submits a transaction for the focused view, advances the document version and sends the corresponding LSP change. Save resolves through the registry, runs bounded participants and applies only edits whose expected versions still match; it writes a snapshot and updates the saved baseline only on success. Each affected view is remapped. An extension command uses the registry and scoped context, while a language-server command is sent to the server when the LSP contract requires it.

## Recommended order

1. Document identity, transactions and view ownership.
2. Command registry and scoped context, migrated incrementally.
3. Shared service contracts and separate LSP and extension-host adapters, followed by compatibility layers.

Each stage should add tests for its invariants while keeping the application usable throughout migration.
