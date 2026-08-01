"""ImageFont — font loading and text rendering via pillow-rs-freetype (pure Rust FreeType compatible)."""
import os
import warnings
from enum import IntEnum

from . import _core


MAX_STRING_LENGTH = 1_000_000


class Layout(IntEnum):
    BASIC = 0
    RAQM = 1


def _normalize_layout_engine(layout_engine):
    if layout_engine not in (Layout.BASIC, Layout.RAQM):
        return Layout.BASIC
    if layout_engine == Layout.RAQM:
        warnings.warn(
            "Raqm layout was requested, but Raqm is not available. "
            "Falling back to basic layout.",
            stacklevel=3,
        )
    return Layout.BASIC


class ImagingCore:
    """Stable Python facade for Pillow's internal mask storage contract."""

    __slots__ = ("_image", "_mode", "_size", "_bytes")

    def __init__(self, image=None, mode=None, size=None, data=None):
        self._image = image
        self._mode = mode
        self._size = size
        self._bytes = data

    @property
    def _rust_image(self):
        if self._image is None:
            raise ValueError("zero-sized mask has no Rust image storage")
        return self._image._rust_image

    @property
    def mode(self):
        if self._image is None:
            return self._mode
        return self._image.mode

    @property
    def size(self):
        if self._image is None:
            return self._size
        return self._image.size

    def tobytes(self):
        if self._image is None:
            return self._bytes
        return self._rust_image.tobytes_unpacked()

    def __bytes__(self):
        return self.tobytes()

    def transpose(self, method):
        if self._image is None:
            return ImagingCore(None, self._mode, self._size, self._bytes)
        return ImagingCore(self._image.transpose(method))


class _NativeFont:
    """Thin Python shape wrapper for Pillow's native ``_imagingft.Font`` object."""

    __slots__ = ("_rust_font",)

    def __init__(self, rust_font):
        self._rust_font = rust_font

    @property
    def family(self):
        return self._rust_font.family

    @property
    def style(self):
        return self._rust_font.style

    @property
    def ascent(self):
        return self._rust_font.ascent

    @property
    def descent(self):
        return self._rust_font.descent

    @property
    def height(self):
        return self._rust_font.height

    @property
    def x_ppem(self):
        return self._rust_font.x_ppem

    @property
    def y_ppem(self):
        return self._rust_font.y_ppem

    @property
    def glyphs(self):
        return self._rust_font.glyphs

    def getlength(self, text):
        return self._rust_font.getlength(text)

    def getsize(self, text):
        return self._rust_font.getsize(text)

    def getvarnames(self):
        return self._rust_font.getvarnames()

    def getvaraxes(self):
        return self._rust_font.getvaraxes()

    def setvarname(self, instance_index):
        return self._rust_font.setvarname(instance_index)

    def setvaraxes(self, axes):
        return self._rust_font.setvaraxes(axes)

    def render(
        self,
        text,
        fill,
        mode,
        direction,
        features,
        language,
        stroke_width,
        stroke_filled,
        anchor,
        ink,
        start,
    ):
        from .image import Image as PILImage

        width, height, pixels, offset = self._rust_font.render_with_options(
            str(text),
            _none_if_empty(mode),
            direction,
            features,
            language,
            float(stroke_width),
            bool(stroke_filled),
            anchor,
            ink,
            start,
        )
        size = (width, height)
        fill(*size)
        if width == 0 or height == 0:
            return ImagingCore(None, "L", size, bytes(pixels)), offset
        return ImagingCore(PILImage.frombytes("L", size, bytes(pixels))), offset


class ImageFont:
    """Pillow-compatible base wrapper for a loaded bitmap font."""

    def getbbox(self, text, *args, **kwargs):
        """Return the bitmap font's bounding box."""
        width, height = self.font.getsize(_pilfont_text(text))
        return 0, 0, width, height

    def getlength(self, text, *args, **kwargs):
        """Return the bitmap font's horizontal advance."""
        width, _height = self.font.getsize(_pilfont_text(text))
        return width

    def getmask(self, text, mode="", *args, **kwargs):
        """Return the loaded bitmap font's native mask object."""
        from .image import Image as PILImage
        return ImagingCore(PILImage(self.font.getmask(_pilfont_text(text), mode)))


def _pilfont_text(text):
    if isinstance(text, str):
        return text.encode("latin-1")
    return text


def _none_if_empty(value):
    return None if value == "" else value


def _pillow_bbox_value(value):
    return int(value) if isinstance(value, float) and value.is_integer() else value


def _pillow_bbox_tuple(bbox):
    return tuple(_pillow_bbox_value(value) for value in bbox)


def _validate_layout_options(features=None, direction=None, language=None):
    if features is not None or direction is not None or language is not None:
        # This build intentionally has no libraqm, matching Pillow's public
        # failure for layout options that require it.
        raise KeyError("setting text direction, language or font features is not supported without libraqm")


def _validate_start(start):
    if start is not None and not (
        isinstance(start, (tuple, list)) and len(start) == 2
    ):
        raise TypeError("render() argument 11 must be 2-item sequence, not float")


def _wrap_pilfont(font):
    wrapped = ImageFont()
    wrapped.font = font
    wrapped.info = font.info
    if font.file is not None:
        wrapped.file = font.file
    return wrapped


class FreeTypeFont:
    """TrueType/OpenType font loaded via pillow-rs-freetype.

    Pure Rust font rendering — no PIL dependency required.
    """

    def __init__(self, font, size=10, index=0, encoding="", layout_engine=None):
        if index != 0:
            raise OSError("invalid argument")
        layout_engine = _normalize_layout_engine(layout_engine)
        layout_engine_name = layout_engine.name if layout_engine is not None else None
        self.path = font
        if isinstance(font, (str, bytes, os.PathLike)):
            font_path = os.fspath(font)
            self._font_path = font
            self._rust_font = _core.ImageFont.truetype(
                os.fsdecode(font_path), float(size), int(index), encoding, layout_engine_name
            )
        elif hasattr(font, 'read'):
            self._font_data = font.read()
            self._rust_font = _core.ImageFont.truetype_from_bytes(
                self._font_data, float(size), int(index), encoding, layout_engine_name
            )
        else:
            raise TypeError("font must be a file path or file-like object")
        self.size = float(size)
        self.index = index
        self.encoding = encoding
        self.layout_engine = layout_engine
        self.font = _NativeFont(self._rust_font)
        # Note: PIL fallback for pixel-identical font rendering was removed.
        # Font rendering uses pillow-rs-freetype. Font rendering may differ
        # slightly from PIL's FreeType output in edge cases.
        self._pil_font = None

    @classmethod
    def _from_font_data(cls, data, size=10, index=0, encoding="", layout_engine=None):
        layout_engine = _normalize_layout_engine(layout_engine)
        layout_engine_name = layout_engine.name if layout_engine is not None else None
        font = object.__new__(cls)
        font.path = None
        font._font_data = bytes(data)
        font._rust_font = _core.ImageFont.truetype_from_bytes(
            font._font_data, float(size), int(index), encoding, layout_engine_name
        )
        font.size = float(size)
        font.index = index
        font.encoding = encoding
        font.layout_engine = layout_engine
        font.font = _NativeFont(font._rust_font)
        font._pil_font = None
        return font

    def getbbox(self, text, mode="", direction=None, features=None, language=None,
                stroke_width=0, anchor=None):
        _validate_layout_options(features, direction, language)
        text = str(text)
        if mode == "" and stroke_width == 0 and anchor is None:
            return self._rust_font.getbbox(text)
        return _pillow_bbox_tuple(self._rust_font.getbbox_with_options(
            text, _none_if_empty(mode), direction, features, language,
            float(stroke_width), anchor
        ))

    def getlength(self, text, mode="", direction=None, features=None, language=None):
        _validate_layout_options(features, direction, language)
        text = str(text)
        if mode == "":
            return self._rust_font.getlength_alpha(text)
        return self._rust_font.getlength_with_options(
            text, _none_if_empty(mode), direction, features, language
        )

    def getmask(self, text, mode="", direction=None, features=None, language=None,
                stroke_width=0, anchor=None, ink=0, start=None):
        """Return glyph mask through Pillow's ImagingCore-compatible contract."""
        from .image import Image as PILImage
        _validate_layout_options(features, direction, language)
        _validate_start(start)
        text = str(text)
        if mode == "" and stroke_width == 0 and anchor is None and ink == 0 and start is None:
            w, h, alpha = self._rust_font.getmask_alpha(text)
            return ImagingCore(PILImage.frombytes("L", (w, h), bytes(alpha)))
        w, h, alpha = self._rust_font.getmask_alpha_with_options(
            text, _none_if_empty(mode), direction, features, language,
            float(stroke_width), anchor, ink, start
        )
        return ImagingCore(PILImage.frombytes("L", (w, h), bytes(alpha)))

    def getmask2(self, text, mode="", direction=None, features=None, language=None,
                 stroke_width=0, anchor=None, ink=0, start=None, *args, **kwargs):
        """Create a bitmap for the text and return the text offset using pillow-rs-freetype.

        :param text: Text to render.
        :param mode: Used by some graphics drivers to indicate what mode the
                     driver prefers; if empty, the renderer may return either
                     mode.
        :param direction: Direction of the text. It can be 'rtl' (right to
                          left), 'ltr' (left to right) or 'ttb' (top to bottom).
                          Requires libraqm; unsupported builds raise the same
                          no-libraqm error category as Pillow.
        :param features: A list of OpenType font features to be used during text
                         layout.
        :param language: Language of the text.
        :param stroke_width: The width of the text stroke.
        :param anchor: The text anchor alignment.
        :param ink: Foreground ink for rendering.
        :param start: Tuple of horizontal and vertical offset.

        :return: A tuple of the mask (L-mode Image) and the text offset
                 ``(offset_x, offset_y)``.
        """
        from .image import Image as PILImage
        _validate_layout_options(features, direction, language)
        _validate_start(start)
        text = str(text)
        if (
            mode == ""
            and stroke_width == 0
            and anchor is None
            and ink == 0
            and start is None
            and not args
            and not kwargs
        ):
            image, offset = self._rust_font.getmask2_image(text)
            return ImagingCore(PILImage(image)), offset
        image, offset = self._rust_font.getmask2_image_with_options(
            text, _none_if_empty(mode), direction, features, language,
            float(stroke_width), anchor, ink, start,
            bool(kwargs.get("stroke_filled", False)), bool(args), bool(kwargs)
        )
        mask = ImagingCore(PILImage(image))
        return mask, offset

    def getmetrics(self):
        """Get font metrics: (ascent, descent) in pixels."""
        return self._rust_font.getmetrics()

    def getname(self):
        """Return font family name and style name.

        :return: A tuple ``(family, style)``. Missing names are returned as
                 ``None``, matching Pillow.
        """
        return self._rust_font.get_name()

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
        if index not in (None, 0):
            raise OSError("invalid argument")
        if font is None and size is None and index is None and encoding is None and layout_engine is None:
            return self
        # Default font (loaded via load_default) has no source path/bytes.
        # Fall back to calling load_default again with the new size.
        if getattr(self, '_is_default', False):
            new_size = self.size if size is None else float(size)
            return load_default(size=new_size)
        if font is None and hasattr(self, '_font_data'):
            return FreeTypeFont._from_font_data(
                self._font_data,
                size=self.size if size is None else float(size),
                index=self.index if index is None else index,
                encoding=self.encoding if encoding is None else encoding,
                layout_engine=layout_engine if layout_engine is not None else self.layout_engine,
            )
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

        :return: A list of named styles (bytes).
        :raises OSError: If the font is not a variation font.
        """
        return self._rust_font.get_variation_names()

    def set_variation_by_name(self, name):
        """Set variation by name.

        :param name: The name of the style.
        :raises OSError: If the font is not a variation font.
        """
        if not isinstance(name, bytes):
            name = name.encode()
        return self._rust_font.set_variation_by_name(name)

    def get_variation_axes(self):
        """Get variation axes.

        :return: A list of axis dictionaries.
        :raises OSError: If the font is not a variation font.
        """
        return [
            {
                "minimum": minimum,
                "default": default,
                "maximum": maximum,
                "name": name,
            }
            for minimum, default, maximum, name in self._rust_font.get_variation_axes()
        ]

    def set_variation_by_axes(self, axes):
        """Set variation by axes values.

        :param axes: A list of values for each axis.
        :raises OSError: If the font is not a variation font.
        """
        if not isinstance(axes, list):
            raise TypeError("argument must be a list")
        return self._rust_font.set_variation_by_axes(axes)


class TransposedFont:
    """Wrapper for writing rotated or mirrored text."""

    _TRANSPOSE_NAMES = {
        0: "FLIP_LEFT_RIGHT",
        1: "FLIP_TOP_BOTTOM",
        2: "ROTATE_90",
        3: "ROTATE_180",
        4: "ROTATE_270",
        5: "TRANSPOSE",
        6: "TRANSVERSE",
    }

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
        if isinstance(orientation, int):
            self._orientation_name = self._TRANSPOSE_NAMES.get(orientation)
        else:
            self._orientation_name = orientation

    def getmask(self, text, mode="", *args, **kwargs):
        """Create a bitmap for the text, optionally transposed."""
        if isinstance(self.font, FreeTypeFont):
            from .image import Image as PILImage
            image = self.font._rust_font.get_transposed_mask_image(
                str(text), self._orientation_name
            )
            return ImagingCore(PILImage(image))
        im = self.font.getmask(text, mode, *args, **kwargs)
        if self.orientation is not None:
            return im.transpose(self.orientation)
        return im

    def getbbox(self, text, *args, **kwargs):
        """Get bounding box for text, adjusted for orientation.

        For rotated text (90/270 degrees), width and height are swapped.
        """
        return _core.transposed_font_bbox(
            self.font.getbbox(text, *args, **kwargs), self._orientation_name
        )

    def getlength(self, text, *args, **kwargs):
        """Get text length.

        :raises ValueError: If text is rotated by 90 or 270 degrees,
            where length is undefined.
        """
        _core.validate_transposed_font_length(self._orientation_name)
        return self.font.getlength(text, *args, **kwargs)


def load(filename):
    """Load a PILfont metrics file and its sibling glyph bitmap."""
    return _wrap_pilfont(_core.PilFont.load(str(filename)))


def load_path(filename):
    """Load a PILfont by searching for it along ``sys.path``."""
    if not isinstance(filename, str):
        filename = filename.decode("utf-8")
    return _wrap_pilfont(_core.PilFont.load_path(filename))


def load_default(size=None):
    """Load Pillow's embedded Aileron Regular subset with BASIC layout."""
    if size is None:
        size = 10
    font = object.__new__(FreeTypeFont)
    font._rust_font = _core.ImageFont.load_default(float(size))
    font.size = float(size)
    font.layout_engine = Layout.BASIC
    font.font = _NativeFont(font._rust_font)
    font._is_default = True
    font._pil_font = None
    return font


def load_default_imagefont():
    """Load Pillow's embedded courB08 legacy PILfont."""
    return _wrap_pilfont(_core.PilFont.load_default())


def truetype(font, size=10, index=0, encoding="", layout_engine=None):
    """Load a TrueType/OpenType font."""
    return FreeTypeFont(font, size, index, encoding, layout_engine)
