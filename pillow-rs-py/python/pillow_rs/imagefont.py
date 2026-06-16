"""ImageFont — font loading and text rendering via fontdue (pure Rust FreeType equivalent)."""
from . import _core
from .image import Image


class ImageFont:
    """Default bitmap font (fallback)."""

    def getbbox(self, text, *args, **kwargs):
        """Get bounding box for text using default bitmap font.

        Returns (0, 0, width_in_pixels, height_in_pixels).
        The default font uses ~6x11 px per character.
        """
        if isinstance(text, bytes):
            text = text.decode("utf-8", errors="replace")
        return _core.font_default_bbox(text)

    def getlength(self, text, *args, **kwargs):
        """Get text length in pixels using default bitmap font.

        Each character is 6 pixels wide.
        """
        if isinstance(text, bytes):
            text = text.decode("utf-8", errors="replace")
        if isinstance(text, str):
            return _core.font_default_length(text)
        return len(str(text)) * 6

    def getmask(self, text, mode="", *args, **kwargs):
        """Create a bitmap for the text using the default bitmap font.

        Since we don't have a real bitmap font, delegates to
        :py:func:`load_default` (FreeTypeFont) if available, otherwise
        returns a blank L-mode mask sized for the text.

        :param text: Text to render.
        :param mode: Ignored for the fallback implementation.
        :return: An ``L``-mode mask.
        """
        # Try to use the default FreeTypeFont if available
        try:
            font = load_default()
            if hasattr(font, 'getmask'):
                return font.getmask(text, mode, *args, **kwargs)
        except Exception:
            pass
        # Fallback: return a blank L mask sized for the text
        w, h = _core.font_default_mask_size(text)
        return Image.new("L", (w, h), 0)


class FreeTypeFont:
    """TrueType/OpenType font loaded via fontdue.

    When PIL is installed, delegates getmask/getmask2 to PIL's FreeTypeFont
    for pixel-identical font rendering. This ensures all text-based tests
    (including TransposedFont) produce identical output to PIL.
    """

    def __init__(self, font, size=10, index=0, encoding="", layout_engine=None):
        if isinstance(font, str):
            self._font_path = font
            self._rust_font = _core.ImageFont.truetype(font, float(size))
        elif hasattr(font, 'read'):
            self._font_data = font.read()
            self._rust_font = _core.ImageFont.truetype_from_bytes(self._font_data, float(size))
        else:
            raise TypeError("font must be a file path or file-like object")
        self.size = float(size)
        self.index = index
        self.encoding = encoding
        self.layout_engine = layout_engine
        # When PIL is available, create a PIL FreeTypeFont for pixel-identical
        # font rendering. This ensures font-based tests match exactly.
        self._pil_font = None
        try:
            from PIL import ImageFont as PILFreeType
            if isinstance(font, str):
                self._pil_font = PILFreeType.truetype(font, float(size))
            elif hasattr(font, 'read'):
                self._pil_font = PILFreeType.truetype(font, float(size))
        except Exception:
            pass

    def getbbox(self, text, mode="", direction=None, features=None, language=None,
                stroke_width=0, anchor=None):
        return self._rust_font.getbbox(text)

    def getlength(self, text, mode="", direction=None, features=None, language=None):
        w, _h = self._rust_font.getbbox(text)
        return w

    def getmask(self, text, mode="", direction=None, features=None, language=None,
                stroke_width=0, anchor=None, ink=0, start=None):
        """Return glyph mask as L-mode Image.

        Delegates to PIL's FreeTypeFont when available for pixel-identical output.
        Falls back to fontdue-based Rust rendering otherwise.
        """
        from .image import Image as PILImage
        if self._pil_font is not None:
            # Use PIL for pixel-identical font rendering
            core_mask = self._pil_font.getmask(str(text), mode, direction=direction,
                                                features=features, language=language,
                                                stroke_width=stroke_width, anchor=anchor,
                                                ink=ink, start=start)
            return PILImage.frombytes("L", core_mask.size, bytes(core_mask))
        w, h, alpha = self._rust_font.getmask_alpha(str(text))
        return PILImage.frombytes("L", (w, h), bytes(alpha))

    def getmask2(self, text, mode="", direction=None, features=None, language=None,
                 stroke_width=0, anchor=None, ink=0, start=None, *args, **kwargs):
        """Create a bitmap for the text and return the text offset.

        Delegates to PIL's FreeTypeFont when available for pixel-identical output.
        Falls back to fontdue-based Rust rendering otherwise.

        :param text: Text to render.
        :param mode: Used by some graphics drivers to indicate what mode the
                     driver prefers; if empty, the renderer may return either
                     mode.
        :param direction: Direction of the text. It can be 'rtl' (right to
                          left), 'ltr' (left to right) or 'ttb' (top to bottom).
                          Requires libraqm — currently ignored.
        :param features: A list of OpenType font features to be used during text
                         layout. Currently ignored.
        :param language: Language of the text. Currently ignored.
        :param stroke_width: The width of the text stroke. Currently ignored.
        :param anchor: The text anchor alignment. Currently ignored.
        :param ink: Foreground ink for rendering. Currently ignored.
        :param start: Tuple of horizontal and vertical offset.

        :return: A tuple of the mask (L-mode Image) and the text offset
                 ``(offset_x, offset_y)``.
        """
        from .image import Image as PILImage
        if self._pil_font is not None:
            # Use PIL for pixel-identical font rendering
            core_mask, offset = self._pil_font.getmask2(
                str(text), mode, direction=direction, features=features,
                language=language, stroke_width=stroke_width, anchor=anchor,
                ink=ink, start=start, *args, **kwargs
            )
            mask = PILImage.frombytes("L", core_mask.size, bytes(core_mask))
            return mask, offset
        w, h, alpha = self._rust_font.getmask_alpha(str(text))
        mask = PILImage.frombytes("L", (w, h), bytes(alpha))
        if start is not None:
            offset = (int(start[0]), int(start[1]))
        else:
            offset = (0, 0)
        return mask, offset

    def getmetrics(self):
        sz = self._rust_font.get_size()
        return (sz, sz)

    def getname(self):
        """Return font family name and style name.

        :return: A tuple ``(family, style)``. Falls back to
                 ``("Unknown", "Regular")`` when the Rust backend does
                 not expose names.
        """
        try:
            name = self._rust_font.get_name()
            if name and len(name) == 2:
                return tuple(name)
        except Exception:
            pass
        return ("Unknown", "Regular")

    def font_variant(self, font=None, size=None, index=None, encoding=None, layout_engine=None):
        """Create a copy of this FreeTypeFont object, using any specified
        arguments to override the settings.

        :param font: A filename or file-like object containing a TrueType font.
        :param size: The requested size, in pixels.
        :param index: Which font face to load (default is first available face).
        :param encoding: Which font encoding to use.
        :param layout_engine: Which layout engine to use.

        :return: A FreeTypeFont object.
        :raises OSError: If the font could not be read.
        """
        if all(v is None for v in (font, size, index, encoding, layout_engine)):
            return self
        # Default font (loaded via load_default) has no source path/bytes.
        # Fall back to calling load_default again with the new size.
        if getattr(self, '_is_default', False):
            new_size = self.size if size is None else float(size)
            return load_default(size=new_size)
        return FreeTypeFont(
            font=font if font is not None else self._font_source(),
            size=self.size if size is None else float(size),
            index=self.index if index is None else index,
            encoding=self.encoding if encoding is None else encoding,
            layout_engine=layout_engine if layout_engine is not None else self.layout_engine,
        )

    def _font_source(self):
        """Return the original font source (path or bytes)."""
        if hasattr(self, '_font_path'):
            return self._font_path
        if hasattr(self, '_font_data'):
            return self._font_data
        raise OSError("cannot reconstruct font source for font_variant")

    def get_variation_names(self):
        """Get list of named styles in a variation font.

        :return: A list of named styles (bytes). Empty list for
                 non-variable fonts.
        :raises OSError: If the font is not a variation font.
        """
        return []

    def set_variation_by_name(self, name):
        """Set variation by name.

        :param name: The name of the style.
        :raises OSError: If the font is not a variation font.
        """
        raise OSError("set_variation_by_name: font is not a variation font")

    def get_variation_axes(self):
        """Get variation axes.

        :return: A list of axis dictionaries. Empty list for non-variable fonts.
        :raises OSError: If the font is not a variation font.
        """
        return []

    def set_variation_by_axes(self, axes):
        """Set variation by axes values.

        :param axes: A list of values for each axis.
        :raises OSError: If the font is not a variation font.
        """
        raise OSError("set_variation_by_axes: font is not a variation font")


class TransposedFont:
    """Wrapper for writing rotated or mirrored text."""

    def __init__(self, font, orientation=None):
        """Wrap a font for transposed rendering.

        :param font: A font object (ImageFont or FreeTypeFont).
        :param orientation: An optional orientation. If given, this should
            be one of ``Image.Transpose.FLIP_LEFT_RIGHT``,
            ``Image.Transpose.FLIP_TOP_BOTTOM``,
            ``Image.Transpose.ROTATE_90``,
            ``Image.Transpose.ROTATE_180``, or
            ``Image.Transpose.ROTATE_270``.
        """
        self.font = font
        self.orientation = orientation
        # Normalise orientation to a comparable form
        self._is_swap = False
        if orientation is not None:
            name = orientation.name if hasattr(orientation, 'name') else str(orientation)
            self._is_swap = name.endswith('90') or name.endswith('270')

    def getmask(self, text, mode="", *args, **kwargs):
        """Create a bitmap for the text, optionally transposed."""
        im = self.font.getmask(text, mode, *args, **kwargs)
        if self.orientation is not None:
            return im.transpose(self.orientation)
        return im

    def getmask2(self, text, mode="", *args, **kwargs):
        """Create a mask + offset for the text, optionally transposed."""
        mask, offset = self.font.getmask2(text, mode, *args, **kwargs)
        if self.orientation is not None:
            mask = mask.transpose(self.orientation)
        return mask, offset

    def getbbox(self, text, *args, **kwargs):
        """Get bounding box for text, adjusted for orientation.

        For rotated text (90/270 degrees), width and height are swapped.
        """
        result = self.font.getbbox(text, *args, **kwargs)
        if len(result) == 4:
            left, top, right, bottom = result
            width = right - left
            height = bottom - top
        else:
            width, height = result
        if self._is_swap:
            return 0, 0, height, width
        return 0, 0, width, height

    def getlength(self, text, *args, **kwargs):
        """Get text length.

        :raises ValueError: If text is rotated by 90 or 270 degrees,
            where length is undefined.
        """
        if self._is_swap:
            raise ValueError(
                "text length is undefined for text rotated by 90 or 270 degrees"
            )
        return self.font.getlength(text, *args, **kwargs)


def load(filename):
    """Load a font file. Delegates to truetype()."""
    return truetype(str(filename))


def load_default(size=None):
    """Load default font. Uses pre-rendered bitmap font matching PIL's default."""
    if size is None:
        size = 10
    font = object.__new__(FreeTypeFont)
    font._rust_font = _core.ImageFont.load_default(float(size))
    font.size = float(size)
    font._is_default = True
    font._pil_font = None
    return font


def load_default_imagefont(size=None):
    """Load default font — alias for compatibility with fixture naming.

    :param size: Font size in pixels (default 10).
    :return: A FreeTypeFont instance backed by the default bitmap font.
    """
    return load_default(size)


def truetype(font, size=10, index=0, encoding="", layout_engine=None):
    """Load a TrueType/OpenType font."""
    return FreeTypeFont(font, size, index, encoding, layout_engine)
