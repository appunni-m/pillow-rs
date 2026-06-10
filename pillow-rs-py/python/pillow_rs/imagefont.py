"""ImageFont — font loading and text rendering. Pillow-compatible stubs."""


class ImageFont:
    """Default bitmap font (stub)."""

    def getbbox(self, text, *args, **kwargs):
        raise NotImplementedError("ImageFont.getbbox")

    def getlength(self, text, *args, **kwargs):
        raise NotImplementedError("ImageFont.getlength")

    def getmask(self, text, mode="", *args, **kwargs):
        raise NotImplementedError("ImageFont.getmask")


class FreeTypeFont:
    """TrueType/OpenType font via FreeType (stub)."""

    def __init__(self, font, size=10, index=0, encoding="", layout_engine=None):
        raise NotImplementedError("FreeTypeFont requires FreeType library")

    def getbbox(self, text, mode="", direction=None, features=None, language=None,
                stroke_width=0, anchor=None):
        raise NotImplementedError("FreeTypeFont.getbbox")

    def getlength(self, text, mode="", direction=None, features=None, language=None):
        raise NotImplementedError("FreeTypeFont.getlength")

    def getmask(self, text, mode="", direction=None, features=None, language=None,
                stroke_width=0, anchor=None, ink=0, start=None):
        raise NotImplementedError("FreeTypeFont.getmask")

    def getmetrics(self):
        raise NotImplementedError("FreeTypeFont.getmetrics")

    def getname(self):
        raise NotImplementedError("FreeTypeFont.getname")


class TransposedFont:
    """Transposed font wrapper (stub)."""
    pass


def load(filename):
    """Load a font file."""
    raise NotImplementedError("ImageFont.load requires FreeType")


def load_default(size=None):
    """Load default PIL font."""
    return ImageFont()


def truetype(font, size=10, index=0, encoding="", layout_engine=None):
    """Load a TrueType/OpenType font."""
    raise NotImplementedError("ImageFont.truetype requires FreeType")
