"""Recorded target-only outputs for supported Rust mode extensions.

These contracts use the public Python facade and intentionally have no Pillow
oracle. The raw fixtures are fixed byte strings so generated inputs are stable
across hosts; the Rust mode parser interprets typed samples in native endian.
"""

from __future__ import annotations

import base64
import copy
import hashlib
import struct
from functools import lru_cache
from pathlib import Path
from typing import Any

import numpy as np

_DEFAULT_TARGET_IMAGE_INFO = object()

try:
    from migration_parity.mode_size_matrix import (
        MODE_SIZE_INPUT_VARIANTS,
        MODE_SIZE_INPUT_VARIANTS_BY_SIZE,
        MODE_SIZE_MATRIX_SIZES,
        MODE_SIZE_PILLOW_MODES,
        seeded_noise_sample,
    )
except ModuleNotFoundError:
    from scripts.migration_parity.mode_size_matrix import (
        MODE_SIZE_INPUT_VARIANTS,
        MODE_SIZE_INPUT_VARIANTS_BY_SIZE,
        MODE_SIZE_MATRIX_SIZES,
        MODE_SIZE_PILLOW_MODES,
        seeded_noise_sample,
    )


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

# Distinguish premultiplied identities from their straight-alpha spellings.
_DIRECT_ROTATE_MODE_IDS = {
    "La": "la-premultiplied",
    "RGBa": "rgba-premultiplied",
}
DIRECT_ROTATE_MODES = tuple(
    (mode, _DIRECT_ROTATE_MODE_IDS.get(mode, mode.lower().replace(";", "-")))
    for mode in (
        *MODE_SIZE_PILLOW_MODES,
        *(mode for mode, _mode_id, _raw in CORE_ONLY_RAW_MODES),
    )
)

# PyImage's test-only byte observer returns native DynamicImage carrier bytes,
# including expanded One and the internal typed CoreOnly layouts.
_DIRECT_ROTATE_BYTES_PER_PIXEL = {
    "1": 1,
    "L": 1,
    "LA": 2,
    "La": 2,
    "P": 1,
    "PA": 2,
    "RGB": 3,
    "RGBA": 4,
    "RGBa": 4,
    "RGBX": 4,
    "CMYK": 4,
    "YCbCr": 3,
    "LAB": 3,
    "HSV": 3,
    "I": 4,
    "F": 4,
    "I;16": 2,
    "I;16L": 2,
    "I;16B": 2,
    "I;16N": 2,
    "ImageLumaA16": 4,
    "ImageRgb16": 6,
    "ImageRgba16": 8,
    "ImageRgb32F": 12,
    "ImageRgba32F": 16,
}

CORE_ONLY_CHANNELS = {
    "ImageLumaA16": 2,
    "ImageRgb16": 3,
    "ImageRgba16": 4,
    "ImageRgb32F": 3,
    "ImageRgba32F": 4,
}

# Reduce keeps target-only outputs for native sample identities that Pillow
# cannot reduce. Pillow 13 rejects I;16 by storage type, and the five internal
# high-depth identities have no Pillow constructor; all nine remain explicit
# CoreOnly contracts while preserving their exact native sample layouts.
REDUCE_CORE_ONLY_MODES: tuple[tuple[str, str, str], ...] = (
    ("I;16", "i16", "u16n"),
    ("I;16L", "i16l", "u16le"),
    ("I;16B", "i16b", "u16be"),
    ("I;16N", "i16n", "u16n"),
    ("ImageLumaA16", "core-only-luma-a16", "u16n"),
    ("ImageRgb16", "core-only-rgb16", "u16n"),
    ("ImageRgba16", "core-only-rgba16", "u16n"),
    ("ImageRgb32F", "core-only-rgb32f", "f32n"),
    ("ImageRgba32F", "core-only-rgba32f", "f32n"),
)

REDUCE_MATRIX_FACTORS = {
    "factor-2x2": (2, 2),
    "factor-2x3": (2, 3),
}

_MODE_MATRIX_CHANNELS = {
    **CORE_ONLY_CHANNELS,
    "La": 2,
    "RGBa": 4,
    "I": 1,
    "F": 1,
    "I;16": 1,
    "I;16L": 1,
    "I;16B": 1,
    "I;16N": 1,
    "YCbCr": 3,
    "LAB": 3,
    "HSV": 3,
}


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


def chops_duplicate_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Add size and pixel-pattern variants for native-only duplicate modes.

    Duplicate is an identity operation, so its expected bytes are independently
    determined by the input bytes. These cases keep the five internal modes in
    the parity matrix without asking Pillow to construct unsupported modes.
    """

    # Recorded target outputs duplicate every source byte in the expectation.
    # The input builder stores large CoreOnly outputs in their own indexed
    # document so the full mode/size matrix stays under the per-file limit.
    sizes = MODE_SIZE_MATRIX_SIZES
    variants = MODE_SIZE_INPUT_VARIANTS
    base_cases = chops_duplicate_target_output_cases()
    expanded: list[dict[str, Any]] = []

    if len(base_cases) != len(CHOPS_DUPLICATE_CORE_ONLY_MODES):
        raise RuntimeError("native duplicate fixtures and mode declarations differ")

    for base_case, (mode, mode_id, sample_kind, _samples) in zip(
        base_cases, CHOPS_DUPLICATE_CORE_ONLY_MODES
    ):
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        for variant in variants:
            for width, height in sizes:
                if variant not in MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(width, height)]:
                    continue
                raw = _core_only_matrix_input(
                    mode, sample_kind, width, height, variant
                )
                case = _target_only_matrix_case(
                    base_case, raw, width, height, variant, assets_root
                )
                case["target_expectation"]["observations"][0]["value"] = (
                    _native_image_result_record(mode, raw, width, height)
                )
                expanded.append(case)

    return expanded


def core_only_frombytes_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Expand typed CoreOnly construction across the shared size/pattern matrix."""

    base_cases = {
        case["covers"][1].rsplit(".", 1)[-1]: case
        for case in core_only_frombytes_cases()
    }
    expanded: list[dict[str, Any]] = []
    for mode, mode_id, _raw in CORE_ONLY_RAW_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        if mode in {"ImageLumaA16", "ImageRgb16", "ImageRgba16"}:
            sample_kind = "u16n"
        elif mode in {"ImageRgb32F", "ImageRgba32F"}:
            sample_kind = "f32n"
        else:
            raise ValueError(f"missing CoreOnly sample kind for {mode}")
        base_case = base_cases[mode_id]
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(mode, sample_kind, width, height, variant)
            case = _target_only_matrix_case(
                base_case,
                source,
                width,
                height,
                variant,
                assets_root,
                case_suffix=mode_id,
            )
            case["target_expectation"]["observations"][0]["value"] = (
                _native_image_result_record(mode, source, width, height)
            )
            expanded.append(case)
    return expanded


def core_only_getbbox_cases() -> list[dict[str, Any]]:
    """Record native typed GetBBox outcomes for all CoreOnly carriers."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, raw in CORE_ONLY_RAW_MODES:
        encoded = base64.b64encode(raw).decode("ascii")
        cases.append(
            {
                "case_id": (
                    "PIL.Image.Image.getbbox.expected-output."
                    f"{mode_id}"
                ),
                "surface": "PIL.Image.Image",
                "operation": "getbbox",
                "verification": "expected-output",
                "covers": [f"PIL.Image.Image.getbbox.mode.{mode_id}"],
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
                    },
                    {
                        "step_id": "call",
                        "surface": "PIL.Image.Image",
                        "operation": "getbbox",
                        "receiver": {"kind": "binding", "step_id": "image"},
                        "arguments": {},
                    },
                ],
                "observations": ["call"],
                "target_expectation": {
                    "observations": [
                        {
                            "step_id": "call",
                            "value": _core_only_getbbox_value(
                                mode, _core_only_sample_kind(mode), raw, 1, 1
                            ),
                        }
                    ]
                },
            }
        )
    return cases


def core_only_getbbox_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Expand CoreOnly GetBBox outputs across size and pixel-pattern inputs."""

    base_cases = {
        case["covers"][0].rsplit(".", 1)[-1]: case
        for case in core_only_getbbox_cases()
    }
    expanded: list[dict[str, Any]] = []
    for mode, mode_id, _raw in CORE_ONLY_RAW_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        sample_kind = _core_only_sample_kind(mode)
        base_case = base_cases[mode_id]
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(mode, sample_kind, width, height, variant)
            case = _target_only_matrix_case(
                base_case,
                source,
                width,
                height,
                variant,
                assets_root,
                case_suffix=mode_id,
            )
            case["target_expectation"]["observations"][0]["value"] = (
                _core_only_getbbox_value(mode, sample_kind, source, width, height)
            )
            expanded.append(case)
    return expanded


# VERIFY: source=reviewed@2026-11-13; parity=pass@migration-parity-dee7a1fa052b47138d0e3c735ad78c01; benchmark=n/a
def _core_only_getdata_bytes(mode: str, source: bytes) -> bytes:
    """Return GetData's byte projection in logical sample-band order."""

    if mode in {"ImageLumaA16", "ImageRgb16", "ImageRgba16"}:
        samples = np.frombuffer(source, dtype=np.dtype("=u2"))
        values = ((samples.astype(np.uint32) + 128) // 257).astype(np.uint8)
    elif mode in {"ImageRgb32F", "ImageRgba32F"}:
        samples = np.frombuffer(source, dtype=np.dtype("=f4"))
        clamped = np.where(
            samples < np.float32(1.0),
            np.fmax(samples, np.float32(0.0)),
            np.float32(1.0),
        )
        scaled = np.multiply(clamped, np.float32(255.0), dtype=np.float32)
        rounded = np.floor(
            np.add(scaled, np.float32(0.5), dtype=np.float32)
        )
        values = rounded.astype(np.uint8)
    else:
        raise ValueError(f"GetData has no CoreOnly projection for {mode}")

    return values.tobytes()


# VERIFY: source=reviewed@2026-11-13; parity=pass@migration-parity-dee7a1fa052b47138d0e3c735ad78c01; benchmark=n/a
def _core_only_getdata_expectation(
    mode: str,
    source: bytes,
    width: int,
    height: int,
) -> dict[str, Any]:
    """Encode typed GetData pixels compactly for target-only comparisons."""

    projected = _core_only_getdata_bytes(mode, source)
    return {
        "kind": "packed_pixels",
        "mode": mode,
        "size": [width, height],
        "bands": CORE_ONLY_CHANNELS[mode],
        "data": base64.b64encode(projected).decode("ascii"),
    }


# VERIFY: source=reviewed@2026-11-13; parity=pass@migration-parity-dee7a1fa052b47138d0e3c735ad78c01; benchmark=n/a
def core_only_getdata_cases() -> list[dict[str, Any]]:
    """Record GetData and flattened-data outputs for all CoreOnly carriers."""

    cases: list[dict[str, Any]] = []
    for operation in ("getdata", "get_flattened_data"):
        for mode, mode_id, raw in CORE_ONLY_RAW_MODES:
            encoded = base64.b64encode(raw).decode("ascii")
            cases.append(
                {
                    "case_id": (
                        f"PIL.Image.Image.{operation}.expected-output.{mode_id}"
                    ),
                    "surface": "PIL.Image.Image",
                    "operation": operation,
                    "verification": "expected-output",
                    "covers": [
                        f"PIL.Image.Image.{operation}.mode.{mode_id}"
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
                                "data": {
                                    "kind": "asset",
                                    "asset_id": "raw-mode-data",
                                },
                            },
                        },
                        {
                            "step_id": "call",
                            "surface": "PIL.Image.Image",
                            "operation": operation,
                            "receiver": {"kind": "binding", "step_id": "image"},
                            "arguments": {},
                        },
                    ],
                    "observations": ["call"],
                    "target_expectation": {
                        "observations": [
                            {
                                "step_id": "call",
                                "value": _core_only_getdata_expectation(
                                    mode, raw, 1, 1
                                ),
                            }
                        ]
                    },
                }
            )
    return cases


# VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
def core_only_getdata_matrix_cases(
    assets_root: Path,
    operation: str,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Expand CoreOnly GetData outputs across the size/pattern matrix."""

    base_cases = {
        case["covers"][0].rsplit(".", 1)[-1]: case
        for case in core_only_getdata_cases()
        if case["operation"] == operation
    }
    expanded: list[dict[str, Any]] = []
    for mode, mode_id, _raw in CORE_ONLY_RAW_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        base_case = base_cases[mode_id]
        sample_kind = _core_only_sample_kind(mode)
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(mode, sample_kind, width, height, variant)
            case = _target_only_matrix_case(
                base_case,
                source,
                width,
                height,
                variant,
                assets_root,
                case_suffix=mode_id,
            )
            case["target_expectation"]["observations"][0]["value"] = (
                _core_only_getdata_expectation(mode, source, width, height)
            )
            expanded.append(case)
    return expanded


def _core_only_sample_kind(mode: str) -> str:
    if mode in {"ImageLumaA16", "ImageRgb16", "ImageRgba16"}:
        return "u16n"
    if mode in {"ImageRgb32F", "ImageRgba32F"}:
        return "f32n"
    raise ValueError(f"missing CoreOnly sample kind for {mode}")


def _core_only_getbbox_value(
    mode: str,
    sample_kind: str,
    source: bytes,
    width: int,
    height: int,
) -> list[int] | None:
    channels = _MODE_MATRIX_CHANNELS[mode]
    sample_formats = {"u16n": "=H", "f32n": "=f"}
    sample_format = sample_formats[sample_kind]
    pixel_format = f"{sample_format[0]}{channels}{sample_format[1]}"
    bounds = [width, height, -1, -1]
    for index, pixel in enumerate(struct.iter_unpack(pixel_format, source)):
        if any(sample != 0 for sample in pixel):
            x = index % width
            y = index // width
            bounds[0] = min(bounds[0], x)
            bounds[1] = min(bounds[1], y)
            bounds[2] = max(bounds[2], x)
            bounds[3] = max(bounds[3], y)
    if bounds[2] < bounds[0]:
        return None
    return [bounds[0], bounds[1], bounds[2] + 1, bounds[3] + 1]


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def _core_only_matrix_pixel(
    sample_kind: str,
    variant: str,
    index: int,
    parity: int,
    channels: int,
    seed: int,
    pixel_format: str,
) -> bytes:
    values: list[int | float] = []
    for lane in range(channels):
        if sample_kind in {"u16n", "u16le", "u16be"}:
            if variant == "varied":
                sample = (index * 7919 + lane * 4051 + seed) & 0xFFFF
            elif variant == "seeded-noise":
                sample = seeded_noise_sample(index, seed, lane)
            elif variant == "checkerboard":
                sample = 0x1111 + lane * 0x101 if parity else 0xEEEE - lane * 0x101
            elif variant == "boundary":
                sample = (0, 1, 65534, 65535)[(index + lane + seed) & 3]
            else:
                raise ValueError(f"unknown CoreOnly matrix input pattern: {variant}")
        elif sample_kind == "f32n":
            if variant == "varied":
                sample = (((index * 31 + lane * 17 + seed) % 8191) - 4095) / 128.0
            elif variant == "seeded-noise":
                sample = (seeded_noise_sample(index, seed, lane) - 32768) / 128.0
            elif variant == "checkerboard":
                sample = (
                    (-1.0, 0.0, 0.5, 2048.0)[lane]
                    if parity
                    else (1.0, -0.5, 256.0, -2048.0)[lane]
                )
            elif variant == "boundary":
                sample = (-65504.0, -0.0, 0.0, 65504.0)[
                    (index + lane + seed) & 3
                ]
            else:
                raise ValueError(f"unknown CoreOnly matrix input pattern: {variant}")
        elif sample_kind == "i32n":
            if variant == "varied":
                sample = ((index * 7919 + seed) & 0xFFFF) - 32768
            elif variant == "seeded-noise":
                sample = seeded_noise_sample(index, seed) - 32768
            elif variant == "checkerboard":
                sample = -(2**31) if parity else 2**31 - 1
            elif variant == "boundary":
                sample = (-(2**31), -1, 0, 2**31 - 1)[(index + seed) & 3]
            else:
                raise ValueError(f"unknown CoreOnly matrix input pattern: {variant}")
        elif sample_kind == "u8":
            if variant == "varied":
                sample = (index * 31 + lane * 17 + seed) & 0xFF
            elif variant == "seeded-noise":
                sample = seeded_noise_sample(index, seed, lane) & 0xFF
            elif variant == "checkerboard":
                sample = 0xAA if parity else 0x55
            elif variant == "boundary":
                sample = (0, 1, 254, 255)[(index + lane + seed) & 3]
            else:
                raise ValueError(f"unknown CoreOnly matrix input pattern: {variant}")
        else:
            raise ValueError(f"unknown CoreOnly sample type: {sample_kind}")
        values.append(sample)
    return struct.pack(pixel_format, *values)


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
@lru_cache(maxsize=64)
def _core_only_matrix_cycle(
    sample_kind: str,
    variant: str,
    channels: int,
    seed: int,
    pixel_format: str,
    cycle_count: int,
) -> bytes:
    """Cache each bounded native sample pattern for reuse across operations."""

    cycle = bytearray()
    for index in range(cycle_count):
        cycle.extend(
            _core_only_matrix_pixel(
                sample_kind, variant, index, 0, channels, seed, pixel_format
            )
        )
    return bytes(cycle)


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def _core_only_matrix_input(
    mode: str,
    sample_kind: str,
    width: int,
    height: int,
    variant: str,
) -> bytes:
    """Create one deterministic native-sample matrix input pattern."""

    channels = _MODE_MATRIX_CHANNELS[mode]
    seed = int.from_bytes(hashlib.sha256(mode.encode("ascii")).digest()[:2], "big")
    sample_formats = {
        "u8": ("", "B"),
        "u16n": ("=", "H"),
        "u16le": ("<", "H"),
        "u16be": (">", "H"),
        "i32n": ("=", "i"),
        "f32n": ("=", "f"),
    }
    prefix, code = sample_formats[sample_kind]
    pixel_format = f"{prefix}{channels}{code}"
    pixel_size = struct.calcsize(pixel_format)
    pixel_count = width * height

    if variant == "impulse":
        peak = {
            "u8": 255,
            "u16n": 65535,
            "u16le": 65535,
            "u16be": 65535,
            "i32n": 2**31 - 1,
            "f32n": 1.0,
        }[sample_kind]
        first_pixel = struct.pack(pixel_format, *([peak] * channels))
        return first_pixel + bytes((pixel_count - 1) * pixel_size)

    if variant == "checkerboard":
        even = _core_only_matrix_pixel(
            sample_kind, variant, 0, 0, channels, seed, pixel_format
        )
        odd = _core_only_matrix_pixel(
            sample_kind, variant, 1, 1, channels, seed, pixel_format
        )
        even_row = (even + odd) * (width // 2) + even * (width & 1)
        odd_row = (odd + even) * (width // 2) + odd * (width & 1)
        raw = bytearray()
        for row in range(height):
            raw.extend(odd_row if row & 1 else even_row)
        return bytes(raw)

    if variant == "varied":
        period = {
            "u8": 256,
            "u16n": 65536,
            "u16le": 65536,
            "u16be": 65536,
            "i32n": 65536,
            "f32n": 8191,
        }[sample_kind]
    elif variant == "boundary":
        period = 4
    elif variant == "seeded-noise":
        period = 4096
    else:
        raise ValueError(f"unknown CoreOnly matrix input pattern: {variant}")

    cycle_count = min(pixel_count, period)
    cycle = _core_only_matrix_cycle(
        sample_kind,
        variant,
        channels,
        seed,
        pixel_format,
        cycle_count,
    )
    repeats, remainder = divmod(pixel_count, cycle_count)
    return bytes(cycle * repeats + cycle[: remainder * pixel_size])


def _nearest_axis_indices(source_size: int, output_size: int) -> list[int]:
    scale = source_size / output_size
    position = scale * 0.5
    indices = []
    for _ in range(output_size):
        indices.append(min(int(position), source_size - 1))
        position += scale
    return indices


def _nearest_native_samples(
    source: bytes,
    sample_kind: str,
    channels: int,
    source_size: tuple[int, int],
    output_size: tuple[int, int],
) -> bytes:
    """Record the exact nearest gather independently from the Resize code."""

    sample_size = 2 if sample_kind == "u16n" else 4
    pixel_size = channels * sample_size
    source_width, source_height = source_size
    output_width, output_height = output_size
    x_indices = _nearest_axis_indices(source_width, output_width)
    y_indices = _nearest_axis_indices(source_height, output_height)
    output = bytearray(output_width * output_height * pixel_size)
    destination_offset = 0
    for source_y in y_indices:
        for source_x in x_indices:
            source_offset = (source_y * source_width + source_x) * pixel_size
            output[destination_offset : destination_offset + pixel_size] = source[
                source_offset : source_offset + pixel_size
            ]
            destination_offset += pixel_size
    return bytes(output)


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def _reduce_native_samples(
    source: bytes,
    mode: str,
    sample_kind: str,
    width: int,
    height: int,
    x_factor: int,
    y_factor: int,
) -> tuple[bytes, int, int]:
    """Record native typed box means without calling the target reducer."""

    channels = _MODE_MATRIX_CHANNELS[mode]
    prefix, sample_code = {
        "u16n": ("=", "H"),
        "u16le": ("<", "H"),
        "u16be": (">", "H"),
        "f32n": ("=", "f"),
    }[sample_kind]
    pixel_format = f"{prefix}{channels}{sample_code}"
    sample_size = struct.calcsize(f"{prefix}{sample_code}")
    pixel_size = channels * sample_size
    output_width = (width + x_factor - 1) // x_factor
    output_height = (height + y_factor - 1) // y_factor
    output = bytearray(output_width * output_height * pixel_size)
    destination_offset = 0

    for output_y in range(output_height):
        source_y = output_y * y_factor
        block_height = min(y_factor, height - source_y)
        for output_x in range(output_width):
            source_x = output_x * x_factor
            block_width = min(x_factor, width - source_x)
            sums = [0 if sample_kind != "f32n" else 0.0 for _ in range(channels)]
            for dy in range(block_height):
                for dx in range(block_width):
                    source_offset = (
                        ((source_y + dy) * width + source_x + dx) * pixel_size
                    )
                    samples = struct.unpack_from(pixel_format, source, source_offset)
                    for channel, sample in enumerate(samples):
                        sums[channel] += sample
            count = block_width * block_height
            if sample_kind == "f32n":
                values = [sample / count for sample in sums]
            else:
                values = [(sample + count // 2) // count for sample in sums]
            struct.pack_into(pixel_format, output, destination_offset, *values)
            destination_offset += pixel_size

    return bytes(output), output_width, output_height


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def _write_target_only_matrix_asset(
    data: bytes,
    assets_root: Path,
    asset_id: str = "resize-source",
) -> dict[str, str]:
    digest = hashlib.sha256(data).hexdigest()
    relative = Path("generated") / f"{digest}.bin"
    path = assets_root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    validated_path = relative.as_posix()
    if path.exists() and validated_path not in _VALIDATED_TARGET_MATRIX_ASSETS:
        if path.read_bytes() != data:
            raise ValueError(f"content-addressed asset mismatch: {relative}")
        _VALIDATED_TARGET_MATRIX_ASSETS.add(validated_path)
    elif not path.exists():
        path.write_bytes(data)
        _VALIDATED_TARGET_MATRIX_ASSETS.add(validated_path)
    return {
        "id": asset_id,
        "kind": "ref_bytes",
        "path": relative.as_posix(),
        "sha256": digest,
        "media_type": "application/octet-stream",
    }


_VALIDATED_TARGET_MATRIX_ASSETS: set[str] = set()


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def _core_only_matrix_points():
    for variant in MODE_SIZE_INPUT_VARIANTS:
        for width, height in MODE_SIZE_MATRIX_SIZES:
            if variant in MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(width, height)]:
                yield variant, width, height


def _target_only_matrix_case(
    base_case: dict[str, Any],
    source: bytes,
    width: int,
    height: int,
    variant: str,
    assets_root: Path,
    case_suffix: str | None = None,
) -> dict[str, Any]:
    case = copy.deepcopy(base_case)
    prefix = case["case_id"].rsplit(".", 1)[0]
    suffix = f".{case_suffix}" if case_suffix is not None else ""
    case["case_id"] = (
        f"{prefix}{suffix}.matrix-input-{variant}.matrix-size-{width}x{height}"
    )
    asset_id = case["assets"][0]["id"]
    case["assets"] = [
        _write_target_only_matrix_asset(source, assets_root, asset_id)
    ]
    image_step = next(
        step
        for step in case["steps"]
        if step["surface"] == "PIL.Image" and step["operation"] == "frombytes"
    )
    image_step["arguments"]["size"]["value"] = [width, height]
    return case


def _core_only_brightness_output(
    mode: str,
    sample_kind: str,
    source: bytes,
    factor: float,
) -> bytes:
    channels = _MODE_MATRIX_CHANNELS[mode]
    sample_dtypes = {
        "u8": np.dtype("u1"),
        "u16n": np.dtype("=u2"),
        "u16le": np.dtype("<u2"),
        "u16be": np.dtype(">u2"),
        "i32n": np.dtype("=i4"),
        "f32n": np.dtype("=f4"),
    }
    dtype = sample_dtypes[sample_kind]
    samples = np.frombuffer(source, dtype=dtype).reshape(-1, channels)
    output = samples.copy()
    alpha_lane = channels - 1 if mode in {
        "ImageLumaA16", "ImageRgba16", "ImageRgba32F"
    } else None
    color_channels = channels - int(alpha_lane is not None)
    colors = samples[:, :color_channels].astype(np.float64)
    scaled = colors * factor
    if sample_kind in {"u8", "u16n", "u16le", "u16be"}:
        scaled = np.trunc(scaled)
        limit = 255 if sample_kind == "u8" else 65535
        np.clip(scaled, 0, limit, out=scaled)
    elif sample_kind == "i32n":
        scaled = np.trunc(scaled)
        np.clip(scaled, -(2**31), 2**31 - 1, out=scaled)
    output[:, :color_channels] = scaled.astype(dtype)
    return output.tobytes()


def brightness_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
    factor_names: set[str] | None = None,
) -> list[dict[str, Any]]:
    """Expand native high-depth Brightness with deterministic image inputs."""

    base_cases = {
        case["covers"][0].rsplit(".", 1)[-1]: case
        for case in brightness_target_output_cases()
    }
    expanded: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, *_outputs in BRIGHTNESS_TARGET_ONLY_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        base_case = base_cases[mode_id]
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(
                mode, sample_kind, width, height, variant
            )
            for name, factor in (
                ("zero", 0.0),
                ("identity", 1.0),
                ("double", 2.0),
            ):
                if factor_names is not None and name not in factor_names:
                    continue
                case = _target_only_matrix_case(
                    base_case,
                    source,
                    width,
                    height,
                    variant,
                    assets_root,
                    case_suffix=f"factor-{name}",
                )
                result_step_id = f"result-{name}"
                result_step_ids = {
                    "result-zero", "result-identity", "result-double"
                }
                case["steps"] = [
                    step
                    for step in case["steps"]
                    if step["step_id"] not in result_step_ids
                    or step["step_id"] == result_step_id
                ]
                case["observations"] = [result_step_id]
                case["target_expectation"]["observations"] = [
                    {
                        "step_id": result_step_id,
                        "value": _native_image_result_record(
                            mode,
                            _core_only_brightness_output(
                                mode, sample_kind, source, factor
                            ),
                            width,
                            height,
                        ),
                    }
                ]
                expanded.append(case)
    return expanded


def chops_invert_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Expand raw stored-byte inversion across every target-only source mode."""

    base_cases = {
        case["covers"][0].rsplit(".", 1)[-1]: case
        for case in chops_invert_target_output_cases()
    }
    expanded: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, _samples in CHOPS_INVERT_CORE_ONLY_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        base_case = base_cases[mode_id]
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(
                mode, sample_kind, width, height, variant
            )
            case = _target_only_matrix_case(
                base_case, source, width, height, variant, assets_root
            )
            inverted = bytes(value ^ 0xFF for value in source)
            case["target_expectation"]["observations"][0]["value"] = (
                _native_image_result_record(mode, inverted, width, height)
            )
            expanded.append(case)
    return expanded


def chops_constant_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Expand the native high-depth Constant source contract over sizes."""

    base_cases = {
        case["covers"][0].rsplit(".", 1)[-1]: case
        for case in chops_constant_target_output_cases()
    }
    expanded: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, _samples in CHOPS_CONSTANT_CORE_ONLY_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        base_case = base_cases[mode_id]
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(mode, sample_kind, width, height, variant)
            case = _target_only_matrix_case(
                base_case, source, width, height, variant, assets_root
            )
            constant = bytes([73]) * (width * height)
            case["target_expectation"]["observations"][0]["value"] = (
                _native_image_result_record("L", constant, width, height)
            )
            expanded.append(case)
    return expanded


def crop_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Expand typed Crop over every declared internal mode/size/pattern."""

    base_cases = {
        case["covers"][0].rsplit(".", 1)[-1]: case
        for case in crop_core_only_target_output_cases()
    }
    expanded: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, *_samples in CROP_CORE_ONLY_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        channels = CORE_ONLY_CHANNELS[mode]
        sample_size = 2 if sample_kind == "u16n" else 4
        pixel_size = channels * sample_size
        base_case = base_cases[mode_id]
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(
                mode, sample_kind, width, height, variant
            )
            left, top = width // 2, height // 2
            output_width, output_height = width - left, height - top
            output = bytearray(output_width * output_height * pixel_size)
            row_bytes = output_width * pixel_size
            for y in range(top, height):
                source_offset = (y * width + left) * pixel_size
                destination_offset = (y - top) * row_bytes
                output[destination_offset : destination_offset + row_bytes] = (
                    source[source_offset : source_offset + row_bytes]
                )
            case = _target_only_matrix_case(
                base_case, source, width, height, variant, assets_root
            )
            crop_step = next(
                step for step in case["steps"] if step["operation"] == "crop"
            )
            crop_step["arguments"]["box"]["value"] = [
                left, top, width, height
            ]
            case["target_expectation"]["observations"][0]["value"] = (
                _native_image_result_record(
                    mode, bytes(output), output_width, output_height
                )
            )
            expanded.append(case)
    return expanded


def chops_offset_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Expand circular Offset with native samples and nonzero x/y shifts."""

    base_cases = {
        case["covers"][0].rsplit(".", 1)[-1]: case
        for case in chops_offset_target_output_cases()
    }
    expanded: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, _samples in CHOPS_DUPLICATE_CORE_ONLY_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        channels = CORE_ONLY_CHANNELS[mode]
        sample_size = 2 if sample_kind == "u16n" else 4
        pixel_size = channels * sample_size
        base_case = base_cases[mode_id]
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(
                mode, sample_kind, width, height, variant
            )
            output = bytearray(len(source))
            row_bytes = width * pixel_size
            for y in range(height):
                source_y = (y + 1) % height
                source_offset = source_y * row_bytes
                source_row = source[source_offset : source_offset + row_bytes]
                destination = y * row_bytes
                output[destination : destination + pixel_size] = source_row[-pixel_size:]
                output[destination + pixel_size : destination + row_bytes] = (
                    source_row[:-pixel_size]
                )
            case = _target_only_matrix_case(
                base_case, source, width, height, variant, assets_root
            )
            offset_step = next(
                step for step in case["steps"] if step["operation"] == "offset"
            )
            offset_step["arguments"]["xoffset"]["value"] = 1
            offset_step["arguments"]["yoffset"]["value"] = -1
            case["target_expectation"]["observations"][0]["value"] = (
                _native_image_result_record(mode, bytes(output), width, height)
            )
            expanded.append(case)
    return expanded


def resize_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Record native nearest-Resize outputs for every internal raster mode."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, _samples in CHOPS_DUPLICATE_CORE_ONLY_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        for variant in MODE_SIZE_INPUT_VARIANTS:
            for width, height in MODE_SIZE_MATRIX_SIZES:
                if variant not in MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(width, height)]:
                    continue
                output_size = (max(1, width // 2), max(1, height // 2))
                source = _core_only_matrix_input(
                    mode, sample_kind, width, height, variant
                )
                output = _nearest_native_samples(
                    source,
                    sample_kind,
                    CORE_ONLY_CHANNELS[mode],
                    (width, height),
                    output_size,
                )
                cases.append(
                    {
                        "case_id": (
                            "PIL.Image.Image.resize.expected-output."
                            f"{mode_id}.matrix-input-{variant}."
                            f"matrix-size-{width}x{height}"
                        ),
                        "surface": "PIL.Image.Image",
                        "operation": "resize",
                        "verification": "expected-output",
                        "covers": [f"PIL.Image.Image.resize.mode.{mode_id}"],
                        "assets": [
                            _write_target_only_matrix_asset(source, assets_root)
                        ],
                        "steps": [
                            {
                                "step_id": "image",
                                "surface": "PIL.Image",
                                "operation": "frombytes",
                                "receiver": None,
                                "arguments": {
                                    "mode": {"kind": "literal", "value": mode},
                                    "size": {
                                        "kind": "literal",
                                        "value": [width, height],
                                    },
                                    "data": {
                                        "kind": "asset",
                                        "asset_id": "resize-source",
                                    },
                                },
                            },
                            {
                                "step_id": "result",
                                "surface": "PIL.Image.Image",
                                "operation": "resize",
                                "receiver": {"kind": "binding", "step_id": "image"},
                                "arguments": {
                                    "size": {
                                        "kind": "literal",
                                        "value": list(output_size),
                                    },
                                    "resample": {"kind": "literal", "value": 0},
                                },
                            },
                        ],
                        "observations": ["result"],
                        "target_expectation": {
                            "observations": [
                                {
                                    "step_id": "result",
                                    "value": _native_image_result_record(
                                        mode, output, *output_size
                                    ),
                                }
                            ]
                        },
                    }
                )
    return cases


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def reduce_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
    selected_factor: str | None = None,
) -> list[dict[str, Any]]:
    """Record native typed Reduce outputs across the shared mode-size matrix."""

    factor_names = (
        (selected_factor,) if selected_factor is not None else tuple(REDUCE_MATRIX_FACTORS)
    )
    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind in REDUCE_CORE_ONLY_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        for factor_name in factor_names:
            x_factor, y_factor = REDUCE_MATRIX_FACTORS[factor_name]
            for variant, width, height in _core_only_matrix_points():
                source = _core_only_matrix_input(
                    mode, sample_kind, width, height, variant
                )
                output, output_width, output_height = _reduce_native_samples(
                    source,
                    mode,
                    sample_kind,
                    width,
                    height,
                    x_factor,
                    y_factor,
                )
                cases.append(
                    {
                        "case_id": (
                            "PIL.Image.Image.reduce.expected-output."
                            f"{mode_id}.{factor_name}.matrix-input-{variant}."
                            f"matrix-size-{width}x{height}"
                        ),
                        "surface": "PIL.Image.Image",
                        "operation": "reduce",
                        "verification": "expected-output",
                        "covers": [f"PIL.Image.Image.reduce.mode.{mode_id}"],
                        "assets": [
                            _write_target_only_matrix_asset(
                                source, assets_root, "reduce-source"
                            )
                        ],
                        "steps": [
                            {
                                "step_id": "image",
                                "surface": "PIL.Image",
                                "operation": "frombytes",
                                "receiver": None,
                                "arguments": {
                                    "mode": {"kind": "literal", "value": mode},
                                    "size": {
                                        "kind": "literal",
                                        "value": [width, height],
                                    },
                                    "data": {
                                        "kind": "asset",
                                        "asset_id": "reduce-source",
                                    },
                                },
                            },
                            {
                                "step_id": "result",
                                "surface": "PIL.Image.Image",
                                "operation": "reduce",
                                "receiver": {"kind": "binding", "step_id": "image"},
                                "arguments": {
                                    "factor": {
                                        "kind": "literal",
                                        "value": [x_factor, y_factor],
                                    },
                                    "box": {"kind": "literal", "value": None},
                                },
                            },
                        ],
                        "observations": ["result"],
                        "target_expectation": {
                            "observations": [
                                {
                                    "step_id": "result",
                                    "value": _native_image_result_record(
                                        mode,
                                        output,
                                        output_width,
                                        output_height,
                                    ),
                                }
                            ]
                        },
                    }
                )
    return cases


def putdata_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Record native typed PutData writes across the CoreOnly size matrix."""

    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, _samples in CHOPS_DUPLICATE_CORE_ONLY_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        channels = CORE_ONLY_CHANNELS[mode]
        sample_format = "=H" if sample_kind == "u16n" else "=f"
        sample_size = struct.calcsize(sample_format)
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(mode, sample_kind, width, height, variant)
            output = bytearray(source)
            updates: list[list[int | float]] = []

            for pixel_index in range(min(width * height, 5)):
                start = pixel_index * channels * sample_size
                original = list(
                    struct.unpack_from(f"={channels}{'H' if sample_kind == 'u16n' else 'f'}", source, start)
                )
                if sample_kind == "u16n":
                    values: list[int | float] = [int(value) for value in original]
                    if pixel_index == 0:
                        values[0] = -1
                        values[-1] = 65536
                    elif pixel_index == 1:
                        values[0] = 70000
                        values[-1] = -70000
                    arity = channels
                    if mode in {"ImageRgb16", "ImageRgb32F"} and pixel_index == 0:
                        values.append(12345)
                        arity = 4
                    elif mode in {"ImageRgba16", "ImageRgba32F"} and pixel_index % 2 == 1:
                        arity = 3
                    normalized = values[: min(channels, arity)]
                    if channels == 4 and arity == 3:
                        normalized.append(65535)
                    normalized = [min(65535, max(0, int(value))) for value in normalized]
                    encoded = struct.pack("=4H" if channels == 4 else f"={channels}H", *normalized)
                    values = values[:arity]
                else:
                    values = [float(value) + (lane + 1) * 0.125 for lane, value in enumerate(original)]
                    arity = channels
                    if mode == "ImageRgb32F" and pixel_index == 0:
                        values.append(0.75)
                        arity = 4
                    elif mode == "ImageRgba32F" and pixel_index % 2 == 1:
                        arity = 3
                    normalized = [
                        float(value) for value in values[: min(channels, arity)]
                    ]
                    if channels == 4 and arity == 3:
                        normalized.append(1.0)
                    encoded = struct.pack(f"={channels}f", *normalized)
                    values = values[:arity]

                updates.append(values)
                output[start : start + channels * sample_size] = encoded

            case_id = (
                "PIL.Image.Image.putdata.expected-output."
                f"{mode_id}.matrix-input-{variant}.matrix-size-{width}x{height}"
            )
            # CoreOnly component tuples write native samples directly, so the
            # non-identity scale/offset below are intentionally not applied.
            cases.append(
                {
                    "case_id": case_id,
                    "surface": "PIL.Image.Image",
                    "operation": "putdata",
                    "verification": "expected-output",
                    "covers": [
                        "PIL.Image.Image.putdata.behavior.default",
                        f"PIL.Image.Image.putdata.mode.{mode_id}",
                    ],
                    "assets": [_write_target_only_matrix_asset(source, assets_root, "putdata-source")],
                    "steps": [
                        {
                            "step_id": "image",
                            "surface": "PIL.Image",
                            "operation": "frombytes",
                            "receiver": None,
                            "arguments": {
                                "mode": {"kind": "literal", "value": mode},
                                "size": {"kind": "literal", "value": [width, height]},
                                "data": {"kind": "asset", "asset_id": "putdata-source"},
                            },
                        },
                        {
                            "step_id": "call",
                            "surface": "PIL.Image.Image",
                            "operation": "putdata",
                            "receiver": {"kind": "binding", "step_id": "image"},
                            "arguments": {
                                "data": {"kind": "literal", "value": {"protocol": "list", "items": updates}},
                                "scale": {"kind": "literal", "value": 3.0},
                                "offset": {"kind": "literal", "value": -7.0},
                            },
                        },
                        {
                            "step_id": "observe-receiver",
                            "surface": "PIL.Image.Image",
                            "operation": "tobytes",
                            "receiver": {"kind": "binding", "step_id": "image"},
                            "arguments": {},
                        },
                    ],
                    "observations": ["call", "observe-receiver"],
                    "target_expectation": {
                        "observations": [
                            {"step_id": "call", "value": None},
                            {
                                "step_id": "observe-receiver",
                                "value": _native_image_result_record(mode, bytes(output), width, height),
                            }
                        ]
                    },
                }
            )
    cases.extend(
        putdata_core_only_full_frame_cases(assets_root, selected_mode_id)
    )
    return cases


def putdata_core_only_full_frame_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Exercise every native PutData lane in size-scaled full-frame writes."""

    cases: list[dict[str, Any]] = []
    for width, height in ((257, 193), (512, 384)):
        for variant in ("varied", "seeded-noise"):
            for mode, mode_id, sample_kind, _samples in CHOPS_DUPLICATE_CORE_ONLY_MODES:
                if selected_mode_id is not None and mode_id != selected_mode_id:
                    continue
                source = _core_only_matrix_input(
                    mode, sample_kind, width, height, variant
                )
                updates, output = _native_putdata_frame(mode, sample_kind, source)
                cases.append(
                    {
                        "case_id": (
                            "PIL.Image.Image.putdata.expected-output."
                            f"{mode_id}.full-frame.matrix-input-{variant}."
                            f"matrix-size-{width}x{height}"
                        ),
                        "surface": "PIL.Image.Image",
                        "operation": "putdata",
                        "verification": "expected-output",
                        "covers": [
                            "PIL.Image.Image.putdata.behavior.default",
                            f"PIL.Image.Image.putdata.mode.{mode_id}",
                        ],
                        "assets": [
                            _write_target_only_matrix_asset(
                                source, assets_root, "putdata-source"
                            )
                        ],
                        "steps": [
                            {
                                "step_id": "image",
                                "surface": "PIL.Image",
                                "operation": "frombytes",
                                "receiver": None,
                                "arguments": {
                                    "mode": {"kind": "literal", "value": mode},
                                    "size": {
                                        "kind": "literal",
                                        "value": [width, height],
                                    },
                                    "data": {
                                        "kind": "asset",
                                        "asset_id": "putdata-source",
                                    },
                                },
                            },
                            {
                                "step_id": "call",
                                "surface": "PIL.Image.Image",
                                "operation": "putdata",
                                    "receiver": {
                                        "kind": "binding",
                                        "step_id": "image",
                                    },
                                "arguments": {
                                    "data": {
                                        "kind": "literal",
                                            "value": {
                                                "protocol": "list",
                                                "items": updates,
                                            },
                                    },
                                    "scale": {"kind": "literal", "value": 3.0},
                                    "offset": {"kind": "literal", "value": -7.0},
                                },
                            },
                            {
                                "step_id": "observe-receiver",
                                "surface": "PIL.Image.Image",
                                "operation": "tobytes",
                                "receiver": {
                                    "kind": "binding",
                                    "step_id": "image",
                                },
                                "arguments": {},
                            },
                        ],
                        "observations": ["call", "observe-receiver"],
                        "target_expectation": {
                            "observations": [
                                {"step_id": "call", "value": None},
                                {
                                    "step_id": "observe-receiver",
                                    "value": _native_image_result_record(
                                        mode, output, width, height
                                    ),
                                }
                            ]
                        },
                    }
                )
    return cases


def _native_putdata_frame(
    mode: str, sample_kind: str, source: bytes
) -> tuple[list[int | list[int | float]], bytes]:
    """Build typed PutData values and independent native-sample output bytes."""

    channels = CORE_ONLY_CHANNELS[mode]
    sample_code = "H" if sample_kind == "u16n" else "f"
    sample_size = struct.calcsize(f"={sample_code}")
    output = bytearray(source)
    updates: list[int | list[int | float]] = []
    packed = 0x0002_0001_0006_0005
    for pixel_index in range(len(source) // (channels * sample_size)):
        start = pixel_index * channels * sample_size
        original = struct.unpack_from(f"={channels}{sample_code}", source, start)
        if sample_kind == "u16n" and pixel_index == 2:
            updates.append(packed)
            output[start : start + channels * sample_size] = packed.to_bytes(
                8, "little"
            )[: channels * sample_size]
            continue
        if sample_kind == "u16n":
            values = [int(sample) for sample in original]
            if pixel_index == 0:
                values[0], values[-1] = -1, 65536
            elif pixel_index == 1:
                values[0], values[-1] = 70000, -70000
            else:
                delta = ((pixel_index % 31) - 15) * 7
                values = [sample + delta for sample in values]
            arity = channels
            if mode == "ImageRgb16" and pixel_index == 0:
                values.append(12345)
                arity = 4
            elif mode == "ImageRgba16" and pixel_index % 2 == 1:
                arity = 3
            normalized = [
                min(65535, max(0, value))
                for value in values[: min(channels, arity)]
            ]
            if channels == 4 and arity == 3:
                normalized.append(65535)
            encoded = struct.pack(f"={channels}H", *normalized)
            updates.append(values[:arity])
        else:
            values = [
                float(sample) + (lane + 1) * 0.125
                for lane, sample in enumerate(original)
            ]
            arity = channels
            if mode == "ImageRgb32F" and pixel_index == 0:
                values.append(0.75)
                arity = 4
            elif mode == "ImageRgba32F" and pixel_index % 2 == 1:
                arity = 3
            normalized = values[: min(channels, arity)]
            if channels == 4 and arity == 3:
                normalized.append(1.0)
            encoded = struct.pack(f"={channels}f", *normalized)
            updates.append(values[:arity])
        output[start : start + channels * sample_size] = encoded
    return updates, bytes(output)


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


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def _native_image_result_record(
    mode: str,
    raw: bytes,
    width: int = 3,
    height: int = 1,
    *,
    info: Any = _DEFAULT_TARGET_IMAGE_INFO,
) -> dict[str, Any]:
    return {
        "kind": "image",
        "mode": mode,
        "size": [width, height],
        "format": None,
        "info": {} if info is _DEFAULT_TARGET_IMAGE_INFO else info,
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


def sharpness_core_only_matrix_cases(
    assets_root: Path,
    selected_mode_id: str | None = None,
) -> list[dict[str, Any]]:
    """Record native-only Sharpness outputs for every mode and matrix size."""

    base_cases = {
        case["covers"][0].rsplit(".", 1)[-1]: case
        for case in sharpness_core_only_cases()
    }
    cases: list[dict[str, Any]] = []
    for mode, mode_id, sample_kind, *_samples in SHARPNESS_CORE_ONLY_MODES:
        if selected_mode_id is not None and mode_id != selected_mode_id:
            continue
        base_case = base_cases[mode_id]
        for variant, width, height in _core_only_matrix_points():
            source = _core_only_matrix_input(
                mode, sample_kind, width, height, variant
            )
            output = _sharpness_matrix_output(
                mode, sample_kind, source, width, height
            )
            case = _target_only_matrix_case(
                base_case,
                source,
                width,
                height,
                variant,
                assets_root,
                case_suffix="factor-sharpened",
            )
            result_step = "result-sharpened"
            case["steps"] = [
                step
                for step in case["steps"]
                if step["step_id"] not in {"result-degenerate", "result-identity"}
            ]
            case["observations"] = [result_step]
            result_record = _native_image_result_record(
                mode, output, width, height
            )
            case["target_expectation"]["observations"] = [
                {
                    "step_id": result_step,
                    "value": result_record,
                }
            ]
            cases.append(case)
    return cases


def _sharpness_matrix_output(
    mode: str,
    sample_kind: str,
    source: bytes,
    width: int,
    height: int,
) -> bytes:
    """Calculate factor-two Sharpness outputs from the native 3×3 smooth."""

    sample_dtypes = {
        "u8": np.dtype("u1"),
        "u16n": np.dtype("=u2"),
        "u16le": np.dtype("<u2"),
        "u16be": np.dtype(">u2"),
        "i32n": np.dtype("=i4"),
        "f32n": np.dtype("=f4"),
    }
    dtype = sample_dtypes[sample_kind]
    channels = _MODE_MATRIX_CHANNELS[mode]
    active_channels = {
        "La": 1,
        "RGBa": 3,
        "ImageLumaA16": 1,
        "ImageRgba16": 3,
        "ImageRgba32F": 3,
    }.get(mode, channels)
    samples = np.frombuffer(source, dtype=dtype).reshape(
        height, width, channels
    )
    output = samples.copy()
    if width < 3 or height < 3:
        return output.tobytes()

    center = samples[1:-1, 1:-1, :active_channels]
    neighborhoods = (
        samples[:-2, :-2, :active_channels],
        samples[:-2, 1:-1, :active_channels],
        samples[:-2, 2:, :active_channels],
        samples[1:-1, :-2, :active_channels],
        samples[1:-1, 2:, :active_channels],
        samples[2:, :-2, :active_channels],
        samples[2:, 1:-1, :active_channels],
        samples[2:, 2:, :active_channels],
    )

    if sample_kind in {"u8", "u16n", "u16le", "u16be"}:
        center_integer = center.astype(np.int64)
        weighted = sum(
            (neighbor.astype(np.int64) for neighbor in neighborhoods),
            start=np.zeros_like(center_integer),
        )
        weighted += center_integer * 5
        smooth = (weighted + 6) // 13
        limit = 255 if sample_kind == "u8" else 65535
        sharpened = np.clip(2 * center_integer - smooth, 0, limit)
    elif sample_kind == "i32n":
        center_float = center.astype(np.float64)
        weighted = sum(
            (neighbor.astype(np.int64) for neighbor in neighborhoods),
            start=np.zeros(center.shape, dtype=np.int64),
        )
        weighted += center.astype(np.int64) * 5
        smooth = weighted.astype(np.float64) / 13.0
        sharpened = np.trunc(
            2.0 * (center_float - smooth) + smooth
        )
        sharpened = np.clip(sharpened, -(2**31), 2**31 - 1)
    else:
        weighted = np.add(neighborhoods[0], neighborhoods[1])
        weighted = np.add(weighted, neighborhoods[2])
        weighted = np.add(weighted, neighborhoods[3])
        weighted = np.add(weighted, np.multiply(center, np.float32(5.0)))
        weighted = np.add(weighted, neighborhoods[4])
        weighted = np.add(weighted, neighborhoods[5])
        weighted = np.add(weighted, neighborhoods[6])
        weighted = np.add(weighted, neighborhoods[7])
        smooth = np.divide(weighted, np.float32(13.0))
        difference = np.subtract(center, smooth)
        # Multiplication by two is exact here; the following f32 add is the
        # same single rounding used by the scalar reference expression.
        sharpened = np.add(np.multiply(difference, np.float32(2.0)), smooth)

    output[1:-1, 1:-1, :active_channels] = sharpened.astype(dtype)
    return output.tobytes()


def _round_f32(value: float) -> float:
    return struct.unpack("=f", struct.pack("=f", value))[0]


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
    cases.extend(core_only_l1_rawmode_cases())
    return cases


def core_only_l1_rawmode_cases() -> list[dict[str, Any]]:
    """Record the target-only L;1 raw decoder extension across varied inputs."""

    # NO_PILLOW_SUPPORT: Pillow 13 declares L;1 but has no matching unpacker.
    cases: list[dict[str, Any]] = []
    for width, height in MODE_SIZE_MATRIX_SIZES:
        for variant in MODE_SIZE_INPUT_VARIANTS_BY_SIZE[(width, height)]:
            seed_source = f"frombytes-l1:{width}x{height}:{variant}".encode()
            seed = int.from_bytes(hashlib.sha256(seed_source).digest()[:4], "big")
            encoded = bytearray()
            pixels = bytearray()
            for y in range(height):
                row = bytearray((width + 7) // 8)
                for x in range(width):
                    index = y * width + x
                    if variant == "varied":
                        sample = ((x * 31 + y * 17 + x * y * 13 + seed) >> (x % 3)) & 1
                    elif variant == "checkerboard":
                        sample = (x + y) & 1
                    elif variant == "boundary":
                        sample = int(
                            x == 0 or y == 0 or x == width - 1 or y == height - 1
                        )
                    elif variant == "impulse":
                        sample = int(index == 0)
                    elif variant == "seeded-noise":
                        sample = seeded_noise_sample(index, seed) & 1
                    else:
                        raise ValueError(f"unknown L;1 pattern variant: {variant}")
                    pixels.append(sample * 255)
                    if sample:
                        row[x // 8] |= 1 << (7 - x % 8)
                encoded.extend(row)

            case = _target_case(
                "L",
                "l",
                f"l1-extension.{variant}.matrix-size-{width}x{height}",
                (width, height),
                bytes(encoded),
                {
                    "kind": "image",
                    "mode": "L",
                    "size": [width, height],
                    "format": None,
                    "info": {},
                    "palette": None,
                    "bytes": base64.b64encode(pixels).decode("ascii"),
                },
            )
            arguments = case["steps"][0]["arguments"]
            arguments["decoder_name"] = {"kind": "literal", "value": "raw"}
            arguments["args"] = {"kind": "literal", "value": ["L;1"]}
            case["covers"].insert(
                0,
                "PIL.Image.frombytes.code_path.rawmode-l1",
            )
            cases.append(case)
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


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def rotate_direct_target_output_cases() -> list[dict[str, Any]]:
    """Record direct Rust Rotate shortcuts and exact-mode identity outcomes."""

    cases = []
    for mode, mode_id in DIRECT_ROTATE_MODES:
        bytes_per_pixel = _DIRECT_ROTATE_BYTES_PER_PIXEL[mode]
        raw = bytes(bytes_per_pixel)
        asset_id = f"source-data-{mode_id}"
        encoded = base64.b64encode(raw).decode("ascii")
        cases.append(
            {
                "case_id": (
                    "PIL.Image.Image._migration_direct_rotate.expected-output."
                    f"identity.{mode_id}"
                ),
                "surface": "PIL.Image.Image",
                "operation": "_migration_direct_rotate",
                "verification": "expected-output",
                "covers": [
                    "PIL.Image.Image._migration_direct_rotate.behavior.default",
                    f"PIL.Image.Image._migration_direct_rotate.mode.{mode_id}",
                    "PIL.Image.Image._migration_direct_rotate.fast-path.identity",
                ],
                "assets": [
                    {
                        "id": asset_id,
                        "kind": "inline",
                        "encoding": "base64",
                        "data": encoded,
                        "sha256": hashlib.sha256(raw).hexdigest(),
                        "media_type": "application/octet-stream",
                    }
                ],
                "steps": _direct_rotate_steps(mode, [1, 1], asset_id, 0, False),
                "observations": ["rotated"],
                "target_expectation": {
                    "observations": [
                        {
                            "step_id": "rotated",
                            "value": _native_image_result_record(
                                mode,
                                bytes(bytes_per_pixel),
                                1,
                                1,
                                info=None,
                            ),
                        }
                    ]
                },
            }
        )
    cases.extend(_direct_rotate_branch_cases())
    return cases


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def _direct_rotate_steps(
    mode: str,
    size: list[int],
    asset_id: str,
    angle: int,
    expand: bool,
) -> list[dict[str, Any]]:
    """Build the live-handle workflow used by direct Rotate output cases."""

    return [
        {
            "step_id": "image",
            "surface": "PIL.Image",
            "operation": "frombytes",
            "receiver": None,
            "arguments": {
                "mode": {"kind": "literal", "value": mode},
                "size": {"kind": "literal", "value": size},
                "data": {"kind": "asset", "asset_id": asset_id},
            },
        },
        {
            "step_id": "handle",
            "surface": "PIL.Image.Image",
            "operation": "getim",
            "receiver": {"kind": "binding", "step_id": "image"},
            "arguments": {},
        },
        {
            "step_id": "rotated",
            "surface": "PIL.Image.Image",
            "operation": "_migration_direct_rotate",
            "receiver": {"kind": "binding", "step_id": "handle"},
            "arguments": {
                "angle": {"kind": "literal", "value": angle},
                "expand": {"kind": "literal", "value": expand},
            },
        },
    ]


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def _direct_rotate_branch_cases() -> list[dict[str, Any]]:
    """Record direct Rotate's discrete-turn and expand shortcut outputs."""

    branch_inputs = (
        (
            "half-turn",
            180,
            False,
            [2, 2],
            bytes((11, 22, 33, 44)),
            [2, 2],
            bytes((44, 33, 22, 11)),
        ),
        (
            "quarter-turn-90",
            90,
            False,
            [2, 2],
            bytes((11, 22, 33, 44)),
            [2, 2],
            bytes((22, 44, 11, 33)),
        ),
        (
            "quarter-turn-270",
            270,
            False,
            [2, 2],
            bytes((11, 22, 33, 44)),
            [2, 2],
            bytes((33, 11, 44, 22)),
        ),
        (
            "quarter-turn-expand",
            90,
            True,
            [2, 1],
            bytes((17, 42)),
            [1, 2],
            bytes((42, 17)),
        ),
    )
    return [_direct_rotate_branch_case(*branch) for branch in branch_inputs]


# VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
def _direct_rotate_branch_case(
    edge: str,
    angle: int,
    expand: bool,
    size: list[int],
    raw: bytes,
    output_size: list[int],
    output: bytes,
) -> dict[str, Any]:
    """Build one fixed expected-output contract for a direct Rotate branch."""

    encoded = base64.b64encode(raw).decode("ascii")
    return {
        "case_id": (
            "PIL.Image.Image._migration_direct_rotate.expected-output."
            f"{edge}"
        ),
        "surface": "PIL.Image.Image",
        "operation": "_migration_direct_rotate",
        "verification": "expected-output",
        "covers": [
            "PIL.Image.Image._migration_direct_rotate.behavior.default",
            f"PIL.Image.Image._migration_direct_rotate.fast-path.{edge}",
        ],
        "assets": [
            {
                "id": "source-data",
                "kind": "inline",
                "encoding": "base64",
                "data": encoded,
                "sha256": hashlib.sha256(raw).hexdigest(),
                "media_type": "application/octet-stream",
            }
        ],
        "steps": _direct_rotate_steps("L", size, "source-data", angle, expand),
        "observations": ["rotated"],
        "target_expectation": {
            "observations": [
                {
                    "step_id": "rotated",
                    "value": _native_image_result_record(
                        "L", output, *output_size, info=None
                    ),
                }
            ]
        },
    }


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
