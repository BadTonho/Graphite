# Editor Core Contracts

This document describes the implemented **document/view**, **commands and context**, **internal services**, **simulated extension boundary** and **asynchronous saving**. The real Code OSS host remains future work.

## 1. Document and view

### Current state

`Document` owns text, language, format and document caches. `DocumentStore` uses immutable keys independent of document data; there is no mutable slice or positional access. Groups own tab order and each `View` owns its selections, scroll, IME composition and smart-selection navigation state. The active document is derived from the focused view.

`Buffer` contains text and history without an active cursor. `Buffer::edit` borrows a view's `SelectionState` and `ViewId`; text operations commit transactions and expose position mappings through `TransactionOutcome`. Application edit sessions remap peer views and publish document versions to LSP and schema validation. Undo/redo restore the originating view's selection snapshot when it still exists, remap the other views and retain the current focus. Snapshot/restore synchronization adapters have been removed.

Prepared workspace edits retain their open documents and revalidate identities, versions, ranges, permissions and closed-file snapshots before committing. No-op closed targets do not create tabs. Atomicity covers text and history in memory; file operations and disk persistence remain outside this guarantee. See [Document and view implementation](document-view.md) for lifecycle and validation details.

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

The document/view migration is complete. Editing callers use borrowed view state, stable document keys and centralized view remapping. Regression tests cover shared views, history ownership, no-ops, pending-operation retention and multi-document failure cases. Native UI/GPU verification remains pending on Windows and Linux.

## 2. Commands and context

### Current state

`CommandRegistry` describes every local command, with handlers separated by domain. Keyboard, semantic widget actions, accessibility and programmatic calls use `App::execute_command`, validating arguments, explicit targets and enablement before dispatch. Requests and results are serializable; tickets track synchronous completion, cooperative cancellation, worker results and pending confirmations. `ContextKeyService` resolves global, view and widget scopes and refreshes without requiring a keyboard event. The legacy dispatcher and fallback have been removed. See [Commands and context](commands-context.md) for the implemented API, lifecycle and tests.

### Target contract

- A registry associates a stable command ID with metadata, enablement and a handler. Existing IDs and shortcuts are preserved. Arguments and results are serializable. Built-in handlers borrow the event-loop-owned application with an explicit invocation target; external adapters use the shared service contracts. Session-owned host commands participate in the same execution API.
- One execution entry point accepts an ID, arguments, target context and cancellation token, and asynchronously returns a result or a typed error. Cancellation is cooperative and must not imply rollback of effects already committed. Unknown, disabled, failed and cancelled commands are reported without crashing or blocking the UI.
- A global context supplies shared keys; each target view or widget has a scoped context. Resolve a key from the target scope first, then the global scope. Focus and explicit targets determine which scope is evaluated; invocations without a target use the global scope.
- Keybinding `when` expressions control binding eligibility; menu conditions control visibility; command enablement controls whether execution is permitted. They share context evaluation but remain distinct conditions. The registry checks enablement centrally even for programmatic calls.
- Existing command sources enter through the registry; future menu, palette and extension adapters will use the same API. Raw text input and terminal transport retain their input paths. Server-directed commands use `workspace/executeCommand`; unknown local IDs are never forwarded automatically. Text changes use document transactions and undo. Explicit inactive-view edits and undo/redo preserve focus.

### Acceptance criteria

1. A command invoked by a keybinding and programmatically reaches the same handler and observes the intended target scope; a local key overrides the global key.
2. A hidden menu item or ineligible binding does not itself disable a command; a disabled command is rejected even when invoked directly.
3. Unknown IDs, handler errors and cancellation are reported, and a pending asynchronous handler does not freeze the UI. Server-directed LSP commands remain server-directed.
4. A command that edits text produces the same version, view-mapping and undo behavior as another document transaction.

### Incremental migration

The command migration is complete, including all former dispatcher IDs, default bindings and JSON arguments. Workers return owned results to the event loop, which revalidates cancellation, target lifetime, workspace and document versions before applying them. Save and Save All now complete asynchronously after writing. Shared service interfaces and session-owned commands are described below; a command palette remains future work.

## 3. Extension and protocol boundary

### Current state

`graphite-core-api` defines independent service contracts and serializable data; the application implements them on its event loop. `graphite-extension-host` provides a separate Rust simulation process and reusable supervisor with versioned framing, sessions, owned registrations, cancellation, deadlines and manual restart. Host failure releases its resources while editing and LSP continue. See [Internal services and simulated extension host](services-host.md).

LSP publication and edits use the document services; workspace queries and messages use workspace/window services. Server instances identify events, and pending requests have limits, cancellation, deadlines and shutdown cleanup. Incoming edits cancelled before UI claim cannot apply; claimed edits finish atomically. The simulated boundary and save participants are implemented; the real Code OSS host remains a separate stage.

### Target contract

- Reusable document, command, workspace/configuration and window services expose explicit messages, identity, versions, cancellation and lifecycle. The later real bridge must pin its Code OSS release or commit and add services required by actual compatibility layers.
- Keep protocol adapters distinct: LSP and the simulated host have separate transports and lifecycles while calling shared services. The simulation protocol does not claim compatibility with the eventual Code OSS wire protocol.
- Extensions and workers never mutate UI state directly. Deliver results to the owning event loop; do not block the UI waiting for RPC. Track requests, cancellation, deadlines, backpressure, host shutdown and restart, and release host-owned registrations and pending requests on exit.
- Define URI and filesystem abstractions with local files first. Versioned document changes and text edits pass through the document contract; translate positions and protocol-specific guarantees at each boundary. Resource operations and disk persistence are outside the initial multi-document text atomicity guarantee.
- [Asynchronous saving](async-saving.md) runs sequential participants with one-second deadlines, rejects stale edits and writes snapshots off the event loop. Failed writes preserve the saved baseline and preparation is committed only after success against an unchanged source. Newer text and format changes remain dirty.

### Acceptance criteria

1. Versioned LSP changes and extension edits reach the same document invariants through their respective adapters; a stale edit is rejected without modifying text.
2. A save participant returning after intervening typing cannot overwrite it; timeout and failure are reported, the current content can still be saved, and the UI remains responsive.
3. Typing during an outstanding save leaves the newer content dirty after the earlier snapshot is written; a failed write leaves the previous saved baseline intact.
4. Host termination fails pending requests and releases its registrations without terminating the editor; LSP and host lifecycles remain independent.

### Incremental migration

Service interfaces, the LSP adapter, simulated host supervisor and asynchronous save coordinator are implemented. Native tests exercise protocol failures, real process crash/restart, resource cleanup, independent LSP operation and save/close/workspace races. Integration of the actual Code OSS host remains pending.

The real Code OSS host remains a required later step: pin a release or commit, integrate the actual Node host, and validate it with a real extension before claiming compatibility.

## How the contracts interact

Typing submits a transaction for the focused view, advances the document version and sends the corresponding LSP change. Save resolves through the registry, runs bounded participants and writes a snapshot asynchronously, updating the saved baseline only on success. Prepared edits remap affected views in one transaction when their source remains current. Session-owned extension commands use the registry and scoped context; language-server commands use the server transport required by LSP.

## Recommended order

1. Document identity, transactions and view ownership.
2. Command registry and scoped context, migrated incrementally.
3. Shared service contracts and separate LSP and extension-host adapters, followed by compatibility layers.

Each stage should add tests for its invariants while keeping the application usable throughout migration.
