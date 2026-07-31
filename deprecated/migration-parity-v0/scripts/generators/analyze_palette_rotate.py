#!/usr/bin/env python3
"""Reverse-map Pillow palette rotation to its nearest-sample coordinate rule."""

from __future__ import annotations

import math
from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1] / "pillow-rs/tests/fixtures/image_backend"
SOURCE = ROOT / "inputs/png_indexed_alpha.png"
EXPECTED = ROOT / "outputs/operations/rotate_27_expand.bin"
ANGLE = 27.0


def pillow_matrix(width: int, height: int) -> tuple[int, int, tuple[float, ...]]:
    angle = -math.radians(ANGLE)
    a = round(math.cos(angle), 15)
    b = round(math.sin(angle), 15)
    d = round(-math.sin(angle), 15)
    e = round(math.cos(angle), 15)
    center = (width / 2.0, height / 2.0)
    c = a * -center[0] + b * -center[1] + center[0]
    f = d * -center[0] + e * -center[1] + center[1]

    def transform(x: float, y: float) -> tuple[float, float]:
        return a * x + b * y + c, d * x + e * y + f

    corners = [transform(x, y) for x, y in ((0, 0), (width, 0), (width, height), (0, height))]
    new_width = math.ceil(max(x for x, _ in corners)) - math.floor(min(x for x, _ in corners))
    new_height = math.ceil(max(y for _, y in corners)) - math.floor(min(y for _, y in corners))
    shift_x = -(new_width - width) / 2.0
    shift_y = -(new_height - height) / 2.0
    c, f = a * shift_x + b * shift_y + c, d * shift_x + e * shift_y + f
    return new_width, new_height, (a, b, c, d, e, f)


def candidate(
    source: bytes,
    width: int,
    height: int,
    output_width: int,
    output_height: int,
    matrix: tuple[float, ...],
    destination_center: bool,
    nearest_round: bool,
    epsilon: float = 0.0,
) -> bytes:
    a, b, c, d, e, f = matrix
    output = bytearray(output_width * output_height)
    offset = 0.5 if destination_center else 0.0
    for y in range(output_height):
        for x in range(output_width):
            source_x = a * (x + offset) + b * (y + offset) + c
            source_y = d * (x + offset) + e * (y + offset) + f
            if nearest_round:
                source_x = math.floor(source_x + 0.5)
                source_y = math.floor(source_y + 0.5)
            else:
                source_x = math.floor(source_x + epsilon)
                source_y = math.floor(source_y + epsilon)
            if 0 <= source_x < width and 0 <= source_y < height:
                output[y * output_width + x] = source[source_y * width + source_x]
    return bytes(output)


def main() -> None:
    with Image.open(SOURCE) as image:
        source = image.tobytes()
        width, height = image.size
    expected = EXPECTED.read_bytes()
    output_width, output_height, matrix = pillow_matrix(width, height)

    for destination_center in (False, True):
        for nearest_round in (False, True):
            actual = candidate(
                source,
                width,
                height,
                output_width,
                output_height,
                matrix,
                destination_center,
                nearest_round,
            )
            differences = [index for index, pair in enumerate(zip(actual, expected)) if pair[0] != pair[1]]
            print(
                f"destination_center={destination_center} nearest_round={nearest_round} "
                f"mismatches={len(differences)} first={differences[0] if differences else None}"
            )

    for epsilon in (-1e-12, -1e-14, -1e-15, 0.0, 1e-15, 1e-14, 1e-12):
        actual = candidate(
            source,
            width,
            height,
            output_width,
            output_height,
            matrix,
            True,
            False,
            epsilon,
        )
        differences = [index for index, pair in enumerate(zip(actual, expected)) if pair[0] != pair[1]]
        print(f"epsilon={epsilon:+.0e} mismatches={len(differences)} first={differences[0] if differences else None}")

    actual = candidate(source, width, height, output_width, output_height, matrix, True, False)
    a, b, c, d, e, f = matrix
    for index, pair in enumerate(zip(actual, expected)):
        if pair[0] == pair[1]:
            continue
        x, y = index % output_width, index // output_width
        source_x = a * (x + 0.5) + b * (y + 0.5) + c
        source_y = d * (x + 0.5) + e * (y + 0.5) + f
        print(
            f"diff output=({x},{y}) source=({source_x:.17g},{source_y:.17g}) "
            f"actual={pair[0]} expected={pair[1]}"
        )


if __name__ == "__main__":
    main()
