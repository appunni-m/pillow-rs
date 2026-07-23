#!/usr/bin/env python3
"""Generate exact Pillow references for palette-preserving operations.

Run with the Pillow version pinned by image-slash-star/pillow-oracle.lock.yaml.
The script only consumes the checked-in indexed PNG fixture and writes the
operation rows plus raw index buffers used by the Rust migration test.
"""

from __future__ import annotations

from io import BytesIO
import json
from pathlib import Path

from PIL import Image, ImageChops, ImageOps, __version__


ROOT = Path(__file__).resolve().parents[1] / "pillow-rs/tests/fixtures/image_backend"
INPUT = ROOT / "inputs/png_indexed_alpha.png"
OUTPUTS = ROOT / "outputs/operations"
MANIFEST = ROOT / "operations.json"
EXPECTED_PILLOW = "12.2.0"


def transparency_hex(image: Image.Image) -> str | None:
    transparency = image.info.get("transparency")
    if transparency is None:
        return None
    if isinstance(transparency, int):
        return bytes([transparency]).hex()
    return bytes(transparency).hex()


def operation_rows(source: Image.Image) -> list[tuple[str, dict[str, object], Image.Image]]:
    thumbnail = source.copy()
    thumbnail.thumbnail((47, 39), Image.Resampling.NEAREST)
    putpixel = source.copy()
    putpixel.putpixel((3, 4), 7)
    crop_then_putpixel = source.crop((9, 7, 61, 48))
    crop_then_putpixel.putpixel((3, 4), 7)
    putpixel_rgb = source.copy()
    putpixel_rgb.putpixel((3, 4), (1, 2, 3))

    return [
        ("crop", {"box": [9, 7, 61, 48]}, source.crop((9, 7, 61, 48))),
        (
            "resize_nearest",
            {"size": [53, 41]},
            source.resize((53, 41), Image.Resampling.NEAREST),
        ),
        ("thumbnail_nearest", {"size": [47, 39]}, thumbnail),
        (
            "rotate_27_expand",
            {"angle": 27.0, "expand": True},
            source.rotate(27.0, Image.Resampling.NEAREST, expand=True),
        ),
        (
            "transpose_flip_left_right",
            {"method": "FLIP_LEFT_RIGHT"},
            source.transpose(Image.Transpose.FLIP_LEFT_RIGHT),
        ),
        (
            "transpose_flip_top_bottom",
            {"method": "FLIP_TOP_BOTTOM"},
            source.transpose(Image.Transpose.FLIP_TOP_BOTTOM),
        ),
        (
            "transpose_rotate_90",
            {"method": "ROTATE_90"},
            source.transpose(Image.Transpose.ROTATE_90),
        ),
        (
            "transpose_rotate_180",
            {"method": "ROTATE_180"},
            source.transpose(Image.Transpose.ROTATE_180),
        ),
        (
            "transpose_rotate_270",
            {"method": "ROTATE_270"},
            source.transpose(Image.Transpose.ROTATE_270),
        ),
        (
            "transpose_transpose",
            {"method": "TRANSPOSE"},
            source.transpose(Image.Transpose.TRANSPOSE),
        ),
        (
            "transpose_transverse",
            {"method": "TRANSVERSE"},
            source.transpose(Image.Transpose.TRANSVERSE),
        ),
        ("imageops_flip", {}, ImageOps.flip(source)),
        ("imageops_mirror", {}, ImageOps.mirror(source)),
        ("imageops_crop", {"border": 11}, ImageOps.crop(source, 11)),
        (
            "imagechops_offset",
            {"offset": [13, -17]},
            ImageChops.offset(source, 13, -17),
        ),
        ("imagechops_duplicate", {}, ImageChops.duplicate(source)),
        (
            "putpixel_index",
            {"point": [3, 4], "value": 7},
            putpixel,
        ),
        (
            "crop_then_putpixel_index",
            {"box": [9, 7, 61, 48], "point": [3, 4], "value": 7},
            crop_then_putpixel,
        ),
        (
            "putpixel_rgb",
            {"point": [3, 4], "color": [1, 2, 3, 255]},
            putpixel_rgb,
        ),
        (
            "transform_affine_nearest",
            {
                "size": [57, 43],
                "matrix": [1.0, 0.0, 7.0, 0.0, 1.0, 5.0],
            },
            source.transform(
                (57, 43),
                Image.Transform.AFFINE,
                (1.0, 0.0, 7.0, 0.0, 1.0, 5.0),
                Image.Resampling.NEAREST,
            ),
        ),
    ]


def main() -> None:
    if __version__ != EXPECTED_PILLOW:
        raise SystemExit(
            f"Pillow {EXPECTED_PILLOW} is required, found {__version__}"
        )

    OUTPUTS.mkdir(parents=True, exist_ok=True)
    rows: list[dict[str, object]] = []
    expected_outputs: set[Path] = set()
    with Image.open(INPUT) as opened:
        source = opened.copy()
        source.info = opened.info.copy()

    for operation, parameters, result in operation_rows(source):
        if result.mode != "P":
            raise RuntimeError(f"{operation} changed Pillow mode to {result.mode}")
        output = OUTPUTS / f"{operation}.bin"
        output.write_bytes(result.tobytes())
        encoded_output = OUTPUTS / f"{operation}.png"
        expected_outputs.update((output, encoded_output))
        encoded = BytesIO()
        result.save(encoded, format="PNG")
        encoded_output.write_bytes(encoded.getvalue())
        palette = result.getpalette()
        rows.append(
            {
                "id": operation,
                "input": "inputs/png_indexed_alpha.png",
                "pixels": f"outputs/operations/{operation}.bin",
                "encoded": f"outputs/operations/{operation}.png",
                "operation": operation,
                "parameters": parameters,
                "mode": result.mode,
                "width": result.width,
                "height": result.height,
                "palette_hex": bytes(palette).hex() if palette is not None else None,
                "palette_alpha_hex": transparency_hex(result),
            }
        )

    for stale_output in OUTPUTS.iterdir():
        if stale_output.is_file() and stale_output not in expected_outputs:
            stale_output.unlink()

    errors = [
        {
            "id": "putpixel_rgba_nonopaque",
            "input": "inputs/png_indexed_alpha.png",
            "operation": "putpixel_rgba",
            "parameters": {"point": [3, 4], "color": [1, 2, 3, 4]},
            "kind": "ValueError",
            "message": "cannot add non-opaque RGBA color to RGB palette",
        }
    ]

    MANIFEST.write_text(
        json.dumps(
            {
                "oracle": {"implementation": "Pillow", "version": __version__},
                "operations": rows,
                "errors": errors,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
