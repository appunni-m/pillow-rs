"""ImageColor — color string parsing. Pillow-compatible module."""
from functools import lru_cache

from . import _core


def getrgb(color: str) -> tuple:
    """Parse a color string and return an RGB tuple."""
    return _core.getrgb(color)


@lru_cache
def getcolor(color: str, mode: str):
    """Parse a color string and return a mode-appropriate value."""
    return _core.getcolor(color, mode)
