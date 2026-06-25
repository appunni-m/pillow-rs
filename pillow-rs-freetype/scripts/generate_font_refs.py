#!/usr/bin/env python3
"""Generate font coverage matrix + reference dumps using FreeType with
FT_LOAD_NO_HINTING.  The reference output matches pure-scaled outlines
with no bytecode or auto hinting applied — byte-perfect with the pure-Rust
pillow-rs-freetype port that rasterizes unscaled glyph outlines directly.

Usage:
    python scripts/generate_font_refs.py

Requires: freetype-py  (pip install freetype-py)
"""

import freetype
import json
import hashlib
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
INPUT_FONTS = FIXTURES / "input" / "fonts"
OUTPUT_RAWS = FIXTURES / "outputs" / "raws"
MATRIX_PATH = FIXTURES / "coverage_matrix.json"

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
FONTS = {
    "DejaVuSans": "DejaVuSans.ttf",
    "LiberationSerif": "LiberationSerif-Regular.ttf",
}
SIZES = [10, 12, 16, 20, 24]
CHARS = [chr(c) for c in range(33, 127)]  # printable ASCII

# FT_LOAD_NO_HINTING = 0x2, FT_LOAD_RENDER = 0x4
LOAD_FLAGS = 0x2 | 0x4


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def render_glyph(face: freetype.Face, ch: str):
    """Render a single glyph with FT_LOAD_NO_HINTING (pure scaling, no hinting)."""
    face.load_char(ch, LOAD_FLAGS)
    glyph = face.glyph
    bmp = glyph.bitmap

    width = bmp.width
    rows = bmp.rows
    if width <= 0 or rows <= 0:
        return b"", 0, 0, 0.0

    # bitmap.buffer is already a list of row-major bytes (width × rows)
    raw = bytes(bmp.buffer) if isinstance(bmp.buffer, bytearray) else bytes(bmp.buffer)
    advance = glyph.advance.x / 64.0
    return raw, width, rows, advance


def get_metrics(face: freetype.Face) -> list:
    """(ascent, descent) in integer pixels from the size metrics."""
    sz = face.size
    return [sz.ascender >> 6, -sz.descender >> 6]


def get_bbox(face: freetype.Face, ch: str) -> list:
    """(xMin, yMin, xMax, yMax) in pixels.  y=0 is the baseline, y positive UP."""
    face.load_char(ch, LOAD_FLAGS)
    glyph = face.glyph
    bmp = glyph.bitmap
    if bmp.width == 0 or bmp.rows == 0:
        return [0, 0, 0, 0]
    left = glyph.bitmap_left
    top = glyph.bitmap_top
    return [left, top - bmp.rows, left + bmp.width, top]


def get_name(face: freetype.Face) -> list:
    return [
        face.family_name.decode("utf-8", errors="replace"),
        face.style_name.decode("utf-8", errors="replace"),
    ]


def get_length(face: freetype.Face, text: str) -> float:
    total = 0.0
    for ch in text:
        face.load_char(ch, LOAD_FLAGS)
        total += face.glyph.advance.x / 64.0
    return total


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def generate() -> int:
    rows = []
    generated = 0
    OUTPUT_RAWS.mkdir(parents=True, exist_ok=True)

    for font_name, font_file in FONTS.items():
        font_path = INPUT_FONTS / font_file
        if not font_path.exists():
            print(f"  SKIP {font_name}: font file not found at {font_path}", file=sys.stderr)
            continue

        face = freetype.Face(str(font_path))
        for size in SIZES:
            face.set_char_size(size << 6)  # size in 26.6 units

            # ── Font-wide metrics ──
            rows.append({
                "id": f"{font_name}_{size}_getmetrics",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "",
                "operation": "getmetrics", "status": "active",
                "ref_value": get_metrics(face),
            })
            rows.append({
                "id": f"{font_name}_{size}_getname",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "",
                "operation": "getname", "status": "active",
                "ref_value": get_name(face),
            })
            rows.append({
                "id": f"{font_name}_{size}_getlength_hello",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "Hello",
                "operation": "getlength", "status": "active",
                "ref_value": get_length(face, "Hello"),
            })

            # ── Per-glyph: getmask + getbbox ──
            for ch in CHARS:
                cp = ord(ch)

                # getmask
                raw, w, h, _adv = render_glyph(face, ch)
                sha = sha256_hex(raw)
                dump_name = f"{font_name}_{size}_{cp}_getmask.bin"
                (OUTPUT_RAWS / dump_name).write_bytes(raw)
                rows.append({
                    "id": f"{font_name}_{size}_{cp}_getmask",
                    "font": font_name, "size_pt": size,
                    "codepoint": cp, "char": ch,
                    "operation": "getmask", "status": "active",
                    "ref_sha256": sha, "ref_size": [w, h],
                })
                generated += 1

                # getbbox
                rows.append({
                    "id": f"{font_name}_{size}_{cp}_getbbox",
                    "font": font_name, "size_pt": size,
                    "codepoint": cp, "char": ch,
                    "operation": "getbbox", "status": "active",
                    "ref_value": get_bbox(face, ch),
                })

    matrix = {
        "version": "0.2.0",
        "hinting": "off",
        "rows": rows,
        "summary": {
            "total_rows": len(rows),
            "active_rows": sum(1 for r in rows if r.get("status") == "active"),
            "fonts": len(FONTS),
            "sizes": len(SIZES),
            "glyphs": len(CHARS),
            "mode": "FT_LOAD_NO_HINTING",
        },
    }
    MATRIX_PATH.parent.mkdir(parents=True, exist_ok=True)
    MATRIX_PATH.write_text(json.dumps(matrix, indent=2) + "\n")
    print(f"Generated {generated} mask references, {len(rows)} matrix rows")
    print(f"Written: {MATRIX_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(generate())
