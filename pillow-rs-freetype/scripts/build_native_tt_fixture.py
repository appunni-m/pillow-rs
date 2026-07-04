#!/usr/bin/env python3
"""Generate the native TrueType default fixture from vendored FreeType.

This fixture corresponds to FreeType's normal TrueType path:
`FT_Load_Glyph(..., FT_LOAD_RENDER)`.  For TrueType fonts with bytecode, that
means fpgm, prep, and glyph programs run before grayscale rasterization.

Usage:
  cc -I pillow-rs-freetype/freetype/include \
     pillow-rs-freetype/scripts/gen_ft_refs.c \
     -L pillow-rs-freetype/freetype/build -lfreetype -lm \
     -o /tmp/gen_refs_v4
  LD_LIBRARY_PATH=pillow-rs-freetype/freetype/build \
     python3 pillow-rs-freetype/scripts/build_native_tt_fixture.py

Output:
  pillow-rs-freetype/tests/fixtures/native_tt_default_matrix.json
"""

import hashlib
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONT_DIR = os.path.join(ROOT, "tests/fixtures/input/fonts_autohint")
RAW_DIR = os.path.join(ROOT, "tests/fixtures/outputs/raws")
OUTPUT = os.path.join(ROOT, "tests/fixtures/native_tt_default_matrix.json")
REF_BIN = os.environ.get("FT_REF_BIN", "/tmp/gen_refs_v4")

FONT_MAP = {
    "DejaVuSans-ExtraLight": "DejaVuSans-ExtraLight.ttf",
    "DejaVuSans-Oblique": "DejaVuSans-Oblique.ttf",
    "DejaVuSansMono": "DejaVuSansMono.ttf",
    "DejaVuSerif-Bold": "DejaVuSerif-Bold.ttf",
    "DejaVuSerif-Italic": "DejaVuSerif-Italic.ttf",
    "LiberationSans-Regular": "LiberationSans-Regular.ttf",
    "LiberationSansNarrow-Bold": "LiberationSansNarrow-Bold.ttf",
    "NotoSans-Bold": "NotoSans-Bold.ttf",
}


def parse_ref_output(font_name, text):
    rows = []
    size = None
    for line in text.splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "SIZE":
            size = int(parts[1])
            continue
        if parts[0] == "METRICS" and size is not None:
            rows.append({
                "id": f"{font_name}_{size}_getmetrics",
                "font": font_name,
                "size_pt": size,
                "codepoint": 0,
                "char": "",
                "operation": "getmetrics",
                "status": "active",
                "ref_value": [int(parts[1]), int(parts[2])],
            })
            continue
        if parts[0] != "GLYPH" or size is None:
            continue

        cp = int(parts[1])
        width = int(parts[2])
        height = int(parts[3])
        left = int(parts[4])
        top = int(parts[5])
        advance = int(parts[6])
        pixels = bytes.fromhex("".join(parts[8:])) if len(parts) > 8 and parts[7] == "PIXELS" else b""
        row_id = f"{font_name}_{size}_{cp}_getmask"
        raw_name = f"{row_id}.bin"
        with open(os.path.join(RAW_DIR, raw_name), "wb") as raw:
            raw.write(pixels)

        rows.append({
            "id": row_id,
            "font": font_name,
            "size_pt": size,
            "codepoint": cp,
            "char": chr(cp),
            "operation": "getmask",
            "status": "active",
            "ref_sha256": hashlib.sha256(pixels).hexdigest(),
            "ref_size": [width, height],
            "ref_raw": f"outputs/raws/{raw_name}",
        })
        rows.append({
            "id": f"{font_name}_{size}_{cp}_getbbox",
            "font": font_name,
            "size_pt": size,
            "codepoint": cp,
            "char": chr(cp),
            "operation": "getbbox",
            "status": "active",
            "ref_value": [left, top - height, left + width, top],
        })
        rows.append({
            "id": f"{font_name}_{size}_{cp}_getlength",
            "font": font_name,
            "size_pt": size,
            "codepoint": cp,
            "char": chr(cp),
            "operation": "getlength",
            "status": "active",
            "ref_value": advance / 64.0,
        })
    return rows


def main():
    if not os.path.exists(REF_BIN):
        print(f"missing FreeType reference helper: {REF_BIN}", file=sys.stderr)
        return 1

    os.makedirs(RAW_DIR, exist_ok=True)
    rows = []
    for font_name, filename in FONT_MAP.items():
        font_path = os.path.join(FONT_DIR, filename)
        if not os.path.exists(font_path):
            print(f"SKIP {font_name}: {filename} not found", file=sys.stderr)
            continue
        result = subprocess.run(
            [REF_BIN, font_path, "native"],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
        )
        font_rows = parse_ref_output(font_name, result.stdout)
        rows.extend(font_rows)
        print(f"  {font_name}: {len(font_rows)} rows", file=sys.stderr)

    output = {
        "version": "3.0.0",
        "generator": "FreeType 2.14.3 FT_LOAD_RENDER native TrueType default via scripts/build_native_tt_fixture.py",
        "font_source": "fonts_autohint",
        "hinting": "native_tt_default",
        "mode": "FT_LOAD_RENDER",
        "rows": rows,
        "summary": {
            "total_rows": len(rows),
            "active_rows": len(rows),
            "fonts": len(FONT_MAP),
        },
    }
    with open(OUTPUT, "w") as fixture:
        json.dump(output, fixture, indent=2)
    print(f"\nDone: {len(rows)} rows -> {OUTPUT}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
