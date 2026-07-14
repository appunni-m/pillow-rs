#!/usr/bin/env python3
"""Build PIL 12.2.0 `_imagingft.c` integration oracle fixtures.

The generated matrix is consumed by `pillow-rs/tests/imagingft_matrix_tests.rs`.
It intentionally stores raw pixel bytes outside the JSON so byte parity failures
can be diagnosed without lossy hashes.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont
import PIL


ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "pillow-rs"
FIXTURE_DIR = CRATE / "tests" / "fixtures"
RAW_DIR = FIXTURE_DIR / "outputs" / "imagingft" / "raws"
MATRIX_PATH = FIXTURE_DIR / "imagingft_matrix.json"

FONT_PATH = ROOT / "pillow-rs-freetype" / "tests" / "fixtures" / "input" / "fonts" / "DejaVuSans.ttf"
FONT_REF = "../pillow-rs-freetype/tests/fixtures/input/fonts/DejaVuSans.ttf"
FREETYPE_FIXTURE_DIR = ROOT / "pillow-rs-freetype" / "tests" / "fixtures"
LARGE_PIXEL_ROW_LIMIT = 3_816
COMPACT_PIXEL_SIZES = [7, 8, 9, 10, 11, 12, 13, 14, 16, 18, 20, 22, 24, 28, 32, 40]


def codepoints(text: str) -> list[int]:
    return [ord(ch) for ch in text]


def unique_codepoints(values: list[int]) -> list[int]:
    seen: set[int] = set()
    out: list[int] = []
    for value in values:
        if value not in seen:
            out.append(value)
            seen.add(value)
    return out


COMPACT_PIXEL_FONTS = [
    {
        "id": "dejavu-native",
        "path": FREETYPE_FIXTURE_DIR / "fonts" / "native" / "dejavu-coverage.ttf",
        "ref": "../pillow-rs-freetype/tests/fixtures/fonts/native/dejavu-coverage.ttf",
        "codepoints": unique_codepoints(
            codepoints('!"#%&()+0128@ABCEHIJLMOQSTUVWXZabcdefghijklmnopqrstuvwxyz')
            + [
                0x00C0,
                0x00C5,
                0x00C7,
                0x00D6,
                0x00DF,
                0x00E0,
                0x00E4,
                0x00E9,
                0x00F6,
                0x0108,
                0x011C,
                0x011D,
                0x0153,
                0x0392,
                0x0393,
                0x0394,
                0x0395,
                0x0396,
                0x0398,
                0x039E,
                0x039F,
                0x03A9,
                0x03B1,
                0x03B2,
                0x03B3,
            ]
        ),
    },
    {
        "id": "latin-basic",
        "path": FREETYPE_FIXTURE_DIR / "fonts" / "autohint" / "basic-latin.ttf",
        "ref": "../pillow-rs-freetype/tests/fixtures/fonts/autohint/basic-latin.ttf",
        "codepoints": unique_codepoints(
            [0x20]
            + codepoints("0123456789AHOVXafijmnopqx")
            + [
                0x00C0,
                0x00E9,
                0x0301,
                0x0393,
                0x03B1,
                0x0411,
                0x043E,
                0x0915,
                0x0925,
                0x0930,
                0x093E,
                0x094D,
                0x3007,
                0x4E00,
                0x4E09,
                0x53E3,
                0x65E5,
                0x6C38,
                0x7530,
            ]
        ),
    },
    {
        "id": "latin-greek-cyrillic",
        "path": FREETYPE_FIXTURE_DIR / "fonts" / "autohint" / "latin-greek-cyrillic.ttf",
        "ref": "../pillow-rs-freetype/tests/fixtures/fonts/autohint/latin-greek-cyrillic.ttf",
        "codepoints": unique_codepoints(
            [0x20]
            + codepoints("0123456789AHOVXamnopqx")
            + [
                0x00C0,
                0x00E9,
                0x0301,
                0x0393,
                0x03B1,
                0x03BF,
                0x0411,
                0x043E,
                0x0915,
                0x0925,
                0x0930,
                0x093E,
                0x094D,
                0x3007,
                0x4E00,
                0x4E09,
                0x53E3,
                0x65E5,
                0x6C38,
                0x7530,
            ]
        ),
    },
    {
        "id": "indic",
        "path": FREETYPE_FIXTURE_DIR / "fonts" / "autohint" / "indic-coverage.ttf",
        "ref": "../pillow-rs-freetype/tests/fixtures/fonts/autohint/indic-coverage.ttf",
        "codepoints": unique_codepoints(
            [0x20]
            + codepoints("A")
            + [
                0x0393,
                0x03B1,
                0x0411,
                0x043E,
                0x0915,
                0x0920,
                0x0925,
                0x0930,
                0x093E,
                0x094D,
                0x0966,
                0x0967,
                0x0968,
                0x0969,
                0x096A,
                0x096B,
                0x096C,
                0x096D,
                0x096E,
                0x096F,
                0x3007,
                0x4E00,
                0x4E09,
                0x53E3,
                0x65E5,
                0x6C38,
                0x7530,
            ]
        ),
    },
    {
        "id": "cjk",
        "path": FREETYPE_FIXTURE_DIR / "fonts" / "autohint" / "cjk-coverage.ttf",
        "ref": "../pillow-rs-freetype/tests/fixtures/fonts/autohint/cjk-coverage.ttf",
        "codepoints": unique_codepoints(
            [0x20]
            + codepoints("!AHIMOVno")
            + [
                0x00C3,
                0x00D8,
                0x00F8,
                0x0393,
                0x03B1,
                0x0411,
                0x043E,
                0x0915,
                0x0925,
                0x0930,
                0x093E,
                0x094D,
                0x1E1A,
                0x1E4C,
                0x3007,
                0x4E00,
                0x4E09,
                0x4E2A,
                0x4E3B,
                0x4ED6,
                0x519B,
                0x519C,
                0x519D,
                0x53E3,
                0x65E5,
                0x6C38,
                0x7530,
            ]
        ),
    },
    {
        "id": "render-coverage",
        "path": FREETYPE_FIXTURE_DIR / "fonts" / "glyf" / "render-coverage.ttf",
        "ref": "../pillow-rs-freetype/tests/fixtures/fonts/glyf/render-coverage.ttf",
        "codepoints": [0xE100, 0xE101, 0xE102, 0xE103, 0xE104],
    },
]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_raw(row_id: str, data: bytes) -> str:
    RAW_DIR.mkdir(parents=True, exist_ok=True)
    path = RAW_DIR / f"{row_id}.bin"
    path.write_bytes(data)
    return str(path.relative_to(FIXTURE_DIR))


def add_scalar(rows: list[dict], row_id: str, operation: str, text: str, expected, status: str = "parity"):
    rows.append(
        {
            "id": row_id,
            "operation": operation,
            "font": FONT_REF,
            "size": 20,
            "text": text,
            "status": status,
            "expected": expected,
        }
    )


def add_raw(
    rows: list[dict],
    row_id: str,
    operation: str,
    text: str,
    mode: str,
    size,
    data: bytes,
    offset=None,
    status: str = "parity",
):
    row = {
        "id": row_id,
        "operation": operation,
        "font": FONT_REF,
        "size": 20,
        "text": text,
        "mode": mode,
        "status": status,
        "expected_size": list(size),
        "expected_sha256": sha256(data),
        "expected_raw": write_raw(row_id, data),
    }
    if offset is not None:
        row["expected_offset"] = list(offset)
    rows.append(row)


def add_hash_only_raw(
    rows: list[dict],
    row_id: str,
    operation: str,
    font: str,
    size_pt: int,
    text: str,
    size,
    data: bytes,
    offset=None,
):
    row = {
        "id": row_id,
        "operation": operation,
        "font": font,
        "size": size_pt,
        "text": text,
        "mode": "L",
        "status": "pixel_matrix",
        "expected_size": list(size),
        "expected_sha256": sha256(data),
    }
    if offset is not None:
        row["expected_offset"] = list(offset)
    rows.append(row)


def compact_pixel_rows() -> list[dict]:
    rows: list[dict] = []
    for font in COMPACT_PIXEL_FONTS:
        for size_pt in COMPACT_PIXEL_SIZES:
            for codepoint in font["codepoints"]:
                rows.append(
                    {
                        "id": f"{font['id']}_s{size_pt}_u{codepoint:04x}",
                        "font": font["ref"],
                        "font_path": font["path"],
                        "size_pt": size_pt,
                        "char": chr(codepoint),
                    }
                )
    if len(rows) < LARGE_PIXEL_ROW_LIMIT:
        raise SystemExit(
            f"compact imagingft pixel matrix has {len(rows)} rows; "
            f"expected at least {LARGE_PIXEL_ROW_LIMIT}"
        )
    return rows[:LARGE_PIXEL_ROW_LIMIT]


def add_large_pixel_matrix(rows: list[dict]) -> None:
    selected = compact_pixel_rows()
    font_cache: dict[tuple[Path, int], ImageFont.FreeTypeFont] = {}

    for row in selected:
        font_path = row["font_path"]
        font_ref = row["font"]
        size_pt = int(row["size_pt"])
        font_key = (font_path, size_pt)
        font = font_cache.get(font_key)
        if font is None:
            font = ImageFont.truetype(str(font_path), size_pt)
            font_cache[font_key] = font
        mask = font.getmask(row["char"], mode="L")
        mask_bytes = bytes(mask)

        mask2, offset = font.getmask2(row["char"], mode="L")
        if mask2.size != mask.size or bytes(mask2) != mask_bytes:
            raise SystemExit(f"{row['id']} getmask2 pixels differ from getmask")

        # The native FreeType matrix supplies deterministic font/glyph/size
        # coverage. PIL `_imagingft` adds run-origin padding for some glyphs,
        # so this parent matrix stores PIL's own byte hash instead of reusing
        # raw FreeType slot bitmaps.
        add_hash_only_raw(
            rows,
            f"{row['id']}_pil122_getmask",
            "getmask",
            font_ref,
            int(row["size_pt"]),
            row["char"],
            mask.size,
            mask_bytes,
        )
        add_hash_only_raw(
            rows,
            f"{row['id']}_pil122_getmask2",
            "getmask2",
            font_ref,
            int(row["size_pt"]),
            row["char"],
            mask.size,
            mask_bytes,
            offset,
        )


def main() -> None:
    if PIL.__version__ != "12.2.0":
        raise SystemExit(f"expected Pillow 12.2.0, got {PIL.__version__}")

    font = ImageFont.truetype(str(FONT_PATH), 20)
    rows: list[dict] = []

    for text in ("Hello", "AV", "jQ"):
        slug = "".join(ch if ch.isalnum() else f"{ord(ch):x}" for ch in text).lower()
        add_scalar(
            rows,
            f"dejavusans20_{slug}_getbbox",
            "getbbox",
            text,
            list(font.getbbox(text)),
        )
        add_scalar(
            rows,
            f"dejavusans20_{slug}_getlength",
            "getlength",
            text,
            font.getlength(text),
        )

        mask = font.getmask(text, mode="L")
        add_raw(rows, f"dejavusans20_{slug}_getmask_l", "getmask", text, "L", mask.size, bytes(mask))

        mask2, offset = font.getmask2(text, mode="L")
        add_raw(
            rows,
            f"dejavusans20_{slug}_getmask2_l",
            "getmask2",
            text,
            "L",
            mask2.size,
            bytes(mask2),
            offset,
        )

        image = Image.new("RGBA", (96, 64), (0, 0, 0, 0))
        draw = ImageDraw.Draw(image)
        draw.text((10, 18), text, font=font, fill=(20, 40, 200, 255))
        add_raw(rows, f"dejavusans20_{slug}_draw_text_rgba", "draw_text", text, "RGBA", image.size, image.tobytes())

    add_scalar(rows, "dejavusans20_getmetrics", "getmetrics", "", list(font.getmetrics()))
    add_scalar(rows, "dejavusans20_getname", "getname", "", list(font.getname()))
    add_large_pixel_matrix(rows)

    matrix = {
        "fixture_family": "pillow-rs-imagingft",
        "generator": "pillow-rs/scripts/build_imagingft_fixtures.py",
        "pillow_version": PIL.__version__,
        "oracle": "PIL.ImageFont.FreeTypeFont and PIL.ImageDraw text output",
        "pixel_matrix_min_passed": 7000,
        "rows": rows,
    }
    FIXTURE_DIR.mkdir(parents=True, exist_ok=True)
    tmp_path = MATRIX_PATH.with_suffix(".json.tmp")
    tmp_path.write_text(json.dumps(matrix, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp_path.replace(MATRIX_PATH)


if __name__ == "__main__":
    main()
