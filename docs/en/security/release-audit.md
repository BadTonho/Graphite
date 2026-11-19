# Release audit

Checks to run before tagging a release, with the result of the audit for **0.1.0** (2026-11-17).

## Secrets and sensitive files

| Check | How | Result for 0.1.0 |
|---|---|---|
| Sensitive files tracked | `git ls-files` filtered for `.env`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.db`, `*.sqlite`, credentials | None. Two source files match by name only (`semantic_tokens.rs`, `tokenizer.rs`) |
| Sensitive files ever committed | `git log --all --diff-filter=A --name-only` with the same filter | None |
| Secret patterns in the code | `git grep -E` for GitHub tokens (`ghp_`, `gho_`, `github_pat_`), AWS keys, private-key headers, Slack and API-key shapes | Only made-up tokens in the tests of the error redactor |
| Secret patterns in history | `git log --all -G` for the same patterns | Only the commit that added those test fixtures |
| `.gitignore` | `git check-ignore -v .env x.pem x.key x.db` | `.env*`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.db`, `*.sqlite` and `/target` are ignored |
| Personal data | `git grep` for user names and local paths; `git log --format=%ae` | One placeholder path in a test. Commits use the GitHub no-reply address |

Run these again at the release revision. A real match must be removed from the history, not only from the latest commit, and the credential revoked.

## Licenses

- `THIRD_PARTY_NOTICES.txt` lists 622 components for 0.1.0: 621 Rust crates and the embedded Emmet bundle. All declare permissive licenses (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, 0BSD, CC0 and similar), apart from two MPL-2.0 crates (`option-ext`, `uluru`), used unmodified, and `self_cell`, which is `Apache-2.0 OR GPL-2.0-only` and is used under the Apache license. No GPL- or AGPL-only crate is linked in.
- Re-run `python packaging/release/licenses.py --output-dir <folder>` after changing dependencies and read the list of licenses for anything copyleft.

## Release pipeline

- The release workflow only creates a **draft**; publishing is manual.
- Build jobs receive no signing key. Only the signing step reads `GRAPHITE_UPDATE_SIGNING_KEY`, from a temporary file that is removed afterwards. Never commit or upload a private key.
- The signing key is stored as a secret of the `release` environment, and only the job that attaches the packages uses that environment. The environment requires a maintainer's approval, so pushing a tag alone never gives a workflow access to the key.
- Release tags (`v*`) are protected by a repository ruleset, and every third-party action is pinned to a commit SHA.
- Without the update signing key, assets get checksums only and the app refuses to install them as in-app updates.

## Manual checks

Package and run the real installers on clean Windows and Linux machines, following [manual verification](../contributing/manual-verification.md), and confirm the notices are inside the package.
