"""Pillow's text-layout value object for ImageDraw."""

from __future__ import annotations

import math
from typing import AnyStr, Generic, NamedTuple

from . import imagefont as ImageFont
from . import _core

Font = ImageFont.ImageFont | ImageFont.FreeTypeFont | ImageFont.TransposedFont
_Ink = float | tuple[int, ...] | str

_BASE_FREETYPE_GETBBOX = ImageFont.FreeTypeFont.getbbox
_BASE_FREETYPE_GETLENGTH = ImageFont.FreeTypeFont.getlength
_BASE_FREETYPE_GETMASK = ImageFont.FreeTypeFont.getmask
_BASE_FREETYPE_GETMASK2 = ImageFont.FreeTypeFont.getmask2
_BASE_FREETYPE_FONT_VARIANT = ImageFont.FreeTypeFont.font_variant


def _wrap_limit_as_float(value):
    try:
        return float(value)
    except OverflowError:
        return math.inf if value > 0 else -math.inf


def _core_wrap_scaling(scaling):
    if type(scaling) is str:
        return scaling, 1.0
    if type(scaling) is tuple and len(scaling) == 2:
        name, limit = scaling
        if type(name) is str and type(limit) in (bool, int, float):
            return name, _wrap_limit_as_float(limit)
    return None


class _Line(NamedTuple):
    x: float
    y: float
    anchor: str
    text: str | bytes


class Text(Generic[AnyStr]):
    """Text, font, and layout options carried together for ImageDraw."""

    def __init__(
        self,
        text: AnyStr,
        font: Font | None = None,
        mode: str = "RGB",
        spacing: float = 4,
        direction: str | None = None,
        features: list[str] | None = None,
        language: str | None = None,
    ) -> None:
        self.text = text
        self.font = font or ImageFont.load_default()
        self.mode = mode
        self.spacing = spacing
        self.direction = direction
        self.features = features
        self.language = language
        self.embedded_color = False
        self.stroke_width = 0
        self.stroke_fill = None

    def embed_color(self) -> None:
        if self.mode not in ("RGB", "RGBA"):
            raise ValueError("Embedded color supported only in RGB and RGBA modes")
        self.embedded_color = True

    def stroke(self, width: float = 0, fill: _Ink | None = None) -> None:
        self.stroke_width = width
        self.stroke_fill = fill

    def _get_fontmode(self):
        if type(self.mode) is str and type(self.embedded_color) is bool:
            return _core.imagefont_text_font_mode(self.mode, self.embedded_color)
        if self.mode in ("1", "P", "I", "F"):
            return "1"
        if self.embedded_color:
            return "RGBA"
        return "L"

    def get_length(self) -> float:
        if self._can_get_length_in_core():
            return self.font._rust_font._image_text_get_length(
                self.text,
                self.mode,
                self.embedded_color,
            )

        newline = "\n" if isinstance(self.text, str) else b"\n"
        if newline in self.text:
            raise ValueError("can't measure length of multiline text")
        return self.font.getlength(
            self.text,
            self._get_fontmode(),
            self.direction,
            self.features,
            self.language,
        )

    def _can_get_length_in_core(self):
        font = self.font
        return (
            type(self.text) in (str, bytes)
            and type(self) is Text
            and type(font) is ImageFont.FreeTypeFont
            and ImageFont.FreeTypeFont.getlength is _BASE_FREETYPE_GETLENGTH
            and "getlength" not in font.__dict__
            and Text._get_fontmode is _BASE_TEXT_GET_FONTMODE
            and Text._can_get_length_in_core is _BASE_TEXT_CAN_GET_LENGTH
            and "_get_fontmode" not in self.__dict__
            and "_can_get_length_in_core" not in self.__dict__
            and hasattr(type(font._rust_font), "_image_text_get_length")
            and type(self.mode) is str
            and type(self.embedded_color) is bool
            and self.direction is None
            and self.features is None
            and self.language is None
        )

    def _split(self, xy=(0, 0), anchor=None, align="left", lines=None):
        if self._can_split_in_core(xy, anchor, align, lines):
            positioned = self.font._rust_font._image_text_split_positions(
                self.text,
                xy,
                self.mode,
                self.embedded_color,
                float(self.spacing),
                float(self.stroke_width),
                anchor,
                align,
            )
            return [_Line(*line) for line in positioned]

        return _core.imagefont_text_split_with_callbacks(
            self,
            xy,
            anchor,
            align,
            lines,
        )

    def _can_split_in_core(self, xy, anchor, align, lines):
        font = self.font
        return (
            type(self) is Text
            and lines is None
            and type(self.text) is str
            and type(font) is ImageFont.FreeTypeFont
            and ImageFont.FreeTypeFont.getbbox is _BASE_FREETYPE_GETBBOX
            and ImageFont.FreeTypeFont.getlength is _BASE_FREETYPE_GETLENGTH
            and "getbbox" not in font.__dict__
            and "getlength" not in font.__dict__
            and Text._split is _BASE_TEXT_SPLIT
            and Text._get_fontmode is _BASE_TEXT_GET_FONTMODE
            and Text._get_bbox is _BASE_TEXT_GET_BBOX
            and Text._can_split_in_core is _BASE_TEXT_CAN_SPLIT
            and "_split" not in self.__dict__
            and "_get_fontmode" not in self.__dict__
            and "_get_bbox" not in self.__dict__
            and "_can_split_in_core" not in self.__dict__
            and hasattr(type(font._rust_font), "_image_text_split_positions")
            and type(self.mode) is str
            and type(self.embedded_color) is bool
            and (
                (type(self.spacing) is int and abs(self.spacing) <= 2**53)
                or (type(self.spacing) is float and math.isfinite(self.spacing))
            )
            and (
                (type(self.stroke_width) is int and abs(self.stroke_width) <= 2**24)
                or (
                    type(self.stroke_width) is float
                    and math.isfinite(self.stroke_width)
                    and abs(self.stroke_width) <= 3.4028234663852886e38
                )
            )
            and self.direction is None
            and self.features is None
            and self.language is None
            and (anchor is None or type(anchor) is str)
            and type(align) is str
            and type(xy) is tuple
            and len(xy) == 2
            and all(
                (type(value) is float and math.isfinite(value))
                or (type(value) is int and abs(value) <= 2**53)
                for value in xy
            )
        )

    def _get_bbox(self, text, font=None, anchor=None):
        return (font or self.font).getbbox(
            text,
            self._get_fontmode(),
            self.direction,
            self.features,
            self.language,
            self.stroke_width,
            anchor,
        )

    def get_bbox(
        self,
        xy: tuple[float, float] = (0, 0),
        anchor: str | None = None,
        align: str = "left",
    ) -> tuple[float, float, float, float]:
        if self._can_split_in_core(xy, anchor, align, None):
            return self.font._rust_font._image_text_layout_bbox(
                self.text,
                xy,
                self.mode,
                self.embedded_color,
                float(self.spacing),
                float(self.stroke_width),
                anchor,
                align,
            )

        return _core.imagefont_text_layout_bbox_with_callbacks(
            self,
            xy,
            anchor,
            align,
        )

    def wrap(
        self,
        width: int,
        height: int | None = None,
        scaling: str | tuple[str, int] | None = None,
    ) -> Text[AnyStr] | None:
        if isinstance(self.font, ImageFont.TransposedFont):
            raise ValueError("TransposedFont not supported")
        if self.direction not in (None, "ltr"):
            raise ValueError("Only ltr direction supported")

        if scaling is None:
            if Text._can_wrap_in_core(self, width, height):
                lines, remaining_text = self.font._rust_font._image_text_wrap(
                    self.text,
                    _wrap_limit_as_float(width),
                    None if height is None else _wrap_limit_as_float(height),
                    self.mode,
                    self.embedded_color,
                    float(self.spacing),
                    float(self.stroke_width),
                )
                return Text._finish_wrap(self, lines, remaining_text)
            result = _core.imagefont_text_wrap_with_callbacks(
                self,
                self.text,
                width,
                height,
            )
            if result is None:
                return None
            lines, remaining_text, _, _ = result
            return Text._finish_wrap(self, lines, remaining_text)
        else:
            if not isinstance(self.font, ImageFont.FreeTypeFont):
                raise ValueError("'scaling' only supports FreeTypeFont")
            if height is None:
                raise ValueError("'scaling' requires 'height'")

            scaling_input = _core_wrap_scaling(scaling)
            if scaling_input is not None and Text._can_scaled_wrap_in_core(
                self, width, height
            ):
                scaling_name, limit = scaling_input
                result = self.font._rust_font._image_text_wrap_scaled(
                    self.text,
                    _wrap_limit_as_float(width),
                    _wrap_limit_as_float(height),
                    self.mode,
                    self.embedded_color,
                    float(self.spacing),
                    float(self.stroke_width),
                    scaling_name,
                    limit,
                )
                if result is None:
                    return None
                lines, remaining_text, rust_font = result
                if rust_font is not None:
                    self.font = ImageFont.FreeTypeFont._from_rust_font(
                        rust_font,
                        size=rust_font.get_size(),
                        index=self.font.index,
                        encoding=self.font.encoding,
                    )
                return Text._finish_wrap(self, lines, remaining_text)

            if isinstance(scaling, str):
                scaling_name, limit = scaling, 1
            else:
                scaling_name, limit = scaling
            font = self.font
            result = _core.imagefont_text_wrap_with_callbacks(
                self,
                self.text,
                width,
                height,
                scaling_name,
                limit,
                True,
                font,
            )
            if result is None:
                return None
            lines, remaining_text, font, _ = result
            if font is not None:
                self.font = font
            return Text._finish_wrap(self, lines, remaining_text)

    def _can_wrap_in_core(self, width, height):
        font = self.font
        return (
            type(self) is Text
            and type(self.text) in (str, bytes)
            and type(font) is ImageFont.FreeTypeFont
            and ImageFont.FreeTypeFont.getbbox is _BASE_FREETYPE_GETBBOX
            and ImageFont.FreeTypeFont.getlength is _BASE_FREETYPE_GETLENGTH
            and "getbbox" not in font.__dict__
            and "getlength" not in font.__dict__
            and Text._get_bbox is _BASE_TEXT_GET_BBOX
            and Text._get_fontmode is _BASE_TEXT_GET_FONTMODE
            and Text._split is _BASE_TEXT_SPLIT
            and Text._can_wrap_in_core is _BASE_TEXT_CAN_WRAP
            and Text.wrap is _BASE_TEXT_WRAP
            and "_get_bbox" not in self.__dict__
            and "_get_fontmode" not in self.__dict__
            and "_split" not in self.__dict__
            and "_can_wrap_in_core" not in self.__dict__
            and "wrap" not in self.__dict__
            and hasattr(type(font._rust_font), "_image_text_wrap")
            and type(self.mode) is str
            and type(self.embedded_color) is bool
            and type(width) in (bool, int, float)
            and (
                height is None
                or type(height) in (bool, int, float)
            )
            and (
                (type(self.spacing) is int and abs(self.spacing) <= 2**53)
                or (type(self.spacing) is float and math.isfinite(self.spacing))
            )
            and (
                (type(self.stroke_width) is int and abs(self.stroke_width) <= 2**24)
                or (
                    type(self.stroke_width) is float
                    and math.isfinite(self.stroke_width)
                    and abs(self.stroke_width) <= 3.4028234663852886e38
                )
            )
            and self.direction is None
            and self.features is None
            and self.language is None
        )

    def _can_scaled_wrap_in_core(self, width, height):
        font = self.font
        return (
            Text._can_wrap_in_core(self, width, height)
            and type(font) is ImageFont.FreeTypeFont
            and ImageFont.FreeTypeFont.font_variant is _BASE_FREETYPE_FONT_VARIANT
            and "font_variant" not in font.__dict__
            and Text._can_scaled_wrap_in_core is _BASE_TEXT_CAN_SCALED_WRAP
            and "_can_scaled_wrap_in_core" not in self.__dict__
            and getattr(font, "_native_font_bytes", None) is not None
            and getattr(font, "font_bytes", None) is font._native_font_bytes
            and font.index == 0
            and font.encoding == ""
            and font.layout_engine is ImageFont.Layout.BASIC
            and type(font.size) is float
            and math.isfinite(font.size)
            and font.size == font._rust_font.get_size()
        )

    def _finish_wrap(self, lines, remaining_text):
        if remaining_text:
            remainder = Text(
                text=remaining_text,
                font=self.font,
                mode=self.mode,
                spacing=self.spacing,
                direction=self.direction,
                features=self.features,
                language=self.language,
            )
            remainder.embedded_color = self.embedded_color
            remainder.stroke_width = self.stroke_width
            remainder.stroke_fill = self.stroke_fill
        else:
            remainder = None

        newline = "\n" if isinstance(self.text, str) else b"\n"
        self.text = newline.join(lines)
        return remainder


_BASE_TEXT_SPLIT = Text._split
_BASE_TEXT_GET_FONTMODE = Text._get_fontmode
_BASE_TEXT_GET_BBOX = Text._get_bbox
_BASE_TEXT_CAN_GET_LENGTH = Text._can_get_length_in_core
_BASE_TEXT_CAN_SPLIT = Text._can_split_in_core
_BASE_TEXT_CAN_WRAP = Text._can_wrap_in_core
_BASE_TEXT_CAN_SCALED_WRAP = Text._can_scaled_wrap_in_core
_BASE_TEXT_WRAP = Text.wrap


def _can_use_native_font_metrics(font):
    """Whether native layout can preserve the font's public metric methods."""
    return (
        type(font) is ImageFont.FreeTypeFont
        and type(font._rust_font) is ImageFont._core.ImageFont
        and ImageFont.FreeTypeFont.getbbox is _BASE_FREETYPE_GETBBOX
        and ImageFont.FreeTypeFont.getlength is _BASE_FREETYPE_GETLENGTH
        and "getbbox" not in font.__dict__
        and "getlength" not in font.__dict__
    )


def _can_use_native_draw_font(font):
    """Whether direct Rust drawing preserves the built-in font API route."""
    return (
        _can_use_native_font_metrics(font)
        and ImageFont.FreeTypeFont.getmask is _BASE_FREETYPE_GETMASK
        and ImageFont.FreeTypeFont.getmask2 is _BASE_FREETYPE_GETMASK2
        and "getmask" not in font.__dict__
        and "getmask2" not in font.__dict__
    )


def _has_base_text_layout_methods():
    """Whether direct layout preserves ImageText.Text's method dispatch."""
    return (
        Text._split is _BASE_TEXT_SPLIT
        and Text._get_fontmode is _BASE_TEXT_GET_FONTMODE
        and Text._get_bbox is _BASE_TEXT_GET_BBOX
        and Text._can_split_in_core is _BASE_TEXT_CAN_SPLIT
    )


def _can_use_native_draw_text(image_text):
    """Whether direct drawing can bypass the Python ImageText callbacks."""
    return (
        type(image_text) is Text
        and _has_base_text_layout_methods()
        and all(
            name not in image_text.__dict__
            for name in ("_split", "_get_fontmode", "_get_bbox", "_can_split_in_core")
        )
    )
