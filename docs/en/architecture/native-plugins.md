# Native plugins and VSIX inspection

Graphite API v1 plugins run as JavaScript in one system Node.js 24+ process per plugin. TypeScript is compiled before packaging. `graphite-plugins` owns manifests, ZIP validation, installation, scaffolding and GitHub discovery without depending on GPU/application types. The application owns plugin sidebar state and session routing; its existing document and command services apply changes on the event loop.

## Runtime boundary

The bundled CommonJS host loads the plugin's JavaScript entry and calls `activate(context, graphite)`. Both ES modules and CommonJS exports are supported. `deactivate()` is optional. The host reuses protocol v2 framing, handshake, bounded queues, deadlines, cancellation and resource disposal from [Services and simulated host](services-host.md). This is Graphite RPC, not the Code OSS wire protocol. Stdout is reserved for framed messages; console output is redirected to stderr.

Each process owns its supervisor, commands, document subscriptions, request IDs and session. The application routes invocations to the owning plugin and keeps its simulator separate. Commands must match the manifest and cannot shadow built-ins. API v1 exposes commands, document queries/events/versioned edits, workspace/configuration queries and messages. It does not expose arbitrary panels, webviews, save participants, command execution, file saving or view focus. Document changes retain UTF-16 range checks, read-only checks, atomic edits and undo.

Disabling, replacing or removing a plugin clears its registrations and pending work without stopping other hosts or LSP. Manual restart creates a new session. There is no automatic crash restart. Normal shutdown gives `deactivate()` a bounded opportunity to run before terminating the process. A process boundary contains crashes; it does not sandbox malicious Node code. Enable only trusted code.

## Packages and persistent state

Native packages are ZIP files ending in `.graphite-plugin`, with `graphite-plugin.json` at their root. VSIX packages are read through `extension/package.json`. Installation never runs npm, lifecycle scripts or package code. VSIX remains metadata only: no entry point or contributions are activated, and it cannot be enabled.

The user configuration directory contains `plugins/`, per-plugin JSON records and immutable version directories. Installation validates and extracts into a fresh staging directory, then atomically replaces the registry record. A failed publication retains the previous record and package. Previous directories remain available to live hosts until removal; removal matches an exact plugin identity boundary. Every successful installation starts disabled with no trusted version, including updates and reinstalls. Trust is recorded for the installed version.

Packages/downloads are limited to 100 MiB; archives to 500 MiB expanded and 10,000 entries. Absolute/traversing paths, Windows aliases, duplicate paths, case collisions, file/directory collisions, symlinks and special files are rejected. Failed extraction removes staging data. Packing excludes hidden files and `target`, rejects sensitive key/database file extensions and links, and requires a valid JavaScript entry.

## Discovery and UI

The Plugins sidebar opens with `Ctrl+Shift+X` or `graphite.plugins.open`. It reuses the 260 px workbench sidebar next to the activity bar, so the editor remains visible. The sidebar owns keyboard input only while focused; the editor, Quick Open, find, project search, Source Control, the terminal and confirmation surfaces keep precedence otherwise. Filesystem, network and Node startup checks run on workers. The catalog searches public GitHub repositories using `topic:graphite-plugin`, paginates 30 results per page, caches searches for 60 seconds and honors rate-limit/retry headers. It uses no GitHub credentials.

Selecting a repository fetches its latest stable release, then lists `.graphite-plugin` and `.vsix` assets. Downloads use the local package validator/installer. The user reviews package identity/version before installation and separately grants trust before executing native code. Updates are manually selected from releases; there is no background update or repository cloning. Offline local installation remains available.

Tests use isolated directories, a local HTTP server and real Node fixtures. See [Plugin development](../contributing/plugins.md) for the manifest, SDK and controls.
