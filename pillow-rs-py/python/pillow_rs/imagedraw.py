"""ImageDraw — drawing primitives. Pillow-compatible module."""
from ._core import ImageDraw as RustDraw
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
        """Sync drawing output back to the Python Image, preserving original mode."""
        drawn = Image(self._draw.image)
        # F/I modes store raw 32-bit LE values in the RGBA canvas — no conversion.
        # Standard modes: the RGBA canvas must be converted back to native format.
        _RAW_MODES = {"F", "I"}
        if self._orig_mode not in _RAW_MODES and drawn.mode != self._orig_mode:
            drawn = drawn.convert(self._orig_mode)
        self._image._rust_image = drawn._rust_image

    def line(self, xy, fill=None, width: int = 0, joint: str | None = None):
        if fill is None:
            fill = (0, 0, 0)
        pts = [(int(p[0]), int(p[1])) for p in xy]
        self._draw.line(pts, fill, width if width > 0 else 1)
        self._sync()

    def rectangle(self, xy, fill=None, outline=None, width: int = 1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        self._draw.rectangle((x0, y0, x1, y1), fill, outline, width)
        self._sync()

    def ellipse(self, xy, fill=None, outline=None, width: int = 1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        self._draw.ellipse((x0, y0, x1, y1), fill, outline, width)
        self._sync()

    def polygon(self, xy, fill=None, outline=None, width: int = 1):
        pts = [(int(p[0]), int(p[1])) for p in xy]
        self._draw.polygon(pts, fill, outline, width)
        self._sync()

    def point(self, xy, fill=None):
        if fill is None:
            fill = (0, 0, 0)
        if isinstance(xy[0], (int, float)):
            xy = [xy]
        pts = [(int(p[0]), int(p[1])) for p in xy]
        self._draw.point(pts, fill)
        self._sync()

    def arc(self, xy, start, end, fill=None, width=1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        self._draw.arc((x0, y0, x1, y1), float(start), float(end), fill, width)
        self._sync()

    def chord(self, xy, start, end, fill=None, outline=None, width=1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        self._draw.chord((x0, y0, x1, y1), float(start), float(end), fill, outline, width)
        self._sync()

    def pieslice(self, xy, start, end, fill=None, outline=None, width=1):
        x0, y0, x1, y1 = int(xy[0]), int(xy[1]), int(xy[2]), int(xy[3])
        self._draw.pieslice((x0, y0, x1, y1), float(start), float(end), fill, outline, width)
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
        if fill is None:
            fill = (0, 0, 0)
        bmp = bitmap.convert("1")
        self._draw.bitmap((float(xy[0]), float(xy[1])), bmp._rust_image, fill)
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
        """Draw multiple lines of text. Text layout done in Rust."""
        font = self._get_font(font)
        rust_font = font._rust_font if hasattr(font, '_rust_font') else font
        self._draw.multiline_text((float(xy[0]), float(xy[1])), str(text), fill, rust_font, int(spacing))
        self._sync()

    def textbbox(self, xy, text, font=None, **kwargs):
        if font and hasattr(font, 'getbbox'):
            w, h = font.getbbox(str(text))
            return (xy[0], xy[1], xy[0] + w, xy[1] + h)
        return (xy[0], xy[1], xy[0] + 80, xy[1] + 12)

    def textlength(self, text, font=None, **kwargs):
        if font and hasattr(font, 'getbbox'):
            w, _ = font.getbbox(str(text))
            return w
        return len(str(text)) * 8

    def getfont(self):
        """Return the current font."""
        return self._font

    def multiline_textbbox(self, xy, text, font=None, anchor=None, spacing=4, align='left',
                           direction=None, features=None, language=None, stroke_width=0,
                           embedded_color=False, *, font_size=None):
        """Get the bounding box of multiline text."""
        text = str(text)
        font = self._get_font(font)

        lines = text.split('\n')
        if len(lines) == 1:
            return self.textbbox(xy, text, font=font)

        # Calculate line height (font height + spacing)
        if font and hasattr(font, 'getbbox'):
            _, h = font.getbbox('A')
            line_height = h + spacing
        else:
            line_height = 12 + spacing

        # Calculate widths for each line
        widths = []
        for line in lines:
            if font and hasattr(font, 'getbbox'):
                w, _ = font.getbbox(line)
                widths.append(w)
            else:
                widths.append(len(line) * 8)

        max_width = max(widths) if widths else 0
        x0, y0 = float(xy[0]), float(xy[1])

        left = float('inf')
        top = float('inf')
        right = float('-inf')
        bottom = float('-inf')

        for i, line in enumerate(lines):
            line_y = y0 + i * line_height

            if align == 'center':
                line_x = x0 + (max_width - widths[i]) / 2.0
            elif align == 'right':
                line_x = x0 + max_width - widths[i]
            else:  # left
                line_x = x0

            if font and hasattr(font, 'getbbox'):
                w, h = font.getbbox(line)
                left = min(left, line_x)
                top = min(top, line_y)
                right = max(right, line_x + w)
                bottom = max(bottom, line_y + h)
            else:
                left = min(left, line_x)
                top = min(top, line_y)
                right = max(right, line_x + widths[i])
                bottom = max(bottom, line_y + line_height)

        return (left, top, right, bottom)

    def shape(self, shape, fill=None, outline=None):
        """Draw a shape defined by a sequence of coordinates."""
        if isinstance(shape, (list, tuple)):
            # Accept a single polygon-like sequence of (x,y) pairs
            if all(isinstance(p, (list, tuple)) and len(p) == 2 for p in shape):
                self.polygon(shape, fill=fill, outline=outline)
            else:
                raise TypeError(f"Unsupported shape format")
        else:
            raise TypeError(f"unsupported shape type: {type(shape)}")

    def regular_polygon(self, bounding_circle, n_sides, rotation=0, fill=None, outline=None, width=1):
        """Draw a regular polygon. Vertex computation done in Rust."""
        self._draw.regular_polygon(bounding_circle, n_sides, float(rotation), fill, outline, width)
        self._sync()

    def text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
             align="left", direction=None, features=None, language=None,
             stroke_width=0, stroke_fill=None, embedded_color=False):
        font = self._get_font(font)
        if hasattr(font, '_rust_font'):
            self._draw.text((float(xy[0]), float(xy[1])), str(text), fill, font._rust_font)
        elif hasattr(font, 'getmask'):
            raise NotImplementedError("PIL ImageFont not yet supported")
        else:
            self._draw.text((float(xy[0]), float(xy[1])), str(text), fill, font)
        self._sync()
        self._font = font
