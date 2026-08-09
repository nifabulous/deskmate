#!/usr/bin/env python3
"""Render the app icon from the owl sprite in ui/index.html.

    python3 tools/make-icon.py [out.png]

The icon is the same 16x16 grid the pet is drawn from, scaled with
nearest-neighbour so it stays crisp pixel art at any size. Reading the grid
out of ui/index.html rather than keeping a second copy means a redrawn owl
cannot drift away from the icon.

Feed the result to `cargo tauri icon` to regenerate the platform icon set.
"""

import re
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui" / "index.html"

SIZE = 1024
MARGIN = 64  # keeps the sprite off the very edge, where macOS masks can clip


def parse_block(source: str, name: str) -> str:
    """Return the text of a `const NAME = ...;` declaration."""
    match = re.search(rf"const {name} = (.*?);\n", source, re.S)
    if not match:
        raise SystemExit(f"make-icon: could not find {name} in {UI}")
    return match.group(1)


def parse_palette(source: str) -> dict:
    palette = {}
    for key, value in re.findall(r'"(.)":\s*("#[0-9a-fA-F]{6}"|null)', parse_block(source, "OWL_PALETTE")):
        palette[key] = None if value == "null" else value.strip('"')
    return palette


def parse_frame(source: str) -> list:
    return re.findall(r'"([^"]{16})"', parse_block(source, "EYES_OPEN"))


def main() -> None:
    source = UI.read_text(encoding="utf8")
    palette = parse_palette(source)
    frame = parse_frame(source)
    if len(frame) != 16:
        raise SystemExit(f"make-icon: expected 16 sprite rows, found {len(frame)}")

    grid = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    for y, row in enumerate(frame):
        for x, char in enumerate(row):
            color = palette.get(char)
            if color:
                rgb = tuple(int(color[i:i + 2], 16) for i in (1, 3, 5))
                grid.putpixel((x, y), rgb + (255,))

    inner = SIZE - 2 * MARGIN
    scaled = grid.resize((inner, inner), Image.NEAREST)
    icon = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    icon.paste(scaled, (MARGIN, MARGIN), scaled)

    out = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "src-tauri" / "icons" / "icon-source.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    icon.save(out)
    print(f"wrote {out} ({SIZE}x{SIZE}) from the {len(frame)}-row owl sprite")


if __name__ == "__main__":
    main()
