"""Collect the third-party license notices that ship with Graphite.

Run by the packaging step. It writes ``THIRD_PARTY_NOTICES.txt`` (the license text of every Rust
crate linked into the program, plus pointers to the other bundled components) and copies the
icon-font licenses to ``licenses/icon-font``. Nothing here needs the network once the crates have
been downloaded by a build.
"""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
TARGETS = ("x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu")
LICENSE_PREFIXES = ("license", "licence", "copying", "notice", "unlicense")
ICON_FONT_ASSETS = Path("crates/graphite-render/assets")
# Third-party JavaScript copied into the repository and embedded in the program, with its license.
VENDORED_SOURCES = (Path("crates/graphite-app/runtime/emmet"),)

HEADER = """\
Graphite - third-party notices

Graphite is released under the MIT license (see LICENSE). It includes the software listed below,
each under its own license. This file reproduces the license text of every Rust crate linked into
the program; the other bundled components carry their own notices.

Bundled components
------------------
* Code OSS runtime (code-oss/): MIT. Its license is in code-oss/LICENSE.txt and the licenses of
  its own dependencies are in code-oss/ThirdPartyNotices.txt.
* Node.js runtime (code-oss/NODE-LICENSE): MIT, with the licenses of Node's dependencies in that
  file.
* Emmet (bundled JavaScript embedded in the program): MIT, reproduced below with the Rust crates.
* Icon font (licenses/icon-font/): a subset of Nerd Fonts Symbols Only, which draws on Codicons
  (CC BY 4.0), Devicons (MIT), Font Awesome (CC BY 4.0 icons, SIL OFL 1.1 fonts) and Seti (MIT).
  See licenses/icon-font/NOTICE.md and the LICENSE-*.txt files next to it.

Rust crates
-----------
"""


def load_metadata(root=ROOT, target=None):
    """`cargo metadata` for the workspace, optionally limited to one platform."""
    command = ["cargo", "metadata", "--locked", "--format-version", "1"]
    if target:
        command += ["--filter-platform", target]
    result = subprocess.run(command, cwd=root, capture_output=True, check=False)
    if result.returncode:
        raise RuntimeError(result.stderr.decode("utf-8", "replace").strip() or "cargo metadata failed")
    return json.loads(result.stdout.decode("utf-8"))


def shipped_packages(metadata):
    """Packages linked into the program: reachable through normal dependencies from the workspace.

    Development-only and build-script-only dependencies are not part of what is distributed.
    """
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    members = set(metadata["workspace_members"])
    seen, queue = set(), list(members)
    while queue:
        current = queue.pop()
        if current in seen or current not in nodes:
            continue
        seen.add(current)
        for dependency in nodes[current]["deps"]:
            if any(kind.get("kind") is None for kind in dependency.get("dep_kinds", [])):
                queue.append(dependency["pkg"])
    return [packages[identifier] for identifier in sorted(seen - members) if identifier in packages]


def license_texts(directory):
    """Contents of the license files at the top of a crate's directory."""
    texts = []
    if not directory.is_dir():
        return texts
    for path in sorted(directory.iterdir()):
        if path.is_file() and path.name.lower().startswith(LICENSE_PREFIXES):
            texts.append((path.name, path.read_text(encoding="utf-8", errors="replace").strip()))
    return texts


def entries_for(packages):
    """One entry per crate: identity, declared license, and license texts found in its sources."""
    entries = []
    for package in packages:
        directory = Path(package["manifest_path"]).parent
        texts = license_texts(directory)
        declared = package.get("license") or package.get("license_file") or ""
        entries.append({
            "name": package["name"],
            "version": package["version"],
            "license": declared,
            "repository": package.get("repository") or "",
            "texts": texts,
        })
    return entries


def vendored_entries(root=ROOT, sources=VENDORED_SOURCES):
    """Entries for third-party code that lives in the repository rather than in a crate."""
    entries = []
    for source in sources:
        directory = Path(root) / source
        description = json.loads((directory / "package.json").read_text(encoding="utf-8"))
        repository = description.get("repository") or ""
        if isinstance(repository, dict):
            repository = repository.get("url", "")
        entries.append({
            "name": description["name"] + " (bundled JavaScript)",
            "version": description["version"],
            "license": description.get("license", ""),
            "repository": repository,
            "texts": license_texts(directory),
        })
    return entries


def render(entries):
    """The notices file: crates grouped by identical license text so nothing is repeated."""
    groups = {}
    for entry in sorted(entries, key=lambda e: (e["name"], e["version"])):
        key = tuple(text for _, text in entry["texts"])
        groups.setdefault(key, []).append(entry)
    parts = [HEADER]
    for texts, members in sorted(groups.items(), key=lambda item: item[1][0]["name"]):
        for entry in members:
            line = f"* {entry['name']} {entry['version']} - {entry['license'] or 'license not declared'}"
            if entry["repository"]:
                line += f" ({entry['repository']})"
            parts.append(line)
        if texts:
            for text in texts:
                parts.append("\n" + text + "\n")
        else:
            parts.append("\n(The crate ships no license file; the license above is the one it declares.)\n")
        parts.append("-" * 78 + "\n")
    return "\n".join(parts)


def without_license_files(entries):
    """Crates that declare no license at all and ship no text: these must be reviewed by hand."""
    return [e for e in entries if not e["texts"] and not e["license"]]


def collect(root=ROOT, targets=TARGETS):
    """Entries for every crate shipped on any of the release platforms."""
    merged = {}
    for target in targets:
        for package in shipped_packages(load_metadata(root, target)):
            merged[(package["name"], package["version"])] = package
    return entries_for(list(merged.values())) + vendored_entries(root)


def write_bundle(destination, root=ROOT, entries=None):
    """Write the notices and the icon-font licenses into a package directory."""
    destination = Path(destination)
    entries = collect(root) if entries is None else entries
    missing = without_license_files(entries)
    if missing:
        names = ", ".join(f"{e['name']} {e['version']}" for e in missing)
        raise ValueError(f"No license declared and no license file for: {names}")
    (destination / "THIRD_PARTY_NOTICES.txt").write_text(render(entries), encoding="utf-8")
    fonts = destination / "licenses" / "icon-font"
    fonts.mkdir(parents=True, exist_ok=True)
    source = Path(root) / ICON_FONT_ASSETS
    for path in sorted(source.iterdir()):
        if path.name == "NOTICE.md" or path.name.startswith("LICENSE-"):
            shutil.copy2(path, fonts / path.name)
    return len(entries)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True,
                        help="package directory that receives the notices")
    args = parser.parse_args()
    try:
        count = write_bundle(args.output_dir)
    except (RuntimeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(f"Wrote notices for {count} crates to {args.output_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
