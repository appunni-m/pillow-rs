#!/usr/bin/env python3
"""Generate independent Pillow ImageFont.getmask2 inputs and shared oracle."""

import json
from pathlib import Path

from PIL import ImageFont, __version__ as pillow_version


ROOT = Path(__file__).resolve().parent.parent
EXPECTED_PILLOW = "12.2.0"
EXPECTED_FREETYPE = "2.14.3"


def write_suite(relative_dir, suite, cases):
    path = ROOT / relative_dir / "input/jsons/ImageFont.getmask2.json"
    document = {
        "format_version": 2,
        **({"suite": suite} if suite else {}),
        "operation": {"module": "ImageFont", "target": "getmask2"},
        "cases": cases,
    }
    path.write_text(json.dumps(document, indent=2) + "\n")


suite0 = [
    {"id": "baseline", "params": {"text": "Hello"}},
    {"id": "integer_start", "params": {"text": "Hello", "start": [3, 4]}},
    {
        "id": "fractional_start",
        "params": {"text": "Hello", "start": [0.5, 0.75]},
    },
]
suite1 = [
    {"id": "empty_suite1", "params": {"text": ""}},
    {"id": "unicode_suite1", "params": {"text": "é"}},
]

if pillow_version != EXPECTED_PILLOW:
    raise RuntimeError(f"expected Pillow {EXPECTED_PILLOW}, found {pillow_version}")
if ImageFont.core.freetype2_version != EXPECTED_FREETYPE:
    raise RuntimeError(
        f"expected FreeType {EXPECTED_FREETYPE}, "
        f"found {ImageFont.core.freetype2_version}"
    )

write_suite("tests/fixtures", 0, suite0)
write_suite("tests/fixtures_2", 1, suite1)

font = ImageFont.load_default()
shared_cases = []
for case in [*suite0, *suite1]:
    params = case["params"]
    mask, offset = font.getmask2(
        params["text"],
        start=tuple(params["start"]) if "start" in params else None,
    )
    shared_cases.append(
        {
            "id": case["id"],
            "text": params["text"],
            "start": params.get("start"),
            "expected": {
                "mode": mask.mode,
                "size": list(mask.size),
                "offset": list(offset),
                "pixels_hex": bytes(mask).hex(),
            },
        }
    )

shared_path = (
    ROOT / "pillow-rs/tests/fixtures/imagefont/getmask2.json"
)
shared_path.parent.mkdir(parents=True, exist_ok=True)
shared_path.write_text(
    json.dumps(
        {
            "format_version": 1,
            "oracle": {
                "implementation": "Pillow",
                "version": pillow_version,
                "freetype_version": ImageFont.core.freetype2_version,
            },
            "cases": shared_cases,
        },
        indent=2,
    )
    + "\n"
)
