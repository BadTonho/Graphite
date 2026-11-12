"""Static compatibility survey for VSIX extensions from Open VSX.

Downloads each extension, reads its manifest and scans its entry point for `vscode.*` API
calls, then compares them with the APIs Graphite supports (`supported_apis.txt`). Nothing
from a downloaded package is ever executed or extracted to disk: files are read straight out
of the archive.

Usage: python3 -I packaging/compat/survey.py LIST_FILE OUTPUT_DIR
LIST_FILE has one `namespace.name` per line. OUTPUT_DIR receives the packages and
`report.md`. The scan is a heuristic: it matches `window.`, `workspace.`, `languages.` ...
member accesses, so bundled or obfuscated code can hide some calls.
"""

import collections
import json
import re
import sys
import urllib.request
import zipfile
from pathlib import Path

REGISTRY = "https://open-vsx.org/api"
LIMIT = 150 * 1024 * 1024
NAMESPACES = ("window", "workspace", "languages", "commands", "debug", "tasks", "env",
              "extensions", "scm", "authentication", "notebooks", "tests", "comments")
USAGE = re.compile(r"\b(" + "|".join(NAMESPACES) + r")\.([A-Za-z_]\w*)")
HERE = Path(__file__).resolve().parent


def fetch(url, limit=LIMIT):
    request = urllib.request.Request(url, headers={"User-Agent": "graphite-compat-survey"})
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError("response too large")
    return data


def api_members():
    """Every public `namespace.member` of the vscode API, to drop false matches."""
    lines = (HERE / "api_members.txt").read_text().splitlines()
    return {line.strip() for line in lines if line.strip() and not line.startswith("#")}


def supported():
    lines = (HERE / "supported_apis.txt").read_text().splitlines()
    return {line.strip() for line in lines if line.strip() and not line.startswith("#")}


def read_member(archive, name):
    try:
        info = archive.getinfo(name)
    except KeyError:
        return None
    if info.file_size > 40 * 1024 * 1024:
        return None
    return archive.read(info)


def scan(path):
    with zipfile.ZipFile(path) as archive:
        manifest = json.loads(read_member(archive, "extension/package.json") or b"{}")
        main = manifest.get("main") or manifest.get("browser")
        used = collections.Counter()
        client = False
        if main:
            entry = "extension/" + main.removeprefix("./")
            candidates = [entry, entry + ".js"]
            source = next((read_member(archive, c) for c in candidates if read_member(archive, c)), b"")
            text = source.decode("utf-8", "replace")
            valid = api_members()
            for match in USAGE.finditer(text):
                api = f"{match.group(1)}.{match.group(2)}"
                if api in valid:
                    used[api] += 1
            client = "vscode-languageclient" in text or "LanguageClient" in text
    return manifest, used, client


def main():
    listing, output = Path(sys.argv[1]), Path(sys.argv[2])
    output.mkdir(parents=True, exist_ok=True)
    known = supported()
    rows, missing_total, client_total = [], collections.Counter(), collections.Counter()
    for line in listing.read_text().split():
        namespace, _, name = line.partition(".")
        try:
            meta = json.loads(fetch(f"{REGISTRY}/{namespace}/{name}", 2_000_000))
            package = output / f"{line}.vsix"
            package.write_bytes(fetch(meta["files"]["download"]))
            manifest, used, client = scan(package)
        except Exception as error:  # report and keep going
            rows.append((line, "", f"failed: {error}", [], [], False))
            continue
        gaps = sorted(api for api in used if api not in known)
        contributes = sorted((manifest.get("contributes") or {}).keys())
        # A language client registers every provider generically; count those separately.
        for api in gaps:
            (client_total if client else missing_total)[api] += 1
        rows.append((line, meta.get("version", ""), "ok" if manifest.get("main") else "declarative",
                     contributes, gaps, client))
    report = ["# VSIX compatibility survey (static)\n"]
    report.append(f"{len(rows)} extensions. APIs counted once per extension.\n")
    report.append("## Most requested unsupported APIs\n")
    for api, count in missing_total.most_common(40):
        report.append(f"- `{api}`: {count} extensions")
    report.append("\n## Unsupported APIs used through vscode-languageclient\n")
    report.append("Language-client based extensions register many providers only when the "
                  "language server offers the matching feature.\n")
    for api, count in client_total.most_common(25):
        report.append(f"- `{api}`: {count} extensions")
    report.append("\n## Per extension\n")
    for line, version, kind, contributes, gaps, client in rows:
        report.append(f"### {line} {version} ({kind}{', language client' if client else ''})")
        report.append(f"- contributes: {', '.join(contributes) or 'nothing'}")
        report.append(f"- unsupported APIs used: {', '.join(gaps) or 'none found'}\n")
    (output / "report.md").write_text("\n".join(report) + "\n")
    print(output / "report.md")


if __name__ == "__main__":
    main()
