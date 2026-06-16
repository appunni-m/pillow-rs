"""ImagePalette — color palette for 'P' mode images. Pillow-compatible module."""
from . import _core


class ImagePalette:
    """Color palette for palette-mapped images."""

    def __init__(self, mode="RGB"):
        self.mode = mode
        self.palette = []

    def copy(self):
        """Return a copy of the palette."""
        p = ImagePalette(self.mode)
        p.palette = list(self.palette)
        return p

    def getcolor(self, color, image=None):
        """Given an rgb tuple, allocate palette entry."""
        if not isinstance(color, (tuple, list)):
            raise ValueError(f"unknown color specifier: {repr(color)}")

        color = tuple(color)
        if self.mode == "RGB" and len(color) >= 4 and color[3] != 255:
            raise ValueError("cannot add non-opaque RGBA color to RGB palette")
        if self.mode == "RGBA" and len(color) == 3:
            color = color + (255,)

        r, g, b = color[0], color[1], color[2]
        a = color[3] if self.mode == "RGBA" and len(color) >= 4 else 255

        return _core.palette_getcolor_append(self.palette, r, g, b, a, self.mode)

    def getdata(self):
        """Return palette data as (mode, raw_data)."""
        return (self.mode, bytes(self.palette))

    def save(self, fp):
        """Save palette to text file."""
        if isinstance(fp, str):
            with open(fp, "w") as f:
                f.write(_core.palette_to_text(self.palette, self.mode))
        else:
            fp.write(_core.palette_to_text(self.palette, self.mode))

    def tobytes(self):
        """Return palette as bytes."""
        return bytes(self.palette)
