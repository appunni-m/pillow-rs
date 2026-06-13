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
        """Get palette index for a color. Thin wrapper over Rust search."""
        if not self.palette:
            raise ValueError("empty palette")
        if isinstance(color, (tuple, list)) and len(color) >= 3:
            r, g, b = color[0], color[1], color[2]
            idx = _core.palette_search(self.palette, r, g, b)
            if idx is not None:
                return idx
        return 0

    def getdata(self):
        """Return palette data as (mode, raw_data)."""
        return (self.mode, bytes(self.palette))

    def save(self, fp):
        """Save palette to file."""
        with open(fp, 'w') if isinstance(fp, str) else fp as f:
            f.write(str(self.palette))

    def tobytes(self):
        """Return palette as bytes."""
        return bytes(self.palette)
