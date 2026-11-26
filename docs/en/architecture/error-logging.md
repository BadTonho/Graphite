# Four-Tier Error Logging

Graphite persists every runtime error, unhandled panic, and subsystem failure into four separate severity-level log files under the user's configuration directory.

## Storage Location

- **Windows:** `%APPDATA%\graphite\logs\errors\`
- **Linux:** `$XDG_CONFIG_HOME/graphite/logs/errors/` (or `~/.config/graphite/logs/errors/`)

Each severity level writes to its own JSON Lines (`.jsonl`) file:

| Level | File | Scope |
| :--- | :--- | :--- |
| **4 (Highest)** | `critical.jsonl` | Fatal crashes, unhandled `panic!` hooks, GPU/window initialization failures. |
| **3 (High)** | `high.jsonl` | Core workflow failures (file IO, corrupted config, update download/apply/SHA-256 errors, extension host crashes). |
| **2 (Medium)** | `medium.jsonl` | Subsystem degradation where the editor keeps running (LSP, Git/SCM, terminal, plugins, manual update check errors). |
| **1 (Lowest)** | `low.jsonl` | Non-critical background/UI failures or minor stalls (silent background update check failures, optional metadata timeouts). |

## Entry Schema & Rotation

Each line is a self-contained JSON object containing:

- `timestamp_unix_ms`: UTC epoch timestamp in milliseconds
- `severity`: `"critical" | "high" | "medium" | "low"`
- `source`: Subsystem identifier (e.g. `"panic"`, `"updates"`, `"window.create"`)
- `message`: Sanitized error message
- `os`: Target operating system (`"windows"`, `"linux"`, etc.)
- `version`: Graphite package version

When a log file reaches 5 MB, it is rotated to `<severity>.1.jsonl` before new entries are appended.

## Security & Privacy

Before writing to disk, messages pass through `sanitize_message`, which uses the same rules as the issue reporter (`app::redaction::redact_credentials`). It redacts `Bearer`, `Basic`, and `token` credentials; `key=value` and JSON secret fields such as `api_key`, `x-api-key`, `password`, and `*_TOKEN=`; passwords in URLs; PEM private keys; and GitHub (`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`, `github_pat_`), `sk-`, Google (`AIza`), GitLab, Slack, and AWS access tokens. Raw file contents and credentials must never be logged.
