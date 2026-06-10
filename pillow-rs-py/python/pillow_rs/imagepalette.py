"""ImagePalette — color palette for 'P' mode images. Pillow-compatible stub."""
from .image import Image


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
        """Get palette index for a color."""
        raise NotImplementedError("ImagePalette.getcolor")

    def getdata(self):
        """Return palette data as (mode, raw_data)."""
        return (self.mode, bytes(self.palette))

    def save(self, fp):
        """Save palette to file."""
        raise NotImplementedError("ImagePalette.save")

    def tobytes(self):
        """Return palette as bytes."""
        return bytes(self.palette)
