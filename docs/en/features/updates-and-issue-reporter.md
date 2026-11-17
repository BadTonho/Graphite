# Application Updates and Redacted Issue Reporter

Graphite includes an application update manager backed by GitHub Releases and a diagnostic Issue Reporter that strips credentials before generating bug reports.

## Updates (`crates/graphite-app/src/app/updates/`)

- **Channels (`state.rs`)**: Supports `stable` (`/releases/latest`) and `prerelease` (`/releases`), configured via `<profile>/updates-config.json`.
- **Semantic Version Comparison (`SemVer`)**: Compares `major.minor.patch` and prerelease tags strictly before marking an update available.
- **Mandatory integrity (`signature.rs`, `mod.rs`)**: A package is installed only if the release ships `<asset>.sig` (Ed25519 signature by a key in `TRUSTED_UPDATE_KEYS`) over `graphite-update-v1\n<tag>\n<asset>\n<sha256>\n` **and** the downloaded bytes hash (streamed SHA-256) to that digest. The digest comes from the release notes (`<hex>  <asset>`) or the `<asset>.sha256` file. Unsigned releases, missing digests, untrusted keys and replayed signatures are rejected before staging. The signature is checked before the package is downloaded.
- **Streaming download (`transport.rs`)**: Packages stream to `<asset>.part` with live progress, a size cap, truncation detection and cancellation; they get their final name only after verification.
- **Asset selection**: Matches OS and CPU architecture (`windows`/`linux`/`macos`, `x64`/`arm64`). There is no cross-platform fallback.
- **Installer (`apply.rs`)**: Re-checks the staged file's SHA-256, requires it to live in `<profile>/updates/`, validates the executable header (PE/ELF/Mach-O), keeps the old binary as `.old` and rolls back if the new one cannot start. A bad marker is discarded, never retried forever.
- **Cancellation and polling**: Cancelling bumps a generation so late results are ignored while the background poller keeps working. The poller reads the current channel/`auto_check` each cycle and backs off (10 min up to 1 h) after failures; GitHub rate limits produce a clear message. The prerelease channel picks the highest semantic version.
- **Background Transport (`transport.rs`)**: Uses `GitHubReleasesTransport` on a background thread and reports results over `UserEvent::Update`.

### Signing a release
1. One time: `cargo run -p graphite-app --example update_signer -- keygen <private-key-file>`, keep the private key secret (never commit it) and paste the printed public key into `TRUSTED_UPDATE_KEYS`. To rotate, add the new key first.
2. Per release asset: `cargo run -p graphite-app --example update_signer -- sign <private-key-file> <tag> <asset-file>`; upload the asset together with the generated `<asset>.sig` and `<asset>.sha256`.
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
