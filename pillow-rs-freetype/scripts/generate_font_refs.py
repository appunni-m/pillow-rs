#!/usr/bin/env python3
"""Generate font coverage matrix + reference dumps using PIL's own rendering.

PIL 12.2.0 bundles FreeType 2.14.3.  These references capture the exact
output pillow-rs-freetype's PureRust backend must match byte-for-byte.

Usage:
    python scripts/generate_font_refs.py

Requires: Pillow >= 12.2.0
"""

import hashlib
import json
import sys
from pathlib import Path

from PIL import ImageFont

ROOT = Path(__file__).parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
INPUT_FONTS = FIXTURES / "input" / "fonts_autohint"
MATRIX_PATH = FIXTURES / "coverage_matrix.json"

FONTS = {
    "DejaVuSans": "DejaVuSans.ttf",
    "DejaVuSerif": "DejaVuSerif.ttf",
    "DejaVuSansMono": "DejaVuSansMono.ttf",
    "DejaVuSansCondensed": "DejaVuSansCondensed.ttf",
    "LiberationSerif": "LiberationSerif-Regular.ttf",
    "NotoSans": "NotoSans-Regular.ttf",
    "Ubuntu": "Ubuntu-Regular.ttf",
}
FONT_NAMES = {
    "DejaVuSans": ("DejaVu Sans", "Book"),
    "DejaVuSerif": ("DejaVu Serif", "Book"),
    "DejaVuSansMono": ("DejaVu Sans Mono", "Book"),
    "DejaVuSansCondensed": ("DejaVu Sans", "Condensed"),
    "LiberationSerif": ("Liberation Serif", "Regular"),
    "NotoSans": ("Noto Sans", "Regular"),
    "Ubuntu": ("Ubuntu", "Regular"),
}
SIZES = [10, 12, 16, 20, 24]
CHARS = [chr(c) for c in range(33, 127)]  # printable ASCII


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def generate() -> int:
    rows = []
    for font_name, font_file in FONTS.items():
        font_path = INPUT_FONTS / font_file
        if not font_path.exists():
            print(f"  SKIP {font_name}: font file not found at {font_path}", file=sys.stderr)
            continue

        for size in SIZES:
            font = ImageFont.truetype(str(font_path), size)
            asc, desc = font.getmetrics()

            rows.append({
                "id": f"{font_name}_{size}_getmetrics",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "",
                "operation": "getmetrics", "status": "active",
                "ref_value": [asc, desc],
            })
            rows.append({
                "id": f"{font_name}_{size}_getname",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "",
                "operation": "getname", "status": "active",
                "ref_value": list(FONT_NAMES[font_name]),
            })
            rows.append({
                "id": f"{font_name}_{size}_getlength_hello",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "Hello",
                "operation": "getlength", "status": "active",
                "ref_value": font.getlength("Hello"),
            })

            for ch in CHARS:
                cp = ord(ch)
                mask = font.getmask(ch)
                pixels = bytes(mask)
                sha = sha256_hex(pixels) if pixels else sha256_hex(b"\x00")

                rows.append({
                    "id": f"{font_name}_{size}_{cp}_getmask",
                    "font": font_name, "size_pt": size,
                    "codepoint": cp, "char": ch,
                    "operation": "getmask", "status": "active",
                    "ref_sha256": sha,
                    "ref_size": list(mask.size),
                })

                bbox = font.getbbox(ch)
                rows.append({
                    "id": f"{font_name}_{size}_{cp}_getbbox",
                    "font": font_name, "size_pt": size,
                    "codepoint": cp, "char": ch,
                    "operation": "getbbox", "status": "active",
                    "ref_value": list(bbox),
                })

    matrix = {
        "version": "0.4.0",
        "font_source": "fonts_autohint",
        "hinting": "autohint",
        "generator": "PIL 12.2.0 (FreeType 2.14.3)",
        "rows": rows,
        "summary": {
            "total_rows": len(rows),
            "active_rows": sum(1 for r in rows if r.get("status") == "active"),
            "fonts": len(FONTS),
            "sizes": len(SIZES),
            "glyphs": len(CHARS),
        },
    }
    MATRIX_PATH.parent.mkdir(parents=True, exist_ok=True)
    MATRIX_PATH.write_text(json.dumps(matrix, indent=2) + "\n")
    print(f"Generated {len(rows)} matrix rows (PIL 12.2.0 FreeType 2.14.3)")
    print(f"Written: {MATRIX_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(generate())
