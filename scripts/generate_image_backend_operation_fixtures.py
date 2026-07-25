#!/usr/bin/env python3
"""Generate exact Pillow references for backend and palette operations.

Run with the Pillow version pinned by the ``pillow-rs-py`` ``dev`` extra in
``pillow-rs-py/pyproject.toml`` (the root editable-install workflow). The script
consumes the checked-in indexed PNG fixture, creates deterministic additional
oracle inputs, and writes exact pixel/metadata expectations.
"""

from __future__ import annotations

from io import BytesIO
import json
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont, ImageOps, __version__


ROOT = Path(__file__).resolve().parents[1] / "pillow-rs/tests/fixtures/image_backend"
INPUT = ROOT / "inputs/png_indexed_alpha.png"
TABLE_INPUT = ROOT / "inputs/png_indexed_alpha_table.png"
OUTPUTS = ROOT / "outputs/operations"
MANIFEST = ROOT / "operations.json"
BACKEND_PARITY_MANIFEST = ROOT / "backend_parity.json"
EXPECTED_PILLOW = "12.2.0"
EXPECTED_FREETYPE = "2.14.3"


def transparency_hex(image: Image.Image) -> str | None:
    transparency = image.info.get("transparency")
    if transparency is None:
        return None
    if isinstance(transparency, int):
        return bytes([transparency]).hex()
    return bytes(transparency).hex()


def transparency_info_descriptor(image: Image.Image) -> dict[str, object]:
    """Serialize Pillow's typed P-mode transparency metadata losslessly."""
    transparency = image.info.get("transparency")
    if transparency is None:
        return {}
    if isinstance(transparency, int):
        return {"transparency": {"kind": "index", "value": transparency}}
    return {
        "transparency": {
            "kind": "table",
            "value_hex": bytes(transparency).hex(),
        }
    }


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


def image_spec(image: Image.Image) -> dict[str, object]:
    """Serialize an uncompressed Pillow image for backend-execution tests."""
    palette = image.getpalette()
    return {
        "mode": image.mode,
        "size": list(image.size),
        "pixels_hex": image.tobytes().hex(),
        "palette_hex": bytes(palette).hex() if palette is not None else None,
    }


def patterned_image(mode: str, size: tuple[int, int], seed: int) -> Image.Image:
    bands = Image.getmodebands(mode)
    length = size[0] * size[1] * bands
    pixels = bytes((seed + index * 37) % 256 for index in range(length))
    return Image.frombytes(mode, size, pixels)


def paste_source_spec(
    source: Image.Image | int | tuple[int, ...],
) -> dict[str, object]:
    if isinstance(source, Image.Image):
        return {
            "kind": "image",
            "image": image_spec(source),
        }
    if isinstance(source, tuple):
        return {"kind": "tuple", "value": list(source)}
    return {"kind": "scalar", "value": source}


def paste_case(
    case_id: str,
    destination: Image.Image,
    source: Image.Image | int | tuple[int, ...],
    box: tuple[int, ...],
    mask: Image.Image | None = None,
) -> dict[str, object]:
    result = destination.copy()
    result.paste(source, box, mask)
    return {
        "id": case_id,
        "destination": image_spec(destination),
        "source": paste_source_spec(source),
        "box": list(box),
        "mask": image_spec(mask) if mask is not None else None,
        "expected": image_spec(result),
        "backends": ["cpu", "simd", "gpu"],
    }


def paste_error_case(
    case_id: str,
    destination: Image.Image,
    source: Image.Image | int | tuple[int, ...],
    box: tuple[int, ...] | None,
    mask: Image.Image | None = None,
) -> dict[str, object]:
    result = destination.copy()
    try:
        result.paste(source, box, mask)
    except Exception as error:
        expected_error = {
            "type": type(error).__name__,
            "message": str(error),
        }
    else:
        raise AssertionError(f"{case_id} must fail in Pillow")
    return {
        "id": case_id,
        "destination": image_spec(destination),
        "source": paste_source_spec(source),
        "box": list(box) if box is not None else None,
        "mask": image_spec(mask) if mask is not None else None,
        "expected_error": expected_error,
    }


def draw_case(
    case_id: str,
    source: Image.Image,
    operation: str,
    parameters: dict[str, object],
) -> dict[str, object]:
    result = source.copy()
    draw = ImageDraw.Draw(result)
    oracle_parameters = parameters.copy()
    for name in ("fill", "outline"):
        value = oracle_parameters.get(name)
        if isinstance(value, list):
            oracle_parameters[name] = tuple(value)
    getattr(draw, operation)(**oracle_parameters)
    return {
        "id": case_id,
        "source": image_spec(source),
        "operation": operation,
        "parameters": parameters,
        "expected": image_spec(result),
        "backends": ["cpu"],
        "unsupported_backends": ["simd", "gpu"],
    }


def write_table_transparency_input() -> None:
    """Create a P PNG whose tRNS chunk cannot collapse to one index."""
    image = Image.frombytes("P", (4, 2), bytes([0, 1, 2, 3, 3, 2, 1, 0]))
    palette = [
        channel
        for index in range(256)
        for channel in (
            (index * 17 + 3) % 256,
            (index * 29 + 5) % 256,
            (index * 43 + 7) % 256,
        )
    ]
    image.putpalette(palette)
    image.save(TABLE_INPUT, format="PNG", transparency=bytes([0, 64, 128, 255]))


def apply_transparency_case(
    case_id: str,
    input_path: Path,
    *,
    prepare_alpha: int | None = None,
) -> dict[str, object]:
    with Image.open(input_path) as opened:
        indexed = opened.copy()
        indexed.info = opened.info.copy()
    if prepare_alpha is not None:
        indexed.putalpha(prepare_alpha)
    before_info = transparency_info_descriptor(indexed)
    before_palette_mode = indexed.palette.mode if indexed.palette is not None else None
    before_has_transparency = indexed.has_transparency_data
    indexed.apply_transparency()
    rgba_palette = indexed.getpalette("RGBA")
    assert rgba_palette is not None
    return {
        "id": case_id,
        "input": input_path.relative_to(ROOT).as_posix(),
        "prepare_alpha": prepare_alpha,
        "backends": ["cpu", "simd", "gpu"] if prepare_alpha is not None else ["cpu"],
        "expected": {
            **image_spec(indexed),
            "palette_rgba_hex": bytes(rgba_palette).hex(),
            "before_info": before_info,
            "before_palette_mode": before_palette_mode,
            "before_has_transparency_data": before_has_transparency,
            "info": transparency_info_descriptor(indexed),
            "palette_mode": indexed.palette.mode if indexed.palette is not None else None,
            "has_transparency_data": indexed.has_transparency_data,
        },
    }


def indexed_immediate_draw_case(
    case_id: str,
    operation: str,
    parameters: dict[str, object],
) -> dict[str, object]:
    with Image.open(TABLE_INPUT) as target:
        target.load()
        if operation == "bitmap":
            bitmap = Image.frombytes("L", (1, 1), bytes([255]))
            ImageDraw.Draw(target).bitmap(
                tuple(parameters["xy"]),
                bitmap,
                fill=parameters["fill"],
            )
        elif operation == "text":
            font = ImageFont.load_default(size=parameters["font_size"])
            ImageDraw.Draw(target).text(
                tuple(parameters["xy"]),
                parameters["text"],
                font=font,
                fill=parameters["fill"],
            )
        else:
            raise AssertionError(f"unsupported indexed draw operation {operation}")
        rgba_palette = target.getpalette("RGBA")
        assert rgba_palette is not None
        expected = {
            **image_spec(target),
            "format": target.format,
            "info": transparency_info_descriptor(target),
            "palette_mode": target.palette.mode if target.palette is not None else None,
            "has_transparency_data": target.has_transparency_data,
            "palette_rgba_hex": bytes(rgba_palette).hex(),
        }
    return {
        "id": case_id,
        "input": TABLE_INPUT.relative_to(ROOT).as_posix(),
        "operation": operation,
        "parameters": parameters,
        "expected": expected,
    }


def coverage_row(
    operation: str,
    case: dict[str, object],
) -> dict[str, object]:
    """Bind one exact oracle image to the semantic operation it covers."""
    expected = case["expected"]
    assert isinstance(expected, dict)
    return {
        "operation": operation,
        "mode": expected["mode"],
        "case_id": case["id"],
        "expected": {
            key: expected.get(key)
            for key in ("mode", "size", "pixels_hex", "palette_hex")
        },
    }


def backend_parity_manifest() -> dict[str, object]:
    destination = patterned_image("RGBA", (4, 3), 0)
    source = patterned_image("RGBA", (3, 2), 200)
    mask_l = Image.frombytes("L", (3, 2), bytes([0, 1, 127, 128, 254, 255]))
    mask_la = Image.frombytes(
        "LA",
        (3, 2),
        bytes([9, 0, 9, 1, 9, 127, 9, 128, 9, 254, 9, 255]),
    )
    mask_rgba = Image.frombytes(
        "RGBA",
        (3, 2),
        bytes(
            channel
            for alpha in [0, 1, 127, 128, 254, 255]
            for channel in [9, 8, 7, alpha]
        ),
    )
    p_destination = Image.frombytes("P", (4, 3), bytes(range(12)))
    p_source = Image.frombytes("P", (2, 2), bytes([7, 8, 9, 10]))
    palette = bytes(
        channel
        for index in range(256)
        for channel in (index, index * 3 % 256, index * 7 % 256)
    )
    pa_destination = Image.frombytes(
        "PA",
        (4, 3),
        bytes(
            channel
            for index, alpha in zip(range(12), range(17, 221, 17), strict=True)
            for channel in (index, alpha)
        ),
    )
    pa_destination.putpalette(palette)
    pa_source = Image.frombytes("PA", (2, 2), bytes([7, 31, 8, 63, 9, 127, 10, 255]))
    pa_source.putpalette(palette)

    paste_cases = [
        paste_case("rgba_position", destination, source, (1, 1)),
        paste_case("rgba_negative", destination, source, (-1, -1)),
        paste_case("rgba_mask_l", destination, source, (1, 0), mask_l),
        paste_case("rgba_mask_la", destination, source, (1, 0), mask_la),
        paste_case("rgba_mask_rgba", destination, source, (1, 0), mask_rgba),
        paste_case(
            "one_bit_copy",
            patterned_image("1", (9, 4), 3),
            patterned_image("1", (5, 2), 197),
            (2, 1),
        ),
        paste_case(
            "l_mask_la",
            patterned_image("L", (4, 3), 7),
            patterned_image("L", (3, 2), 117),
            (1, 0),
            mask_la,
        ),
        paste_case(
            "la_mask_rgba",
            patterned_image("LA", (4, 3), 11),
            patterned_image("LA", (3, 2), 121),
            (1, 0),
            mask_rgba,
        ),
        paste_case(
            "cmyk_copy",
            patterned_image("CMYK", (4, 3), 15),
            patterned_image("CMYK", (3, 2), 125),
            (1, 1),
        ),
        paste_case(
            "rgb_mode_conversion",
            patterned_image("RGB", (4, 3), 17),
            patterned_image("L", (2, 2), 93),
            (1, 1, 3, 3),
        ),
        paste_case(
            "rgb_keeps_rgba_channels",
            patterned_image("RGB", (4, 3), 19),
            patterned_image("RGBA", (2, 2), 97),
            (1, 1),
        ),
        paste_case(
            "rgba_mode_conversion",
            patterned_image("RGBA", (4, 3), 23),
            patterned_image("L", (2, 2), 101),
            (1, 1),
        ),
        paste_case("p_index_copy", p_destination, p_source, (1, 1)),
        paste_case("pa_index_alpha_copy", pa_destination, pa_source, (1, 1)),
        paste_case(
            "p_index_mask",
            p_destination,
            p_source,
            (1, 1),
            Image.frombytes("L", (2, 2), bytes([0, 127, 128, 255])),
        ),
        paste_case(
            "source_larger_than_destination",
            patterned_image("RGBA", (2, 2), 29),
            patterned_image("RGBA", (4, 4), 109),
            (-1, -1),
        ),
        paste_case(
            "fully_clipped",
            patterned_image("RGBA", (4, 3), 33),
            patterned_image("RGBA", (2, 2), 113),
            (-5, -4),
        ),
        paste_case(
            "rgb_scalar_fill",
            patterned_image("RGB", (4, 3), 31),
            7,
            (1, 1, 3, 3),
        ),
        paste_case(
            "rgba_tuple_fill",
            patterned_image("RGBA", (4, 3), 41),
            (7, 8, 9, 10),
            (1, 0, 3, 2),
        ),
        paste_case(
            "rgb_mask_sized_fill",
            patterned_image("RGB", (4, 3), 51),
            (7, 8, 9),
            (1, 0),
            Image.frombytes("L", (2, 2), bytes([0, 127, 128, 255])),
        ),
        paste_case(
            "la_tuple_fill",
            patterned_image("LA", (4, 3), 55),
            (7, 123),
            (1, 0, 3, 2),
        ),
    ]
    paste_error_cases = [
        paste_error_case(
            "solid_without_sized_region",
            patterned_image("RGB", (4, 3), 61),
            7,
            None,
        ),
        paste_error_case(
            "source_region_mismatch",
            patterned_image("RGB", (4, 3), 71),
            patterned_image("RGB", (2, 2), 81),
            (0, 0, 1, 1),
        ),
        paste_error_case(
            "mask_mode_invalid",
            patterned_image("RGB", (4, 3), 91),
            patterned_image("RGB", (2, 2), 101),
            (0, 0),
            patterned_image("RGB", (2, 2), 111),
        ),
        paste_error_case(
            "mask_size_mismatch",
            patterned_image("RGB", (4, 3), 121),
            patterned_image("RGB", (2, 2), 131),
            (0, 0),
            patterned_image("L", (1, 1), 141),
        ),
        paste_error_case(
            "rgb_two_element_color",
            patterned_image("RGB", (4, 3), 151),
            (1, 2),
            (0, 0, 1, 1),
        ),
        paste_error_case(
            "l_three_element_color",
            patterned_image("L", (4, 3), 161),
            (1, 2, 3),
            (0, 0, 1, 1),
        ),
    ]

    rgb = patterned_image("RGB", (32, 24), 13)
    draw_cases = [
        draw_case(
            "line",
            rgb,
            "line",
            {"xy": [2, 3, 27, 18], "fill": [200, 30, 90], "width": 1},
        ),
        draw_case(
            "wide_line",
            rgb,
            "line",
            {"xy": [2, 18, 27, 3], "fill": [20, 210, 70], "width": 5},
        ),
        draw_case(
            "rectangle",
            rgb,
            "rectangle",
            {
                "xy": [3, 4, 27, 19],
                "fill": [80, 120, 160],
                "outline": [230, 20, 40],
                "width": 3,
            },
        ),
        draw_case(
            "ellipse",
            rgb,
            "ellipse",
            {
                "xy": [3, 3, 28, 20],
                "fill": [80, 120, 160],
                "outline": [230, 20, 40],
                "width": 2,
            },
        ),
        draw_case(
            "polygon",
            rgb,
            "polygon",
            {
                "xy": [[3, 19], [8, 3], [25, 5], [29, 18], [14, 21]],
                "fill": [80, 120, 160],
                "outline": [230, 20, 40],
                "width": 1,
            },
        ),
        draw_case(
            "polygon_wide_inward",
            rgb,
            "polygon",
            {
                "xy": [[3, 19], [8, 3], [25, 5], [29, 18], [14, 21]],
                "fill": [80, 120, 160],
                "outline": [230, 20, 40],
                "width": 4,
            },
        ),
        draw_case(
            "point",
            rgb,
            "point",
            {"xy": [[0, 0], [7, 9], [31, 23]], "fill": [230, 20, 40]},
        ),
        draw_case(
            "arc",
            rgb,
            "arc",
            {
                "xy": [3, 3, 28, 20],
                "start": 25.0,
                "end": 275.0,
                "fill": [230, 20, 40],
                "width": 2,
            },
        ),
        draw_case(
            "arc_wraparound",
            rgb,
            "arc",
            {
                "xy": [3, 3, 28, 20],
                "start": 300.0,
                "end": 60.0,
                "fill": [230, 20, 40],
                "width": 3,
            },
        ),
        draw_case(
            "chord",
            rgb,
            "chord",
            {
                "xy": [3, 3, 28, 20],
                "start": 25.0,
                "end": 275.0,
                "fill": [80, 120, 160],
                "outline": [230, 20, 40],
                "width": 2,
            },
        ),
        draw_case(
            "pieslice",
            rgb,
            "pieslice",
            {
                "xy": [3, 3, 28, 20],
                "start": 25.0,
                "end": 275.0,
                "fill": [80, 120, 160],
                "outline": [230, 20, 40],
                "width": 2,
            },
        ),
        draw_case(
            "circle",
            rgb,
            "circle",
            {
                "xy": [16, 12],
                "radius": 8.0,
                "fill": [80, 120, 160],
                "outline": [230, 20, 40],
                "width": 2,
            },
        ),
        draw_case(
            "rounded_rectangle",
            rgb,
            "rounded_rectangle",
            {
                "xy": [3, 3, 28, 20],
                "radius": 5.0,
                "fill": [80, 120, 160],
                "outline": [230, 20, 40],
                "width": 2,
            },
        ),
        draw_case(
            "rectangle_l",
            patterned_image("L", (16, 12), 11),
            "rectangle",
            {"xy": [2, 2, 13, 9], "fill": 77, "outline": 201, "width": 2},
        ),
        draw_case(
            "rectangle_la",
            patterned_image("LA", (16, 12), 21),
            "rectangle",
            {
                "xy": [2, 2, 13, 9],
                "fill": [77, 123],
                "outline": [201, 45],
                "width": 2,
            },
        ),
        draw_case(
            "rectangle_rgba",
            patterned_image("RGBA", (16, 12), 31),
            "rectangle",
            {
                "xy": [2, 2, 13, 9],
                "fill": [77, 88, 99, 123],
                "outline": [201, 11, 22, 45],
                "width": 2,
            },
        ),
        draw_case(
            "rectangle_p",
            patterned_image("P", (16, 12), 41),
            "rectangle",
            {"xy": [2, 2, 13, 9], "fill": 7, "outline": 9, "width": 2},
        ),
    ]
    pa = Image.frombytes(
        "PA",
        (5, 5),
        bytes(channel for _ in range(25) for channel in (0, 128)),
    )
    pa.putpalette(palette)
    pa_fill = [2, 33]
    pa_outline = [3, 44]
    draw_cases.extend(
        [
            draw_case(
                "pa_line",
                pa,
                "line",
                {"xy": [0, 0, 4, 4], "fill": pa_fill, "width": 1},
            ),
            draw_case(
                "pa_rectangle",
                pa,
                "rectangle",
                {
                    "xy": [1, 1, 3, 3],
                    "fill": pa_fill,
                    "outline": pa_outline,
                    "width": 1,
                },
            ),
            draw_case(
                "pa_rectangle_default_outline",
                pa,
                "rectangle",
                {"xy": [1, 1, 3, 3], "width": 1},
            ),
            draw_case(
                "pa_ellipse",
                pa,
                "ellipse",
                {
                    "xy": [1, 1, 3, 3],
                    "fill": pa_fill,
                    "outline": pa_outline,
                    "width": 1,
                },
            ),
            draw_case(
                "pa_polygon",
                pa,
                "polygon",
                {
                    "xy": [[0, 4], [2, 0], [4, 4]],
                    "fill": pa_fill,
                    "outline": pa_outline,
                    "width": 1,
                },
            ),
            draw_case(
                "pa_point",
                pa,
                "point",
                {"xy": [[1, 2], [3, 4]], "fill": pa_fill},
            ),
            draw_case(
                "pa_arc",
                pa,
                "arc",
                {
                    "xy": [0, 0, 4, 4],
                    "start": 0.0,
                    "end": 180.0,
                    "fill": pa_fill,
                    "width": 1,
                },
            ),
            draw_case(
                "pa_chord",
                pa,
                "chord",
                {
                    "xy": [0, 0, 4, 4],
                    "start": 0.0,
                    "end": 180.0,
                    "fill": pa_fill,
                    "outline": pa_outline,
                    "width": 1,
                },
            ),
            draw_case(
                "pa_pieslice",
                pa,
                "pieslice",
                {
                    "xy": [0, 0, 4, 4],
                    "start": 0.0,
                    "end": 180.0,
                    "fill": pa_fill,
                    "outline": pa_outline,
                    "width": 1,
                },
            ),
            draw_case(
                "pa_circle",
                pa,
                "circle",
                {
                    "xy": [2, 2],
                    "radius": 2.0,
                    "fill": pa_fill,
                    "outline": pa_outline,
                    "width": 1,
                },
            ),
            draw_case(
                "pa_rounded_rectangle",
                pa,
                "rounded_rectangle",
                {
                    "xy": [0, 0, 4, 4],
                    "radius": 1.0,
                    "fill": pa_fill,
                    "outline": pa_outline,
                    "width": 1,
                },
            ),
        ]
    )

    apply_cases = [
        apply_transparency_case("indexed_png_single_index", INPUT),
        apply_transparency_case("indexed_png_alpha_table", TABLE_INPUT),
        apply_transparency_case(
            "indexed_png_alpha_table_after_pa_promotion",
            TABLE_INPUT,
            prepare_alpha=128,
        ),
    ]
    indexed_immediate_draw_cases = [
        indexed_immediate_draw_case(
            "indexed_bitmap",
            "bitmap",
            {"xy": [1, 0], "fill": 3},
        ),
        indexed_immediate_draw_case(
            "indexed_text",
            "text",
            {"xy": [0, -2], "text": "A", "font_size": 10.0, "fill": 3},
        ),
    ]
    coverage = [
        *(coverage_row("Image.paste", case) for case in paste_cases),
        *(
            coverage_row(f"ImageDraw.{case['operation']}", case)
            for case in draw_cases
        ),
        *(
            coverage_row("Image.apply_transparency", case)
            for case in apply_cases
        ),
        *(
            coverage_row(f"ImageDraw.{case['operation']}", case)
            for case in indexed_immediate_draw_cases
        ),
    ]
    return {
        "oracle": {
            "implementation": "Pillow",
            "version": __version__,
            "freetype_version": ImageFont.core.freetype2_version,
        },
        "coverage": coverage,
        "paste_cases": paste_cases,
        "paste_error_cases": paste_error_cases,
        "draw_cases": draw_cases,
        "apply_transparency_cases": apply_cases,
        "indexed_immediate_draw_cases": indexed_immediate_draw_cases,
    }


def main() -> None:
    if __version__ != EXPECTED_PILLOW:
        raise SystemExit(
            f"Pillow {EXPECTED_PILLOW} is required, found {__version__}"
        )
    if ImageFont.core.freetype2_version != EXPECTED_FREETYPE:
        raise SystemExit(
            f"FreeType {EXPECTED_FREETYPE} is required, "
            f"found {ImageFont.core.freetype2_version}"
        )

    write_table_transparency_input()
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
    BACKEND_PARITY_MANIFEST.write_text(
        json.dumps(backend_parity_manifest(), indent=2) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
