"""ImageDraw — drawing primitives. Pillow-compatible module."""

from __future__ import annotations

import struct

from ._core import ImageDraw as RustDraw
from ._core import Outline  # public compatibility re-export
from . import imagecolor
from .image import Image


_UNSET = object()


class Draw:
    """Draw lines, rectangles, ellipses, polygons, and text on images."""

    def __init__(self, image: Image, mode: str | None = None):
        # Pillow's ImageDraw constructor calls Image._ensure_mutable(), which
        # loads the image before validating the optional draw mode.
        image.load()
        self._orig_mode = image.mode
        self._draw_mode = image.mode if mode is None else mode
        self._draw = RustDraw(image._rust_image, mode)
        self._image = image
        self._font = None  # current font for text

    def _sync(self):
        """Install Rust pixels without replacing the Python-owned palette."""
        self._image._rust_image = self._draw.image

    def _resolve_draw_ink(self, ink):
        if isinstance(ink, str):
            # Pillow resolves color strings in the drawing context mode before
            # passing the resulting native ink to the drawing backend.
            ink = imagecolor.getcolor(ink, self._draw_mode)
        if self._image.mode != "P":
            return ink
        if not self._image._rust_image.has_attached_palette():
            return ink
        if not isinstance(ink, tuple):
            return ink
        self._prepare_palette_allocation()
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
        outline = self._resolve_draw_ink(outline)
        fill = self._resolve_draw_ink(fill)
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
        # TODO(pillow-rs#3ptb): freeze the None-versus-falsey font_size cases
        # in CPU Draw parity.
        # Pillow creates ImageText.Text, including its default font, before
        # validating embedded-color mode or resolving the fill ink.
        if font is None:
            font = self._getfont(font_size)
        elif not font:
            # ImageText.Text uses `font or ImageFont.load_default()` after
            # ImageDraw's explicit None-only font-size branch.
            from . import imagefont as ImageFont
            font = ImageFont.load_default()
        embedded_color = bool(embedded_color)
        if embedded_color:
            self._draw._validate_embedded_color()
        return font, embedded_color

    def _prepare_text_bbox_font(self, font, font_size, embedded_color):
        # Pillow resolves the default font before checking embedded-color mode
        # or testing stroke_width for truthiness.
        if font is None:
            font = self._getfont(font_size)
        return self._prepare_text_font(font, None, embedded_color)

    def _font_mask_image(self, mask, mode):
        image = getattr(mask, "_image", None)
        if image is not None:
            return image
        if isinstance(mask, Image):
            return mask
        size = getattr(mask, "size", None)
        if size is None:
            raise TypeError("font mask has no image size")
        mask_mode = getattr(mask, "mode", mode)
        return Image.frombytes(mask_mode, size, mask.tobytes())

    def _draw_custom_font_mask(
        self, xy, text, font, fill, mode, direction, features, language,
        stroke_width, anchor, ink, *args, **kwargs
    ):
        x, y = float(xy[0]), float(xy[1])
        x_int, y_int = int(x), int(y)
        start = (x - x_int, y - y_int)
        try:
            mask, offset = font.getmask2(
                text,
                mode,
                direction=direction,
                features=features,
                language=language,
                stroke_width=stroke_width,
                stroke_filled=True,
                anchor=anchor,
                ink=ink,
                start=start,
                *args,
                **kwargs,
            )
            x_int += offset[0]
            y_int += offset[1]
        except AttributeError:
            try:
                mask = font.getmask(
                    text,
                    mode,
                    direction,
                    features,
                    language,
                    stroke_width,
                    anchor,
                    ink,
                    start=start,
                    *args,
                    **kwargs,
                )
            except TypeError:
                mask = font.getmask(text)

        mask_size = getattr(mask, "size", None)
        if mask_size is not None and (mask_size[0] == 0 or mask_size[1] == 0):
            return
        mask_image = self._font_mask_image(mask, mode)
        if mode == "RGBA":
            coverage = mask_image.getchannel("A")
            mask_image.putalpha((ink >> 24) & 0xFF)
            self._draw.text_color_mask(
                (x_int, y_int), mask_image._rust_image, coverage._rust_image
            )
        else:
            self.bitmap((x_int, y_int), mask_image, fill=fill)

    def _custom_font_text_lines(
        self, xy, text, font, anchor, align, spacing, direction, features,
        language, stroke_width, embedded_color, *, coerce_xy=True
    ):
        # TODO(pillow-rs#3ptb): Move line-position calculations to a core
        # layout API that accepts metrics from these observable Python font
        # callbacks; keep only callback invocation and mask adaptation here.
        if anchor is None:
            anchor = "lt" if direction == "ttb" else "la"
        elif len(anchor) != 2:
            raise ValueError("anchor must be a 2 character string")
        newline = "\n" if isinstance(text, str) else b"\n"
        lines = text.split(newline)
        if len(lines) == 1:
            x, y = xy[0], xy[1]
            if coerce_xy:
                x, y = float(x), float(y)
            return [(x, y, anchor, lines[0])]
        if anchor[1] in "tb" and direction != "ttb":
            raise ValueError("anchor not supported for multiline text")

        font_mode = (
            "1" if self._draw_mode in ("1", "P", "I", "F")
            else "RGBA" if embedded_color else "L"
        )
        line_spacing = (
            font.getbbox("A", font_mode, None, features, language, stroke_width)[3]
            + stroke_width
            + spacing
        )
        x, y = xy[0], xy[1]
        if coerce_xy:
            x, y = float(x), float(y)
        if direction == "ttb":
            result = []
            for line in lines:
                result.append((x, y, anchor, line))
                x += line_spacing
            return result

        widths = [
            font.getlength(line, font_mode, direction, features, language)
            for line in lines
        ]
        max_width = max(widths, default=0)
        if anchor[1] == "m":
            y -= (len(lines) - 1) * line_spacing / 2.0
        elif anchor[1] == "d":
            y -= (len(lines) - 1) * line_spacing

        result = []
        for line_index, (line, width) in enumerate(zip(lines, widths)):
            left = x
            width_difference = max_width - width
            if align == "center":
                left += width_difference / 2.0
            elif align == "right":
                left += width_difference
            elif align not in ("left", "justify"):
                raise ValueError('align must be "left", "center", "right" or "justify"')

            if align == "justify" and width_difference != 0 and line_index != len(lines) - 1:
                separator = " " if isinstance(line, str) else b" "
                words = line.split(separator)
                if len(words) > 1:
                    if anchor[0] == "m":
                        left -= max_width / 2.0
                    elif anchor[0] == "r":
                        left -= max_width
                    word_widths = [
                        font.getlength(word, font_mode, direction, features, language)
                        for word in words
                    ]
                    extra = max_width - sum(word_widths)
                    word_x = left
                    for word_index, word in enumerate(words):
                        result.append((word_x, y, "l" + anchor[1], word))
                        word_x += word_widths[word_index] + extra / (len(words) - 1)
                    y += line_spacing
                    continue

            if anchor[0] == "m":
                left -= width_difference / 2.0
            elif anchor[0] == "r":
                left -= width_difference
            result.append((left, y, anchor, line))
            y += line_spacing
        return result

    def _custom_font_text_bbox(
        self, xy, text, font, anchor, spacing, align, direction, features,
        language, stroke_width, embedded_color
    ):
        lines = self._custom_font_text_lines(
            xy, text, font, anchor, align, spacing, direction, features,
            language, stroke_width, embedded_color, coerce_xy=False,
        )
        font_mode = (
            "1" if self._draw_mode in ("1", "P", "I", "F")
            else "RGBA" if embedded_color else "L"
        )
        bounds = None
        for x, y, line_anchor, line_text in lines:
            box = font.getbbox(
                line_text, font_mode, direction, features, language,
                stroke_width, line_anchor,
            )
            line_bounds = (
                box[0] + x,
                box[1] + y,
                box[2] + x,
                box[3] + y,
            )
            if bounds is None:
                bounds = line_bounds
            else:
                bounds = (
                    min(bounds[0], line_bounds[0]),
                    min(bounds[1], line_bounds[1]),
                    max(bounds[2], line_bounds[2]),
                    max(bounds[3], line_bounds[3]),
                )
        assert bounds is not None
        return bounds

    def line(self, xy, fill=None, width: int = 0, joint: str | None = None):
        fill = self._resolve_draw_ink(fill)
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
        fill = self._resolve_draw_ink(fill)
        self._mutate(self._draw.point, xy, fill)

    def arc(self, xy, start, end, fill=None, width=1):
        fill = self._resolve_draw_ink(fill)
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
        fill = self._resolve_draw_ink(fill)
        self._mutate(self._draw.bitmap, xy, bitmap._rust_image, fill)

    def multiline_text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
                       align="left", direction=None, features=None, language=None,
                       stroke_width=0, stroke_fill=None, embedded_color=False, *,
                       font_size=None):
        """Draw multiple lines of text through the shared text layout path."""
        return self.text(
            xy, text, fill=fill, font=font, anchor=anchor, spacing=spacing,
            align=align, direction=direction, features=features,
            language=language, stroke_width=stroke_width,
            stroke_fill=stroke_fill, embedded_color=embedded_color,
            font_size=font_size,
        )

    def textbbox(self, xy, text, font=None, **kwargs):
        embedded_color = kwargs.get("embedded_color", False)
        font_size = kwargs.get("font_size")
        font, embedded_color = self._prepare_text_bbox_font(
            font, font_size, embedded_color
        )
        stroke_width = kwargs.get("stroke_width", 0)
        if not stroke_width:
            stroke_width = 0
        if not hasattr(font, "_rust_font"):
            return self._custom_font_text_bbox(
                xy,
                text,
                font,
                kwargs.get("anchor"),
                kwargs.get("spacing", 4),
                kwargs.get("align", "left"),
                kwargs.get("direction"),
                kwargs.get("features"),
                kwargs.get("language"),
                stroke_width,
                embedded_color,
            )
        font = font._rust_font
        return self._draw.textbbox(
            xy,
            text,
            font,
            kwargs.get("direction"),
            kwargs.get("features"),
            kwargs.get("language"),
            stroke_width,
            kwargs.get("anchor"),
            embedded_color,
            None,
        )

    def textlength(self, text, font=None, **kwargs):
        # TODO(pillow-rs#3ptb): verify custom text/font callbacks and faults in CPU parity.
        if font is None:
            font = self._getfont(kwargs.get("font_size"))
        from .imagetext import Text as ImageText

        image_text = ImageText(
            text,
            font,
            self._draw_mode,
            direction=kwargs.get("direction"),
            features=kwargs.get("features"),
            language=kwargs.get("language"),
        )
        if kwargs.get("embedded_color", False):
            image_text.embed_color()
        return image_text.get_length()

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
        font, embedded_color = self._prepare_text_bbox_font(
            font, font_size, embedded_color
        )
        if not stroke_width:
            stroke_width = 0
        if not hasattr(font, "_rust_font"):
            return self._custom_font_text_bbox(
                xy, text, font, anchor, spacing, align, direction,
                features, language, stroke_width, embedded_color,
            )
        font = font._rust_font
        return self._draw.multiline_textbbox(
            xy,
            text,
            font,
            spacing,
            align,
            direction,
            features,
            language,
            stroke_width,
            anchor,
            embedded_color,
            None,
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
             stroke_width=0, stroke_fill=None, embedded_color=False, *args,
             font_size=_UNSET, **kwargs):
        from .imagetext import Text as ImageText
        native_text_input = type(text) in (str, bytes)
        if isinstance(text, ImageText):
            native_text_input = False
            image_text = text
        else:
            requested_font_size = None if font_size is _UNSET else font_size
            if font is None:
                font = self._getfont(requested_font_size)
            image_text = ImageText(
                text, font, self._draw_mode, spacing, direction, features, language
            )
        if embedded_color:
            image_text.embed_color()
        if stroke_width:
            image_text.stroke(stroke_width, stroke_fill)
        text = image_text.text
        font = image_text.font
        spacing = image_text.spacing
        direction = image_text.direction
        features = image_text.features
        language = image_text.language
        embedded_color = image_text.embedded_color
        stroke_width = image_text.stroke_width
        stroke_fill = image_text.stroke_fill
        mask_kwargs = dict(kwargs)
        if font_size is not _UNSET:
            mask_kwargs["font_size"] = font_size
        try:
            self._draw_text_lines(
                xy,
                image_text,
                font,
                fill,
                stroke_width,
                stroke_fill,
                anchor,
                align,
                direction,
                features,
                language,
                embedded_color,
                args,
                mask_kwargs,
                native_text_input,
            )
        finally:
            self._sync()

    def _draw_text_lines(
        self, xy, image_text, font, fill, stroke_width, stroke_fill, anchor, align,
        direction, features, language, embedded_color, args, mask_kwargs,
        native_text_input,
    ):
        text = image_text.text
        fill = self._resolve_draw_ink(fill)
        # Normalize once before layout; per-line draws receive native numeric
        # inks, so palette callbacks cannot run after Text._split.
        fill_ink = self._draw.text_ink(fill)
        fill = self._text_fill_from_ink(fill_ink)

        stroke_ink = None
        if stroke_width:
            if stroke_fill is None:
                stroke_ink = fill_ink
                stroke_fill = fill
            else:
                stroke_fill = self._resolve_draw_ink(stroke_fill)
                stroke_ink = self._draw.text_ink(stroke_fill)
                stroke_fill = self._text_fill_from_ink(stroke_ink)

        self._prepare_palette_allocation()
        if self._can_delegate_text_layout(
            xy, image_text, font, stroke_width, anchor, align,
            direction, features, language, embedded_color, args, mask_kwargs,
            native_text_input,
        ):
            # The core owns multiline metrics, alignment, and line placement.
            # Python has already resolved inks and palette allocation in the
            # same order as ImageText._split; custom host objects stay on the
            # callback-preserving path below.
            self._draw.multiline_text(
                xy,
                text,
                fill,
                font._rust_font,
                image_text.spacing,
                direction,
                features,
                language,
                float(stroke_width),
                anchor,
                embedded_color,
                None,
                align,
                stroke_fill,
            )
            return

        # Pillow resolves ink before Text._split asks the font for metrics.
        text_lines = image_text._split(xy, anchor, align)
        if hasattr(font, '_rust_font'):
            for line_x, line_y, line_anchor, line_text in text_lines:
                self._draw.text(
                    (line_x, line_y),
                    line_text,
                    fill,
                    font._rust_font,
                    direction,
                    features,
                    language,
                    float(stroke_width),
                    line_anchor,
                    embedded_color,
                    stroke_fill,
                    None,
                    has_args=bool(args),
                    has_kwargs=bool(mask_kwargs),
                )
        elif hasattr(font, 'getmask') or hasattr(font, 'getmask2'):
            stroke_color = stroke_fill if stroke_fill is not None else fill
            for line_x, line_y, line_anchor, line_text in text_lines:
                if stroke_ink is not None:
                    stroke_mode = "1" if self._draw_mode in ("1", "P", "I", "F") else "L"
                    self._draw_custom_font_mask(
                        (line_x, line_y), line_text, font, stroke_color,
                        stroke_mode, direction, features, language,
                        stroke_width, line_anchor, stroke_ink,
                        *args, **mask_kwargs,
                    )
                    if stroke_ink == fill_ink:
                        continue
                fill_mode = (
                    "RGBA" if embedded_color
                    else "1" if self._draw_mode in ("1", "P", "I", "F") else "L"
                )
                self._draw_custom_font_mask(
                    (line_x, line_y), line_text, font, fill, fill_mode,
                    direction, features, language, 0, line_anchor, fill_ink,
                    *args, **mask_kwargs,
                )
        else:
            self._draw.text(
                (float(xy[0]), float(xy[1])), text, fill, font,
                direction, features, language, float(stroke_width), anchor,
                embedded_color, stroke_fill,
                has_args=bool(args),
                has_kwargs=bool(mask_kwargs),
            )

    def _can_delegate_text_layout(
        self, xy, image_text, font, stroke_width, anchor, align,
        direction, features, language, embedded_color, args, mask_kwargs,
        native_text_input,
    ):
        from . import imagefont as ImageFont
        from .imagetext import Text as ImageText

        spacing = image_text.spacing
        return (
            native_text_input
            and type(image_text) is ImageText
            and type(image_text.text) in (str, bytes)
            and type(image_text.mode) is str
            and image_text.mode == self._draw_mode
            and type(font) is ImageFont.FreeTypeFont
            and type(spacing) is int
            and -(2**31) <= spacing < 2**31
            and type(stroke_width) is int
            and -(2**24) <= stroke_width <= 2**24
            and type(embedded_color) is bool
            and (anchor is None or type(anchor) is str)
            and type(align) is str
            and direction is None
            and features is None
            and language is None
            and type(xy) is tuple
            and len(xy) == 2
            and all(
                type(value) is float
                or (type(value) is int and abs(value) <= 2**53)
                for value in xy
            )
            and not args
            and not mask_kwargs
            and "_split" not in image_text.__dict__
        )

    def _text_fill_from_ink(self, ink):
        if self._draw_mode == "F":
            return struct.unpack("<f", struct.pack("<i", ink))[0]
        return ink
