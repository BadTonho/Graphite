# Commands and context

## Registry and entry point

`app/command_registry.rs` owns command descriptions: stable ID, English title, category, accepted target kinds, argument schema, enablement, optional menu visibility and handler. Duplicate IDs and malformed conditions are rejected. Domain handlers live under `app/command_handlers/`; `app/commands.rs` only adapts shortcuts and contains shortcut regressions. All 106 former dispatcher IDs and every default binding are registered. There is no legacy fallback.

`App::execute_command` accepts a serializable `CommandRequest` and returns a ticket with an execution ID, cancellation token and nonblocking result receiver. An omitted target means global context. UI adapters specify a view or widget instance explicitly. For example:

```json
{"id":"type","arguments":{"text":"hello"},"target":{"kind":"view","id":42}}
```

Results are JSON values, with `null` for commands without return data. Language queries return their completion items, location, hover text or code actions; server commands return the server's JSON result. Errors distinguish unknown commands, invalid arguments, invalid or closed targets, disabled commands, cancellation, execution failure and stale documents. The executor validates cancellation, target, arguments and enablement before invoking a handler or dismissing suggestions. Custom keybindings preserve their JSON `args` through resolution.

Editor handlers borrow the explicit target view's selection state and use the document transactions described in [Document and view](document-view.md). Execution does not temporarily change focus. Commands whose purpose is navigation, opening an editor or focusing a group can change focus. Undo/redo retain the current focus and the existing history-origin rules.

## Context scopes

Resolution proceeds from widget instance to its owning view and then global values, or directly from a global widget to global values. Local keys override ancestors. Widget IDs identify an appearance of a widget; closing and reopening creates a new ID. Closed views and widgets lose their scopes and pending work. Context is refreshed at event-loop boundaries, before shortcut resolution and before and after command execution, including programmatic calls without keyboard input.

Values remain strings compatible with the existing expression evaluator. Keys describe focus, document availability, path, permissions, selections, undo/redo, widget visibility and contents, pending commands and diff loading. Editing enablement uses target document permissions independently of physical focus. Widget commands validate both the instance and its kind.

Three conditions remain independent: shortcut `when` selects eligible bindings, `menu_when` controls menu visibility, and command enablement authorizes execution. The menu predicate is an API for consumers; this change does not add a command palette or menu UI. Hidden items and ineligible shortcuts do not prohibit an otherwise enabled programmatic call.

## Pending execution and workers

Pending executions are keyed by ID. Workers own input snapshots and cooperative cancellation tokens and send results to the event loop, which alone applies document or UI changes. Search query generations also reject superseded results. Opening a search or snippet widget completes when it opens; a command requesting a query, Git operation, diff or LSP response completes when that work returns. Dirty-close and window-close commands remain pending while the existing save/discard/cancel confirmation is displayed.

Before applying a result, the event loop rechecks cancellation, target lifetime, workspace and relevant document revisions. Checkout additionally validates its entire open-document snapshot, preserving text typed while Git runs. A closed target cancels associated work; late results cannot revive it. Cancellation does not roll back effects already committed to memory, the filesystem or Git. Native calls already running may finish before the worker observes cancellation. Finished executions release their pending records and result senders.

LSP jobs correlate UI execution IDs with protocol request IDs. Cancellation sends `$/cancelRequest`; invalid responses, disconnected transports and server shutdown finish tickets with errors. Quick fixes containing server commands remain on `workspace/executeCommand`, including their final server result. Unknown local IDs are never forwarded automatically. LSP keeps its separate transport and lifecycle.

## Validation and boundaries

Native tests freeze the former command inventory and check default bindings, metadata, duplicates, JSON arguments/results, errors, scope precedence and cleanup, distinct predicates, inactive targets, read-only rejection without effects, transaction regressions, confirmations, simulated workers and simulated LSP responses/cancellation/shutdown. Tests use isolated data without external language servers.

This stage preserves synchronous saving. Item 3 now provides [shared services and a simulated extension host](services-host.md), including session-owned commands in this execution API; asynchronous save coordination and participants remain item 4. Windows native tests/build are required for this implementation; Linux execution and interactive UI/GPU checks remain unperformed in this Windows environment.
