"""Create/upload only draft releases. Never publish or modify a published release."""

import argparse
import json
from pathlib import Path
import subprocess
import tempfile

from metadata import release_notes, release_version


def gh(*args, **kwargs):
    return subprocess.run(["gh", *map(str, args)], check=True, text=True,
                          capture_output=True, **kwargs).stdout


def find_release(tag, command=gh):
    # An API failure must not be mistaken for a missing release. Pagination includes drafts.
    pages = json.loads(command("api", "--paginate", "--slurp", "repos/{owner}/{repo}/releases"))
    return next((release for page in pages for release in page if release["tag_name"] == tag), None)


def require_draft(release):
    if release and not release["draft"]:
        raise ValueError("Release is already published; refusing to change it")


def ensure_draft(tag, command=gh):
    release_version(tag)
    release = find_release(tag, command)
    require_draft(release)
    if release:
        return  # Preserve manually edited draft notes on reruns.
    with tempfile.TemporaryDirectory(prefix="graphite-release-notes-") as temporary:
        notes = Path(temporary) / "notes.md"
        notes.write_text(release_notes(tag), encoding="utf-8")
        args = ["release", "create", tag, "--draft", "--verify-tag", "--title",
                f"Graphite {tag}", "--notes-file", notes]
        if "-" in tag:
            args.append("--prerelease")
        command(*args)


def upload(tag, directory, command=gh):
    release_version(tag)
    release = find_release(tag, command)
    if release is None:
        raise ValueError("Expected an existing draft release")
    require_draft(release)
    from assets import verify_assets
    files = verify_assets(directory, tag)
    # Upload individually to avoid Windows command-line limits; recheck the draft every time.
    for asset in files:
        current = find_release(tag, command)
        if current is None:
            raise ValueError("Draft disappeared during upload")
        require_draft(current)
        command("release", "upload", tag, asset, "--clobber")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("draft", "upload"))
    parser.add_argument("--tag", required=True)
    parser.add_argument("--assets", type=Path)
    args = parser.parse_args()
    if args.action == "draft":
        ensure_draft(args.tag)
    else:
        if not args.assets:
            parser.error("upload requires --assets")
        upload(args.tag, args.assets)


if __name__ == "__main__":
    main()
