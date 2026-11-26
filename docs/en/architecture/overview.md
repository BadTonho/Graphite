# Architecture Overview

## Crates

```
graphite-app ──► graphite-render     (wgpu + cosmic-text + winit Window)
           ├─► graphite-text       (ropey buffer)
           ├─► graphite-workspace  (ignore crate)
           └─► graphite-syntax     (tree-sitter)
```

## Module responsibilities

- `graphite-core-api` supplies UI-independent service contracts and serializable data. `graphite-extension-host` supplies a reusable process supervisor and an opt-in Rust simulation peer. LSP uses the same services with its own transport and instance lifecycle. See [Services and simulated host](services-host.md).
- `graphite-app` separates window events, editor input, document operations, drawing, accessibility, theme and LSP/terminal integration under `app/`. Configuration modules separate settings, themes, snippets, paths and VS Code import; LSP modules separate the manager, worker, message dispatch and protocol conversion. Window state is grouped by area instead of kept as loose `App` fields: source control state lives in `ScmState` (`app/source_control/state.rs`), reached as `app.scm`.
- `graphite-text` separates filesystem I/O, edits and history, and cursor navigation. `graphite-workspace` separates project search from folder listing.
- `graphite-render` separates text shaping and glyph rasterization from the GPU surface pipeline. `graphite-syntax` keeps its incremental and full-document highlighters in separate modules.

## Rendering

- **UI stack:** custom UI on `wgpu` (GPU drawing) and `cosmic-text` (shaping, fonts, bidi, fallback), with `winit` for windows and input. Chosen over egui/GPUI/iced for independence, performance control and clean licensing.
- **Pipeline:** one textured-quad pipeline. Rectangles sample a reserved solid 2×2 block of the atlas; glyphs sample an `R8` coverage atlas filled lazily by a shelf packer.
- **Text cache:** shaped lines are cached by `(text, font size)`; glyph images are rasterized once per `CacheKey`.
- **Surface format:** non-sRGB when available, so colors appear as authored.
- **Clipping:** rectangles and glyphs are clipped to editor bounds, with texture coordinates adjusted for partially visible glyphs. The same primitive can constrain future split panes.

## Text and highlighting

- Text lives in a `ropey::Rope`; the cursor is a character offset. Every edit bumps a `revision` counter.
- Documents use stable keys and share text/history across views. Each view exclusively owns selections, scroll, IME composition and selection navigation. Borrowed edit sessions remap peers and synchronize LSP; undo/redo preserve focus. See [Document and view](document-view.md). Native UI/GPU verification is pending on Windows and Linux.
- Visual-line mapping computes wrapped rows and maps Unicode character positions and pointer columns without modifying the document. Rendering, pointer input and vertical cursor movement use this mapping when wrapping is enabled; interactive verification is pending.
- `graphite-syntax` provides incremental tree-sitter parsing over Rope chunks and a full-document highlighter; both return per-line byte-range spans with a semantic `Class`. The app maps classes to colors and re-highlights only when the revision changes.

## Commands

`CommandRegistry` centralizes metadata, explicit targets, enablement and typed execution results. Domain handlers share the same entry point for keyboard, widgets and accessibility. Global, view and widget contexts resolve through their ancestors. Workers return results to the event loop with cancellation and version checks; inactive-view edits preserve focus. See [Commands and context](commands-context.md).

## Event loop

The loop waits for input or for an event from a background thread; it does not poll. A thread that leaves results in a queue the loop reads (file writes, debug adapter messages, web page messages) calls `app::wake::request`, which sends one coalesced wake-up. Extension hosts, tasks, language servers, and notebook kernels send their own events. While background work runs, the loop also checks deadlines every 250 ms; on Linux it pumps GTK every 16 ms while a web page is open, since GTK has no events of its own there.

## Platforms

Windows and Linux; platform differences are delegated to `winit`, `wgpu` and `cosmic-text`.

Never duplicate a TCP connection with `TcpStream::try_clone`. On Windows the duplicate is inheritable, so every process started afterwards (a terminal, a task, a notebook kernel, a command run by Chat) holds the connection, and closing it in Graphite does not end it. Read and write through `&TcpStream`, which implements `Read` and `Write`, and share a connection between threads with `Arc<TcpStream>`. Sockets returned by `TcpListener::accept` and files duplicated with `File::try_clone` are not inheritable.

## Known limitations

- Incremental highlighting can invalidate the remainder of a document after an edit.
- The glyph cache is never evicted.
- Color glyphs (emoji) render as alpha masks.
- Interactive verification of the window and GPU recovery is still pending on Windows and Linux.

