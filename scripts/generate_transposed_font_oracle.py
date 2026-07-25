#!/usr/bin/env python3
"""Generate the shared independent-path Pillow TransposedFont oracle."""

import json
from pathlib import Path

from PIL import Image, ImageFont, __version__ as pillow_version


ROOT = Path(__file__).resolve().parent.parent
EXPECTED_PILLOW = "12.2.0"
EXPECTED_FREETYPE = "2.14.3"
TEXT = "Parity"
ORIENTATIONS = (None, "ROTATE_90", "FLIP_LEFT_RIGHT")

if pillow_version != EXPECTED_PILLOW:
    raise RuntimeError(f"expected Pillow {EXPECTED_PILLOW}, found {pillow_version}")
if ImageFont.core.freetype2_version != EXPECTED_FREETYPE:
    raise RuntimeError(
        f"expected FreeType {EXPECTED_FREETYPE}, "
        f"found {ImageFont.core.freetype2_version}"
    )

font = ImageFont.load_default()
cases = []
for orientation_name in ORIENTATIONS:
    orientation = (
        getattr(Image.Transpose, orientation_name)
        if orientation_name is not None
        else None
    )
    transposed = ImageFont.TransposedFont(font, orientation)
    mask = transposed.getmask(TEXT)
    expected = {
        "bbox": list(transposed.getbbox(TEXT)),
        "mask": {
            "mode": mask.mode,
            "size": list(mask.size),
            "pixels_hex": bytes(mask).hex(),
        },
    }
    try:
        expected["length"] = transposed.getlength(TEXT)
    except ValueError as error:
        expected["length_error"] = {
            "type": type(error).__name__,
            "message": str(error),
        }
    cases.append(
        {
            "id": "untransposed" if orientation_name is None else orientation_name.lower(),
            "orientation": orientation_name,
            "text": TEXT,
            "expected": expected,
        }
    )

path = ROOT / "pillow-rs/tests/fixtures/imagefont/transposed_font.json"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(
    json.dumps(
        {
            "format_version": 1,
            "oracle": {
                "implementation": "Pillow",
                "version": pillow_version,
                "freetype_version": ImageFont.core.freetype2_version,
            },
            "font": {"kind": "load_default", "size": 10},
            "cases": cases,
        },
        indent=2,
    )
    + "\n"
)
