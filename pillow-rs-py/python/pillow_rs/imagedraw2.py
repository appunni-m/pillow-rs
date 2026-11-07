"""Experimental WCK-style drawing interface backed by :mod:`imagedraw`."""

from __future__ import annotations

from collections.abc import Sequence
from os import PathLike
import sys
from typing import Any, AnyStr, BinaryIO

from . import image as Image
from . import imagecolor as ImageColor
from . import imagefont as ImageFont
from . import imagepath as ImagePath
from . import imagedraw as ImageDraw

Coords = Sequence[float] | Sequence[Sequence[float]]
StrOrBytesPath = str | bytes | PathLike[str] | PathLike[bytes]


class Pen:
    """Stores an outline color and width."""

    def __init__(self, color: str, width: int = 1, opacity: int = 255) -> None:
        self.color = ImageColor.getrgb(color)
        self.width = width


class Brush:
    """Stores a fill color."""

    def __init__(self, color: str, opacity: int = 255) -> None:
        self.color = ImageColor.getrgb(color)


class Font:
    """Stores a TrueType font and color."""

    def __init__(
        self,
        color: str,
        file: StrOrBytesPath | BinaryIO,
        size: float = 12,
    ) -> None:
        # FIXME: add support for bitmap fonts
        self.color = ImageColor.getrgb(color)
        self.font = ImageFont.truetype(file, size)


def _transform_points(xy: Coords, transform: tuple[float, ...]) -> ImagePath.Path:
    """Convert and transform coordinates through Pillow's path contract."""
    path = ImagePath.Path(xy)
    path.transform(transform)
    return path


def _new_image(mode: str, size: tuple[int, int] | list[int], color) -> Image.Image:
    public_image = sys.modules.get("PIL.Image")
    if public_image is not None:
        return public_image.new(mode, size, color)
    return Image.Image.new(mode, size, color)


class Draw:
    """(Experimental) WCK-style drawing interface."""

    def __init__(
        self,
        image: Image.Image | str,
        size: tuple[int, int] | list[int] | None = None,
        color: float | tuple[float, ...] | str | None = None,
    ) -> None:
        if isinstance(image, str):
            if size is None:
                msg = "If image argument is mode string, size must be a list or tuple"
                raise ValueError(msg)
            image = _new_image(image, size, color)
        self.draw = ImageDraw.Draw(image)
        self.image = image
        self.transform: tuple[float, float, float, float, float, float] | None = None

    def flush(self) -> Image.Image:
        return self.image

    def render(
        self,
        op: str,
        xy: Coords,
        pen: Pen | Brush | None,
        brush: Brush | Pen | None = None,
        **kwargs: Any,
    ) -> None:
        outline = fill = None
        width = 1
        if isinstance(pen, Pen):
            outline = pen.color
            width = pen.width
        elif isinstance(brush, Pen):
            outline = brush.color
            width = brush.width
        if isinstance(brush, Brush):
            fill = brush.color
        elif isinstance(pen, Brush):
            fill = pen.color
        if self.transform:
            xy = _transform_points(xy, self.transform)
        if op in ("arc", "line"):
            kwargs.setdefault("fill", outline)
        else:
            kwargs.setdefault("fill", fill)
            kwargs.setdefault("outline", outline)
        if op == "line":
            kwargs.setdefault("width", width)
        getattr(self.draw, op)(xy, **kwargs)

    def settransform(self, offset: tuple[float, float]) -> None:
        xoffset, yoffset = offset
        self.transform = (1, 0, xoffset, 0, 1, yoffset)

    def arc(
        self, xy: Coords, pen: Pen | Brush | None, start: float, end: float, *options: Any
    ) -> None:
        """Draw an arc inside the given bounding box."""
        self.render("arc", xy, pen, *options, start=start, end=end)

    def chord(
        self, xy: Coords, pen: Pen | Brush | None, start: float, end: float, *options: Any
    ) -> None:
        """Draw a chord inside the given bounding box."""
        self.render("chord", xy, pen, *options, start=start, end=end)

    def ellipse(self, xy: Coords, pen: Pen | Brush | None, *options: Any) -> None:
        """Draw an ellipse inside the given bounding box."""
        self.render("ellipse", xy, pen, *options)

    def line(self, xy: Coords, pen: Pen | Brush | None, *options: Any) -> None:
        """Draw a line between the given coordinates."""
        self.render("line", xy, pen, *options)

    def pieslice(
        self, xy: Coords, pen: Pen | Brush | None, start: float, end: float, *options: Any
    ) -> None:
        """Draw a pieslice inside the given bounding box."""
        self.render("pieslice", xy, pen, *options, start=start, end=end)

    def polygon(self, xy: Coords, pen: Pen | Brush | None, *options: Any) -> None:
        """Draw a polygon."""
        self.render("polygon", xy, pen, *options)

    def rectangle(self, xy: Coords, pen: Pen | Brush | None, *options: Any) -> None:
        """Draw a rectangle."""
        self.render("rectangle", xy, pen, *options)

    def text(self, xy: tuple[float, float], text: AnyStr, font: Font) -> None:
        """Draw text at the given position."""
        if self.transform:
            xy = _transform_points(xy, self.transform)
        self.draw.text(xy, text, font=font.font, fill=font.color)

    def textbbox(
        self, xy: tuple[float, float], text: AnyStr, font: Font
    ) -> tuple[float, float, float, float]:
        """Return the bounding box of text at the given position."""
        if self.transform:
            xy = _transform_points(xy, self.transform)
        return self.draw.textbbox(xy, text, font=font.font)

    def textlength(self, text: AnyStr, font: Font) -> float:
        """Return the advance width of text."""
        return self.draw.textlength(text, font=font.font)
