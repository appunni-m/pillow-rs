"""ImageDraw — drawing primitives. Pillow-compatible module."""

from __future__ import annotations

from ._core import ImageDraw as RustDraw
from ._core import Outline  # public compatibility re-export
from . import imagecolor
from .image import Image


class Draw:
    """Draw lines, rectangles, ellipses, polygons, and text on images."""

    def __init__(self, image: Image, mode: str | None = None):
        # Pillow's ImageDraw constructor calls Image._ensure_mutable(), which
        # loads the image before validating the optional draw mode.
        image.load()
        self._orig_mode = image.mode
        self._draw = RustDraw(image._rust_image, mode)
        self._image = image
        self._font = None  # current font for text

    def _sync(self):
        """Install Rust pixels without replacing the Python-owned palette."""
        self._image._rust_image = self._draw.image

    def _palette_ink(self, ink):
        if self._image.mode != "P" or not self._image._rust_image.has_attached_palette():
            return ink
        self._prepare_palette_allocation()
        if isinstance(ink, str):
            ink = imagecolor.getcolor(ink, self._image.mode)
        if not isinstance(ink, tuple):
            return ink
        palette = self._image.palette
        previous_palette = palette.palette[:]
        index = palette.getcolor(ink, self._image)
        if palette.palette != previous_palette:
            # Rust updates the entries usable by indexed pixels. Keep Python's
            # possibly malformed raw palette bytes until Image.load realizes it.
            entry_size = {"RGB": 3, "RGBA": 4, "LA": 2}.get(palette.mode)
            palette_data = palette.palette
            if entry_size is not None:
                usable_size = min(len(palette_data) // entry_size, 256) * entry_size
                palette_data = palette_data[:usable_size]
            self._draw._set_palette(palette_data, palette.mode)
            self._sync()
        return index

    def _palette_fill_outline(self, fill, outline):
        outline = self._palette_ink(outline)
        fill = self._palette_ink(fill)
        return fill, outline

    def _mutate(self, operation, *args, **kwargs):
        """Sync mutations even when Pillow reports a later input error."""
        self._prepare_palette_allocation()
        try:
            return operation(*args, **kwargs)
        finally:
            self._sync()

    def _prepare_palette_allocation(self):
        if self._image.mode != "P" or not self._image._rust_image.has_attached_palette():
            self._draw.set_palette_reserved_indices([])
            return
        info = self._image.info
        special_colors = (info.get("background"), info.get("transparency"))
        reserved = [index for index in range(256) if index in special_colors]
        self._draw.set_palette_reserved_indices(reserved)

    def _getfont(self, font_size=None):
        if font_size is not None:
            from . import imagefont as ImageFont
            return ImageFont.load_default(font_size)
        return self.getfont()

    def _prepare_text_font(self, font, font_size, embedded_color):
        # Pillow creates ImageText.Text, including its default font, before
        # validating embedded-color mode or resolving the fill ink.
        if not font:
            font = self._getfont(font_size)
        embedded_color = bool(embedded_color)
        if embedded_color:
            self._draw._validate_embedded_color()
        return font, embedded_color

    def line(self, xy, fill=None, width: int = 0, joint: str | None = None):
        fill = self._palette_ink(fill)
        self._mutate(self._draw.line, xy, fill, width, joint)

    def rectangle(self, xy, fill=None, outline=None, width: int = 1):
        fill, outline = self._palette_fill_outline(fill, outline)
        self._mutate(self._draw.rectangle, xy, fill, outline, width)

    def ellipse(self, xy, fill=None, outline=None, width: int = 1):
        fill, outline = self._palette_fill_outline(fill, outline)
        self._mutate(self._draw.ellipse, xy, fill, outline, width)

    def polygon(self, xy, fill=None, outline=None, width: int = 1):
        fill, outline = self._palette_fill_outline(fill, outline)
        self._mutate(self._draw.polygon, xy, fill, outline, width)

    def point(self, xy, fill=None):
        fill = self._palette_ink(fill)
        self._mutate(self._draw.point, xy, fill)

    def arc(self, xy, start, end, fill=None, width=1):
        fill = self._palette_ink(fill)
        self._mutate(self._draw.arc, xy, start, end, fill, width)

    def chord(self, xy, start, end, fill=None, outline=None, width=1):
        fill, outline = self._palette_fill_outline(fill, outline)
        self._mutate(self._draw.chord, xy, start, end, fill, outline, width)

    def pieslice(self, xy, start, end, fill=None, outline=None, width=1):
        fill, outline = self._palette_fill_outline(fill, outline)
        self._mutate(self._draw.pieslice, xy, start, end, fill, outline, width)

    def circle(self, xy, radius, fill=None, outline=None, width=1):
        fill, outline = self._palette_fill_outline(fill, outline)
        self._mutate(self._draw.circle, xy, radius, fill, outline, width)

    def rounded_rectangle(self, xy, radius=0, fill=None, outline=None, width=1, *, corners=None):
        fill, outline = self._palette_fill_outline(fill, outline)
        self._mutate(self._draw.rounded_rectangle, xy, radius, fill, outline, width)

    def bitmap(self, xy, bitmap, fill=None):
        """Draw a bitmap. Pixel iteration done in Rust."""
        bitmap.load()
        fill = self._palette_ink(fill)
        self._mutate(self._draw.bitmap, xy, bitmap._rust_image, fill)

    def multiline_text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
                       align="left", direction=None, features=None, language=None,
                       stroke_width=0, stroke_fill=None, embedded_color=False, *,
                       font_size=None):
        """Draw multiple lines of text through the Rust drawing path."""
        font, embedded_color = self._prepare_text_font(font, font_size, embedded_color)
        fill = self._palette_ink(fill)
        if stroke_width:
            stroke_fill = self._palette_ink(stroke_fill)
        self._prepare_palette_allocation()
        try:
            if hasattr(font, "_rust_font"):
                self._draw.multiline_text(
                    xy, text, fill, font._rust_font, spacing, direction,
                    features, language, float(stroke_width), anchor, embedded_color,
                    None, align, stroke_fill,
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
                    xy, text, fill, font, spacing, direction, features,
                    language, float(stroke_width), anchor, embedded_color,
                    None, align, stroke_fill,
                )
        finally:
            self._sync()

    def textbbox(self, xy, text, font=None, **kwargs):
        return self._draw.textbbox(
            xy,
            text,
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
            text,
            font._rust_font if hasattr(font, "_rust_font") else font,
            kwargs.get("direction"),
            kwargs.get("features"),
            kwargs.get("language"),
            kwargs.get("embedded_color", False),
            kwargs.get("font_size"),
        )

    def getfont(self):
        """Return the current font."""
        if self._font:
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
            text,
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
        fill, outline = self._palette_fill_outline(fill, outline)
        self._mutate(self._draw.shape, shape, fill, outline)

    def regular_polygon(self, bounding_circle, n_sides, rotation=0, fill=None, outline=None, width=1):
        """Draw a regular polygon. Vertex computation done in Rust."""
        fill, outline = self._palette_fill_outline(fill, outline)
        self._mutate(
            self._draw.regular_polygon,
            bounding_circle,
            n_sides,
            rotation,
            fill,
            outline,
            width,
        )

    def text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
             align="left", direction=None, features=None, language=None,
             stroke_width=0, stroke_fill=None, embedded_color=False, *,
             font_size=None):
        # TODO(pillow-rs#3ptb): preserve ImageText.Text state and arbitrary
        # custom-font getmask2 arguments through the Rust text boundary.
        font, embedded_color = self._prepare_text_font(font, font_size, embedded_color)
        fill = self._palette_ink(fill)
        if stroke_width:
            stroke_fill = self._palette_ink(stroke_fill)
        self._prepare_palette_allocation()
        try:
            if hasattr(font, '_rust_font'):
                self._draw.text(
                    (float(xy[0]), float(xy[1])),
                    text,
                    fill,
                    font._rust_font,
                    direction,
                    features,
                    language,
                    float(stroke_width),
                    anchor,
                    embedded_color,
                    stroke_fill,
                    None,
                )
            elif hasattr(font, 'getmask'):
                mask = font.getmask(text, mode="1" if self._orig_mode == "1" else "L")
                self.bitmap(xy, mask, fill=fill)
            else:
                self._draw.text(
                    (float(xy[0]), float(xy[1])), text, fill, font,
                    direction, features, language, float(stroke_width), anchor,
                    embedded_color, stroke_fill,
                )
        finally:
            self._sync()
