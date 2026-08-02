"""ImageDraw — drawing primitives. Pillow-compatible module."""
from ._core import ImageDraw as RustDraw
from ._core import Outline  # public compatibility re-export
from .image import Image


class Draw:
    """Draw lines, rectangles, ellipses, polygons, and text on images."""

    def __init__(self, image: Image, mode: str | None = None):
        self._orig_mode = image.mode
        # Pass explicit mode to Rust so it knows the true PIL mode
        # (e.g. "P" stored as Luma8, "CMYK" stored as Rgba8)
        rust_mode = image.mode
        self._draw = RustDraw(image._rust_image, rust_mode)
        self._image = image
        self._font = None  # current font for text

    def _sync(self):
        """Install the native-mode image restored by Rust core."""
        self._image._rust_image = self._draw.image

    def line(self, xy, fill=None, width: int = 0, joint: str | None = None):
        self._draw.line(xy, fill, width)
        self._sync()

    def rectangle(self, xy, fill=None, outline=None, width: int = 1):
        self._draw.rectangle(xy, fill, outline, width)
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
        self._draw.arc(xy, start, end, fill, width)
        self._sync()

    def chord(self, xy, start, end, fill=None, outline=None, width=1):
        self._draw.chord(xy, start, end, fill, outline, width)
        self._sync()

    def pieslice(self, xy, start, end, fill=None, outline=None, width=1):
        self._draw.pieslice(xy, start, end, fill, outline, width)
        self._sync()

    def circle(self, xy, radius, fill=None, outline=None, width=1):
        self._draw.circle(xy, radius, fill, outline, width)
        self._sync()

    def rounded_rectangle(self, xy, radius=0, fill=None, outline=None, width=1, *, corners=None):
        self._draw.rounded_rectangle(xy, radius, fill, outline, width)
        self._sync()

    def bitmap(self, xy, bitmap, fill=None):
        """Draw a bitmap. Pixel iteration done in Rust."""
        self._draw.bitmap(xy, bitmap._rust_image, fill)
        self._sync()

    def multiline_text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
                       align="left", direction=None, features=None, language=None,
                       stroke_width=0, stroke_fill=None, embedded_color=False, **kwargs):
        """Draw multiple lines of text through the Rust drawing path."""
        if hasattr(font, "_rust_font"):
            self._draw.multiline_text(
                xy, str(text), fill, font._rust_font, spacing, direction,
                features, language, float(stroke_width), anchor, embedded_color,
                kwargs.get("font_size"),
            )
        elif hasattr(font, "getmask"):
            return self.text(
                xy, text, fill=fill, font=font, anchor=anchor, spacing=spacing,
                align=align, direction=direction, features=features,
                language=language, stroke_width=stroke_width,
                stroke_fill=stroke_fill, embedded_color=embedded_color,
            )
        else:
            self._draw.multiline_text(
                xy, str(text), fill, font, spacing, direction, features,
                language, float(stroke_width), anchor, embedded_color,
                kwargs.get("font_size"),
            )
        self._sync()
        self._font = font

    def textbbox(self, xy, text, font=None, **kwargs):
        return self._draw.textbbox(
            xy,
            str(text),
            font._rust_font if hasattr(font, "_rust_font") else font,
            kwargs.get("direction"),
            kwargs.get("features"),
            kwargs.get("language"),
            float(kwargs.get("stroke_width", 0)),
            kwargs.get("anchor"),
            kwargs.get("embedded_color", False),
            kwargs.get("font_size"),
        )

    def textlength(self, text, font=None, **kwargs):
        return self._draw.textlength(
            str(text),
            font._rust_font if hasattr(font, "_rust_font") else font,
            kwargs.get("direction"),
            kwargs.get("features"),
            kwargs.get("language"),
            kwargs.get("embedded_color", False),
            kwargs.get("font_size"),
        )

    def getfont(self):
        """Return the current font."""
        if self._font is not None:
            return self._font
        from . import imagefont as ImageFont
        self._font = ImageFont.load_default()
        return self._font

    def multiline_textbbox(self, xy, text, font=None, anchor=None, spacing=4, align='left',
                           direction=None, features=None, language=None, stroke_width=0,
                           embedded_color=False, *, font_size=None):
        """Get the bounding box of multiline text."""
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
            embedded_color,
            font_size,
        )

    def shape(self, shape, fill=None, outline=None):
        """Draw a shape using Rust's Pillow-compatible outline semantics."""
        self._draw.shape(shape, fill, outline)
        self._sync()

    def regular_polygon(self, bounding_circle, n_sides, rotation=0, fill=None, outline=None, width=1):
        """Draw a regular polygon. Vertex computation done in Rust."""
        self._draw.regular_polygon(bounding_circle, n_sides, rotation, fill, outline, width)
        self._sync()

    def text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
             align="left", direction=None, features=None, language=None,
             stroke_width=0, stroke_fill=None, embedded_color=False):
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
                embedded_color,
            )
        elif hasattr(font, 'getmask'):
            mask = font.getmask(text, mode="1" if self._orig_mode == "1" else "L")
            self.bitmap(xy, mask, fill=fill)
        else:
            self._draw.text(
                (float(xy[0]), float(xy[1])), str(text), fill, font,
                direction, features, language, float(stroke_width), anchor,
                embedded_color,
            )
        self._sync()
        self._font = font
