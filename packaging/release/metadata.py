"""Validate release identity before any external release mutation."""

import argparse
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[2]
VERSION = re.compile(r"v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:[0-9A-Za-z-]+\.)*[0-9A-Za-z-]+))?\Z")


def release_version(tag, root=ROOT):
    match = VERSION.fullmatch(tag)
    if not match:
        raise ValueError("Use a version tag such as v0.1.0 or v0.2.0-rc.1")
    if match[4] and any(part.isdigit() and len(part) > 1 and part[0] == "0"
                        for part in match[4].split(".")):
        raise ValueError("Numeric prerelease identifiers cannot have leading zeroes")
    version = tag[1:]
    workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    if workspace["workspace"]["package"]["version"] != version:
        raise ValueError("Tag version must match workspace.package.version in Cargo.toml")
    if version != "0.1.0":
        for language in ("en", "pt-BR"):
            path = root / "changelog" / version / f"{language}.md"
            if not path.is_file() or not path.read_text(encoding="utf-8").strip():
                raise ValueError(f"Missing nonempty changelog: {path.relative_to(root)}")
    return version


def known_limitations(root=ROOT):
    """The known-limitations pages in both languages, shaped as release-note sections."""
    sections = []
    for heading, relative in (("Known limitations", "docs/en/roadmap/known-limitations.md"),
                              ("Limitações conhecidas", "docs/pt-BR/roadmap/known-limitations.md")):
        path = root / relative
        text = path.read_text(encoding="utf-8").strip() if path.is_file() else ""
        if not text:
            raise ValueError(f"Missing nonempty known-limitations page: {relative}")
        body = "\n".join(text.splitlines()[1:]).strip().replace("\n## ", "\n### ")
        if body.startswith("## "):
            body = "#" + body
        sections.append(f"## {heading}\n\n{body}")
    return sections


def release_notes(tag, root=ROOT):
    version = release_version(tag, root)
    sections = [f"# Graphite {tag}",
                "Draft for manual review. Build failures can leave this draft incomplete. "
                "Review all assets, checksums, signatures and platform checks before publishing."]
    if version == "0.1.0":
        sections.append("Initial release / Lançamento inicial.")
        sections.extend(known_limitations(root))
    else:
        for language in ("en", "pt-BR"):
            sections.append((root / "changelog" / version / f"{language}.md").read_text(encoding="utf-8"))
    sections.append("Packages target x86-64. Linux native packages use their respective package managers. "
                    "In-app updates require trusted Ed25519 signatures; unsigned assets are rejected. "
                    "Windows installers are not Authenticode-signed by this workflow.")
    return "\n\n".join(sections) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()
    version = release_version(args.tag)
    if args.github_output:
        with args.github_output.open("a", encoding="utf-8") as output:
            output.write(f"version={version}\n")
    print(version)


if __name__ == "__main__":
    main()
