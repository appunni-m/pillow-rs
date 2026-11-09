"""Recorded target-only outputs for supported Rust mode extensions.

These contracts use the public Python facade and intentionally have no Pillow
oracle. The raw fixtures are fixed byte strings so generated inputs are stable
across hosts; the Rust mode parser interprets typed samples in native endian.
"""

from __future__ import annotations

import base64
import hashlib
from typing import Any


CORE_ONLY_RAW_MODES: tuple[tuple[str, str, bytes], ...] = (
    ("ImageLumaA16", "core-only-luma-a16", bytes.fromhex("34127856")),
    ("ImageRgb16", "core-only-rgb16", bytes.fromhex("34127856bc9a")),
    ("ImageRgba16", "core-only-rgba16", bytes.fromhex("34127856bc9af0de")),
    ("ImageRgb32F", "core-only-rgb32f", bytes.fromhex("0000a03f000000bf00004040")),
    (
        "ImageRgba32F",
        "core-only-rgba32f",
        bytes.fromhex("0000a03f000000bf000040400000403f"),
    ),
)


def core_only_frombytes_cases() -> list[dict[str, Any]]:
    """Return thin-Python, recorded-output cases for all five internal modes."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, raw in CORE_ONLY_RAW_MODES:
        encoded = base64.b64encode(raw).decode("ascii")
        image_record = {
            "kind": "image",
            "mode": mode,
            "size": [1, 1],
            "format": None,
            "info": {},
            "palette": None,
            "bytes": encoded,
        }
        cases.append(
            {
                "case_id": f"PIL.Image.frombytes.expected-output.{mode_id}",
                "surface": "PIL.Image",
                "operation": "frombytes",
                "verification": "expected-output",
                "covers": [
                    "PIL.Image.frombytes.behavior.default",
                    f"PIL.Image.frombytes.mode.{mode_id}",
                ],
                "assets": [
                    {
                        "id": "raw-mode-data",
                        "kind": "inline",
                        "encoding": "base64",
                        "data": encoded,
                        "sha256": hashlib.sha256(raw).hexdigest(),
                        "media_type": "application/octet-stream",
                    }
                ],
                "steps": [
                    {
                        "step_id": "image",
                        "surface": "PIL.Image",
                        "operation": "frombytes",
                        "receiver": None,
                        "arguments": {
                            "mode": {"kind": "literal", "value": mode},
                            "size": {"kind": "literal", "value": [1, 1]},
                            "data": {"kind": "asset", "asset_id": "raw-mode-data"},
                        },
                    }
                ],
                "observations": ["image"],
                "target_expectation": {
                    "observations": [{"step_id": "image", "value": image_record}]
                },
            }
        )
    return cases
