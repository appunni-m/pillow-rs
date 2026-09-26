"""Enumerations and constants matching Pillow's API."""

from enum import IntEnum

from . import _core


class ImageMode:
    L = "L"
    LA = "LA"
    I = "I"
    RGB = "RGB"
    RGBA = "RGBA"
    CMYK = "CMYK"
    YCbCr = "YCbCr"
    HSV = "HSV"
    BINARY = "1"


class ImageFormat:
    JPEG = "JPEG"
    PNG = "PNG"
    GIF = "GIF"
    BMP = "BMP"
    TIFF = "TIFF"
    WEBP = "WEBP"
    ICO = "ICO"
    PNM = "PNM"
    DDS = "DDS"
    TGA = "TGA"
    FARBFELD = "FARBFELD"
    AVIF = "AVIF"


class Resampling(IntEnum):
    """Pillow-compatible resampling filter codes."""

    NEAREST = 0
    LANCZOS = 1
    BILINEAR = 2
    BICUBIC = 3
    BOX = 4
    HAMMING = 5

    # Preserve the former target-facade names as aliases. Their values now
    # follow Pillow's public enum codes instead of the old internal ordering.
    NEAREST_INT = NEAREST
    LANCZOS_INT = LANCZOS
    BILINEAR_INT = BILINEAR
    BICUBIC_INT = BICUBIC

class Transpose:
    FLIP_LEFT_RIGHT = "FLIP_LEFT_RIGHT"
    FLIP_TOP_BOTTOM = "FLIP_TOP_BOTTOM"
    ROTATE_90 = "ROTATE_90"
    ROTATE_180 = "ROTATE_180"
    ROTATE_270 = "ROTATE_270"
    TRANSPOSE = "TRANSPOSE"
    TRANSVERSE = "TRANSVERSE"

    @classmethod
    def from_int(cls, value: int) -> str:
        return _core.transpose_from_int(value)


class Dither:
    NONE = "NONE"
    FLOYDSTEINBERG = "FLOYDSTEINBERG"


class Palette:
    WEB = "WEB"
    ADAPTIVE = "ADAPTIVE"
