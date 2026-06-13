"""ImageFont — font loading and text rendering via fontdue (pure Rust FreeType equivalent)."""
from ._core import ImageFont as RustFont
from .image import Image


class ImageFont:
    """Default bitmap font (fallback)."""

    def getbbox(self, text, *args, **kwargs):
        raise NotImplementedError("ImageFont.getbbox: use ImageFont.truetype() instead")

    def getlength(self, text, *args, **kwargs):
        raise NotImplementedError("ImageFont.getlength: use ImageFont.truetype() instead")

    def getmask(self, text, mode="", *args, **kwargs):
        raise NotImplementedError("ImageFont.getmask: use ImageFont.truetype() instead")


class FreeTypeFont:
    """TrueType/OpenType font loaded via fontdue."""

    def __init__(self, font, size=10, index=0, encoding="", layout_engine=None):
        if isinstance(font, str):
            self._rust_font = RustFont.truetype(font, float(size))
        elif hasattr(font, 'read'):
            data = font.read()
            self._rust_font = RustFont.truetype_from_bytes(data, float(size))
        else:
            raise TypeError("font must be a file path or file-like object")

    def getbbox(self, text, mode="", direction=None, features=None, language=None,
                stroke_width=0, anchor=None):
        return self._rust_font.getbbox(text)

    def getlength(self, text, mode="", direction=None, features=None, language=None):
        w, _h = self._rust_font.getbbox(text)
        return w

    def getmask(self, text, mode="", direction=None, features=None, language=None,
                stroke_width=0, anchor=None, ink=0, start=None):
        """Return glyph mask as L-mode Image. Pixel compositing done in Rust."""
        from .image import Image as PILImage
        w, h, alpha = self._rust_font.getmask_alpha(str(text))
        return PILImage.frombytes("L", (w, h), bytes(alpha))

    def getmetrics(self):
        sz = self._rust_font.get_size()
        return (sz, sz)

    def getname(self):
        return (None, None)

    def font_variant(self, font=None, size=None, index=None, encoding=None, layout_engine=None):
        raise NotImplementedError("FreeTypeFont.font_variant")


class TransposedFont:
    """Transposed font wrapper (stub)."""
    pass


def load(filename):
    """Load a font file. Delegates to truetype()."""
    return truetype(str(filename))


def load_default(size=None):
    """Load default font. Falls back to basic ImageFont."""
    if size is None:
        size = 14
    return ImageFont()




def truetype(font, size=10, index=0, encoding="", layout_engine=None):
    """Load a TrueType/OpenType font."""
    return FreeTypeFont(font, size, index, encoding, layout_engine)
