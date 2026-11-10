# Localization

Graphite ships a compile-time localization system. Every visible interface
string is resolved through a `Translator`, and the translation catalogs are
embedded into the binary, so a running editor never reads locale files from
disk.

## Supported languages

| Language | Code | Catalogs |
| --- | --- | --- |
| English | `en` | Complete (fallback) |
| Portuguese (Brazil) | `pt-BR` | Complete |

English is the source language and the final fallback. Any string missing from
another locale resolves to English before falling back to the raw translation
key.

## Selecting the interface language

The startup language is resolved with this precedence:

1. `GRAPHITE_LANG` environment variable.
2. `workbench.language` in the user settings file.
3. Operating-system locale.
4. English.

`workbench.language` accepts a language tag such as `pt-BR` or `pt`, or the
values `auto` and `system`, which use the operating-system locale. Language tags
are case-insensitive and accept `_` or `-` separators (`pt_br` matches
`pt-BR`). A base language matches a single regional catalog when that match is
unambiguous.

User settings are read from the imported VS Code settings and the Graphite
settings file:

- Windows: `%APPDATA%\Graphite\imported\settings.json` and
  `%APPDATA%\Graphite\settings.json`
- Linux: `$XDG_CONFIG_HOME/Graphite/imported/settings.json` (or
  `~/.config/Graphite/...`) and `$XDG_CONFIG_HOME/Graphite/settings.json`

Example:

```json
{
  "workbench.language": "pt-BR"
}
```

`GRAPHITE_LANG=pt-BR` overrides the setting for a single session. An unknown
value produces a localized warning and falls through to the next source. The
standalone `code` launcher resolves `GRAPHITE_LANG` and then the operating-system
locale for its own diagnostics.

## Catalog layout

All translation sources live in the repository-level `locales/` directory:

```text
locales/
  languages.json
  README.md
  en/
    common.json
    app.json
    commands.json
    terminal.json
    scm.json
    lsp.json
    plugins.json
    accessibility.json
  pt-BR/
    ...same files...
```

`locales/languages.json` declares each compiled-in language with its code,
English name and native name. Each locale directory uses the same file names so
domains stay small and reviewable:

| File | Domain |
| --- | --- |
| `common.json` | Shared actions, language warnings, generic status text |
| `app.json` | Editor UI, overlays, documents, saving, configuration, CLI, JSON schema |
| `commands.json` | Command titles and categories |
| `terminal.json` | Terminal panel, profiles and diagnostics |
| `scm.json` | Source control statuses, diffs and conflicts |
| `lsp.json` | Language server statuses and quick fixes |
| `plugins.json` | Plugins sidebar, runtime and extension host |
| `accessibility.json` | Screen-reader labels |

Files are flat JSON objects mapping stable keys to text. Placeholders use named
braces such as `{path}` or `{error}`; literal braces are written as `{{` and
`}}`. Placeholder names must never be translated.

## Build-time validation

`graphite-i18n/build.rs` validates the catalogs before the crate is compiled. A
build fails when:

- a JSON file is invalid, duplicated or not an object of strings;
- a locale declared in `languages.json` has no directory;
- a key is missing from a locale or exists only outside English;
- a locale uses different placeholders than English.

The validated catalogs are generated into the crate's `OUT_DIR` and embedded in
the binary. This keeps runtime behavior identical on Windows and Linux and
prevents partially translated releases.

## Using translations

The runtime API lives in the `graphite-i18n` crate:

```rust
use graphite_i18n::{Language, Translator};

let translator = Translator::new(Language::parse("pt-BR").unwrap());
assert_eq!(translator.text("common.cancel"), "Cancelar");

let message = translator.format(
    "document.open_failed",
    &[("path", &path.display()), ("error", &error)],
);

let title = translator.text_or("command.myPlugin.command", "My command");
```

Key points:

- `Translator` is `Copy`. Background threads capture it by value and borrow it
  locally instead of sharing a reference across threads.
- `text` falls back to English and then to the key itself; `text_or` accepts an
  explicit English metadata fallback; `has` and `keys` support tooling and
  tests.
- Command titles and categories are looked up by stable keys
  (`command.<id>` and `command.category.<lowercase>`), with the English
  registration metadata as fallback. Plugin commands without catalogs still
  display their declared titles.
- Configuration diagnostics thread `&Translator` into the config layer.
  Terminal exit statuses store the numeric code and are localized when drawn.
- In tests, construct the translator with `Translator::english()` so assertions
  remain deterministic. `App::set_interface_language` switches the UI language
  in tests that verify Portuguese output.

## Scope of localization

Localized surfaces include window chrome, the status bar, editor and search
overlays, panels, command palette entries, keybinding and settings
diagnostics, source control messages, language server statuses and quick
fixes, JSON schema diagnostics, terminal UI, plugin and extension host
statuses, accessibility labels and CLI/lifecycle errors.

Technical text supplied by external tools remains unchanged: OS, Git and
process stderr, language-server response messages, and plugin protocol error
payloads exchanged with extension hosts. Those strings are API-level details;
the surrounding UI labels, statuses and failure wrappers are localized.

## Adding a language

1. Add an entry to `locales/languages.json`.
2. Create `locales/<code>/` with the same eight catalog files.
3. Translate values only; keep keys, placeholder names and escaped braces
   unchanged.
4. Run `cargo test -p graphite-i18n` and `cargo build --workspace`.
5. Update the language table in `locales/README.md` and the documentation index.

The build fails until the new locale defines every English key with identical
placeholders.

## Verification

- `cargo test -p graphite-i18n` checks catalog parity, fallback behavior,
  interpolation and language-tag parsing.
- `cargo test -p graphite-app` exercises the localized UI with English defaults
  and includes tests that switch the interface language for individual cases.
- `cargo test --workspace` and `cargo build --workspace` validate the complete
  workspace on the current platform.
