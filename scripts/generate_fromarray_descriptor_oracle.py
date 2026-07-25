#!/usr/bin/env python3
"""Generate independent Pillow array-interface descriptor outcomes."""

import json
import math
from pathlib import Path

from PIL import Image, __version__ as pillow_version


ROOT = Path(__file__).resolve().parent.parent
EXPECTED_PILLOW = "12.2.0"
if pillow_version != EXPECTED_PILLOW:
    raise RuntimeError(f"expected Pillow {EXPECTED_PILLOW}, found {pillow_version}")

CASES = [
    ("inferred_l", [2, 3], "|u1", None),
    ("inferred_rgb", [2, 3, 3], "|u1", None),
    ("inferred_rgba", [2, 3, 4], "|u1", None),
    ("inferred_i16_le", [2, 3], "<i2", None),
    ("inferred_f32_be", [2, 3], ">f4", None),
    ("explicit_palette", [2, 3], "|u1", "P"),
    ("one_dimensional", [5], "|u1", None),
    ("unsupported_channels", [2, 3, 5], "|u1", None),
    ("unsupported_typestr", [2, 3], "|u8", None),
    ("too_many_l_dimensions", [1, 2, 3], "|u1", "L"),
]


def item_size(typestr):
    digits = "".join(character for character in typestr if character.isdigit())
    return max(1, int(digits or "1"))


class DescriptorArray:
    def __init__(self, shape, typestr):
        byte_count = math.prod(shape) * item_size(typestr)
        self._data = bytes(byte_count)
        self.__array_interface__ = {
            "shape": tuple(shape),
            "strides": tuple(0 for _ in shape),
            "typestr": typestr,
            "version": 3,
            "data": self._data,
        }

    def tobytes(self):
        return self._data


cases = []
for case_id, shape, typestr, mode in CASES:
    try:
        image = Image.fromarray(DescriptorArray(shape, typestr), mode=mode)
        expected = {"mode": image.mode, "size": list(image.size)}
    except Exception as error:
        expected = {
            "error": {
                "type": type(error).__name__,
                "message": str(error),
            }
        }
    cases.append(
        {
            "id": case_id,
            "shape": shape,
            "typestr": typestr,
            "mode": mode,
            "expected": expected,
        }
    )

path = ROOT / "pillow-rs/tests/fixtures/fromarray/descriptor.json"
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
