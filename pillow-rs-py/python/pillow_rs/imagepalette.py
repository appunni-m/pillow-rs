"""ImagePalette — color palette for 'P' mode images. Pillow-compatible module."""
import array
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
        self._colors = None
        self._palette = value

    @property
    def colors(self):
        """Return the first palette index for each stored color tuple."""
        if self._colors is None:
            mode_len = len(self.mode)
            colors = {}
            for offset in range(0, len(self.palette), mode_len):
                color = tuple(self.palette[offset : offset + mode_len])
                if color not in colors:
                    colors[color] = offset // mode_len
            self._colors = colors
        return self._colors

    @colors.setter
    def colors(self, values):
        self._colors = values

    def copy(self):
        """Return a copy of the palette."""
        p = ImagePalette()
        p.mode = self.mode
        p.rawmode = self.rawmode
        if self.palette is not None:
            p.palette = self.palette[:]
        p.dirty = self.dirty
        return p

    def getcolor(self, color, image=None):
        """Given an rgb tuple, allocate palette entry."""
        if self.rawmode:
            raise ValueError("palette contains raw palette data")
        if not isinstance(color, tuple):
            raise ValueError(f"unknown color specifier: {color!r}")
        if self.mode == "RGB" and len(color) == 4:
            if color[3] != 255:
                raise ValueError("cannot add non-opaque RGBA color to RGB palette")
            color = color[:3]
        elif self.mode == "RGBA" and len(color) == 3:
            color += (255,)
        try:
            return self.colors[color]
        except KeyError as error:
            index = self._new_color_index(image, error)
            self.colors[color] = index
            mode_len = len(self.mode)
            if index * mode_len < len(self.palette):
                self._palette = (
                    self._palette[: index * mode_len]
                    + bytes(color)
                    + self._palette[index * mode_len + mode_len :]
                )
            else:
                self._palette += bytes(color)
            self.dirty = 1
            return index

    def _new_color_index(self, image=None, error=None):
        if not isinstance(self.palette, bytearray):
            self._palette = bytearray(self.palette)
        index = len(self.palette) // len(self.mode)
        special_colors = ()
        if image:
            special_colors = (
                image.info.get("background"),
                image.info.get("transparency"),
            )
            while index in special_colors:
                index += 1
        if index >= 256:
            if image:
                for candidate, count in reversed(list(enumerate(image.histogram()))):
                    if count == 0 and candidate not in special_colors:
                        index = candidate
                        break
            if index >= 256:
                raise ValueError("cannot allocate more than 256 colors") from error
        return index

    def getdata(self):
        """Return palette data as (mode, raw_data)."""
        if self.rawmode:
            return (self.rawmode, self.palette)
        return (self.mode, self.tobytes())

    def tobytes(self):
        """Return palette bytes unless the buffer still has a raw encoding."""
        if self.rawmode:
            raise ValueError("palette contains raw palette data")
        if isinstance(self.palette, bytes):
            return self.palette
        return array.array("B", self.palette).tobytes()

    # Pillow keeps the legacy spelling as an alias to the rawmode-aware method.
    tostring = tobytes

    def save(self, fp):
        """Save palette to text file."""
        if self.rawmode:
            raise ValueError("palette contains raw palette data")
        if isinstance(fp, str):
            _core.palette_save_to_file(self.palette, self.mode, fp)
        else:
            fp.write(_core.palette_to_text(self.palette, self.mode))
