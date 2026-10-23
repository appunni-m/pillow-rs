"""Input-only parity cases for quantization boundaries and palette output."""

from __future__ import annotations

import base64
import hashlib
from typing import Any


QUANTIZE_SURFACE = "PIL.Image.Image"
QUANTIZE_COLORS_REQUIREMENT = f"{QUANTIZE_SURFACE}.quantize.parameter.colors"
QUANTIZE_METHOD_REQUIREMENT = f"{QUANTIZE_SURFACE}.quantize.parameter.method"


def _literal(value: Any) -> dict[str, Any]:
    return {"kind": "literal", "value": value}


def _binding(step_id: str) -> dict[str, str]:
    return {"kind": "binding", "step_id": step_id}


def _step(
    step_id: str,
    surface: str,
    operation: str,
    receiver: dict[str, str] | None,
    arguments: dict[str, Any],
) -> dict[str, Any]:
    return {
        "step_id": step_id,
        "surface": surface,
        "operation": operation,
        "receiver": receiver,
        "arguments": arguments,
    }


def _palette_case(
    case_id: str,
    mode: str,
    rawmode: str,
    pixels: bytes,
    *,
    method: int,
) -> dict[str, Any]:
    asset_id = "source-pixels"
    return {
        "case_id": case_id,
        "surface": QUANTIZE_SURFACE,
        "operation": "quantize",
        "covers": [QUANTIZE_COLORS_REQUIREMENT],
        "assets": [
            {
                "id": asset_id,
                "kind": "inline",
                "encoding": "base64",
                "data": base64.b64encode(pixels).decode("ascii"),
                "sha256": hashlib.sha256(pixels).hexdigest(),
                "media_type": "application/octet-stream",
            }
        ],
        "steps": [
            _step(
                "image",
                "PIL.Image",
                "frombytes",
                None,
                {
                    "mode": _literal(mode),
                    "size": _literal([3, 1]),
                    "data": {"kind": "asset", "asset_id": asset_id},
                },
            ),
            _step(
                "quantized",
                QUANTIZE_SURFACE,
                "quantize",
                _binding("image"),
                {
                    "colors": _literal(1),
                    "method": _literal(method),
                    "kmeans": _literal(0),
                },
            ),
            _step(
                "palette",
                QUANTIZE_SURFACE,
                "getpalette",
                _binding("quantized"),
                {"rawmode": _literal(rawmode)},
            ),
            _step(
                "indices",
                QUANTIZE_SURFACE,
                "tobytes",
                _binding("quantized"),
                {},
            ),
        ],
        "observations": ["quantized", "palette", "indices"],
    }


def _invalid_upper_color_count_case() -> dict[str, Any]:
    case_id = f"{QUANTIZE_SURFACE}.quantize.nuanced.invalid-color-count-above-maximum"
    return {
        "case_id": case_id,
        "surface": QUANTIZE_SURFACE,
        "operation": "quantize",
        "covers": [QUANTIZE_COLORS_REQUIREMENT],
        "assets": [],
        "steps": [
            _step(
                "image",
                "PIL.Image",
                "new",
                None,
                {
                    "mode": _literal("RGB"),
                    "size": _literal([1, 1]),
                    "color": _literal([10, 20, 30]),
                },
            ),
            _step(
                "quantized",
                QUANTIZE_SURFACE,
                "quantize",
                _binding("image"),
                {"colors": _literal(257)},
            ),
        ],
        "observations": ["quantized"],
    }


def _large_fast_octree_average_case() -> dict[str, Any]:
    case_id = (
        f"{QUANTIZE_SURFACE}.quantize.nuanced."
        "fast-octree-large-bucket-float-average-4096x4096"
    )
    return {
        "case_id": case_id,
        "surface": QUANTIZE_SURFACE,
        "operation": "quantize",
        "covers": [QUANTIZE_METHOD_REQUIREMENT],
        "assets": [],
        "steps": [
            _step(
                "image",
                "PIL.Image",
                "new",
                None,
                {
                    "mode": _literal("RGBA"),
                    "size": _literal([4096, 4096]),
                    "color": _literal([255, 255, 255, 255]),
                },
            ),
            _step(
                "rare-color",
                QUANTIZE_SURFACE,
                "putpixel",
                _binding("image"),
                {
                    "xy": _literal([0, 0]),
                    "value": _literal([254, 255, 255, 255]),
                },
            ),
            _step(
                "quantized",
                QUANTIZE_SURFACE,
                "quantize",
                _binding("image"),
                {
                    "colors": _literal(2),
                    "method": _literal(2),
                    "kmeans": _literal(0),
                },
            ),
            _step(
                "palette",
                QUANTIZE_SURFACE,
                "getpalette",
                _binding("quantized"),
                {"rawmode": _literal("RGBA")},
            ),
        ],
        # Keep the declared quantize result observable alongside its palette.
        "observations": ["quantized", "palette"],
    }


def quantize_boundary_parity_cases(surface_id: str) -> list[dict[str, Any]]:
    """Return public parity cases for quantize color bounds and FASTOCTREE."""
    if surface_id != QUANTIZE_SURFACE:
        return []
    return [
        _palette_case(
            f"{QUANTIZE_SURFACE}.quantize.nuanced.colors-one-rgb-palette",
            "RGB",
            "RGB",
            bytes([10, 20, 30, 250, 240, 230, 20, 70, 130]),
            method=0,
        ),
        _palette_case(
            f"{QUANTIZE_SURFACE}.quantize.nuanced.colors-one-rgba-palette",
            "RGBA",
            "RGBA",
            bytes([10, 20, 30, 40, 250, 240, 230, 220, 20, 70, 130, 190]),
            method=2,
        ),
        _invalid_upper_color_count_case(),
        _large_fast_octree_average_case(),
    ]
