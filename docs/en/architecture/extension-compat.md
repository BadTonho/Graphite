# VS Code extension compatibility

Graphite launches the official Code OSS **1.145.2** extension host (commit `c4453f8ea430795f941c1b2ff6c9321135a74a02`) in a separate Node.js 24.22.1 process. The portable Windows and Linux x86-64 packages carry the runtime and license notices. The regular Cargo build supports VSIX inspection and installation without provisioning the runtime.

## Runtime and authorization

The Graphite process exchanges bounded messages over private child-process pipes. The Node adapter uses upstream `PersistentProtocol` and `RPCProtocol` over a per-session local socket or named pipe. It performs the upstream initialization handshake and marshals calls. Rust owns the workbench operations, registry, document versions, diagnostics, prompts and lifecycle.

The Plugins sidebar installs VSIX packages disabled. The approved installation, trusted workspace and active profile determine which VSIX packages are visible to the host. The first eligible palette command starts it. Declared commands appear in the palette before activation. Registered commands, providers, diagnostics and outstanding requests are scoped to the session. Revoking trust, switching workspace or profile, or changing enabled installations ends that session. The process runs with the user's permissions, so approve only code you trust.

## Phase 5 API matrix

| Namespace | Supported workflow |
|---|---|
| `commands` | Activate a declared command on demand, register and unregister commands, run registered extension commands and dispatch Graphite commands. |
| `workspace` | Read workspace folders and documents, observe open/change/save/close events, apply versioned text edits with undo, and read configuration. VSIX configuration updates and their events are stored separately by profile and workspace. |
| `window` | Read the active editor and selections, show text documents, and display messages, progress, input boxes and quick picks. Press Escape to cancel an input or pick. |
| `languages` | Register completion, hover and document formatting providers and publish diagnostics to Problems. The existing suggest and hover commands use matching active providers. Format Document invokes a matching formatter. |

The matrix covers supported workflows on the pinned revision. Extensions that depend on other workbench APIs can receive unsupported-operation errors. Themes, keybindings and other declarative VSIX contributions remain inspectable. Webviews, debugging and Test Explorer belong to Phase 6. Open VSX is the public registry used by Graphite.

See [portable builds](../contributing/building.md), [plugin installation](../contributing/plugins.md) and [v0.5 validation](../features/v0.5-extensions-automation.md).
