# Application Updates and Redacted Issue Reporter

Graphite includes an application update manager backed by GitHub Releases and a diagnostic Issue Reporter that strips credentials before generating bug reports.

## Updates (`crates/graphite-app/src/app/updates/`)

- **Channels (`state.rs`)**: Supports `stable` (`/releases/latest`) and `prerelease` (`/releases`), configured via `<profile>/updates-config.json`.
- **Semantic Version Comparison (`SemVer`)**: Compares `major.minor.patch` and prerelease tags strictly before marking an update available.
- **SHA-256 Verification**: Verifies downloaded release packages with `ring::digest::SHA256` against digests declared in release notes before staging into `<profile>/updates/<tag>/`.
- **Background Transport (`transport.rs`)**: Uses `GitHubReleasesTransport` on a background thread and reports results over `UserEvent::Update`.

### Update Commands
- `update.check` / `update.checkForUpdate`: Checks for a newer release.
- `update.downloadNow` / `update.downloadUpdate`: Downloads and SHA-256 verifies the platform release archive.
- `update.install` / `update.installUpdate` / `update.restart` / `update.restartToUpdate`: Stages and schedules installation (`pending-install.json`) for the next restart.
- `update.cancelling`: Cancels an in-flight update flow.
- `update.checking` / `update.downloading` / `update.updating` / `update.showUpdateInfo`: Opens the update status report.
- `update.showUpdateReleaseNotes`: Displays release notes for the active or available version.

## Issue Reporter & Troubleshooting (`crates/graphite-app/src/app/issue_reporter/`)

- **Secret Redaction (`redact.rs`)**: Automatically replaces GitHub tokens, API keys (`sk-`, `sk-ant-`, `AIza`, Slack tokens), `Authorization` headers, credential JSON/env fields, PEM private keys, and local user home paths with `[REDACTED]` / `~`.
- **Interactive Troubleshooting**: Tracks session state and appends environment metadata (version, OS, architecture, installed extensions count, remote alias) without reading secrets.

### Issue Reporter Commands
- `workbench.action.openIssueReporter`: Generates a redacted markdown diagnostic draft and pre-filled GitHub New Issue URL.
- `workbench.action.troubleshootIssue.start` / `workbench.action.troubleshootIssue.stop`: Starts and concludes a diagnostic troubleshooting session.
- `workbench.action.chat.reportIssueForBug`: Opens a redacted bug report pre-titled for chat/agent diagnostics.
