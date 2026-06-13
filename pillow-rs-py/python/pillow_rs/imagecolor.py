"""ImageColor — color string parsing. Pillow-compatible module."""
from . import _core


def getrgb(color: str) -> tuple:
    """Parse a color string and return an RGB tuple."""
    return _core.getrgb(color)


def getcolor(color: str, mode: str):
    """Parse a color string and return a mode-appropriate value."""
    return _core.getcolor(color, mode)
