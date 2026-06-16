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
        palette_bytes, idx = _core.palette_getcolor_validate(self.palette, list(color), self.mode)
        self.palette = list(palette_bytes)
        return idx

    def getdata(self):
        """Return palette data as (mode, raw_data)."""
        return (self.mode, bytes(self.palette))

    def save(self, fp):
        """Save palette to text file."""
        if isinstance(fp, str):
            _core.palette_save_to_file(self.palette, self.mode, fp)
        else:
            fp.write(_core.palette_to_text(self.palette, self.mode))

    def tobytes(self):
        """Return palette as bytes."""
        return bytes(self.palette)
