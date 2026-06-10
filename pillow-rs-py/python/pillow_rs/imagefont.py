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
            import tempfile, os
            with tempfile.NamedTemporaryFile(suffix='.ttf', delete=False) as f:
                f.write(data)
                f.flush()
                self._rust_font = RustFont.truetype(f.name, float(size))
            os.unlink(f.name)
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
        w, h, alpha = self._rust_font.getmask_alpha(str(text))
        from .image import Image as PILImage
        img = PILImage.new("L", (w, h), 0)
        for y in range(h):
            for x in range(w):
                a = alpha[y * w + x]
                if a > 0:
                    img.putpixel((x, y), a if ink == 0 else ink)
        return img

    def getmetrics(self):
        raise NotImplementedError("FreeTypeFont.getmetrics")

    def getname(self):
        return (None, None)

    def font_variant(self, font=None, size=None, index=None, encoding=None, layout_engine=None):
        raise NotImplementedError("FreeTypeFont.font_variant")


class TransposedFont:
    """Transposed font wrapper (stub)."""
    pass


def load(filename):
    """Load a font file."""
    return FreeTypeFont(str(filename))


def load_default(size=None):
    """Load default font. Falls back to FreeTypeFont stub."""
    raise NotImplementedError("ImageFont.load_default: use truetype() with a .ttf file")


def truetype(font, size=10, index=0, encoding="", layout_engine=None):
    """Load a TrueType/OpenType font."""
    return FreeTypeFont(font, size, index, encoding, layout_engine)
