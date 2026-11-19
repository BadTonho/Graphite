# Security Policy

Português: [docs/pt-BR/security/vulnerability-reporting.md](docs/pt-BR/security/vulnerability-reporting.md)

## Supported versions

Security fixes are released for the latest published version of Graphite. Please update to the latest release before reporting, and confirm the problem still happens there.

## Reporting a vulnerability

**Do not open a public issue for a security problem.** Report it privately instead:

1. Go to the [Security tab](https://github.com/BadTonho/Graphite/security/advisories/new) of the repository and choose **Report a vulnerability**.
2. Describe the problem, the version and operating system (Windows or Linux), and the steps to reproduce it.
3. Do not include real credentials, tokens or private files in the report. Redact them first.

You can expect an acknowledgement within a few days and updates as the fix progresses. Once a fixed release is available, the advisory is published and you are credited, unless you prefer to stay anonymous.

## What is in scope

- The in-app update system: release signature and checksum verification, download handling and the install and rollback steps.
- The extension host and the handling of `.vsix` and native plugins.
- Handling of credentials, such as the saved GitHub session, and the redaction of error logs and issue reports.
- Release packaging and the release workflows.

Extensions and plugins are third-party code. A flaw in a specific extension should be reported to that extension's author, unless Graphite fails to enforce a boundary it promises.

## Release integrity

Release assets are published with a SHA-256 checksum and an Ed25519 signature. The editor only installs an update if its signature verifies against a public key built into the application. If you suspect the signing key was compromised, report it through the channel above right away.

## Contributor rules

The rules contributors follow to keep secrets out of the repository are in the [security policy](docs/en/security/policy.md) and the [release audit](docs/en/security/release-audit.md).
