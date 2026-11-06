# Security Policy

## Zero-leak policy

Never commit or upload: `.env` / `.env.*`, API keys, tokens, private keys (`*.pem`, `*.key`, `*.p12`, `*.pfx`), credentials, local databases (`*.db`, `*.sqlite`), signing certificates or build secrets. The root `.gitignore` covers these patterns.

## Before every commit or push

1. `git status` — only intended files are staged.
2. `git diff --staged` — no secrets or credentials.
3. Confirm sensitive patterns are ignored (`git check-ignore <file>`).

## Dependencies and licenses

Audit the dependency tree before releases (`cargo deny` / `cargo license`). Do not copy GPL code (e.g. from the Zed editor) into this MIT project.

## Extensions (future)

Extensions run in a separate Node process, and WASM plugins will be sandboxed. Extensions are untrusted code: they will require explicit install and clear permission boundaries.

