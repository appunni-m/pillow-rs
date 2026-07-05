#!/usr/bin/env python3
"""Build PIL 12.2.0 `_imagingft.c` integration oracle fixtures.

The generated matrix is consumed by `pillow-rs/tests/imagingft_matrix_tests.rs`.
It intentionally stores raw pixel bytes outside the JSON so byte parity failures
can be diagnosed without lossy hashes.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
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
NATIVE_TT_MATRIX = FREETYPE_FIXTURE_DIR / "native_tt_default_matrix.json"
LARGE_PIXEL_ROW_LIMIT = 3_816


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


def add_raw(rows: list[dict], row_id: str, operation: str, text: str, mode: str, size, data: bytes, offset=None):
    row = {
        "id": row_id,
        "operation": operation,
        "font": FONT_REF,
        "size": 20,
        "text": text,
        "mode": mode,
        "status": "incomplete",
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


def ensure_native_tt_matrix() -> None:
    if NATIVE_TT_MATRIX.exists():
        return
    result = subprocess.run(
        ["make", "-C", str(ROOT / "pillow-rs-freetype"), "fixture-native-tt-default"],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        raise SystemExit(
            "failed to generate native TrueType matrix for imagingft fixtures\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )


def add_large_pixel_matrix(rows: list[dict]) -> None:
    ensure_native_tt_matrix()
    native = json.loads(NATIVE_TT_MATRIX.read_text(encoding="utf-8"))
    selected = [
        row
        for row in native["rows"]
        if row["operation"] == "getmask" and row.get("ref_raw") and row.get("ref_sha256")
    ][:LARGE_PIXEL_ROW_LIMIT]

    for row in selected:
        font_path = FREETYPE_FIXTURE_DIR / "input" / "fonts_autohint" / f"{row['font']}.ttf"
        font_ref = f"../pillow-rs-freetype/tests/fixtures/input/fonts_autohint/{row['font']}.ttf"
        font = ImageFont.truetype(str(font_path), int(row["size_pt"]))
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
        scalar_status = "incomplete" if text == "AV" else "parity"
        add_scalar(
            rows,
            f"dejavusans20_{slug}_getbbox",
            "getbbox",
            text,
            list(font.getbbox(text)),
            scalar_status,
        )
        add_scalar(
            rows,
            f"dejavusans20_{slug}_getlength",
            "getlength",
            text,
            font.getlength(text),
            scalar_status,
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
