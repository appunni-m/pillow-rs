#!/usr/bin/env python3
"""Generate font coverage matrix + reference dumps from PIL's FreeType.

For each (font, size, glyph, operation) tuple, runs PIL's ImageFont
and produces SHA-256 references. Outputs:
  - tests/fixtures/coverage_matrix.json  (committed)
  - tests/fixtures/outputs/raws/*.bin   (pixel dumps, committed)
"""
import json, hashlib, sys
from pathlib import Path
from PIL import ImageFont, Image

ROOT = Path(__file__).parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
INPUT_FONTS = FIXTURES / "input" / "fonts"
OUTPUT_RAWS = FIXTURES / "outputs" / "raws"
MATRIX_PATH = FIXTURES / "coverage_matrix.json"

# Test configuration
FONTS = {
    "DejaVuSans": "DejaVuSans.ttf",
    "LiberationSerif": "LiberationSerif-Regular.ttf",
}
SIZES = [10, 12, 16, 20, 24]
# Printable ASCII (skip whitespace: 32=space, 9=tab, 10=LF)
CHARS = [chr(c) for c in range(33, 127)]
ALL_CHARS = CHARS
OPERATIONS = ["getmask", "getbbox", "getmetrics", "getname", "getlength", "font_variant"]

# Set to "fonts_nohint" to use fonts with stripped bytecode programs
FONT_SUBDIR = "fonts"  # change to "fonts_nohint" for unhinted references


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def generate() -> int:
    rows = []
    generated = 0

    for font_name, font_file in FONTS.items():
        font_path = INPUT_FONTS / FONT_SUBDIR / font_file
        if not font_path.exists():
            print(f"  SKIP {font_name}: font file not found at {font_path}", file=sys.stderr)
            continue

        for size in SIZES:
            font = ImageFont.truetype(str(font_path), size)

            # Font-wide operations (not per-glyph)
            metrics = font.getmetrics()
            name = font.getname()
            rows.append({
                "id": f"{font_name}_{size}_getmetrics",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "",
                "operation": "getmetrics",
                "status": "active",
                "ref_value": list(metrics),
            })
            rows.append({
                "id": f"{font_name}_{size}_getname",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "",
                "operation": "getname",
                "status": "active",
                "ref_value": list(name),
            })
            # getlength with "Hello"
            hello_len = font.getlength("Hello")
            rows.append({
                "id": f"{font_name}_{size}_getlength_hello",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "Hello",
                "operation": "getlength",
                "status": "active",
                "ref_value": hello_len,
            })

            # Per-glyph operations
            for ch in ALL_CHARS:
                cp = ord(ch)
                # getmask
                try:
                    mask = font.getmask(ch)
                    raw = bytes(mask)
                    sha = sha256_hex(raw)
                    # Write raw pixel dump
                    dump_name = f"{font_name}_{size}_{cp}_getmask.bin"
                    OUTPUT_RAWS.mkdir(parents=True, exist_ok=True)
                    (OUTPUT_RAWS / dump_name).write_bytes(raw)

                    rows.append({
                        "id": f"{font_name}_{size}_{cp}_getmask",
                        "font": font_name, "size_pt": size,
                        "codepoint": cp, "char": ch,
                        "operation": "getmask",
                        "status": "active",
                        "ref_sha256": sha,
                        "ref_size": list(mask.size),
                    })
                    generated += 1
                except Exception as e:
                    print(f"  SKIP getmask {font_name} {size}pt U+{cp:04X}: {e}", file=sys.stderr)

                # getbbox
                try:
                    bbox = font.getbbox(ch)
                    rows.append({
                        "id": f"{font_name}_{size}_{cp}_getbbox",
                        "font": font_name, "size_pt": size,
                        "codepoint": cp, "char": ch,
                        "operation": "getbbox",
                        "status": "active",
                        "ref_value": list(bbox) if bbox else [0, 0, 0, 0],
                    })
                    generated += 1
                except Exception as e:
                    print(f"  SKIP getbbox {font_name} {size}pt U+{cp:04X}: {e}", file=sys.stderr)

    matrix = {
        "version": "0.1.0",
        "rows": rows,
        "summary": {
            "total_rows": len(rows),
            "active_rows": sum(1 for r in rows if r.get("status") == "active"),
            "fonts": len(FONTS),
            "sizes": len(SIZES),
            "glyphs": len(ALL_CHARS),
        },
    }
    MATRIX_PATH.parent.mkdir(parents=True, exist_ok=True)
    MATRIX_PATH.write_text(json.dumps(matrix, indent=2) + "\n")
    print(f"Generated {generated} references, {len(rows)} matrix rows")
    print(f"Written: {MATRIX_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(generate())
