from io import BytesIO
from pathlib import Path
import sys

import pytest
from PIL import Image
from PIL import ImageDraw as PILImageDraw
from PIL import ImageFont as PILImageFont


FONT_ROOT = Path(__file__).resolve().parents[1] / "pillow-rs/tests/fixtures/font/input/fonts"
PILFONT_ROOT = Path(__file__).resolve().parents[1] / "pillow-rs/tests/fixtures/font/input/pilfont"
DEJAVU = FONT_ROOT / "DejaVuSans.ttf"
VARIABLE = FONT_ROOT / "variable-named-instances.ttf"
VARIABLE_MISSING_SUBFAMILY = FONT_ROOT / "variable-name-missing-subfamily.ttf"
COURB08 = PILFONT_ROOT / "courb08.pil"
NON_BEHAVIORAL_PILLOW_IMAGEFONT_NAMES = {
    "Any",
    "Axis",
    "BinaryIO",
    "BytesIO",
    "DeferredError",
    "IO",
    "Image",
    "ModuleType",
    "StrOrBytesPath",
    "TYPE_CHECKING",
    "TypedDict",
    "annotations",
    "base64",
    "cast",
    "core",
    "is_path",
    "sys",
}


def _font_observation(font):
    return {
        "name": font.getname(),
        "metrics": font.getmetrics(),
        "length": font.getlength("AV"),
        "bbox": font.getbbox("AV"),
    }


def _bitmap_font_observation(font):
    mask = font.getmask("ABC")
    return {
        "info": list(font.info),
        "bbox": font.getbbox("ABC"),
        "length": font.getlength("ABC"),
        "mask_mode": mask.mode,
        "mask_size": mask.size,
        "mask_bytes": bytes(mask),
    }


def _freetype_mask_observation(font, method, text, **kwargs):
    try:
        value = getattr(font, method)(text, **kwargs)
        if method == "getmask2":
            mask, offset = value
            return {
                "status": "ok",
                "mode": mask.mode,
                "size": mask.size,
                "bytes": bytes(mask),
                "offset": offset,
            }
        return {
            "status": "ok",
            "mode": value.mode,
            "size": value.size,
            "bytes": bytes(value),
        }
    except Exception as exc:
        return {
            "status": "err",
            "class": type(exc).__name__,
            "args": exc.args,
            "message": str(exc),
        }


def _freetype_value_observation(font, method, text, **kwargs):
    try:
        return {
            "status": "ok",
            "value": getattr(font, method)(text, **kwargs),
        }
    except Exception as exc:
        return {
            "status": "err",
            "class": type(exc).__name__,
            "args": exc.args,
            "message": str(exc),
        }


def _draw_text_observation(image_module, draw_module, font_module, method, **kwargs):
    image = image_module.new("RGB", (96, 48), (255, 255, 255))
    draw = draw_module.Draw(image)
    font = font_module.truetype(DEJAVU, 20)
    try:
        if method == "text":
            value = draw.text((0, 0), "AV", font=font, fill=(0, 0, 0), **kwargs)
        elif method == "multiline_text":
            value = draw.multiline_text((0, 0), "AV\njQ", font=font, fill=(0, 0, 0), **kwargs)
        elif method == "textbbox":
            value = draw.textbbox((0, 0), "AV", font=font, **kwargs)
        elif method == "textlength":
            value = draw.textlength("AV", font=font, **kwargs)
        elif method == "multiline_textbbox":
            value = draw.multiline_textbbox((0, 0), "AV\njQ", font=font, **kwargs)
        else:
            raise AssertionError(f"unsupported draw text method: {method}")
        return {
            "status": "ok",
            "value": value,
            "mode": image.mode,
            "size": image.size,
            "bytes": image.tobytes(),
        }
    except Exception as exc:
        return {
            "status": "err",
            "class": type(exc).__name__,
            "args": exc.args,
            "message": str(exc),
        }


def _native_font_observation(native_font):
    attrs = {
        name: getattr(native_font, name)
        for name in ("family", "style", "ascent", "descent", "height", "x_ppem", "y_ppem", "glyphs")
    }
    text_values = {
        text: {
            "getsize": native_font.getsize(text),
            "getlength": native_font.getlength(text),
        }
        for text in ("AV", "jQ", "")
    }
    return {
        "attrs": attrs,
        "text": text_values,
    }


def _native_render_observation(native_font, text, mode="", **kwargs):
    calls = []

    def fill(width, height):
        calls.append((width, height))
        return Image.core.fill("RGBA" if mode == "RGBA" else "L", (width, height))

    try:
        mask, offset = native_font.render(
            text,
            fill,
            mode,
            kwargs.get("direction"),
            kwargs.get("features"),
            kwargs.get("language"),
            kwargs.get("stroke_width", 0),
            kwargs.get("stroke_filled", False),
            kwargs.get("anchor"),
            kwargs.get("ink", 0),
            kwargs.get("start", (0, 0)),
        )
        return {
            "status": "ok",
            "calls": calls,
            "mode": mask.mode,
            "size": mask.size,
            "bytes": bytes(mask),
            "offset": offset,
        }
    except Exception as exc:
        return {
            "status": "err",
            "calls": calls,
            "class": type(exc).__name__,
            "args": exc.args,
            "message": str(exc),
        }


def _native_variation_observation(font_factory):
    font = font_factory()
    return {
        "names": font.font.getvarnames(),
        "axes": font.font.getvaraxes(),
    }


def _native_variation_after_setvarname(font_factory, instance_index):
    font = font_factory()
    try:
        font.font.setvarname(instance_index)
        return {
            "status": "ok",
            "name": font.getname(),
            "length": font.getlength("AV"),
        }
    except Exception as exc:
        return {
            "status": "err",
            "class": type(exc).__name__,
            "args": exc.args,
            "message": str(exc),
        }


def _native_variation_after_setvaraxes(font_factory, axes):
    font = font_factory()
    try:
        font.font.setvaraxes(axes)
        return {
            "status": "ok",
            "name": font.getname(),
            "length": font.getlength("AV"),
        }
    except Exception as exc:
        return {
            "status": "err",
            "class": type(exc).__name__,
            "args": exc.args,
            "message": str(exc),
        }


def _transposed_observation(font, text, method):
    try:
        value = getattr(font, method)(text)
        if method == "getmask":
            value = {
                "mode": value.mode,
                "size": value.size,
                "bytes": bytes(value),
            }
        return {"status": "ok", "value": value}
    except Exception as exc:
        return {
            "status": "err",
            "class": type(exc).__name__,
            "message": str(exc),
        }


def _transposed_pairs(RSPIL):
    return (
        (None, None),
        (Image.Transpose.FLIP_LEFT_RIGHT, RSPIL.Transpose.FLIP_LEFT_RIGHT),
        (Image.Transpose.FLIP_TOP_BOTTOM, RSPIL.Transpose.FLIP_TOP_BOTTOM),
        (Image.Transpose.ROTATE_90, RSPIL.Transpose.ROTATE_90),
        (Image.Transpose.ROTATE_180, RSPIL.Transpose.ROTATE_180),
        (Image.Transpose.ROTATE_270, RSPIL.Transpose.ROTATE_270),
    )


def _font_pairs_for_transpose(RSPIL):
    return (
        (
            RSPIL.ImageFont.truetype(DEJAVU, 20),
            PILImageFont.truetype(DEJAVU, 20),
        ),
        (
            RSPIL.ImageFont.load(COURB08),
            PILImageFont.load(COURB08),
        ),
    )


@pytest.mark.coverage_meta
def test_imagefont_public_surface_has_no_missing_behavioral_names(RSPIL):
    pil_names = {name for name in dir(PILImageFont) if not name.startswith("_")}
    rs_names = {name for name in dir(RSPIL.ImageFont) if not name.startswith("_")}
    missing = pil_names - rs_names

    assert "MAX_STRING_LENGTH" in rs_names
    assert missing <= NON_BEHAVIORAL_PILLOW_IMAGEFONT_NAMES


@pytest.mark.coverage_meta
def test_imagefont_freetype_native_font_subset_matches_pillow(RSPIL):
    for source in (DEJAVU, VARIABLE):
        assert _native_font_observation(RSPIL.ImageFont.truetype(source, 20).font) == (
            _native_font_observation(PILImageFont.truetype(source, 20).font)
        )


@pytest.mark.coverage_meta
def test_imagefont_freetype_native_render_subset_matches_pillow(RSPIL):
    rs_font = RSPIL.ImageFont.truetype(DEJAVU, 20).font
    pil_font = PILImageFont.truetype(DEJAVU, 20).font
    for text, kwargs in (
        ("AV", {}),
        ("AV", {"mode": "1"}),
        (" ", {}),
        ("AV", {"stroke_width": 1.5}),
        ("AV", {"start": (1.25, 2.5)}),
        ("AV", {"direction": "rtl"}),
        ("AV", {"anchor": "lx"}),
    ):
        assert _native_render_observation(rs_font, text, **kwargs) == (
            _native_render_observation(pil_font, text, **kwargs)
        )


@pytest.mark.coverage_meta
def test_imagefont_freetype_native_variation_subset_matches_pillow(RSPIL):
    rs_factory = lambda: RSPIL.ImageFont.truetype(VARIABLE, 20)
    pil_factory = lambda: PILImageFont.truetype(VARIABLE, 20)

    assert _native_variation_observation(rs_factory) == _native_variation_observation(pil_factory)
    for instance_index in (0, 1, 3, 13, -1):
        assert _native_variation_after_setvarname(rs_factory, instance_index) == (
            _native_variation_after_setvarname(pil_factory, instance_index)
        )
    assert _native_variation_after_setvaraxes(rs_factory, [100, 400]) == (
        _native_variation_after_setvaraxes(pil_factory, [100, 400])
    )

    rs_missing_factory = lambda: RSPIL.ImageFont.truetype(VARIABLE_MISSING_SUBFAMILY, 20)
    pil_missing_factory = lambda: PILImageFont.truetype(VARIABLE_MISSING_SUBFAMILY, 20)
    assert _native_variation_after_setvarname(rs_missing_factory, 5) == (
        _native_variation_after_setvarname(pil_missing_factory, 5)
    )


@pytest.mark.covers("ImageFont.MAX_STRING_LENGTH")
def test_imagefont_max_string_length_constant_matches_pillow(RSPIL):
    assert RSPIL.ImageFont.MAX_STRING_LENGTH == PILImageFont.MAX_STRING_LENGTH


@pytest.mark.covers("ImageFont.truetype")
def test_imagefont_layout_enum_and_no_raqm_fallback_match_pillow(RSPIL):
    assert int(RSPIL.ImageFont.Layout.BASIC) == int(PILImageFont.Layout.BASIC)
    assert int(RSPIL.ImageFont.Layout.RAQM) == int(PILImageFont.Layout.RAQM)

    with pytest.warns(UserWarning, match="Raqm layout was requested"):
        rs_font = RSPIL.ImageFont.truetype(DEJAVU, 20, layout_engine=RSPIL.ImageFont.Layout.RAQM)
    with pytest.warns(UserWarning, match="Raqm layout was requested"):
        pil_font = PILImageFont.truetype(DEJAVU, 20, layout_engine=PILImageFont.Layout.RAQM)

    assert int(rs_font.layout_engine) == int(pil_font.layout_engine) == int(PILImageFont.Layout.BASIC)
    assert _font_observation(rs_font) == _font_observation(pil_font)


@pytest.mark.covers("ImageFont.load")
def test_imagefont_load_bitmap_font_matches_pillow(RSPIL):
    for source in (str(COURB08), COURB08):
        assert _bitmap_font_observation(RSPIL.ImageFont.load(source)) == (
            _bitmap_font_observation(PILImageFont.load(source))
        )


@pytest.mark.covers("ImageFont.load_path")
def test_imagefont_load_path_bitmap_font_matches_pillow(RSPIL):
    sys.path.insert(0, str(PILFONT_ROOT))
    try:
        for source in ("courb08.pil", b"courb08.pil"):
            assert _bitmap_font_observation(RSPIL.ImageFont.load_path(source)) == (
                _bitmap_font_observation(PILImageFont.load_path(source))
            )
    finally:
        sys.path.remove(str(PILFONT_ROOT))


@pytest.mark.covers("ImageFont.load_default_imagefont")
def test_imagefont_load_default_imagefont_matches_pillow(RSPIL):
    assert _bitmap_font_observation(RSPIL.ImageFont.load_default_imagefont()) == (
        _bitmap_font_observation(PILImageFont.load_default_imagefont())
    )


@pytest.mark.covers("ImageFont.load_default")
def test_imagefont_load_default_freetype_matches_pillow(RSPIL):
    for size in (None, 12):
        assert _font_observation(RSPIL.ImageFont.load_default(size)) == (
            _font_observation(PILImageFont.load_default(size))
        )


@pytest.mark.covers("ImageFont.truetype")
def test_imagefont_truetype_pathlike_and_bytes_path_match_pillow(RSPIL):
    for source in (DEJAVU, bytes(str(DEJAVU), "utf-8")):
        assert _font_observation(RSPIL.ImageFont.truetype(source, 20)) == _font_observation(
            PILImageFont.truetype(source, 20)
        )


@pytest.mark.covers("ImageFont.FreeTypeFont.get_variation_names")
def test_imagefont_variation_names_match_pillow_for_file_like_font(RSPIL):
    data = VARIABLE.read_bytes()
    assert RSPIL.ImageFont.truetype(BytesIO(data), 20).get_variation_names() == (
        PILImageFont.truetype(BytesIO(data), 20).get_variation_names()
    )


@pytest.mark.covers("ImageFont.FreeTypeFont.get_variation_axes")
def test_imagefont_variation_axes_match_pillow_for_file_like_font(RSPIL):
    data = VARIABLE.read_bytes()
    assert RSPIL.ImageFont.truetype(BytesIO(data), 20).get_variation_axes() == (
        PILImageFont.truetype(BytesIO(data), 20).get_variation_axes()
    )


@pytest.mark.covers("ImageFont.FreeTypeFont.set_variation_by_name")
def test_imagefont_set_variation_by_name_matches_pillow_for_file_like_font(RSPIL):
    data = VARIABLE.read_bytes()
    rs_font = RSPIL.ImageFont.truetype(BytesIO(data), 20)
    pil_font = PILImageFont.truetype(BytesIO(data), 20)
    name = pil_font.get_variation_names()[0]

    rs_font.set_variation_by_name(name)
    pil_font.set_variation_by_name(name)

    assert _font_observation(rs_font) == _font_observation(pil_font)


@pytest.mark.covers("ImageFont.FreeTypeFont.set_variation_by_axes")
def test_imagefont_set_variation_by_axes_matches_pillow_for_file_like_font(RSPIL):
    data = VARIABLE.read_bytes()
    rs_font = RSPIL.ImageFont.truetype(BytesIO(data), 20)
    pil_font = PILImageFont.truetype(BytesIO(data), 20)
    axes = [axis["default"] for axis in pil_font.get_variation_axes()]

    rs_font.set_variation_by_axes(axes)
    pil_font.set_variation_by_axes(axes)

    assert _font_observation(rs_font) == _font_observation(pil_font)


@pytest.mark.covers("ImageFont.FreeTypeFont.font_variant")
def test_imagefont_file_like_font_variant_matches_pillow(RSPIL):
    data = VARIABLE.read_bytes()
    rs_font = RSPIL.ImageFont.truetype(BytesIO(data), 20).font_variant(size=21)
    pil_font = PILImageFont.truetype(BytesIO(data), 20).font_variant(size=21)

    assert _font_observation(rs_font) == _font_observation(pil_font)


@pytest.mark.covers("ImageFont.FreeTypeFont.getmetrics")
def test_imagefont_freetype_getmetrics_matches_pillow(RSPIL):
    for rs_font, pil_font in (
        (RSPIL.ImageFont.truetype(DEJAVU, 20), PILImageFont.truetype(DEJAVU, 20)),
        (RSPIL.ImageFont.load_default(12), PILImageFont.load_default(12)),
    ):
        assert rs_font.getmetrics() == pil_font.getmetrics()


@pytest.mark.covers("ImageFont.FreeTypeFont.getname")
def test_imagefont_freetype_getname_matches_pillow(RSPIL):
    for source in (DEJAVU, VARIABLE):
        assert RSPIL.ImageFont.truetype(source, 20).getname() == (
            PILImageFont.truetype(source, 20).getname()
        )


@pytest.mark.covers("ImageFont.FreeTypeFont.getbbox")
def test_imagefont_freetype_getbbox_values_and_errors_match_pillow(RSPIL):
    rs_font = RSPIL.ImageFont.truetype(DEJAVU, 20)
    pil_font = PILImageFont.truetype(DEJAVU, 20)
    for text, kwargs in (
        ("AV", {}),
        ("jQ", {"stroke_width": 1.5}),
        ("AV", {"mode": "1"}),
        ("AV", {"anchor": "mm"}),
        ("AV", {"anchor": "bad"}),
        ("AV", {"direction": "rtl"}),
        ("AV", {"features": []}),
        ("AV", {"language": "en"}),
    ):
        assert _freetype_value_observation(rs_font, "getbbox", text, **kwargs) == (
            _freetype_value_observation(pil_font, "getbbox", text, **kwargs)
        )


@pytest.mark.covers("ImageFont.FreeTypeFont.getlength")
def test_imagefont_freetype_getlength_values_and_errors_match_pillow(RSPIL):
    rs_font = RSPIL.ImageFont.truetype(DEJAVU, 20)
    pil_font = PILImageFont.truetype(DEJAVU, 20)
    for text, kwargs in (
        ("AV", {}),
        ("AV", {"mode": "1"}),
        ("AV", {"direction": "rtl"}),
        ("AV", {"features": []}),
        ("AV", {"features": ["-kern"]}),
        ("AV", {"language": "en"}),
    ):
        assert _freetype_value_observation(rs_font, "getlength", text, **kwargs) == (
            _freetype_value_observation(pil_font, "getlength", text, **kwargs)
        )


@pytest.mark.covers("ImageFont.FreeTypeFont.getmask")
def test_imagefont_freetype_getmask_bytes_and_errors_match_pillow(RSPIL):
    rs_font = RSPIL.ImageFont.truetype(DEJAVU, 20)
    pil_font = PILImageFont.truetype(DEJAVU, 20)
    for text, kwargs in (
        ("AV", {}),
        ("AV", {"mode": "1"}),
        ("jQ", {"stroke_width": 1.5}),
        ("AV", {"mode": "RGBA"}),
        ("AV", {"direction": "rtl"}),
    ):
        assert _freetype_mask_observation(rs_font, "getmask", text, **kwargs) == (
            _freetype_mask_observation(pil_font, "getmask", text, **kwargs)
        )


@pytest.mark.covers("ImageFont.FreeTypeFont.getmask2")
def test_imagefont_freetype_getmask2_bytes_offsets_and_errors_match_pillow(RSPIL):
    rs_font = RSPIL.ImageFont.truetype(DEJAVU, 20)
    pil_font = PILImageFont.truetype(DEJAVU, 20)
    for text, kwargs in (
        ("AV", {}),
        ("AV", {"mode": "1"}),
        ("jQ", {"stroke_width": 1.5}),
        ("AV", {"start": (0.5, 0.25)}),
        ("AV", {"mode": "RGBA"}),
        ("AV", {"direction": "rtl"}),
    ):
        assert _freetype_mask_observation(rs_font, "getmask2", text, **kwargs) == (
            _freetype_mask_observation(pil_font, "getmask2", text, **kwargs)
        )


@pytest.mark.covers("ImageDraw.text")
@pytest.mark.covers("ImageDraw.multiline_text")
@pytest.mark.covers("ImageDraw.textbbox")
@pytest.mark.covers("ImageDraw.textlength")
@pytest.mark.covers("ImageDraw.multiline_textbbox")
def test_imagedraw_no_libraqm_text_options_match_pillow(RSPIL):
    for method in ("text", "multiline_text", "textbbox", "textlength", "multiline_textbbox"):
        assert _draw_text_observation(
            RSPIL.Image,
            RSPIL.ImageDraw,
            RSPIL.ImageFont,
            method,
            direction="rtl",
        ) == _draw_text_observation(
            Image,
            PILImageDraw,
            PILImageFont,
            method,
            direction="rtl",
        )


@pytest.mark.covers("ImageFont.TransposedFont.getbbox")
def test_imagefont_transposed_getbbox_matches_pillow(RSPIL):
    for rs_base, pil_base in _font_pairs_for_transpose(RSPIL):
        for pil_orientation, rs_orientation in _transposed_pairs(RSPIL):
            assert _transposed_observation(
                RSPIL.ImageFont.TransposedFont(rs_base, rs_orientation), "ABC", "getbbox"
            ) == _transposed_observation(
                PILImageFont.TransposedFont(pil_base, pil_orientation), "ABC", "getbbox"
            )


@pytest.mark.covers("ImageFont.TransposedFont.getlength")
def test_imagefont_transposed_getlength_matches_pillow(RSPIL):
    for rs_base, pil_base in _font_pairs_for_transpose(RSPIL):
        for pil_orientation, rs_orientation in _transposed_pairs(RSPIL):
            assert _transposed_observation(
                RSPIL.ImageFont.TransposedFont(rs_base, rs_orientation), "ABC", "getlength"
            ) == _transposed_observation(
                PILImageFont.TransposedFont(pil_base, pil_orientation), "ABC", "getlength"
            )


@pytest.mark.covers("ImageFont.TransposedFont.getmask")
def test_imagefont_transposed_getmask_matches_pillow(RSPIL):
    for rs_base, pil_base in _font_pairs_for_transpose(RSPIL):
        for pil_orientation, rs_orientation in _transposed_pairs(RSPIL):
            assert _transposed_observation(
                RSPIL.ImageFont.TransposedFont(rs_base, rs_orientation), "ABC", "getmask"
            ) == _transposed_observation(
                PILImageFont.TransposedFont(pil_base, pil_orientation), "ABC", "getmask"
            )
