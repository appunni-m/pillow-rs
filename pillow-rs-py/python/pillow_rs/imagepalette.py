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
        if not self.palette:
            raise ValueError("empty palette")
        if isinstance(color, (tuple, list)) and len(color) >= 3:
            r, g, b = color[0], color[1], color[2]
            for i in range(0, len(self.palette), 3):
                if self.palette[i] == r and self.palette[i+1] == g and self.palette[i+2] == b:
                    return i // 3
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
