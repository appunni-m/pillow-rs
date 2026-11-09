"""Recorded target-only outputs for supported Rust mode extensions.

These contracts use the public Python facade and intentionally have no Pillow
oracle. The raw fixtures are fixed byte strings so generated inputs are stable
across hosts; the Rust mode parser interprets typed samples in native endian.
"""

from __future__ import annotations

import base64
import hashlib
import struct
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


# Sharpness has exact native strategies for these 16 modes, but no Pillow
# parity contract. An impulse sample makes the expected 3x3 blur output
# directly recordable: each center sample is thirteen times an integer, so
# the five-center weight divided by the thirteen-kernel sum is exact.
SHARPNESS_CORE_ONLY_MODES: tuple[
    tuple[str, str, str, tuple[int | float, ...], int], ...
] = (
    ("La", "core-only-la-premultiplied", "u8", (13, 26), 1),
    ("RGBa", "core-only-rgb-premultiplied", "u8", (13, 26, 39, 52), 3),
    ("YCbCr", "core-only-ycbcr", "u8", (13, 26, 39), 3),
    ("LAB", "core-only-lab", "u8", (13, 26, 39), 3),
    ("HSV", "core-only-hsv", "u8", (13, 26, 39), 3),
    ("I", "core-only-i", "i32n", (13,), 1),
    ("F", "core-only-f", "f32n", (13.0,), 1),
    ("I;16", "core-only-i16", "u16n", (13,), 1),
    ("I;16L", "core-only-i16l", "u16le", (13,), 1),
    ("I;16B", "core-only-i16b", "u16be", (13,), 1),
    ("I;16N", "core-only-i16n", "u16n", (13,), 1),
    ("ImageLumaA16", "core-only-luma-a16", "u16n", (13, 26), 1),
    ("ImageRgb16", "core-only-rgb16", "u16n", (13, 26, 39), 3),
    ("ImageRgba16", "core-only-rgba16", "u16n", (13, 26, 39, 52), 3),
    ("ImageRgb32F", "core-only-rgb32f", "f32n", (13.0, 26.0, 39.0), 3),
    (
        "ImageRgba32F",
        "core-only-rgba32f",
        "f32n",
        (13.0, 26.0, 39.0, 52.0),
        3,
    ),
)


def sharpness_core_only_cases() -> list[dict[str, Any]]:
    """Return recorded native Sharpness results through public Python calls."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, center, active_channels in SHARPNESS_CORE_ONLY_MODES:
        source = _sharpness_impulse(mode, sample_kind, center)
        result_center = tuple(
            sample * 5 / 13 if channel < active_channels else sample
            for channel, sample in enumerate(center)
        )
        result = _sharpness_impulse(mode, sample_kind, result_center)
        encoded_source = base64.b64encode(source).decode("ascii")
        encoded_result = base64.b64encode(result).decode("ascii")
        image_record = {
            "kind": "image",
            "mode": mode,
            "size": [3, 3],
            "format": None,
            "info": {},
            "palette": None,
            "bytes": encoded_result,
        }
        cases.append(
            {
                "case_id": (
                    "PIL.ImageEnhance.Sharpness.expected-output."
                    f"{mode_id}.impulse-3x3"
                ),
                "surface": "PIL.ImageEnhance.Sharpness",
                "operation": "enhance",
                "verification": "expected-output",
                "covers": [
                    f"PIL.ImageEnhance.Sharpness.enhance.mode.{mode_id}",
                ],
                "assets": [
                    {
                        "id": "sharpness-source",
                        "kind": "inline",
                        "encoding": "base64",
                        "data": encoded_source,
                        "sha256": hashlib.sha256(source).hexdigest(),
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
                            "size": {"kind": "literal", "value": [3, 3]},
                            "data": {
                                "kind": "asset",
                                "asset_id": "sharpness-source",
                            },
                        },
                    },
                    {
                        "step_id": "enhancer",
                        "surface": "PIL.ImageEnhance",
                        "operation": "Sharpness",
                        "receiver": None,
                        "arguments": {
                            "image": {"kind": "binding", "step_id": "image"},
                        },
                    },
                    {
                        "step_id": "result",
                        "surface": "PIL.ImageEnhance.Sharpness",
                        "operation": "enhance",
                        "receiver": {"kind": "binding", "step_id": "enhancer"},
                        "arguments": {"factor": {"kind": "literal", "value": 0.0}},
                    },
                ],
                "observations": ["result"],
                "target_expectation": {
                    "observations": [
                        {"step_id": "result", "value": image_record},
                    ]
                },
            }
        )
    return cases


def _sharpness_impulse(
    mode: str,
    sample_kind: str,
    center: tuple[int | float, ...],
) -> bytes:
    """Pack a 3x3 single-pixel impulse in the mode's native sample layout."""

    def pack(samples: tuple[int | float, ...]) -> bytes:
        if sample_kind == "u8":
            return bytes(int(sample) for sample in samples)
        if sample_kind == "u16n":
            return struct.pack(f"={len(samples)}H", *(int(sample) for sample in samples))
        if sample_kind == "u16le":
            return struct.pack(f"<{len(samples)}H", *(int(sample) for sample in samples))
        if sample_kind == "u16be":
            return struct.pack(f">{len(samples)}H", *(int(sample) for sample in samples))
        if sample_kind == "i32n":
            return struct.pack("=i", int(samples[0]))
        if sample_kind == "f32n":
            return struct.pack(f"={len(samples)}f", *(float(sample) for sample in samples))
        raise ValueError(f"unknown Sharpness sample kind for {mode}: {sample_kind}")

    pixel_bytes = pack(center)
    image = bytearray(len(pixel_bytes) * 9)
    image[4 * len(pixel_bytes) : 5 * len(pixel_bytes)] = pixel_bytes
    return bytes(image)


def core_only_frombytes_cases() -> list[dict[str, Any]]:
    """Return thin-Python output and error contracts for internal modes."""

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
        cases.extend(core_only_frombytes_boundary_cases(mode, mode_id, raw))
    return cases


def core_only_frombytes_boundary_cases(
    mode: str, mode_id: str, raw: bytes
) -> list[dict[str, Any]]:
    """Cover short samples and accepted empty raster dimensions."""

    short_data = raw[:-1]
    cases = [
        _target_case(
            mode,
            mode_id,
            "short-data",
            (1, 1),
            short_data,
            {
                "kind": "expected_error",
                "mode": mode,
                "error": {
                    "class": "ValueError",
                    "kind": "invalid_argument",
                    "message": "not enough image data",
                    "stage": "call",
                    "code": None,
                },
            },
        )
    ]
    for edge, size in (("empty-width", (0, 1)), ("empty-height", (1, 0))):
        cases.append(
            _target_case(
                mode,
                mode_id,
                edge,
                size,
                b"",
                {
                    "kind": "image",
                    "mode": mode,
                    "size": list(size),
                    "format": None,
                    "info": {},
                    "palette": None,
                    "bytes": "",
                },
            )
        )
    return cases


def _target_case(
    mode: str,
    mode_id: str,
    edge: str,
    size: tuple[int, int],
    raw: bytes,
    expected: dict[str, Any],
) -> dict[str, Any]:
    encoded = base64.b64encode(raw).decode("ascii")
    if expected.get("kind") == "expected_error":
        expected = {**expected, "size": list(size)}
    return {
        "case_id": f"PIL.Image.frombytes.expected-output.{mode_id}.{edge}",
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
                    "size": {"kind": "literal", "value": list(size)},
                    "data": {"kind": "asset", "asset_id": "raw-mode-data"},
                },
            }
        ],
        "observations": ["image"],
        "target_expectation": {"observations": [{"step_id": "image", "value": expected}]},
    }
