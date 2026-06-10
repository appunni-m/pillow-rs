"""ImageColor — color string parsing. Pillow-compatible module."""
from . import _core


def getrgb(color: str) -> tuple:
    """Parse a color string and return an RGB tuple."""
    return _core.getrgb(color)


def getcolor(color: str, mode: str):
    """Parse a color string and return a mode-appropriate value."""
    rgb = getrgb(color)
    if mode == "L":
        # ITU-R BT.601 luma
        return int(0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2])
    elif mode == "RGB":
        return rgb
    elif mode == "RGBA":
        return rgb + (255,)
    elif mode == "1":
        luma = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]
        return 255 if luma > 127 else 0
    return rgb
