"""ImageDraw — drawing primitives. Pillow-compatible module."""
from ._core import ImageDraw as RustDraw
from .image import Image


class ImageDraw:
    """Draw lines, rectangles, ellipses, polygons, and text on images."""

    def __init__(self, image: Image, mode: str | None = None):
        self._orig_mode = image.mode
        self._draw = RustDraw(image._rust_image)
        self._image = image

    def _sync(self):
        """Sync drawing output back to the Python Image, preserving original mode."""
        drawn = Image(self._draw.image)
        if drawn.mode != self._orig_mode:
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
        if fill is None:
            fill = (0, 0, 0)
        bmp = bitmap.convert("1")
        for y in range(min(bitmap.height, self._image.height - int(xy[1]))):
            for x in range(min(bitmap.width, self._image.width - int(xy[0]))):
                if bmp.getpixel((x, y)) == 0:
                    self.point([(int(xy[0]) + x, int(xy[1]) + y)], fill=fill)
        self._sync()

    def multiline_text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
                       align="left", direction=None, features=None, language=None,
                       stroke_width=0, stroke_fill=None, embedded_color=False, **kwargs):
        lines = str(text).split('\n')
        x, y = xy
        for line in lines:
            self.text((x, y), line, fill=fill, font=font, anchor=anchor, spacing=spacing,
                      align=align, stroke_width=stroke_width, stroke_fill=stroke_fill)
            if font:
                _, h = font.getbbox(line)
                y += int(h * 1.2)
            else:
                y += 12
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
        return None

    def shape(self, shape, fill=None, outline=None):
        """Draw a shape outline."""
        raise NotImplementedError("ImageDraw.shape")

    def regular_polygon(self, bounding_circle, n_sides, rotation=0, fill=None, outline=None, width=1):
        import math
        (cx, cy), r = bounding_circle
        points = []
        for i in range(n_sides):
            angle = math.radians(rotation + i * 360 / n_sides)
            points.append((cx + r * math.cos(angle), cy + r * math.sin(angle)))
        self.polygon(points, fill=fill, outline=outline, width=width)
        self._sync()

    def text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
             align="left", direction=None, features=None, language=None,
             stroke_width=0, stroke_fill=None, embedded_color=False):
        if font is None:
            raise NotImplementedError("ImageDraw.text requires a font: use ImageFont.truetype()")
        if hasattr(font, '_rust_font'):
            self._draw.text((float(xy[0]), float(xy[1])), str(text), fill, font._rust_font)
        elif hasattr(font, 'getmask'):
            raise NotImplementedError("PIL ImageFont not yet supported")
        else:
            self._draw.text((float(xy[0]), float(xy[1])), str(text), fill, font)
        self._sync()
