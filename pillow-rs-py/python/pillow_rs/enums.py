"""Enumerations and constants matching Pillow's API."""


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


class Resampling:
    NEAREST = "NEAREST"
    BILINEAR = "BILINEAR"
    BICUBIC = "BICUBIC"
    LANCZOS = "LANCZOS"
    NEAREST_INT = 0
    BILINEAR_INT = 1
    BICUBIC_INT = 2
    LANCZOS_INT = 3

    @classmethod
    def from_int(cls, value: int) -> str:
        mapping = {0: "NEAREST", 1: "BILINEAR", 2: "BICUBIC", 3: "LANCZOS"}
        return mapping.get(value, "BILINEAR")


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
        mapping = {
            0: "FLIP_LEFT_RIGHT",
            1: "FLIP_TOP_BOTTOM",
            2: "ROTATE_90",
            3: "ROTATE_180",
            4: "ROTATE_270",
            5: "TRANSPOSE",
            6: "TRANSVERSE",
        }
        return mapping.get(value, "FLIP_LEFT_RIGHT")


class Dither:
    NONE = "NONE"
    FLOYDSTEINBERG = "FLOYDSTEINBERG"


class Palette:
    WEB = "WEB"
    ADAPTIVE = "ADAPTIVE"
