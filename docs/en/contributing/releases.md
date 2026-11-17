# Draft releases and platform packages

Pushing a version tag triggers `.github/workflows/release.yml`. Commits and branch pushes do not run it; the existing CI stays manual. The workflow validates identity, **creates a draft**, builds packages and attaches them only when all builders succeed. It never publishes. Review the draft and publish it yourself in GitHub Releases. The [GitHub CLI draft option](https://cli.github.com/manual/gh_release_create) is explicit.

## Platform coverage

| Platform | Build environment | Assets |
|---|---|---|
| Windows | Windows Server 2022, MSVC | Portable ZIP, per-user NSIS setup EXE |
| Ubuntu | Ubuntu 22.04 | DEB, portable tar.gz, AppImage |
| Debian | Debian 12 container | DEB |
| Fedora | Fedora 44 container | RPM |
| Arch Linux | Rolling `archlinux:base-devel` container | pkg.tar.zst, using makepkg as an unprivileged user |

All targets are **x86-64**, matching the verified Node/Code OSS inputs. ARM and 32-bit packages are not generated. Native Linux packages are built separately for their distributions. DEB dependencies are derived with `dpkg-shlibdeps`; RPM uses automatic dependency generation. Packages include the bundled host, desktop entry, icon and license, install under `/usr/lib/graphite`, and expose `graphite` and `graphite-code`. They do not replace another editor's `code` command.

The Windows installer uses `%LOCALAPPDATA%/Programs/Graphite`, creates a Start Menu shortcut and per-user uninstaller, and leaves PATH unchanged. Windows needs compatible graphics drivers and WebView2 for webviews. This workflow does not Authenticode-sign the installer.

AppImage is built on Ubuntu 22.04, following [AppImage baseline guidance](https://docs.appimage.org/reference/best-practices.html). It bundles linked dependencies, GTK resources and WebKit subprocesses. linuxdeploy is pinned to `1-alpha-20251107-1` with SHA-256 verification. Packaging checks extraction and required files without FUSE/display. Older glibc and every GPU/distribution are not guaranteed; UI/WebKit/X11/Wayland checks remain manual.

## Triggering a release

1. Commit the intended sources, release tooling and `Cargo.lock`. Set `workspace.package.version` to the intended version without `v`.
2. After initial `0.1.0`, include nonempty synchronized `changelog/<version>/en.md` and `pt-BR.md` using the required categories. Version `0.1.0` has no changelog.
3. Run the local verifier and `python -m unittest discover -s packaging/release/tests -v` (Python 3.11+).
4. Create and push the matching tag:

```sh
git tag -a v0.1.0 -m "Graphite 0.1.0"
git push origin v0.1.0
```

Tags use `vMAJOR.MINOR.PATCH`, optionally with a prerelease such as `v0.2.0-rc.1`, and must match Cargo exactly. Invalid tags or missing changelogs fail before draft creation. Prereleases set GitHub's prerelease flag while remaining drafts. Native package versions normalize prerelease separators for each package manager.

## Assets and signing

Ten package/update assets are required, with the tag/platform/architecture in their names. Each receives `<asset>.sha256`. An explicit allowlist rejects missing or unexpected files before upload. Only the draft/upload jobs have release-write permissions. Build jobs receive no signing key.

Optionally configure Actions secret **`GRAPHITE_UPDATE_SIGNING_KEY`** with the base64 PKCS#8 private key used by `update_signer`. Its public key must already be compiled into `TRUSTED_UPDATE_KEYS`; see [update signing](../features/updates-and-issue-reporter.md). The signing job uses a temporary file outside the checkout, cleans it up, and excludes the key variable from signer subprocess environments. Never commit or upload keys.

With the secret, all assets also receive Ed25519 `.sig` sidecars. Without it, the draft receives checksums and a workflow warning; the app rejects unsigned in-app updates. Review key trust/signatures before publishing. This is separate from Authenticode or distribution repository signing.

Dedicated `graphite-update-<tag>-<platform>-x64.zip` assets contain only the main executable. The updater prefers them and excludes native packages, AppImages, tarballs and setup executables. Install the full distribution to refresh Node/Code OSS/helpers. Use the package manager for native Linux installations: managed-installation and AppImage self-update support are not added here.

## Failures, reruns and validation

A failed build can leave an incomplete draft. Inspect Actions and rerun failed jobs. Existing drafts preserve manually edited notes. Build artifacts expire after seven days; rerun all jobs if required artifacts expired. Every upload rechecks draft status and refuses changes to published releases. Do not publish while the workflow runs. The workflow never sets `draft` to false, pushes to AUR, or creates apt/dnf/pacman repositories.

Offline release tests cover version/changelog rules, API failure, draft-only behavior, interrupted upload, asset tampering, temporary-key cleanup and archive traversal. Local Windows validation passed: 17 release-tool tests, workspace build, and 1,762 Rust tests (0 failed, 10 ignored). YAML, Bash syntax through Git Bash and Windows helper syntax also passed. Linux execution was left for the maintainer as requested. Full package/NSIS/AppImage builds and the real GitHub workflow were not run locally: NSIS is unavailable and no tag/release was pushed. Validate the packages and [manual release checks](manual-verification.md) before publishing the first draft.
