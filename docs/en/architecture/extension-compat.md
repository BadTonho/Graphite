# VS Code Extension Compatibility (planned)

**Current implementation:** Graphite can search Open VSX and install inspected VSIX packages, showing their manifest and contribution categories. VSIX code does not execute yet. Native Graphite JS/TS plugins use a separate API and Node host; see [Native plugins and VSIX inspection](native-plugins.md). The pinned Code OSS integration below remains required work.

The compatibility target for Phase 5 is Code OSS **1.145.2**, commit `c4453f8ea430795f941c1b2ff6c9321135a74a02`. The target surface is the public `vscode.d.ts` API in `commands`, `workspace`, `window` and `languages`. This target is documented for implementation and testing; no API in this matrix is currently backed by the Code OSS host.

Shared internal services and an isolated Rust simulation host are implemented; see [Services and simulated host](services-host.md). This validates sessions, resource ownership, requests and crash isolation. It does not speak the Code OSS protocol. The Code OSS/Node host described below is still required: pin its version, implement the main-thread bridge and activate a real VSIX before claiming `.vsix` compatibility.

## Approach

Run the **Code OSS extension host** (MIT licensed, `microsoft/vscode`) as a separate **Node.js process**, connected to the Rust editor over RPC:

```
[ Graphite (Rust) ] ←─ RPC bridge ─→ [ Extension Host (Node) ] → [ .vsix extensions ]
```

- **Reused as-is:** upstream `.vsix` loading, activation, the `vscode` module and RPC transport, after the official host is packaged.
- **Phase 5 bridge target:** Rust `MainThread*` implementations for commands, workspace, window and languages, grown against the pinned declarations and tests.
- **Later phases:** webviews, debugging and Test Explorer are outside this Phase 5 target; other public APIs require separate compatibility decisions.
- **Lazy spawn:** Node only starts when an extension is installed and can be stopped when idle.
- **Progress metric:** the most popular OpenVSX extensions form the compatibility test suite.

## Phase 5 API matrix

| `vscode.d.ts` namespace | Phase 5 target | Current status | Required evidence |
|---|---|---|---|
| `commands` | Register/execute commands and observe command lifecycle | Not implemented for VSIX | Local VSIX activation and command invocation fixture |
| `workspace` | Workspace, documents, edits, configuration and events | Not implemented for VSIX | Local VSIX tests for reads, writes, events and failures |
| `window` | Messages, progress, input/quick picks and editor-facing UI | Not implemented for VSIX | Local VSIX tests for request/reply and cancellation |
| `languages` | Providers, diagnostics and language feature registration | Not implemented for VSIX | Local VSIX tests for provider lifecycle and results |

This matrix describes the acceptance target, not current compatibility. Webviews, debugging and Test Explorer remain assigned to Phase 6.

## Extension layers

| Layer | Content | Cost |
|---|---|---|
| 1. Declarative | Themes, icon themes, snippets, grammars | Low — importer |
| 2. Simple commands | Formatters, linters | Medium — host + minimal bridge |
| 3. Heavy UI | Tree views, decorations, webviews | High — on demand |
| 4. Deep API | Debuggers, classic Copilot | Very high — usually has an alternative |

## Marketplace and licensing

- Extension-system code (host, API, RPC, `.vsix` format) is MIT.
- The official Microsoft Marketplace terms restrict use to Microsoft products: **not used**. Distribution goes through **[OpenVSX](https://open-vsx.org)**.
- Some Microsoft extensions (C# Dev Kit, Pylance, Remote-*) are license-locked to VS Code; open alternatives are used instead.

## Risks

Compatibility is never complete: new APIs ship every release and extensions use undocumented behavior. It is offered as *compatibility*, not as a guarantee.

