"""Packages packaging/examples/decoration-demo as a .vsix under target/examples/."""

import json
import zipfile
from pathlib import Path

SOURCE = Path(__file__).resolve().parent / "decoration-demo"
OUTPUT = Path(__file__).resolve().parents[2] / "target" / "examples"


def main():
    manifest = json.loads((SOURCE / "package.json").read_text())
    name = f"{manifest['publisher']}.{manifest['name']}-{manifest['version']}.vsix"
    OUTPUT.mkdir(parents=True, exist_ok=True)
    target = OUTPUT / name
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        for file in ("package.json", "extension.js"):
            archive.write(SOURCE / file, f"extension/{file}")
    print(target)


if __name__ == "__main__":
    main()
