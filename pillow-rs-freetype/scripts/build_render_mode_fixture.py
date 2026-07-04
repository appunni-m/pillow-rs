#!/usr/bin/env python3
"""Generate FreeType render-mode fixtures from gen_ft_refs.c.

Build the helper first, for example:

  cc -I pillow-rs-freetype/freetype/include \
     pillow-rs-freetype/scripts/gen_ft_refs.c \
     -L "$HOME/.local/lib" -lfreetype -lm \
     -o /tmp/gen_refs_v4

Then run:

  LD_LIBRARY_PATH="$HOME/.local/lib" \
    FT_REF_BIN=/tmp/gen_refs_v4 \
    python3 pillow-rs-freetype/scripts/build_render_mode_fixture.py
"""

import hashlib
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONT_DIR = os.path.join(ROOT, "tests/fixtures/input/fonts_autohint")
RAW_DIR = os.path.join(ROOT, "tests/fixtures/outputs/render_modes")
OUTPUT = os.path.join(ROOT, "tests/fixtures/render_mode_matrix.json")
REF_BIN = os.environ.get("FT_REF_BIN", "/tmp/gen_refs_v4")

CASES = [
    ("DejaVuSans", "DejaVuSans.ttf", 20, 65),
    ("DejaVuSans", "DejaVuSans.ttf", 20, 103),
    ("LiberationSans-Regular", "LiberationSans-Regular.ttf", 16, 81),
    ("NotoSans-Bold", "NotoSans-Bold.ttf", 24, 56),
]

MODES = [
    ("normal", "native", "gray"),
    ("mono", "render-mono", "mono"),
    ("lcd", "render-lcd", "lcd"),
    ("lcd_v", "render-lcd-v", "lcd_v"),
]


def parse_glyphs(text):
    size = None
    glyphs = {}
    for line in text.splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "SIZE":
            size = int(parts[1])
            continue
        if parts[0] != "GLYPH" or size is None:
            continue
        cp = int(parts[1])
        width = int(parts[2])
        rows = int(parts[3])
        left = int(parts[4])
        top = int(parts[5])
        pitch = width
        pixel_mode = "gray"
        cursor = 7
        if cursor < len(parts) and parts[cursor] == "PITCH":
            pitch = int(parts[cursor + 1])
            cursor += 2
        if cursor < len(parts) and parts[cursor] == "PIXEL_MODE":
            pixel_mode = parts[cursor + 1]
            cursor += 2
        pixels = b""
        if cursor < len(parts) and parts[cursor] == "PIXELS":
            pixels = bytes.fromhex("".join(parts[cursor + 1 :]))
        glyphs[(size, cp)] = {
            "width": width,
            "rows": rows,
            "left": left,
            "top": top,
            "pitch": pitch,
            "pixel_mode": pixel_mode,
            "pixels": pixels,
        }
    return glyphs


def main():
    if not os.path.exists(REF_BIN):
        print(f"missing FreeType reference helper: {REF_BIN}", file=sys.stderr)
        return 1
    os.makedirs(RAW_DIR, exist_ok=True)

    rows = []
    for font_name, filename, size, cp in CASES:
        font_path = os.path.join(FONT_DIR, filename)
        for fixture_mode, ft_mode, pixel_mode in MODES:
            output = subprocess.run(
                [REF_BIN, font_path, ft_mode],
                check=True,
                text=True,
                stdout=subprocess.PIPE,
            ).stdout
            glyph = parse_glyphs(output)[(size, cp)]
            row_id = f"{font_name}_{size}_{cp}_render_{fixture_mode}"
            raw_name = f"{row_id}.bin"
            with open(os.path.join(RAW_DIR, raw_name), "wb") as raw:
                raw.write(glyph["pixels"])
            rows.append(
                {
                    "id": row_id,
                    "font": font_name,
                    "size_pt": float(size),
                    "codepoint": cp,
                    "mode": fixture_mode,
                    "pixel_mode": pixel_mode,
                    "width": glyph["width"],
                    "rows": glyph["rows"],
                    "pitch": glyph["pitch"],
                    "left": glyph["left"],
                    "top": glyph["top"],
                    "ref_sha256": hashlib.sha256(glyph["pixels"]).hexdigest(),
                    "ref_raw": f"outputs/render_modes/{raw_name}",
                }
            )

    with open(OUTPUT, "w") as fixture:
        json.dump(
            {
                "version": "1.0.0",
                "generator": "FreeType 2.14.3 render modes via scripts/build_render_mode_fixture.py",
                "rows": rows,
            },
            fixture,
            indent=2,
        )
    print(f"wrote {len(rows)} rows to {OUTPUT}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
