"""ImageDraw — drawing primitives. Pillow-compatible module."""
from ._core import ImageDraw as RustDraw
from ._core import Outline
from .image import Image


class Draw:
    """Draw lines, rectangles, ellipses, polygons, and text on images."""

    def __init__(self, image: Image, mode: str | None = None):
        self._orig_mode = image.mode
        # Pass explicit mode to Rust so it knows the true PIL mode
        # (e.g. "P" stored as Luma8, "CMYK" stored as Rgba8)
        rust_mode = image._explicit_mode
        self._draw = RustDraw(image._rust_image, rust_mode)
        self._image = image
        self._font = None  # current font for text

    def _sync(self):
        """Install the native-mode image restored by Rust core."""
        self._image._rust_image = self._draw.image

    @staticmethod
    def _text_options(anchor, direction, features, language):
        if anchor is not None and (not isinstance(anchor, str) or len(anchor) != 2):
            raise ValueError("anchor must be a 2 character string")
        if direction is not None or features is not None or language is not None:
            raise KeyError("setting text direction, language or font features is not supported without libraqm")

    def line(self, xy, fill=None, width: int = 0, joint: str | None = None):
        self._draw.line(xy, fill, width)
        self._sync()

    def rectangle(self, xy, fill=None, outline=None, width: int = 1):
        try:
            self._draw.rectangle(xy, fill, outline, width)
        except ValueError:
            invalid_color = fill if isinstance(fill, str) else outline if isinstance(outline, str) else None
            if invalid_color is None:
                raise
            raise ValueError(f"unknown color specifier: {invalid_color!r}") from None
        self._sync()

    def ellipse(self, xy, fill=None, outline=None, width: int = 1):
        self._draw.ellipse(xy, fill, outline, width)
        self._sync()

    def polygon(self, xy, fill=None, outline=None, width: int = 1):
        self._draw.polygon(xy, fill, outline, width)
        self._sync()

    def point(self, xy, fill=None):
        self._draw.point(xy, fill)
        self._sync()

    def arc(self, xy, start, end, fill=None, width=1):
        self._draw.arc(xy, float(start), float(end), fill, width)
        self._sync()

    def chord(self, xy, start, end, fill=None, outline=None, width=1):
        self._draw.chord(xy, float(start), float(end), fill, outline, width)
        self._sync()

    def pieslice(self, xy, start, end, fill=None, outline=None, width=1):
        self._draw.pieslice(xy, float(start), float(end), fill, outline, width)
        self._sync()

    def circle(self, xy, radius, fill=None, outline=None, width=1):
        self._draw.circle((float(xy[0]), float(xy[1])), float(radius), fill, outline, width)
        self._sync()

    def rounded_rectangle(self, xy, radius=0, fill=None, outline=None, width=1, *, corners=None):
        self._draw.rounded_rectangle(xy, float(radius), fill, outline, width)
        self._sync()

    def bitmap(self, xy, bitmap, fill=None):
        """Draw a bitmap. Pixel iteration done in Rust."""
        # PIL validates the fill arity against the canvas mode in
        # ``Draw._getink`` before touching the bitmap.
        if isinstance(fill, (tuple, list)):
            n = len(fill)
            if len(self._orig_mode) == 1 and self._orig_mode != "P" and n != 1:
                if self._orig_mode == "F":
                    raise TypeError("must be real number, not tuple")
                raise TypeError("color must be int or single-element tuple")
            if len(self._orig_mode) == 2 and n not in (1, 2):
                raise TypeError(
                    "color must be int, or tuple of one or two elements"
                )
        self._draw.bitmap((float(xy[0]), float(xy[1])), bitmap._rust_image, fill)
        self._sync()

    def _get_font(self, font, size=None):
        """Get font, loading default if needed (PIL-compatible)."""
        if font is not None:
            if size is not None and hasattr(font, "font_variant"):
                return font.font_variant(size=size)
            return font
        if self._font is not None and size is None:
            return self._font
        from . import imagefont as ImageFont
        self._font = ImageFont.load_default(size=size)
        return self._font

    def multiline_text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
                       align="left", direction=None, features=None, language=None,
                       stroke_width=0, stroke_fill=None, embedded_color=False, **kwargs):
        """Draw multiple lines of text. Delegates to text() for PIL-parity rendering."""
        self.text(xy, text, fill=fill, font=font, anchor=anchor, spacing=spacing,
                  align=align, direction=direction, features=features,
                  language=language, stroke_width=stroke_width,
                  stroke_fill=stroke_fill, embedded_color=embedded_color)

    def textbbox(self, xy, text, font=None, **kwargs):
        self._text_options(
            kwargs.get("anchor"),
            kwargs.get("direction"),
            kwargs.get("features"),
            kwargs.get("language"),
        )
        font = self._get_font(font, kwargs.get("font_size"))
        return self._draw.textbbox(
            xy,
            str(text),
            font._rust_font if hasattr(font, "_rust_font") else font,
            kwargs.get("direction"),
            kwargs.get("features"),
            kwargs.get("language"),
            float(kwargs.get("stroke_width", 0)),
            kwargs.get("anchor"),
        )

    def textlength(self, text, font=None, **kwargs):
        self._text_options(
            kwargs.get("anchor"),
            kwargs.get("direction"),
            kwargs.get("features"),
            kwargs.get("language"),
        )
        font = self._get_font(font, kwargs.get("font_size"))
        return self._draw.textlength(
            str(text),
            font._rust_font if hasattr(font, "_rust_font") else font,
            kwargs.get("direction"),
            kwargs.get("features"),
            kwargs.get("language"),
        )

    def getfont(self):
        """Return the current font."""
        return self._get_font(None)

    def multiline_textbbox(self, xy, text, font=None, anchor=None, spacing=4, align='left',
                           direction=None, features=None, language=None, stroke_width=0,
                           embedded_color=False, *, font_size=None):
        """Get the bounding box of multiline text."""
        self._text_options(anchor, direction, features, language)
        font = self._get_font(font, font_size)
        return self._draw.multiline_textbbox(
            xy,
            str(text),
            font._rust_font if hasattr(font, "_rust_font") else font,
            spacing,
            align,
            direction,
            features,
            language,
            float(stroke_width),
            anchor,
        )

    def shape(self, shape, fill=None, outline=None):
        """Draw a shape using Rust's Pillow-compatible outline semantics."""
        if not isinstance(shape, Outline):
            raise TypeError("expected outline object")
        self._draw.shape(shape, fill, outline)
        self._sync()

    def regular_polygon(self, bounding_circle, n_sides, rotation=0, fill=None, outline=None, width=1):
        """Draw a regular polygon. Vertex computation done in Rust."""
        self._draw.regular_polygon(bounding_circle, n_sides, float(rotation), fill, outline, width)
        self._sync()

    def text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
             align="left", direction=None, features=None, language=None,
             stroke_width=0, stroke_fill=None, embedded_color=False):
        self._text_options(anchor, direction, features, language)
        font = self._get_font(font)
        if hasattr(font, '_rust_font'):
            self._draw.text(
                (float(xy[0]), float(xy[1])),
                str(text),
                fill,
                font._rust_font,
                direction,
                features,
                language,
                float(stroke_width),
                anchor,
            )
        elif hasattr(font, 'getmask'):
            mask = font.getmask(text, mode="1" if self._orig_mode == "1" else "L")
            self.bitmap(xy, mask, fill=fill)
        else:
            self._draw.text((float(xy[0]), float(xy[1])), str(text), fill, font)
        self._sync()
        self._font = font
