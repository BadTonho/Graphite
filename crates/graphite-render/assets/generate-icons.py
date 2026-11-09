#!/usr/bin/env python3
r"""Rebuild the embedded file-icon font from Nerd Fonts Symbols Only.

The glyph list is the `AVAILABLE_GLYPHS` array in `../src/icons.rs`. Add or
remove a `'\u{...}'` entry there, then run this script with the source font:

    python generate-icons.py --source /path/to/SymbolsNerdFont-Regular.ttf

Download the source from the Nerd Fonts release page:

    https://github.com/ryanoasis/nerd-fonts/releases/latest/download/NerdFontsSymbolsOnly.zip

Requires `fonttools` (`pip install fonttools`).
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tempfile
from pathlib import Path

from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent
ICONS_RS = ROOT.parent / "src" / "icons.rs"
OUTPUT = ROOT / "graphite-icons.ttf"
FAMILY = "Graphite Icons"


def rust_codepoints() -> list[int]:
    text = ICONS_RS.read_text(encoding="utf-8")
    values = {int(value, 16) for value in re.findall(r"'\\u\{([0-9a-fA-F]+)\}'", text)}
    if not values:
        raise SystemExit(f"no glyphs found in {ICONS_RS}")
    return sorted(values)


def rename_family(font: TTFont) -> None:
    name = font["name"]
    name.names = [record for record in name.names if record.nameID not in (1, 4, 6, 16, 17)]
    name.setName(FAMILY, 1, 3, 1, 0x409)
    name.setName("Regular", 2, 3, 1, 0x409)
    name.setName(f"{FAMILY} Regular", 4, 3, 1, 0x409)
    name.setName("GraphiteIcons-Regular", 6, 3, 1, 0x409)
    name.setName(FAMILY, 16, 3, 1, 0x409)
    name.setName("Regular", 17, 3, 1, 0x409)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        required=True,
        type=Path,
        help="SymbolsNerdFont-Regular.ttf from NerdFontsSymbolsOnly.zip",
    )
    args = parser.parse_args()
    if not args.source.is_file():
        raise SystemExit(f"source font not found: {args.source}")

    codepoints = rust_codepoints()
    unicodes = ",".join(f"U+{value:04X}" for value in codepoints)

    with tempfile.TemporaryDirectory() as temporary:
        subset = Path(temporary) / "subset.ttf"
        subprocess.run(
            [
                sys.executable,
                "-m",
                "fontTools.subset",
                str(args.source),
                f"--unicodes={unicodes}",
                f"--output-file={subset}",
                "--glyph-names",
                "--symbol-cmap",
                "--legacy-cmap",
                "--notdef-glyph",
                "--notdef-outline",
                "--recommended-glyphs",
                "--name-IDs=*",
                "--name-legacy",
                "--name-languages=*",
            ],
            check=True,
        )
        font = TTFont(subset)
        rename_family(font)
        font.save(OUTPUT)

    print(f"wrote {OUTPUT} with {len(codepoints)} glyphs")


if __name__ == "__main__":
    main()
