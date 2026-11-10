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


# A distinct 2x2 source and fixed right-column result for each internal
# carrier. The expected samples are stated independently of the Crop code.
CROP_CORE_ONLY_MODES: tuple[
    tuple[str, str, str, tuple[int | float, ...], tuple[int | float, ...]], ...
] = (
    (
        "ImageLumaA16", "core-only-luma-a16", "u16n",
        (1, 2, 3, 4, 5, 6, 7, 8), (3, 4, 7, 8),
    ),
    (
        "ImageRgb16", "core-only-rgb16", "u16n",
        tuple(range(1, 13)), (4, 5, 6, 10, 11, 12),
    ),
    (
        "ImageRgba16", "core-only-rgba16", "u16n",
        tuple(range(1, 17)), (5, 6, 7, 8, 13, 14, 15, 16),
    ),
    (
        "ImageRgb32F", "core-only-rgb32f", "f32n",
        tuple(float(sample) for sample in range(1, 13)),
        (4.0, 5.0, 6.0, 10.0, 11.0, 12.0),
    ),
    (
        "ImageRgba32F", "core-only-rgba32f", "f32n",
        tuple(float(sample) for sample in range(1, 17)),
        (5.0, 6.0, 7.0, 8.0, 13.0, 14.0, 15.0, 16.0),
    ),
)


# Nonzero native samples exercise byte and typed copies for all Pillow modes.
CROP_PILLOW_MODE_CASES: tuple[tuple[str, str, str, tuple[int | float, ...]], ...] = (
    ("1", "1", "raw", (0x40, 0x80)),
    ("L", "l", "u8", (17, 53, 101, 229)),
    ("LA", "la", "u8", (17, 31, 101, 131, 205, 211, 229, 251)),
    ("La", "la-premultiplied", "u8", (17, 31, 101, 131, 205, 211, 229, 251)),
    ("P", "p", "u8", (7, 33, 129, 250)),
    ("PA", "pa", "u8", (7, 17, 33, 47, 129, 151, 250, 239)),
    ("RGB", "rgb", "u8", (11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133)),
    (
        "RGBA", "rgba", "u8",
        (11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133, 144, 155, 166, 177),
    ),
    (
        "RGBa", "rgba-premultiplied", "u8",
        (11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133, 144, 155, 166, 177),
    ),
    (
        "RGBX", "rgbx", "u8",
        (11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133, 144, 155, 166, 177),
    ),
    (
        "CMYK", "cmyk", "u8",
        (11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133, 144, 155, 166, 177),
    ),
    ("YCbCr", "ycbcr", "u8", (11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133)),
    ("LAB", "lab", "u8", (11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133)),
    ("HSV", "hsv", "u8", (11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133)),
    ("I", "i", "i32n", (12345, -23456, 67890, -1)),
    ("F", "f", "f32n", (1.25, -2.5, 100.75, -0.5)),
    ("I;16", "i16", "u16n", (0x1020, 0x3040, 0x5060, 0x7080)),
    ("I;16L", "i16l", "u16le", (0x1020, 0x3040, 0x5060, 0x7080)),
    ("I;16B", "i16b", "u16be", (0x1020, 0x3040, 0x5060, 0x7080)),
    ("I;16N", "i16n", "u16n", (0x1020, 0x3040, 0x5060, 0x7080)),
)


# Sharpness has exact native strategies for these 16 modes, but no Pillow
# parity contract. Store explicit degenerate, identity, and over-unity outputs
# so the expected values do not depend on the Rust blend implementation.
SHARPNESS_CORE_ONLY_MODES: tuple[
    tuple[
        str,
        str,
        str,
        tuple[int | float, ...],
        tuple[int | float, ...],
        tuple[int | float, ...],
    ],
    ...,
] = (
    ("La", "core-only-la-premultiplied", "u8", (234, 247), (90, 247), (255, 247)),
    (
        "RGBa",
        "core-only-rgb-premultiplied",
        "u8",
        (234, 221, 208, 247),
        (90, 85, 80, 247),
        (255, 255, 255, 247),
    ),
    ("YCbCr", "core-only-ycbcr", "u8", (234, 221, 208), (90, 85, 80), (255, 255, 255)),
    ("LAB", "core-only-lab", "u8", (234, 221, 208), (90, 85, 80), (255, 255, 255)),
    ("HSV", "core-only-hsv", "u8", (234, 221, 208), (90, 85, 80), (255, 255, 255)),
    ("I", "core-only-i", "i32n", (13,), (5,), (21,)),
    ("F", "core-only-f", "f32n", (13.0,), (5.0,), (21.0,)),
    ("I;16", "core-only-i16", "u16n", (52000,), (20000,), (65535,)),
    ("I;16L", "core-only-i16l", "u16le", (52000,), (20000,), (65535,)),
    ("I;16B", "core-only-i16b", "u16be", (52000,), (20000,), (65535,)),
    ("I;16N", "core-only-i16n", "u16n", (52000,), (20000,), (65535,)),
    (
        "ImageLumaA16",
        "core-only-luma-a16",
        "u16n",
        (52000, 65530),
        (20000, 65530),
        (65535, 65530),
    ),
    (
        "ImageRgb16",
        "core-only-rgb16",
        "u16n",
        (52000, 41600, 31200),
        (20000, 16000, 12000),
        (65535, 65535, 50400),
    ),
    (
        "ImageRgba16",
        "core-only-rgba16",
        "u16n",
        (52000, 41600, 31200, 65530),
        (20000, 16000, 12000, 65530),
        (65535, 65535, 50400, 65530),
    ),
    (
        "ImageRgb32F",
        "core-only-rgb32f",
        "f32n",
        (13.0, 26.0, 39.0),
        (5.0, 10.0, 15.0),
        (21.0, 42.0, 63.0),
    ),
    (
        "ImageRgba32F",
        "core-only-rgba32f",
        "f32n",
        (13.0, 26.0, 39.0, 52.0),
        (5.0, 10.0, 15.0, 52.0),
        (21.0, 42.0, 63.0, 52.0),
    ),
)


# These modes have native Brightness paths, but Pillow's Image.blend cannot
# serve as their output oracle. Their fixed result samples are the target-only
# contract for that extension.
BRIGHTNESS_CORE_ONLY_MODES: tuple[
    tuple[
        str,
        str,
        str,
        tuple[int | float, ...],
        tuple[int | float, ...],
        tuple[int | float, ...],
        tuple[int | float, ...],
    ],
    ...,
] = (
    (
        "I", "core-only-i", "i32n", (-7, 13, 12345), (0, 0, 0),
        (-7, 13, 12345), (-14, 26, 24690),
    ),
    (
        "F", "core-only-f", "f32n", (-7.5, 13.0, 12345.5), (-0.0, 0.0, 0.0),
        (-7.5, 13.0, 12345.5), (-15.0, 26.0, 24691.0),
    ),
    (
        "I;16", "core-only-i16", "u16n", (1000, 40000, 65535), (0, 0, 0),
        (1000, 40000, 65535), (2000, 65535, 65535),
    ),
    (
        "I;16L", "core-only-i16l", "u16le", (1000, 40000, 65535), (0, 0, 0),
        (1000, 40000, 65535), (2000, 65535, 65535),
    ),
    (
        "I;16B", "core-only-i16b", "u16be", (1000, 40000, 65535), (0, 0, 0),
        (1000, 40000, 65535), (2000, 65535, 65535),
    ),
    (
        "I;16N", "core-only-i16n", "u16n", (1000, 40000, 65535), (0, 0, 0),
        (1000, 40000, 65535), (2000, 65535, 65535),
    ),
    (
        "ImageLumaA16", "core-only-luma-a16", "u16n",
        (1000, 65530, 40000, 1234, 65535, 4321),
        (0, 65530, 0, 1234, 0, 4321),
        (1000, 65530, 40000, 1234, 65535, 4321),
        (2000, 65530, 65535, 1234, 65535, 4321),
    ),
    (
        "ImageRgb16", "core-only-rgb16", "u16n",
        (1000, 2000, 3000, 40000, 32000, 24000, 65535, 32768, 1),
        (0, 0, 0, 0, 0, 0, 0, 0, 0),
        (1000, 2000, 3000, 40000, 32000, 24000, 65535, 32768, 1),
        (2000, 4000, 6000, 65535, 64000, 48000, 65535, 65535, 2),
    ),
    (
        "ImageRgba16", "core-only-rgba16", "u16n",
        (1000, 2000, 3000, 65530, 40000, 32000, 24000, 1234, 65535, 32768, 1, 4321),
        (0, 0, 0, 65530, 0, 0, 0, 1234, 0, 0, 0, 4321),
        (1000, 2000, 3000, 65530, 40000, 32000, 24000, 1234, 65535, 32768, 1, 4321),
        (2000, 4000, 6000, 65530, 65535, 64000, 48000, 1234, 65535, 65535, 2, 4321),
    ),
    (
        "ImageRgb32F", "core-only-rgb32f", "f32n",
        (1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0),
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        (1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0),
        (2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0),
    ),
    (
        "ImageRgba32F", "core-only-rgba32f", "f32n",
        (1.0, 2.0, 3.0, 10.0, 4.0, 5.0, 6.0, 20.0, 7.0, 8.0, 9.0, 30.0),
        (0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 20.0, 0.0, 0.0, 0.0, 30.0),
        (1.0, 2.0, 3.0, 10.0, 4.0, 5.0, 6.0, 20.0, 7.0, 8.0, 9.0, 30.0),
        (2.0, 4.0, 6.0, 10.0, 8.0, 10.0, 12.0, 20.0, 14.0, 16.0, 18.0, 30.0),
    ),
)

BRIGHTNESS_PILLOW_MODES: tuple[tuple[str, str], ...] = (
    ("L", "l"), ("LA", "la"), ("La", "la-premultiplied"),
    ("RGB", "rgb"), ("RGBA", "rgba"), ("RGBa", "rgb-premultiplied"),
    ("RGBX", "rgbx"), ("CMYK", "cmyk"), ("YCbCr", "ycbcr"),
    ("HSV", "hsv"),
)

BRIGHTNESS_TARGET_ONLY_MODES = (
    (
        "LAB", "lab", "u8", (10, 20, 30, 100, 128, 200, 250, 1, 255),
        (0, 0, 0, 0, 0, 0, 0, 0, 0),
        (10, 20, 30, 100, 128, 200, 250, 1, 255),
        (20, 40, 60, 200, 255, 255, 255, 2, 255),
    ),
    *BRIGHTNESS_CORE_ONLY_MODES,
)

BRIGHTNESS_REJECTED_MODES: tuple[tuple[str, str], ...] = (
    ("1", "1"), ("P", "p"), ("PA", "pa"),
)


CHOPS_INVERT_PILLOW_MODES: tuple[tuple[str, str, int], ...] = (
    ("1", "1", 0),
    ("L", "l", 1),
    ("LA", "la", 2),
    ("La", "la-premultiplied", 2),
    ("P", "p", 1),
    ("PA", "pa", 2),
    ("RGB", "rgb", 3),
    ("RGBA", "rgba", 4),
    ("RGBa", "rgb-premultiplied", 4),
    ("RGBX", "rgbx", 4),
    ("CMYK", "cmyk", 4),
    ("YCbCr", "ycbcr", 3),
    ("LAB", "lab", 3),
    ("HSV", "hsv", 3),
)

CHOPS_DUPLICATE_PILLOW_MODES: tuple[tuple[str, str, int], ...] = (
    *CHOPS_INVERT_PILLOW_MODES,
    ("I", "i", 4),
    ("F", "f", 4),
    ("I;16", "i16", 2),
    ("I;16L", "i16l", 2),
    ("I;16B", "i16b", 2),
    ("I;16N", "i16n", 2),
)
CHOPS_CONSTANT_PILLOW_MODES = CHOPS_DUPLICATE_PILLOW_MODES

# Binary Chops supports every Pillow byte mode, including three-byte LAB.
CHOPS_BINARY_PILLOW_MODES: tuple[tuple[str, str, int], ...] = CHOPS_INVERT_PILLOW_MODES

CHOPS_BINARY_REJECTED_PILLOW_MODES: tuple[tuple[str, str, int], ...] = (
    ("I", "i", 4),
    ("F", "f", 4),
    ("I;16", "i16", 2),
    ("I;16L", "i16l", 2),
    ("I;16B", "i16b", 2),
    ("I;16N", "i16n", 2),
)

CHOPS_BINARY_OPERATIONS = (
    "add",
    "subtract",
    "multiply",
    "screen",
    "darker",
    "lighter",
    "difference",
    "overlay",
    "hard_light",
    "soft_light",
    "add_modulo",
    "subtract_modulo",
)

CHOPS_LOGICAL_OPERATIONS = ("logical_and", "logical_or", "logical_xor")
CHOPS_LOGICAL_PILLOW_MODES = (
    *CHOPS_BINARY_PILLOW_MODES,
    *CHOPS_BINARY_REJECTED_PILLOW_MODES,
)

# Reuse the fixed typed samples already exercised by Brightness so every
# target-only core mode has one deterministic source representation.
CHOPS_INVERT_CORE_ONLY_MODES = tuple(
    (mode, mode_id, kind, source)
    for mode, mode_id, kind, source, *_brightness_outputs in BRIGHTNESS_CORE_ONLY_MODES
)

CHOPS_DUPLICATE_CORE_ONLY_MODES = tuple(
    (mode, mode_id, kind, samples)
    for mode, mode_id, kind, samples, *_outputs
    in BRIGHTNESS_CORE_ONLY_MODES
    if mode.startswith("Image")
)
CHOPS_CONSTANT_CORE_ONLY_MODES = CHOPS_DUPLICATE_CORE_ONLY_MODES


CHOPS_OFFSET_PILLOW_MODES: tuple[tuple[str, str, int], ...] = (
    *CHOPS_INVERT_PILLOW_MODES,
    ("I", "i", 4),
    ("F", "f", 4),
    ("I;16", "i16", 2),
    ("I;16L", "i16l", 2),
    ("I;16B", "i16b", 2),
    ("I;16N", "i16n", 2),
)

CHOPS_OFFSET_CORE_ONLY_MODES = tuple(
    (mode, mode_id, kind, source)
    for mode, mode_id, kind, source, *_brightness_outputs in BRIGHTNESS_CORE_ONLY_MODES
    if mode
    in {
        "ImageLumaA16",
        "ImageRgb16",
        "ImageRgba16",
        "ImageRgb32F",
        "ImageRgba32F",
    }
)


def brightness_target_output_cases() -> list[dict[str, Any]]:
    """Return fixed no-Pillow Brightness outputs through public Python."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, kind, source, black, identity, doubled in BRIGHTNESS_TARGET_ONLY_MODES:
        raw = _native_samples(kind, source)
        factors = (("zero", 0.0, black), ("identity", 1.0, identity), ("double", 2.0, doubled))
        cases.append(
            {
                "case_id": f"PIL.ImageEnhance.Brightness.expected-output.{mode_id}.native-3x1",
                "surface": "PIL.ImageEnhance.Brightness",
                "operation": "enhance",
                "verification": "expected-output",
                "covers": [f"PIL.ImageEnhance.Brightness.enhance.mode.{mode_id}"],
                "assets": [
                    {
                        "id": "brightness-source",
                        "kind": "inline",
                        "encoding": "base64",
                        "data": base64.b64encode(raw).decode("ascii"),
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
                            "size": {"kind": "literal", "value": [3, 1]},
                            "data": {"kind": "asset", "asset_id": "brightness-source"},
                        },
                    },
                    {
                        "step_id": "enhancer",
                        "surface": "PIL.ImageEnhance",
                        "operation": "Brightness",
                        "receiver": None,
                        "arguments": {"image": {"kind": "binding", "step_id": "image"}},
                    },
                    *[
                        {
                            "step_id": "result-" + name,
                            "surface": "PIL.ImageEnhance.Brightness",
                            "operation": "enhance",
                            "receiver": {"kind": "binding", "step_id": "enhancer"},
                            "arguments": {"factor": {"kind": "literal", "value": factor}},
                        }
                        for name, factor, _samples in factors
                    ],
                ],
                "observations": ["result-" + name for name, _factor, _samples in factors],
                "target_expectation": {
                    "observations": [
                        {
                            "step_id": "result-" + name,
                            "value": _brightness_result_record(mode, kind, samples),
                        }
                        for name, _factor, samples in factors
                    ]
                },
            }
        )
    return cases


def chops_invert_target_output_cases() -> list[dict[str, Any]]:
    """Return fixed raw-byte inversion outputs for native-only modes."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, samples in CHOPS_INVERT_CORE_ONLY_MODES:
        source = _native_samples(sample_kind, samples)
        inverted = bytes(value ^ 0xFF for value in source)
        cases.append(
            {
                "case_id": (
                    "PIL.ImageChops.invert.expected-output."
                    f"{mode_id}.native-3x1"
                ),
                "surface": "PIL.ImageChops",
                "operation": "invert",
                "verification": "expected-output",
                "covers": [f"PIL.ImageChops.invert.mode.{mode_id}"],
                "assets": [
                    {
                        "id": "invert-source",
                        "kind": "inline",
                        "encoding": "base64",
                        "data": base64.b64encode(source).decode("ascii"),
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
                            "size": {"kind": "literal", "value": [3, 1]},
                            "data": {"kind": "asset", "asset_id": "invert-source"},
                        },
                    },
                    {
                        "step_id": "result",
                        "surface": "PIL.ImageChops",
                        "operation": "invert",
                        "receiver": None,
                        "arguments": {"image": {"kind": "binding", "step_id": "image"}},
                    },
                ],
                "observations": ["result"],
                "target_expectation": {
                    "observations": [
                        {
                            "step_id": "result",
                            "value": _native_image_result_record(mode, inverted),
                        }
                    ]
                },
            }
        )
    return cases


def chops_duplicate_target_output_cases() -> list[dict[str, Any]]:
    """Return fixed identity outputs for the five internal raster modes."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, samples in CHOPS_DUPLICATE_CORE_ONLY_MODES:
        source = _native_samples(sample_kind, samples)
        cases.append(
            {
                "case_id": (
                    "PIL.ImageChops.duplicate.expected-output."
                    f"{mode_id}.native-3x1"
                ),
                "surface": "PIL.ImageChops",
                "operation": "duplicate",
                "verification": "expected-output",
                "covers": [f"PIL.ImageChops.duplicate.mode.{mode_id}"],
                "assets": [
                    {
                        "id": "duplicate-source",
                        "kind": "inline",
                        "encoding": "base64",
                        "data": base64.b64encode(source).decode("ascii"),
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
                            "size": {"kind": "literal", "value": [3, 1]},
                            "data": {
                                "kind": "asset",
                                "asset_id": "duplicate-source",
                            },
                        },
                    },
                    {
                        "step_id": "result",
                        "surface": "PIL.ImageChops",
                        "operation": "duplicate",
                        "receiver": None,
                        "arguments": {
                            "image": {"kind": "binding", "step_id": "image"}
                        },
                    },
                ],
                "observations": ["result"],
                "target_expectation": {
                    "observations": [
                        {
                            "step_id": "result",
                            "value": _native_image_result_record(mode, source),
                        }
                    ]
                },
            }
        )
    return cases


def chops_constant_target_output_cases() -> list[dict[str, Any]]:
    """Return fixed L-mode Constant outputs for internal source identities."""

    cases: list[dict[str, Any]] = []
    value = 73
    expected = bytes([value]) * 3
    for mode, mode_id, sample_kind, samples in CHOPS_CONSTANT_CORE_ONLY_MODES:
        source = _native_samples(sample_kind, samples)
        cases.append(
            {
                "case_id": (
                    "PIL.ImageChops.constant.expected-output."
                    f"{mode_id}.native-3x1"
                ),
                "surface": "PIL.ImageChops",
                "operation": "constant",
                "verification": "expected-output",
                "covers": [f"PIL.ImageChops.constant.mode.{mode_id}"],
                "assets": [
                    {
                        "id": "constant-source",
                        "kind": "inline",
                        "encoding": "base64",
                        "data": base64.b64encode(source).decode("ascii"),
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
                            "size": {"kind": "literal", "value": [3, 1]},
                            "data": {
                                "kind": "asset",
                                "asset_id": "constant-source",
                            },
                        },
                    },
                    {
                        "step_id": "result",
                        "surface": "PIL.ImageChops",
                        "operation": "constant",
                        "receiver": None,
                        "arguments": {
                            "image": {"kind": "binding", "step_id": "image"},
                            "value": {"kind": "literal", "value": value},
                        },
                    },
                ],
                "observations": ["result"],
                "target_expectation": {
                    "observations": [
                        {
                            "step_id": "result",
                            "value": _native_image_result_record("L", expected),
                        }
                    ]
                },
            }
        )
    return cases


def chops_binary_target_output_cases() -> list[dict[str, Any]]:
    """Record internal high-depth rejection contracts in both input positions."""

    cases = []
    for operation in CHOPS_BINARY_OPERATIONS:
        for mode, mode_id, raw in CORE_ONLY_RAW_MODES:
            for invalid_position in ("image1", "image2"):
                image1_mode, image2_mode = (
                    (mode, "L") if invalid_position == "image1" else ("L", mode)
                )
                error_message = (
                    "image has wrong mode"
                    if invalid_position == "image1"
                    else "images do not match"
                )
                expected_error = {
                    "kind": "expected_error",
                    "mode": image1_mode,
                    "size": [1, 1],
                    "error": {
                        "class": "ValueError",
                        "kind": "invalid_argument",
                        "message": error_message,
                        "stage": "call",
                        "code": None,
                    },
                }
                cases.append(
                    _chops_binary_target_case(
                        operation,
                        mode,
                        mode_id,
                        raw if invalid_position == "image1" else b"\x00",
                        raw if invalid_position == "image2" else b"\x00",
                        (1, 1),
                        expected_error,
                        f"rejected-{invalid_position}",
                        image1_mode=image1_mode,
                        image2_mode=image2_mode,
                    )
                )
    return cases


def chops_logical_target_output_cases() -> list[dict[str, Any]]:
    """Record CoreOnly mode rejections for logical Chops through Python."""

    cases = []
    one_mode_raw = b"\x00"
    for operation in CHOPS_LOGICAL_OPERATIONS:
        for mode, mode_id, raw in CORE_ONLY_RAW_MODES:
            expected_error = {
                "kind": "expected_error",
                "mode": mode,
                "size": [1, 1],
                "error": {
                    "class": "ValueError",
                    "kind": "invalid_argument",
                    "message": "image has wrong mode",
                    "stage": "call",
                    "code": None,
                },
            }
            for invalid_position in ("image1", "image2"):
                inputs = {
                    "image1": ("1", one_mode_raw),
                    "image2": ("1", one_mode_raw),
                }
                inputs[invalid_position] = (mode, raw)
                cases.append(
                    _chops_binary_target_case(
                        operation,
                        mode,
                        mode_id,
                        inputs["image1"][1],
                        inputs["image2"][1],
                        (1, 1),
                        expected_error,
                        f"rejected-{invalid_position}",
                        image1_mode=inputs["image1"][0],
                        image2_mode=inputs["image2"][0],
                    )
                )
    return cases


def _chops_binary_target_case(
    operation: str,
    mode: str,
    mode_id: str,
    left: bytes,
    right: bytes,
    size: tuple[int, int],
    expected: dict[str, Any],
    label: str,
    *,
    image1_mode: str | None = None,
    image2_mode: str | None = None,
) -> dict[str, Any]:
    assets = []
    for asset_id, raw in (("image1-data", left), ("image2-data", right)):
        assets.append(
            {
                "id": asset_id,
                "kind": "inline",
                "encoding": "base64",
                "data": base64.b64encode(raw).decode("ascii"),
                "sha256": hashlib.sha256(raw).hexdigest(),
                "media_type": "application/octet-stream",
            }
        )
    input_modes = (image1_mode or mode, image2_mode or mode)
    steps = [
        {
            "step_id": step_id,
            "surface": "PIL.Image",
            "operation": "frombytes",
            "receiver": None,
            "arguments": {
                "mode": {"kind": "literal", "value": input_mode},
                "size": {"kind": "literal", "value": list(size)},
                "data": {"kind": "asset", "asset_id": asset_id},
            },
        }
        for step_id, asset_id, input_mode in (
            ("image1", "image1-data", input_modes[0]),
            ("image2", "image2-data", input_modes[1]),
        )
    ]
    steps.append(
        {
            "step_id": "result",
            "surface": "PIL.ImageChops",
            "operation": operation,
            "receiver": None,
            "arguments": {
                "image1": {"kind": "binding", "step_id": "image1"},
                "image2": {"kind": "binding", "step_id": "image2"},
            },
        }
    )
    return {
        "case_id": (
            f"PIL.ImageChops.{operation}.expected-output."
            f"{mode_id}.{label}"
        ),
        "surface": "PIL.ImageChops",
        "operation": operation,
        "verification": "expected-output",
        "covers": [f"PIL.ImageChops.{operation}.mode.{mode_id}"],
        "assets": assets,
        "steps": steps,
        "observations": ["result"],
        "target_expectation": {
            "observations": [{"step_id": "result", "value": expected}]
        },
    }


def chops_offset_target_output_cases() -> list[dict[str, Any]]:
    """Return fixed native-sample outputs for internal-only modes."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, samples in CHOPS_OFFSET_CORE_ONLY_MODES:
        source = _native_samples(sample_kind, samples)
        pixel_size = len(source) // 3
        shifted = source[2 * pixel_size :] + source[: 2 * pixel_size]
        cases.append(
            {
                "case_id": (
                    "PIL.ImageChops.offset.expected-output."
                    f"{mode_id}.native-3x1"
                ),
                "surface": "PIL.ImageChops",
                "operation": "offset",
                "verification": "expected-output",
                "covers": [f"PIL.ImageChops.offset.mode.{mode_id}"],
                "assets": [
                    {
                        "id": "offset-source",
                        "kind": "inline",
                        "encoding": "base64",
                        "data": base64.b64encode(source).decode("ascii"),
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
                            "size": {"kind": "literal", "value": [3, 1]},
                            "data": {"kind": "asset", "asset_id": "offset-source"},
                        },
                    },
                    {
                        "step_id": "result",
                        "surface": "PIL.ImageChops",
                        "operation": "offset",
                        "receiver": None,
                        "arguments": {
                            "image": {"kind": "binding", "step_id": "image"},
                            "xoffset": {"kind": "literal", "value": 1},
                            "yoffset": {"kind": "literal", "value": 0},
                        },
                    },
                ],
                "observations": ["result"],
                "target_expectation": {
                    "observations": [
                        {
                            "step_id": "result",
                            "value": _native_image_result_record(mode, shifted),
                        }
                    ]
                },
            }
        )
    return cases


def _brightness_result_record(
    mode: str,
    sample_kind: str,
    samples: tuple[int | float, ...],
) -> dict[str, Any]:
    return _native_image_result_record(mode, _native_samples(sample_kind, samples))


def _native_image_result_record(
    mode: str,
    raw: bytes,
    width: int = 3,
    height: int = 1,
) -> dict[str, Any]:
    return {
        "kind": "image",
        "mode": mode,
        "size": [width, height],
        "format": None,
        "info": {},
        "palette": None,
        "bytes": base64.b64encode(raw).decode("ascii"),
    }


def _native_samples(sample_kind: str, samples: tuple[int | float, ...]) -> bytes:
    if sample_kind == "u8":
        return bytes(int(sample) for sample in samples)
    if sample_kind == "i32n":
        return struct.pack(f"={len(samples)}i", *(int(sample) for sample in samples))
    if sample_kind == "f32n":
        return struct.pack(f"={len(samples)}f", *(float(sample) for sample in samples))
    if sample_kind == "u16n":
        return struct.pack(f"={len(samples)}H", *(int(sample) for sample in samples))
    if sample_kind == "u16le":
        return struct.pack(f"<{len(samples)}H", *(int(sample) for sample in samples))
    if sample_kind == "u16be":
        return struct.pack(f">{len(samples)}H", *(int(sample) for sample in samples))
    raise ValueError(f"unknown native sample kind: {sample_kind}")


def crop_core_only_target_output_cases() -> list[dict[str, Any]]:
    """Return fixed Crop outputs for each internal native pixel carrier."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, source_samples, output_samples in CROP_CORE_ONLY_MODES:
        source = _native_samples(sample_kind, source_samples)
        encoded_source = base64.b64encode(source).decode("ascii")
        output = _native_samples(sample_kind, output_samples)
        cases.append(
            {
                "case_id": (
                    f"PIL.Image.Image.crop.expected-output.{mode_id}.right-column-2x2"
                ),
                "surface": "PIL.Image.Image",
                "operation": "crop",
                "verification": "expected-output",
                "covers": [f"PIL.Image.Image.crop.mode.{mode_id}"],
                "assets": [
                    {
                        "id": "crop-source",
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
                            "size": {"kind": "literal", "value": [2, 2]},
                            "data": {"kind": "asset", "asset_id": "crop-source"},
                        },
                    },
                    {
                        "step_id": "result",
                        "surface": "PIL.Image.Image",
                        "operation": "crop",
                        "receiver": {"kind": "binding", "step_id": "image"},
                        "arguments": {
                            "box": {"kind": "literal", "value": [1, 0, 2, 2]}
                        },
                    },
                ],
                "observations": ["result"],
                "target_expectation": {
                    "observations": [
                        {
                            "step_id": "result",
                            "value": _native_image_result_record(
                                mode, output, width=1, height=2
                            ),
                        }
                    ]
                },
            }
        )
    return cases


def crop_pillow_mode_parity_cases() -> list[dict[str, Any]]:
    """Return nonzero Pillow-oracle Crop workflows for all 20 Pillow modes."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, samples in CROP_PILLOW_MODE_CASES:
        source = (
            bytes(int(sample) for sample in samples)
            if sample_kind == "raw"
            else _native_samples(sample_kind, samples)
        )
        cases.append(
            {
                "case_id": f"PIL.Image.Image.crop.mode.{mode_id}",
                "surface": "PIL.Image.Image",
                "operation": "crop",
                "covers": [f"PIL.Image.Image.crop.mode.{mode_id}"],
                "assets": [
                    {
                        "id": "crop-source",
                        "kind": "inline",
                        "encoding": "base64",
                        "data": base64.b64encode(source).decode("ascii"),
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
                            "size": {"kind": "literal", "value": [2, 2]},
                            "data": {"kind": "asset", "asset_id": "crop-source"},
                        },
                    },
                    {
                        "step_id": "result",
                        "surface": "PIL.Image.Image",
                        "operation": "crop",
                        "receiver": {"kind": "binding", "step_id": "image"},
                        "arguments": {
                            "box": {"kind": "literal", "value": [1, 0, 2, 2]}
                        },
                    },
                ],
                "observations": ["result"],
            }
        )
    return cases


def sharpness_core_only_cases() -> list[dict[str, Any]]:
    """Return recorded native Sharpness results through public Python calls."""

    cases: list[dict[str, Any]] = []
    for (
        mode,
        mode_id,
        sample_kind,
        center,
        degenerate,
        sharpened,
    ) in SHARPNESS_CORE_ONLY_MODES:
        source = _sharpness_impulse(mode, sample_kind, center)
        encoded_source = base64.b64encode(source).decode("ascii")
        factors = (
            ("degenerate", 0.0, degenerate),
            ("identity", 1.0, center),
            ("sharpened", 2.0, sharpened),
        )
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
                    *[
                        {
                            "step_id": "result-" + factor_name,
                            "surface": "PIL.ImageEnhance.Sharpness",
                            "operation": "enhance",
                            "receiver": {"kind": "binding", "step_id": "enhancer"},
                            "arguments": {
                                "factor": {"kind": "literal", "value": factor}
                            },
                        }
                        for factor_name, factor, _expected_center in factors
                    ],
                ],
                "observations": [
                    "result-" + factor_name
                    for factor_name, _factor, _expected_center in factors
                ],
                "target_expectation": {
                    "observations": [
                        {
                            "step_id": "result-" + factor_name,
                            "value": _sharpness_result_record(
                                mode,
                                sample_kind,
                                expected_center,
                            ),
                        }
                        for factor_name, _factor, expected_center in factors
                    ]
                },
            }
        )
    return cases


def _sharpness_result_record(
    mode: str,
    sample_kind: str,
    center: tuple[int | float, ...],
) -> dict[str, Any]:
    """Encode a recorded mode-specific center output."""

    result = _sharpness_impulse(mode, sample_kind, center)
    return {
        "kind": "image",
        "mode": mode,
        "size": [3, 3],
        "format": None,
        "info": {},
        "palette": None,
        "bytes": base64.b64encode(result).decode("ascii"),
    }


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


def get_child_images_target_output_cases() -> list[dict[str, Any]]:
    """Record the retained Rust-only child-image convenience method."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id in (("L", "l"), ("RGB", "rgb")):
        cases.append(
            {
                "case_id": (
                    "PIL.Image.Image.get_child_images.expected-output."
                    f"{mode_id}"
                ),
                "surface": "PIL.Image.Image",
                "operation": "get_child_images",
                "verification": "expected-output",
                "covers": [
                    "PIL.Image.Image.get_child_images.behavior.default",
                    f"PIL.Image.Image.get_child_images.mode.{mode_id}",
                ],
                "assets": [],
                "steps": [
                    {
                        "step_id": "image",
                        "surface": "PIL.Image",
                        "operation": "new",
                        "receiver": None,
                        "arguments": {
                            "mode": {"kind": "literal", "value": mode},
                            "size": {"kind": "literal", "value": [2, 2]},
                        },
                    },
                    {
                        "step_id": "result",
                        "surface": "PIL.Image.Image",
                        "operation": "get_child_images",
                        "receiver": {"kind": "binding", "step_id": "image"},
                        "arguments": {},
                    },
                ],
                "observations": ["result"],
                "target_expectation": {
                    "observations": [{"step_id": "result", "value": []}]
                },
            }
        )
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
