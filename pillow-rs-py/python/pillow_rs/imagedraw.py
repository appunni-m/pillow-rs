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
    def _box(xy):
        """Normalize Pillow's flat or nested two-corner coordinate form."""
        try:
            if len(xy) == 2 and len(xy[0]) == 2 and len(xy[1]) == 2:
                return (int(xy[0][0]), int(xy[0][1]), int(xy[1][0]), int(xy[1][1]))
            if len(xy) == 4:
                return (int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3]))
        except (IndexError, TypeError):
            pass
        raise TypeError("coordinate list must contain exactly 2 coordinates")

    @staticmethod
    def _points(xy):
        """Normalize flat and paired point sequences with Pillow errors."""
        try:
            values = list(xy)
        except TypeError as exc:
            raise TypeError("coordinate list must contain at least 2 coordinates") from exc
        if not values:
            raise TypeError("coordinate list must contain at least 2 coordinates")
        if isinstance(values[0], (list, tuple)):
            if len(values) < 2 or any(len(point) != 2 for point in values):
                raise TypeError("coordinate list must contain at least 2 coordinates")
            return tuple((int(point[0]), int(point[1])) for point in values)
        if len(values) < 4 or len(values) % 2:
            raise TypeError("coordinate list must contain at least 2 coordinates")
        return tuple((int(values[index]), int(values[index + 1])) for index in range(0, len(values), 2))

    def line(self, xy, fill=None, width: int = 0, joint: str | None = None):
        if len(xy) < 2 or (
            len(xy) == 2
            and not isinstance(xy[0], (list, tuple))
            and not isinstance(xy[1], (list, tuple))
        ):
            return None
        self._draw.line(self._points(xy), fill, width if width > 0 else 1)
        self._sync()

    def rectangle(self, xy, fill=None, outline=None, width: int = 1):
        self._draw.rectangle(self._box(xy), fill, outline, width)
        self._sync()

    def ellipse(self, xy, fill=None, outline=None, width: int = 1):
        self._draw.ellipse(self._box(xy), fill, outline, width)
        self._sync()

    def polygon(self, xy, fill=None, outline=None, width: int = 1):
        self._draw.polygon(self._points(xy), fill, outline, width)
        self._sync()

    def point(self, xy, fill=None):
        self._draw.point(xy, fill)
        self._sync()

    def arc(self, xy, start, end, fill=None, width=1):
        self._draw.arc(self._box(xy), float(start), float(end), fill, width)
        self._sync()

    def chord(self, xy, start, end, fill=None, outline=None, width=1):
        self._draw.chord(self._box(xy), float(start), float(end), fill, outline, width)
        self._sync()

    def pieslice(self, xy, start, end, fill=None, outline=None, width=1):
        self._draw.pieslice(self._box(xy), float(start), float(end), fill, outline, width)
        self._sync()

    def circle(self, xy, radius, fill=None, outline=None, width=1):
        self._draw.circle((float(xy[0]), float(xy[1])), float(radius), fill, outline, width)
        self._sync()

    def rounded_rectangle(self, xy, radius=0, fill=None, outline=None, width=1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        self._draw.rounded_rectangle((x0, y0, x1, y1), float(radius), fill, outline, width)
        self._sync()

    def bitmap(self, xy, bitmap, fill=None):
        """Draw a bitmap. Pixel iteration done in Rust."""
        self._draw.bitmap((float(xy[0]), float(xy[1])), bitmap._rust_image, fill)
        self._sync()

    def _get_font(self, font):
        """Get font, loading default if needed (PIL-compatible)."""
        if font is not None:
            return font
        if self._font is not None:
            return self._font
        from . import imagefont as ImageFont
        self._font = ImageFont.load_default()
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
        font = self._get_font(font)
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
        font = self._get_font(font)
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
        font = self._get_font(font)
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
        self._draw.shape(shape, fill, outline)
        self._sync()

    def regular_polygon(self, bounding_circle, n_sides, rotation=0, fill=None, outline=None, width=1):
        """Draw a regular polygon. Vertex computation done in Rust."""
        if not isinstance(n_sides, int) or n_sides <= 2:
            raise ValueError("n_sides should be an int > 2")
        self._draw.regular_polygon(bounding_circle, n_sides, float(rotation), fill, outline, width)
        self._sync()

    def text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
             align="left", direction=None, features=None, language=None,
             stroke_width=0, stroke_fill=None, embedded_color=False):
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
