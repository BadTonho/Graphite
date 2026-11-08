# VS Code Extension Compatibility (planned)

Compatibility with `.vsix` extensions is a **core goal** and shapes the architecture from the start.

## Approach

Run the **Code OSS extension host** (MIT licensed, `microsoft/vscode`) as a separate **Node.js process**, connected to the Rust editor over RPC:

```
[ Graphite (Rust) ] ←─ RPC bridge ─→ [ Extension Host (Node) ] → [ .vsix extensions ]
```

- **Reused as-is:** `.vsix` loading, activation, the `vscode` module, RPC transport.
- **Implemented in Rust:** the `MainThread*` side (documents, commands, workspace, window, languages, tree views, decorations, webviews, SCM, debug), grown on demand.
- **Lazy spawn:** Node only starts when an extension is installed and can be stopped when idle.
- **Progress metric:** the most popular OpenVSX extensions form the compatibility test suite.

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

