"""ImageDraw — drawing primitives. Pillow-compatible module."""
from ._core import ImageDraw as RustDraw
from ._core import outline_curve as _outline_curve
from .image import Image


class Outline:
    """Experimental outline API for ImageDraw.shape().
    Mirrors PIL's ImageDraw.Outline (C-level _Outline)."""

    def __init__(self):
        self._points = []

    def move(self, x, y):
        self._points = [(int(x), int(y))]

    def line(self, x, y):
        self._points.append((int(x), int(y)))

    def curve(self, x1, y1, x2, y2, x3, y3):
        # Cubic Bezier approximation: subdivide into line segments in Rust
        x0, y0 = self._points[-1]
        steps = 20
        new_points = _outline_curve([float(x0), float(y0), float(x1), float(y1), float(x2), float(y2), float(x3), float(y3)], steps)
        self._points.extend(map(tuple, new_points))

    def close(self):
        if len(self._points) > 2 and self._points[0] != self._points[-1]:
            self._points.append(self._points[0])


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
        """Sync drawing output back to the Python Image, preserving original mode."""
        drawn = Image(self._draw.image)
        # F/I modes store raw 32-bit LE values in the RGBA canvas — no conversion.
        # Standard modes: the RGBA canvas must be converted back to native format.
        _RAW_MODES = {"F", "I"}
        if self._orig_mode not in _RAW_MODES and drawn.mode != self._orig_mode:
            # Use no-dither conversion for binary modes to avoid Floyd-Steinberg
            # dither artifacts in the background. PIL draws directly on the native
            # canvas (no RGBA intermediate), so the conversion back must be lossless
            # for unmodified pixels (0/255 -> 0/255).
            dither_arg = "NONE" if self._orig_mode == "1" else None
            drawn = drawn.convert(self._orig_mode, dither=dither_arg)
        self._image._rust_image = drawn._rust_image

    def _shape_inks(self, fill, outline):
        # Pillow ImageDraw starts with ink=-1 and fill=0. When both optional
        # colors are omitted, shape methods therefore draw an all-255 outline.
        if fill is None and outline is None and self._orig_mode == "PA":
            outline = (255, 255)
        return fill, outline

    def line(self, xy, fill=None, width: int = 0, joint: str | None = None):
        if fill is None and self._orig_mode != "PA":
            fill = (0, 0, 0)
        self._draw.line(xy, fill, width if width > 0 else 1)
        self._sync()

    def rectangle(self, xy, fill=None, outline=None, width: int = 1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        fill, outline = self._shape_inks(fill, outline)
        self._draw.rectangle((x0, y0, x1, y1), fill, outline, width)
        self._sync()

    def ellipse(self, xy, fill=None, outline=None, width: int = 1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        fill, outline = self._shape_inks(fill, outline)
        self._draw.ellipse((x0, y0, x1, y1), fill, outline, width)
        self._sync()

    def polygon(self, xy, fill=None, outline=None, width: int = 1):
        fill, outline = self._shape_inks(fill, outline)
        self._draw.polygon(xy, fill, outline, width)
        self._sync()

    def point(self, xy, fill=None):
        if fill is None and self._orig_mode != "PA":
            fill = (0, 0, 0)
        if isinstance(xy[0], (int, float)):
            xy = [xy]
        self._draw.point(xy, fill)
        self._sync()

    def arc(self, xy, start, end, fill=None, width=1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        self._draw.arc((x0, y0, x1, y1), float(start), float(end), fill, width)
        self._sync()

    def chord(self, xy, start, end, fill=None, outline=None, width=1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        fill, outline = self._shape_inks(fill, outline)
        self._draw.chord((x0, y0, x1, y1), float(start), float(end), fill, outline, width)
        self._sync()

    def pieslice(self, xy, start, end, fill=None, outline=None, width=1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        fill, outline = self._shape_inks(fill, outline)
        self._draw.pieslice((x0, y0, x1, y1), float(start), float(end), fill, outline, width)
        self._sync()

    def circle(self, xy, radius, fill=None, outline=None, width=1):
        fill, outline = self._shape_inks(fill, outline)
        self._draw.circle((float(xy[0]), float(xy[1])), float(radius), fill, outline, width)
        self._sync()

    def rounded_rectangle(self, xy, radius=0, fill=None, outline=None, width=1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        fill, outline = self._shape_inks(fill, outline)
        self._draw.rounded_rectangle((x0, y0, x1, y1), float(radius), fill, outline, width)
        self._sync()

    def bitmap(self, xy, bitmap, fill=None):
        """Draw a bitmap. Pixel iteration done in Rust."""
        if fill is None:
            fill = (0, 0, 0)
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
        return self._draw.textbbox(xy, str(text), font)

    def textlength(self, text, font=None, **kwargs):
        return self._draw.textlength(str(text), font)

    def getfont(self):
        """Return the current font."""
        return self._get_font(None)

    def multiline_textbbox(self, xy, text, font=None, anchor=None, spacing=4, align='left',
                           direction=None, features=None, language=None, stroke_width=0,
                           embedded_color=False, *, font_size=None):
        """Get the bounding box of multiline text."""
        return self._draw.multiline_textbbox(xy, str(text), font, spacing, align)

    def shape(self, shape, fill=None, outline=None):
        """Draw a shape defined by an Outline or sequence of coordinates.

        PIL's ImagingDrawOutline always fills the polygon entirely (ignoring
        the `fill` parameter). When both outline and fill are given, the
        outline color overwrites the fill — matching PIL's double-pass:
        draw_outline(shape, fill_ink, 1) then draw_outline(shape, ink, 0).
        """
        if isinstance(shape, Outline):
            shape.close()
            xy = shape._points
            # PIL's draw_outline fills the entire polygon — it never draws
            # a 1px border. The effective color is outline (if given) since
            # it is always drawn last (overwriting fill).
            if outline is not None:
                self.polygon(xy, fill=outline, outline=None)
            elif fill is not None:
                self.polygon(xy, fill=fill, outline=None)
        elif isinstance(shape, (list, tuple)):
            if all(map(lambda p: isinstance(p, (list, tuple)) and len(p) == 2, shape)):
                if outline is not None:
                    self.polygon(shape, fill=outline, outline=None)
                elif fill is not None:
                    self.polygon(shape, fill=fill, outline=None)
            else:
                raise TypeError(f"Unsupported shape format")
        else:
            raise TypeError(f"unsupported shape type: {type(shape)}")

    def regular_polygon(self, bounding_circle, n_sides, rotation=0, fill=None, outline=None, width=1):
        """Draw a regular polygon. Vertex computation done in Rust."""
        fill, outline = self._shape_inks(fill, outline)
        self._draw.regular_polygon(bounding_circle, n_sides, float(rotation), fill, outline, width)
        self._sync()

    def text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
             align="left", direction=None, features=None, language=None,
             stroke_width=0, stroke_fill=None, embedded_color=False):
        font = self._get_font(font)
        if hasattr(font, '_rust_font'):
            # Use PIL-compatible text rendering: get L-mode mask via getmask2
            # then draw_bitmap (matching ImagingFill2 behavior exactly).
            # For modes 1, P, I, F: use the original Rust text pipeline since
            # these require binary fontmode (FT_LOAD_TARGET_MONO).
            if fill is not None and self._orig_mode in ("RGB", "RGBA"):
                mask, offset = font.getmask2(text, mode="L")
                self.bitmap((xy[0] + offset[0], xy[1] + offset[1]), mask, fill=fill)
            else:
                self._draw.text((float(xy[0]), float(xy[1])), str(text), fill, font._rust_font)
        elif hasattr(font, 'getmask'):
            mask = font.getmask(text, mode="1" if self._orig_mode == "1" else "L")
            self.bitmap(xy, mask, fill=fill)
        else:
            self._draw.text((float(xy[0]), float(xy[1])), str(text), fill, font)
        self._sync()
        self._font = font
