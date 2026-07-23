"""Focused Pillow 12.2 PA mutation regressions.

These byte expectations come from the pinned Pillow oracle. They exercise the
public binding normalization that the lower-level backend tests cannot see.
"""

import pytest

from pillow_rs import Image, ImageDraw


PALETTE = [
    255, 0, 0,
    0, 255, 0,
    0, 0, 255,
]


def pa_image():
    image = Image.frombytes("P", (3, 1), bytes([0, 1, 0]))
    image.putpalette(PALETTE, "RGB")
    image.putalpha(128)
    return image


def pa_canvas_5():
    image = Image.frombytes("P", (5, 5), bytes(25))
    image.putpalette(PALETTE, "RGB")
    image.putalpha(128)
    return image


def retained_palette(image):
    """Read Rust-owned palette state, bypassing the Python getter cache."""
    return list(image._rust_image.getpalette_trimmed())


@pytest.mark.covers("Image.paste")
@pytest.mark.parametrize(
    ("fill", "expected"),
    [
        (2, bytes.fromhex("008002000080")),
        ((2,), bytes.fromhex("008002000080")),
        ((2, 33), bytes.fromhex("008002210080")),
    ],
)
def test_pa_solid_paste_normalizes_index_and_alpha(fill, expected):
    image = pa_image()
    palette = retained_palette(image)

    image.paste(fill, (1, 0, 2, 1))

    assert image.mode == "PA"
    assert image.tobytes() == expected
    assert retained_palette(image) == palette


@pytest.mark.covers("ImageDraw.point")
def test_pa_draw_point_rejects_too_many_color_bands_exactly():
    image = pa_image()

    with pytest.raises(
        TypeError,
        match=r"^color must be int, or tuple of one or two elements$",
    ):
        ImageDraw.Draw(image).point((1, 0), fill=(2, 33, 44))


@pytest.mark.covers("ImageDraw.point")
@pytest.mark.parametrize(
    ("fill", "expected"),
    [
        (None, bytes.fromhex("0080ffff0080")),
        (2, bytes.fromhex("008002000080")),
        ((2,), bytes.fromhex("008002000080")),
        ((2, 33), bytes.fromhex("008002210080")),
    ],
)
def test_pa_draw_point_normalizes_index_and_alpha(fill, expected):
    image = pa_image()
    palette = retained_palette(image)

    ImageDraw.Draw(image).point((1, 0), fill=fill)

    assert image.mode == "PA"
    assert image.tobytes() == expected
    assert retained_palette(image) == palette


@pytest.mark.parametrize(
    "operation",
    [
        pytest.param("line", marks=pytest.mark.covers("ImageDraw.line")),
        pytest.param(
            "rectangle",
            marks=pytest.mark.covers("ImageDraw.rectangle"),
        ),
    ],
)
def test_pa_draw_default_ink_is_opaque_index_255(operation):
    image = pa_image()
    draw = ImageDraw.Draw(image)

    if operation == "line":
        draw.line([(1, 0), (1, 0)])
    else:
        draw.rectangle((1, 0, 1, 0))

    assert image.mode == "PA"
    assert image.tobytes() == bytes.fromhex("0080ffff0080")


ELLIPSE_DEFAULT = (
    "0080008000800080008000800080ffff008000800080ffff0080ffff00800080"
    "0080ffff0080008000800080008000800080"
)
POLYGON_DEFAULT = (
    "008000800080008000800080ffffffffffff00800080ffffffff008000800080"
    "0080ffff0080008000800080008000800080"
)
CHORD_DEFAULT = (
    "00800080008000800080008000800080008000800080ffffffffffff00800080"
    "0080ffff0080008000800080008000800080"
)


@pytest.mark.parametrize(
    ("operation", "args", "expected_hex"),
    [
        pytest.param(
            "ellipse",
            ((1, 1, 3, 3),),
            ELLIPSE_DEFAULT,
            marks=pytest.mark.covers("ImageDraw.ellipse"),
        ),
        pytest.param(
            "polygon",
            ([(1, 1), (3, 1), (2, 3)],),
            POLYGON_DEFAULT,
            marks=pytest.mark.covers("ImageDraw.polygon"),
        ),
        pytest.param(
            "chord",
            ((1, 1, 3, 3), 0, 180),
            CHORD_DEFAULT,
            marks=pytest.mark.covers("ImageDraw.chord"),
        ),
        pytest.param(
            "pieslice",
            ((1, 1, 3, 3), 0, 180),
            CHORD_DEFAULT,
            marks=pytest.mark.covers("ImageDraw.pieslice"),
        ),
        pytest.param(
            "rounded_rectangle",
            ((1, 1, 3, 3), 1),
            ELLIPSE_DEFAULT,
            marks=pytest.mark.covers("ImageDraw.rounded_rectangle"),
        ),
        pytest.param(
            "circle",
            ((2, 2), 1),
            ELLIPSE_DEFAULT,
            marks=pytest.mark.covers("ImageDraw.circle"),
        ),
    ],
)
def test_pa_shape_default_ink_matches_pillow(
    operation,
    args,
    expected_hex,
):
    image = pa_canvas_5()
    palette = retained_palette(image)

    getattr(ImageDraw.Draw(image), operation)(*args)

    assert image.mode == "PA"
    assert image.tobytes() == bytes.fromhex(expected_hex)
    assert retained_palette(image) == palette
