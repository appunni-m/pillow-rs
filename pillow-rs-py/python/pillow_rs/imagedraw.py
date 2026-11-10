"""ImageDraw — drawing primitives. Pillow-compatible module."""

from __future__ import annotations

import math
from collections.abc import Sequence as _Sequence

from ._core import ImageDraw as RustDraw
from ._core import Outline  # public compatibility re-export
from . import imagecolor
from . import image as Image
from .image import PixelAccess

_Ink = float | tuple[int, ...] | str
Coords = _Sequence[float] | _Sequence[_Sequence[float]]


def _invoke_text_mask_callback(method, positionals, keywords, extra_keywords):
    """Call a custom font method with Pillow's two keyword mappings."""
    return method(*positionals, **keywords, **extra_keywords)


class ImageDraw:
    """Draw lines, rectangles, ellipses, polygons, and text on images."""

    font = None

    def __init__(self, im: Image.Image, mode: str | None = None) -> None:
        # Pillow's ImageDraw constructor calls Image._ensure_mutable(), which
        # loads the image before validating the optional draw mode.
        im.load()
        self.mode = im.mode if mode is None else mode
        self.draw = RustDraw(im._rust_image, mode)
        self.im = im._rust_image
        self.palette = im.palette if self.mode == "P" else None
        self._image = im
        self.fontmode = "1" if self.mode in ("1", "P", "I", "F") else "L"
        self.ink = self._draw.draw_ink(1 if self.mode in ("I", "F") else -1)
        self.fill = False

    @property
    def draw(self):
        """The native drawing context associated with this image."""
        return self._draw

    @draw.setter
    def draw(self, value):
        self._draw = value

    def _sync(self):
        """Install Rust pixels without replacing the Python-owned palette."""
        self._image._rust_image = self._draw.image
        self.im = self._image._rust_image

    def _resolve_draw_ink(self, ink):
        if isinstance(ink, str):
            # Pillow resolves color strings in the drawing context mode before
            # passing the resulting native ink to the drawing backend.
            ink = imagecolor.getcolor(ink, self.mode)
        palette = self.palette
        if not palette or not isinstance(ink, tuple):
            return ink
        previous_palette = palette.palette[:]
        index = palette.getcolor(ink, self._image)
        if (
            palette is self._image.palette
            and self._image.mode == "P"
            and self._image._rust_image.has_attached_palette()
            and palette.palette != previous_palette
        ):
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

    def _getink(self, ink, fill=None):
        """Resolve Pillow's default ink state and explicit colors."""
        if ink is None and fill is None:
            if self.fill:
                return None, self.ink
            return self.ink, None
        result_ink = (
            self._draw.draw_ink(self._resolve_draw_ink(ink))
            if ink is not None
            else None
        )
        result_fill = (
            self._draw.draw_ink(self._resolve_draw_ink(fill))
            if fill is not None
            else None
        )
        return result_ink, result_fill

    def _palette_fill_outline(self, fill, outline):
        outline_ink, fill_ink = self._getink(outline, fill)
        return fill_ink, outline_ink

    def _draw_fill_outline(self, operation, xy, fill, outline, width, *geometry):
        if fill is not None:
            self._mutate(operation, xy, *geometry, fill, None, 1)
        if outline is not None and outline != fill and width != 0:
            self._mutate(operation, xy, *geometry, None, outline, width)

    def _run_rounded_rectangle_program(
        self,
        xy,
        diameter,
        radius,
        fill,
        outline,
        width,
        corners,
        full_x,
        full_y,
    ):
        program = self._draw._rounded_rectangle_program(
            xy, diameter, radius, fill, outline, full_x, full_y
        )
        step = program.next()
        while step[0] != "done":
            if step[0] == "corner":
                step = program.next(corner=bool(corners[step[1]]))
            elif step[0] == "outline_width":
                step = program.next(outline_width=width)
            elif step[0] == "width":
                step = program.next(width=width)
            elif step[0] == "border":
                _, kind, bounds, corner_radius, color = step
                bounds = self._draw._rounded_rectangle_border_bounds(
                    kind, bounds, corner_radius, width, corners
                )
                self._mutate(
                    self._draw._rounded_rectangle_step,
                    ("draw", "rectangle", bounds, color),
                )
                step = program.next()
            else:
                self._mutate(self._draw._rounded_rectangle_step, step)
                step = program.next()

    def _mutate(self, operation, *args, **kwargs):
        """Sync mutations even when Pillow reports a later input error."""
        try:
            return operation(*args, **kwargs)
        finally:
            self._sync()

    def _getfont(self, font_size=None):
        if font_size is not None:
            from . import imagefont as ImageFont
            return ImageFont.load_default(font_size)
        return self.getfont()

    def _prepare_text_font(self, font, font_size, embedded_color):
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

    def line(
        self,
        xy: Coords,
        fill: _Ink | None = None,
        width: int = 0,
        joint: str | None = None,
    ) -> None:
        """Draw a line, or a connected sequence of line segments."""
        ink, _ = self._getink(fill)
        if ink is not None:
            self._mutate(self._draw._line_base, xy, ink, width)
            if joint == "curve" and width > 4:
                # Pillow inspects and groups the original coordinates only
                # after its native line draw has completed.
                if isinstance(xy[0], (list, tuple)):
                    points = xy
                else:
                    points = [tuple(xy[i : i + 2]) for i in range(0, len(xy), 2)]
                for index in range(1, len(points) - 1):
                    point = points[index]
                    angles = [
                        math.degrees(
                            math.atan2(end[0] - start[0], start[1] - end[1])
                        )
                        % 360
                        for start, end in (
                            (points[index - 1], point),
                            (point, points[index + 1]),
                        )
                    ]
                    if angles[0] == angles[1]:
                        continue
                    # Pillow calls the public methods here. Keep dispatch on
                    # `self` so subclasses can override either operation, and
                    # pass the original fill so each call resolves it again.
                    plan = self._draw._line_curve_joint_plan(angles)
                    if plan is None:
                        continue
                    start, end = plan._angles()
                    coords = [
                        (
                            point[0] - width / 2 + 1,
                            point[1] - width / 2 + 1,
                        ),
                        (
                            point[0] + width / 2 - 1,
                            point[1] + width / 2 - 1,
                        ),
                    ]
                    self.pieslice(coords, start, end, fill)
                    if width > 8:
                        def coord_at_angle(coord, angle):
                            x, y = coord
                            angle -= 90
                            distance = width / 2 - 1
                            return tuple(
                                p
                                + (
                                    math.floor(p_delta)
                                    if p_delta > 0
                                    else math.ceil(p_delta)
                                )
                                for p, p_delta in (
                                    (x, distance * math.cos(math.radians(angle))),
                                    (y, distance * math.sin(math.radians(angle))),
                                )
                            )

                        gap_coords = [
                            coord_at_angle(point, plan._gap_angle(True)),
                            point,
                            coord_at_angle(point, plan._gap_angle(False)),
                        ]
                        self.line(gap_coords, fill, width=3)

    def rectangle(self, xy, fill=None, outline=None, width: int = 1):
        fill, outline = self._palette_fill_outline(fill, outline)
        if fill is not None or outline is not None:
            self._draw_fill_outline(self._draw.rectangle, xy, fill, outline, width)

    def ellipse(self, xy, fill=None, outline=None, width: int = 1):
        fill, outline = self._palette_fill_outline(fill, outline)
        if fill is not None or outline is not None:
            self._draw_fill_outline(self._draw.ellipse, xy, fill, outline, width)

    def polygon(self, xy, fill=None, outline=None, width: int = 1):
        fill, outline = self._palette_fill_outline(fill, outline)
        if fill is not None:
            self._mutate(self._draw.polygon, xy, fill, None, 1)
        if outline is None or outline == fill or width == 0:
            return
        if width == 1:
            self._mutate(self._draw.polygon, xy, None, outline, 1)
        elif self.im is not None:
            # Pillow prepares a mode-1 fill mask before applying its transformed
            # signed C-int outline width. Rust builds the same binary mask in
            # the selected polygon strategy and keeps the Python call order.
            self._getink(1)
            self._draw._prepare_polygon_mask_input(xy)
            outline_width = width * 2 - 1
            self._mutate(self._draw._polygon_outline, xy, outline, outline_width)

    def point(self, xy, fill=None):
        ink, _ = self._getink(fill)
        if ink is not None:
            self._mutate(self._draw.point, xy, ink)

    def arc(self, xy, start, end, fill=None, width=1):
        ink, _ = self._getink(fill)
        if ink is not None:
            self._mutate(self._draw.arc, xy, start, end, ink, width)

    def chord(self, xy, start, end, fill=None, outline=None, width=1):
        fill, outline = self._palette_fill_outline(fill, outline)
        if fill is not None or outline is not None:
            self._draw_fill_outline(
                self._draw.chord, xy, fill, outline, width, start, end
            )

    def pieslice(self, xy, start, end, fill=None, outline=None, width=1):
        fill, outline = self._palette_fill_outline(fill, outline)
        if fill is not None or outline is not None:
            self._draw_fill_outline(
                self._draw.pieslice, xy, fill, outline, width, start, end
            )

    def circle(self, xy, radius, fill=None, outline=None, width=1):
        ellipse_xy = (
            xy[0] - radius,
            xy[1] - radius,
            xy[0] + radius,
            xy[1] + radius,
        )
        self.ellipse(ellipse_xy, fill, outline, width)

    def rounded_rectangle(
        self,
        xy: Coords,
        radius: float = 0,
        fill: _Ink | None = None,
        outline: _Ink | None = None,
        width: int = 1,
        *,
        corners: tuple[bool, bool, bool, bool] | None = None,
    ) -> None:
        # Keep Pillow's normalized sequence unpacking here; Rust owns the
        # ordered numeric geometry. Public ellipse/rectangle override dispatch
        # stays here.
        original_xy = xy
        (x0, y0), (x1, y1) = self._normalize_coords(xy)
        if corners is None:
            corners = (True, True, True, True)
        geometry = self._draw._prepare_rounded_rectangle(
            (x0, y0, x1, y1), radius, corners
        )
        route, xy, diameter, _corner_radius, full_x, full_y = geometry
        if route == "ellipse":
            return self.ellipse(original_xy, fill, outline, width)
        if route == "rectangle":
            return self.rectangle(original_xy, fill, outline, width)

        fill, outline = self._palette_fill_outline(fill, outline)
        if fill is not None or outline is not None:
            self._run_rounded_rectangle_program(
                xy,
                diameter,
                _corner_radius,
                fill,
                outline,
                width,
                corners,
                full_x,
                full_y,
            )

    def _normalize_coords(self, xy: Coords):
        """Normalize flat coordinates into pairs as Pillow's draw wrapper does."""
        if isinstance(xy[0], (list, tuple)):
            return xy
        return [tuple(xy[index : index + 2]) for index in range(0, len(xy), 2)]

    def bitmap(self, xy, bitmap, fill=None):
        """Draw a bitmap. Pixel iteration done in Rust."""
        bitmap.load()
        ink, fill_ink = self._getink(fill)
        if ink is None:
            ink = fill_ink
        if ink is not None:
            self._mutate(self._draw.bitmap, xy, bitmap._rust_image, ink)

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

    def textbbox(
        self, xy, text, font=None, anchor=None, spacing=4, align="left",
        direction=None, features=None, language=None, stroke_width=0,
        embedded_color=False, *, font_size=None,
    ):
        from .imagetext import Text as ImageText

        font, embedded_color = self._prepare_text_bbox_font(
            font, font_size, embedded_color
        )
        image_text = ImageText(
            text,
            font,
            self.mode,
            spacing,
            direction,
            features,
            language,
        )
        if embedded_color:
            image_text.embed_color()
        if stroke_width:
            image_text.stroke(stroke_width)
        return image_text.get_bbox(xy, anchor, align)

    def textlength(
        self, text, font=None, direction=None, features=None, language=None,
        embedded_color=False, *, font_size=None,
    ):
        if font is None:
            font = self._getfont(font_size)
        from .imagetext import Text as ImageText

        image_text = ImageText(
            text,
            font,
            self.mode,
            direction=direction,
            features=features,
            language=language,
        )
        if embedded_color:
            image_text.embed_color()
        return image_text.get_length()

    def getfont(self):
        """Return the current font."""
        if self.font:
            return self.font
        from . import imagefont as ImageFont
        self.font = ImageFont.load_default()
        return self.font

    def multiline_textbbox(self, xy, text, font=None, anchor=None, spacing=4, align='left',
                           direction=None, features=None, language=None, stroke_width=0,
                           embedded_color=False, *, font_size=None):
        return self.textbbox(
            xy,
            text,
            font,
            anchor,
            spacing,
            align,
            direction,
            features,
            language,
            stroke_width,
            embedded_color,
            font_size=font_size,
        )

    def shape(self, shape, fill=None, outline=None):
        """Draw a shape using Rust's Pillow-compatible outline semantics."""
        # Pillow closes the outline before resolving either color, so custom
        # close callbacks and color conversion faults stay in observable order.
        shape.close()
        fill, outline = self._palette_fill_outline(fill, outline)
        if fill is not None or outline is not None:
            self._mutate(self._draw.shape, shape, fill, outline)

    def regular_polygon(self, bounding_circle, n_sides, rotation=0, fill=None, outline=None, width=1):
        """Draw a regular polygon. Vertex computation done in Rust."""
        points = self._draw.regular_polygon_vertices(
            bounding_circle, n_sides, rotation
        )
        return self.polygon(points, fill, outline, width)

    def text(self, xy, text, fill=None, font=None, anchor=None, spacing=4,
             align="left", direction=None, features=None, language=None,
             stroke_width=0, stroke_fill=None, embedded_color=False, *args,
             **kwargs):
        from .imagetext import Text as ImageText
        native_text_input = type(text) in (str, bytes)
        if isinstance(text, ImageText):
            native_text_input = False
            image_text = text
        else:
            if font is None:
                font = self._getfont(kwargs.get("font_size"))
            image_text = ImageText(
                text, font, self.mode, spacing, direction, features, language
            )
        if embedded_color:
            image_text.embed_color()
        if stroke_width:
            image_text.stroke(stroke_width, stroke_fill)
        mask_kwargs = dict(kwargs)
        try:
            self._draw_text_lines(
                xy,
                image_text,
                fill,
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
        self, xy, image_text, fill, anchor, align, direction, features,
        language, embedded_color, args, mask_kwargs, native_text_input,
    ):
        # Pillow's nested text getink chooses the configured ink or fill
        # state, then sends the packed value to font callbacks.
        fill_ink, fill_fallback = self._getink(fill)
        if fill_ink is None:
            assert fill_fallback is not None
            fill_ink = fill_fallback
        if fill_ink is None:
            return
        fill = self._draw.text_fill_from_ink(fill_ink)

        stroke_width = image_text.stroke_width
        stroke_ink = None
        stroke_fill = None
        if stroke_width:
            if image_text.stroke_fill is None:
                stroke_ink = fill_ink
                stroke_fill = fill
            else:
                stroke_fill = image_text.stroke_fill
                stroke_ink, stroke_fallback = self._getink(stroke_fill)
                if stroke_ink is None:
                    assert stroke_fallback is not None
                    stroke_ink = stroke_fallback
                stroke_fill = self._draw.text_fill_from_ink(stroke_ink)

        text = image_text.text if native_text_input else None
        font = image_text.font if native_text_input else None
        route_stroke_width = stroke_width if native_text_input else None
        host_helpers = lambda: (Image.Image, getattr, isinstance, int, math.modf)
        host_float = lambda value: float(value)
        self._draw._draw_text_lines(
            self,
            host_helpers,
            host_float,
            xy,
            image_text,
            text,
            font,
            fill,
            stroke_fill,
            fill_ink,
            stroke_ink,
            route_stroke_width,
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


def Draw(im: Image.Image, mode: str | None = None) -> ImageDraw:
    """Create a drawing context, allowing images to override ``getdraw``."""
    try:
        return getattr(im, "getdraw")(mode)
    except AttributeError:
        return ImageDraw(im, mode)


def getdraw(im: Image.Image | None = None):
    """Return an experimental WCK drawing context and its resource module."""
    from . import imagedraw2 as ImageDraw2

    draw = ImageDraw2.Draw(im) if im is not None else None
    return draw, ImageDraw2


def floodfill(
    image: Image.Image,
    xy: tuple[int, int],
    value: float | tuple[int, ...],
    border: float | tuple[int, ...] | None = None,
    thresh: float = 0,
) -> None:
    """Flood-fill a four-connected region through Pillow's pixel-access API."""
    pixel = image.load()
    assert pixel is not None
    x, y = xy
    native_image = None
    if type(pixel) is PixelAccess:
        try:
            native_image = object.__getattribute__(pixel, "_image")
        except AttributeError:
            pass
    return RustDraw._floodfill(pixel, x, y, value, border, thresh, native_image)
