# Graphite translations

Each subdirectory contains one locale. Files are flat JSON objects mapping
stable translation keys to text. `en/` is the fallback locale and every other
locale must contain the same keys and placeholders.

## Translation progress

| Language | Locale | Completion |
| --- | --- | ---: |
| English | `en` | 100% |
| Portuguese (Brazil) | `pt-BR` | 100% |

English is the source language and the final fallback. Both locales are kept
at full parity by the build; a missing key or placeholder mismatch fails
compilation. To add another language, add it to `languages.json`, create a
directory with the same eight catalog files, and translate values only. See the
contributor guide below for the full checklist.

- English contributor guide: [../docs/en/features/localization.md](../docs/en/features/localization.md)
- Guia em português: [../docs/pt-BR/features/localization.md](../docs/pt-BR/features/localization.md)

Do not translate `{placeholders}`; keep their names unchanged.
