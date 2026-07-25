#!/usr/bin/env python3
"""Generate Pillow's Python-ABI Image.eval argument-error oracle."""

import json
from pathlib import Path

from PIL import Image, __version__ as pillow_version


ROOT = Path(__file__).resolve().parent.parent
EXPECTED_PILLOW = "12.2.0"
if pillow_version != EXPECTED_PILLOW:
    raise RuntimeError(f"expected Pillow {EXPECTED_PILLOW}, found {pillow_version}")

image = Image.new("L", (1, 1))
cases = []
for case_id, args in (("missing_argument", ()), ("non_iterable_integer", (3,))):
    try:
        Image.eval(image, *args)
    except Exception as error:
        cases.append(
            {
                "id": case_id,
                "argument": None if not args else args[0],
                "expected": {
                    "type": type(error).__name__,
                    "message": str(error),
                },
            }
        )
    else:
        raise RuntimeError(f"{case_id}: Pillow unexpectedly accepted invalid input")

path = ROOT / "pillow-rs/tests/fixtures/image_eval/python_abi_errors.json"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(
    json.dumps(
        {
            "format_version": 1,
            "oracle": {"implementation": "Pillow", "version": pillow_version},
            "cases": cases,
        },
        indent=2,
    )
    + "\n"
)
