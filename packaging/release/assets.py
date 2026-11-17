"""Checksums, optional update signatures and an explicit release asset allowlist."""

import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

from metadata import ROOT, release_version


def expected_assets(tag):
    return {
        f"graphite-{tag}-windows-x64.zip",
        f"graphite-{tag}-windows-x64-setup.exe",
        f"graphite-{tag}-linux-debian12-x64.deb",
        f"graphite-{tag}-linux-ubuntu22.04-x64.deb",
        f"graphite-{tag}-linux-fedora44-x64.rpm",
        f"graphite-{tag}-linux-arch-x64.pkg.tar.zst",
        f"graphite-{tag}-linux-x64.tar.gz",
        f"graphite-{tag}-linux-x64.AppImage",
        f"graphite-update-{tag}-linux-x64.zip",
        f"graphite-update-{tag}-windows-x64.zip",
    }


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def finalize(directory, tag):
    release_version(tag)
    files = sorted(directory.iterdir())
    if not files or {p.name for p in files} != expected_assets(tag):
        raise ValueError("Missing, duplicate or unexpected release assets")
    if any(not p.is_file() or p.is_symlink() for p in files):
        raise ValueError("Release assets must be regular files")
    for path in files:
        path.with_name(path.name + ".sha256").write_text(
            f"{digest(path)}  {path.name}\n", encoding="ascii")
    key = os.environ.get("GRAPHITE_UPDATE_SIGNING_KEY", "")
    if key:
        # Key never enters the repository, command line, uploaded assets or logs.
        with tempfile.TemporaryDirectory(prefix="graphite-release-key-") as temporary:
            private = Path(temporary) / "signing.key"
            private.write_text(key, encoding="ascii")
            private.chmod(0o600)
            signer_env = dict(os.environ)
            signer_env.pop("GRAPHITE_UPDATE_SIGNING_KEY", None)
            for path in files:
                subprocess.run(["cargo", "run", "--locked", "-p", "graphite-app",
                                "--example", "update_signer", "--", "sign",
                                str(private), tag, str(path)], cwd=ROOT, env=signer_env, check=True)
    else:
        print("::warning::No update signing key configured. Assets have checksums only; "
              "in-app updates will reject them. Review before manually publishing.")


def verify_assets(directory, tag):
    expected = expected_assets(tag)
    names = {p.name for p in directory.iterdir()}
    unsigned = expected | {n + ".sha256" for n in expected}
    signed = unsigned | {n + ".sig" for n in expected}
    if names not in (unsigned, signed):
        raise ValueError("Release asset set is incomplete or contains unexpected files")
    files = sorted(directory.iterdir())
    if any(not p.is_file() or p.is_symlink() for p in files):
        raise ValueError("Release assets must be regular files")
    for name in expected:
        actual = digest(directory / name)
        if (directory / (name + ".sha256")).read_text(encoding="ascii") != f"{actual}  {name}\n":
            raise ValueError(f"Checksum mismatch: {name}")
    return files


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--assets", type=Path, required=True)
    args = parser.parse_args()
    finalize(args.assets, args.tag)
