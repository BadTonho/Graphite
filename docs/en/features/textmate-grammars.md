# TextMate grammars from extensions

Graphite highlights many languages with built-in tree-sitter grammars. For every other file, an enabled extension can supply a **TextMate grammar** (`contributes.grammars`), and Graphite colors the file with it.

## How a file gets a grammar

1. The extension declares the language in `contributes.languages`: an `id` and the file name endings (`extensions`, such as `.toy`) or whole file names (`filenames`, such as `Toyfile`) it covers. Matching ignores case, and the longest ending wins (`.d.ts` beats `.ts`).
2. `contributes.grammars` names the grammar for that language (`language`, `scopeName`, `path`). A grammar without a `language` is not used for files of its own; other grammars can include it.
3. Files tree-sitter already knows keep tree-sitter. The grammar is used only when there is no built-in one.

Grammar files can be `.tmLanguage.json` or XML property lists (`.tmLanguage`, `.plist`). A grammar file must be inside the extension folder.

## What the engine supports

- `match` rules, `begin`/`end` rules (including `applyEndPatternLast`) and `begin`/`while` rules, with `name` and `contentName`.
- Capture groups with their own scope (`captures`, `beginCaptures`, `endCaptures`, `whileCaptures`), including groups that are tokenized by their own `patterns`.
- `end` patterns that use the text of the begin match (`\1`).
- `include` of the grammar itself (`$self`), the document's grammar (`$base`), a repository rule (`#name`) and other grammars (`source.other`, `source.other#rule`). Included grammars are loaded from the extension that contributes them.
- `\G`, and the Oniguruma `\h` and `\H` classes. A pattern the regular expression engine cannot compile is skipped; the rest of the grammar still works.
- Highlighting is incremental: after an edit only the lines up to the point where the open rules match what they were before are tokenized again.

Not supported: injections (`injections`, `injectionSelector`), `embeddedLanguages`, `tokenTypes`, and Oniguruma features the engine lacks (for example some property names such as `\p{Alpha}` and variable-length lookbehind).

## Colors

A token's scopes are mapped to the classes themes color: the innermost scope that has a class decides, so the quotes of a string take the string color and plain `meta` scopes pass the choice outward.

| Scope prefix | Class |
|---|---|
| `comment` | comment |
| `string`, `string.regexp`, `markup.italic`, `markup.raw` | string |
| `constant.numeric` | number |
| `constant`, `variable.language`, `support.constant` | constant |
| `keyword`, `storage`, `markup.heading` | keyword |
| `keyword.operator`, `markup.list` | operator |
| `entity.name.function`, `support.function`, `meta.function-call` | function |
| `entity.name.type`, `entity.name.class`, `support.type`, `entity.name.tag` | type |
| `variable.parameter`, `variable` | variable |
| `variable.other.property`, `support.type.property-name`, `entity.other.attribute-name` | property |

The colors are the ones of the active color theme, including themes from extensions.
