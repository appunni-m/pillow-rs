"""ImagePalette — color palette for 'P' mode images. Pillow-compatible module."""
from . import _core


class ImagePalette:
    """Color palette for palette-mapped images."""

    def __init__(self, mode="RGB", palette=None, size=0):
        self.mode = mode
        self.rawmode = None
        self._colors = None
        self._palette = bytearray() if palette is None else palette
        self.dirty = None

    @property
    def palette(self):
        return self._palette

    @palette.setter
    def palette(self, value):
        self._palette = value

    def copy(self):
        """Return a copy of the palette."""
        p = ImagePalette(self.mode)
        p.rawmode = self.rawmode
        p._colors = self._colors.copy() if self._colors is not None else None
        p._palette = bytearray(self._palette) if isinstance(self._palette, (bytes, bytearray)) else self._palette
        p.dirty = self.dirty
        return p

    def getcolor(self, color, image=None):
        """Given an rgb tuple, allocate palette entry."""
        palette_bytes, idx = _core.palette_getcolor_validate(self.palette, color, self.mode)
        self.palette = bytearray(palette_bytes)
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
