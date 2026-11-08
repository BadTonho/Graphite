# Internal services and simulated extension host

## Boundaries

`graphite-core-api` defines document identity, validated URIs, snapshots, UTF-16 edits, command requests/results, cancellation, errors and document/command/workspace/window service interfaces. It depends on serialization and URI handling, without GPU, application or LSP types. Event-loop implementations live under `graphite-app/app/services/`. Adapters convert their own protocol types at the boundary.

Document snapshots read open documents from memory, including dirty text; closed files use the local filesystem provider and existing UTF-8/UTF-16 decoding. `file:` URIs round-trip OS paths without lossy string conversion; `untitled:` identifies unsaved documents. Unsupported schemes and paths that cannot be represented produce errors. Snapshots of closed files do not create documents or views.

Versioned edits reuse preparation, document leases and atomic in-memory commit. All targets are validated before publication or mutation. Existing workspace boundaries, read-only checks, saved formats, no-op behavior and undo remain in force. UTF-16 surrogate halves and invalid ranges are rejected. Nested host service calls carry their parent execution ID so edits use that execution's view for selection history without changing focus. Disk writes remain separate and use the [asynchronous save coordinator](async-saving.md).

Document subscriptions emit opened, changed and closed snapshots. Borrowed edit sessions publish changes; reconciliation catches opening, collection and metadata changes. Unchanged documents are not copied on every event-loop turn. Each subscription has a bounded queue; overflow is reported, and dropping a subscription releases its ownership. A retained document is not closed merely because its last view disappears.

Workspace configuration combines existing settings layers with effective editor preferences. Window services open documents, explicitly focus views and display messages. Commands use the item 2 execution API. Built-in handlers remain application-owned; protocol adapters cannot mutate application state from worker threads.

## Process and protocol

`graphite-extension-host` contains a reusable supervisor and an executable simulation peer. It is disabled by default and runs as a separate Rust process, hidden on Windows. It does not load extensions, Node or Code OSS.

The simulation protocol is version 2: JSON messages framed by a four-byte big-endian byte count over stdin/stdout. Stderr is reserved for logs. Handshake messages establish a fresh session ID; requests correlate session and request IDs, separately in each direction. Old-session and duplicate responses are ignored; invalid framing or incompatible messages stop that session.

The handshake deadline is five seconds. Ordinary pending requests have a 30-second deadline and a maximum of 128 entries per direction; frames are limited to 16 MiB. Queues are bounded and return typed errors on saturation. The application caps session-owned registrations at 1,024. Cancellation is cooperative and cannot roll back confirmed edits or filesystem/Git effects. Calls waiting for an existing user confirmation suspend their RPC deadline and retain cancellation support.

Host commands have English metadata, target kinds and enablement conditions. Their registrations cannot shadow built-ins or duplicate another command. Registration and subscription handles belong to one session and support explicit disposal. The central command entry point resolves registered host commands; unknown local IDs are never sent automatically to LSP.

Workers own transport data and wake the editor. Only the event loop applies service requests and results, after checking session, cancellation, target lifetime and document versions. The loop schedules deadline checks while work is pending and can sleep while idle. Process exit, EOF, protocol failure or manual restart fails pending executions and releases commands and subscriptions. Child reaping happens outside the event loop. Restart is manual and creates a fresh session; it does not replay resources.

## LSP integration

LSP retains its own transport, framing and server lifecycle. Its document publication and workspace edits use the same service contracts. Each server instance tags its events, so an old instance cannot publish diagnostics, apply edits or terminate a replacement instance's requests.

Outgoing requests have a 30-second deadline, a limit of 128 outstanding entries, validated responses and exactly one completion. Expiration sends `$/cancelRequest`; transport failure and shutdown fail pending requests. Initialization retains its absolute 20-second deadline. Incoming requests retain the existing 64-entry limit and 30-second deadline; cancellation or expiry before event-loop claim prevents application, while an edit already claimed completes atomically.

`workspace/configuration` and `workspace/workspaceFolders` query workspace services. Window messages use the window service; `window/showMessageRequest` displays the message and returns a dismissed (`null`) choice. Quick-fix server commands remain `workspace/executeCommand`. The host supervisor and LSP manager do not shut down each other.

## Development and validation

The local `graphite-test-lsp` fixture exercises the production LSP worker over stdin/stdout, including initialization, JSON results, applyEdit service transactions, malformed responses, cancellation, late replies and EOF with pending requests. See [Regression testing](../contributing/testing.md) for commands and the complete coverage matrix. Channel-based tests additionally verify old-instance rejection and controlled deadlines.

After `cargo build --workspace`, run:

```bash
cargo run -p graphite-app -- --simulate-extension-host <folder>
```

The helper executable is located beside Graphite. For targeted application tests involving the real helper process, build it first with `cargo build -p graphite-extension-host`; workspace tests build it through the process integration test target. No external language server or network service is required.

The simulator registers `shoppe.simulated.echo`, `shoppe.simulated.wait`, `shoppe.simulated.crash` and `shoppe.simulated.service`. Echo returns JSON, wait exercises cancellation/deadlines, crash exercises isolation, and service invokes the editor's service request contract. Restart with `shoppe.extensions.restartHost`, for example through a custom binding:

```json
{"key":"ctrl+alt+r","command":"shoppe.extensions.restartHost","when":"extensionHostEnabled"}
```

Tests cover snapshots, subscriptions and overflow, mixed open/closed/untitled atomic edits, permissions, UTF-16, metadata/registration errors, real-process handshake and crash/restart, cancellation and deadlines with controlled time, stale messages, inactive-view history, pending user decisions, LSP survival and stale server instances. Native automated validation is performed on Windows; Linux execution and interactive UI/GPU checks remain unperformed in this environment.

Session resources also include save participants. `ServiceRequest::Save` returns a save ticket result, and parent-linked participant calls permit queries only. Participants have independent one-second deadlines; disposing a resource or ending a session invalidates its pending result. User decisions suspend ordinary RPC deadlines and `Resumed` restarts them after the decision. See [Asynchronous saving](async-saving.md) for staging, write and lifecycle contracts.

The real Code OSS host remains a required later step: pin a release or commit, integrate its actual Node host and validate a real extension before claiming `.vsix` compatibility.
