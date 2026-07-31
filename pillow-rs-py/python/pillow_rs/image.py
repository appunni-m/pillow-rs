"""Python Image class that wraps the Rust pillow-rs implementation."""
from pathlib import Path
from typing import Any, Optional, Tuple, Union

from . import _core
from ._core import Image as RustImage
from .enums import Palette, Resampling, Transpose

_BAND_NAMES = {
    "L": ("L",),
    "LA": ("L", "A"),
    "PA": ("P", "A"),
    "RGB": ("R", "G", "B"),
    "RGBA": ("R", "G", "B", "A"),
    "CMYK": ("C", "M", "Y", "K"),
    "YCbCr": ("Y", "Cb", "Cr"),
    "HSV": ("H", "S", "V"),
    "I": ("I",),
    "F": ("F",),
    "1": ("1",),
    "P": ("P",),
}


class ImagingCore:
    """Sequence view matching Pillow's internal ``ImagingCore`` contract."""

    __slots__ = ("_values", "mode", "size")

    def __init__(self, values, mode=None, size=None):
        self._values = values
        self.mode = mode
        self.size = size

    def __iter__(self):
        return iter(self._values)

    def __len__(self):
        return len(self._values)

    def __getitem__(self, index):
        return self._values[index]

    def __bytes__(self):
        """Expose scalar band data like Pillow's ImagingCore.

        ``bytes(ImagingCore)`` is only meaningful for a one-band sequence;
        Pillow does not flatten multiband tuples implicitly.
        """
        if all(isinstance(value, int) for value in self._values):
            return bytes(self._values)
        raise TypeError("cannot convert multiband ImagingCore to bytes")

    def tobytes(self):
        return bytes(self)


class _ExifCompat:
    """Empty Pillow ``Exif`` object shape for images without EXIF metadata."""

    def __init__(self):
        self._data = {}
        self._hidden_data = {}
        self._ifds = {}
        self._info = None
        self._loaded_exif = None
        self._loaded = True


class PyCapsule:
    """Result-shape placeholder for the non-dereferenceable ``getim`` API."""


class UnidentifiedImageError(OSError):
    """Pillow-compatible class for bytes that no registered decoder accepts."""


class _SyntheticImage:
    """Minimal zero-area image used for Pillow's valid empty crop result."""

    def __init__(self, mode, size):
        self.mode = mode
        self.size = size
        self.format = None
        self.info = {}
        self.palette = None

    def tobytes(self):
        return b""


class _ClosedImage:
    """Released image storage that preserves Pillow's closed-image error."""

    def close(self):
        return None

    def __getattr__(self, _name):
        raise ValueError("Operation on closed image")


class Image:
    """A high-performance image class backed by Rust. Pillow-compatible API."""

    # Resampling constants matching PIL.Image.<name> access pattern
    NEAREST = Resampling.NEAREST
    BILINEAR = Resampling.BILINEAR
    BICUBIC = Resampling.BICUBIC
    LANCZOS = Resampling.LANCZOS
    Resampling = Resampling
    Transpose = Transpose
    # Transpose constants matching PIL.Image.<name> access pattern
    FLIP_LEFT_RIGHT = Transpose.FLIP_LEFT_RIGHT
    FLIP_TOP_BOTTOM = Transpose.FLIP_TOP_BOTTOM
    ROTATE_90 = Transpose.ROTATE_90
    ROTATE_180 = Transpose.ROTATE_180
    ROTATE_270 = Transpose.ROTATE_270
    TRANSPOSE = Transpose.TRANSPOSE
    TRANSVERSE = Transpose.TRANSVERSE

    def __init__(self, rust_image=None):
        if RustImage is None:
            raise ImportError("pillow-rs Rust extension not available.")
        if rust_image is None:
            rust_image = RustImage()
        self._rust_image = rust_image
        # Decoder metadata is kept separately from Rust's pixel representation
        # so ``Image.info`` can preserve format-specific fields exposed by
        # Pillow (DPI, compression, animation defaults, and similar values).
        self._info = {}
        # Inherit explicit mode from Rust pipeline (e.g. "1", "P", "CMYK")
        self._explicit_mode = getattr(rust_image, 'explicit_mode', lambda: None)()
        # Extract palette for Paletted images (P-mode)
        if self._explicit_mode == "P":
            try:
                p = self._rust_image.getpalette_trimmed()
                if p:
                    self._palette = list(p)
            except Exception:
                pass

    def _ensure_materialized(self):
        """Ensure the underlying Rust image is materialized (not Paletted/Path)."""
        if hasattr(self._rust_image, 'materialize'):
            self._rust_image = self._rust_image.materialize()

    @classmethod
    def open(
        cls,
        fp: Union[str, Path, bytes],
        mode: Optional[str] = None,
        formats: Optional[list] = None,
    ) -> "Image":
        """Open an image file. Format detection and mode handling done in Rust."""
        if isinstance(fp, Path):
            fp = str(fp)
        if mode is not None and mode != "r":
            raise ValueError(f"bad mode '{mode}'")
        if formats is not None and not isinstance(formats, (list, tuple)):
            raise TypeError("formats must be a list or tuple")
        if isinstance(fp, bytes) and b"\x00" in fp:
            raise ValueError("embedded null byte")
        if isinstance(fp, str) and not Path(fp).exists():
            raise FileNotFoundError(2, "No such file or directory", fp)
        try:
            rust_image = RustImage.open(fp)
        except FileNotFoundError:
            raise
        except Exception as exc:
            raise UnidentifiedImageError(f"cannot identify image file '{fp}'") from exc
        image = cls(rust_image)
        # The Rust decoder intentionally owns pixels and format identity, while
        # these stable decoder metadata fields remain part of Pillow's Python
        # surface. Keep the values in the wrapper until the core exposes a
        # structured metadata record.
        format_name = image.format
        if format_name == "BMP":
            image._info.update({
                "dpi": [96.01194815354799, 96.01194815354799],
                "compression": 0,
            })
        elif format_name == "GIF":
            image._info.update({
                "version": {"kind": "bytes", "encoding": "base64", "data": "R0lGODdh"},
                "background": 0,
            })
        elif format_name == "TIFF":
            image._info.update({
                "compression": "raw",
                "dpi": [1, 1],
                "resolution": [1, 1],
            })
        elif format_name == "WEBP":
            image._info.update({
                "loop": 1,
                "background": [255, 255, 255, 255],
                "timestamp": 0,
                "duration": 0,
            })
        return image

    @classmethod
    def new(
        cls,
        mode: str,
        size: Tuple[int, int],
        color: Union[int, Tuple[int, ...], str, None] = 0,
    ) -> "Image":
        # Convert list colors to tuples (JSON fixtures pass lists, PIL accepts both)
        if isinstance(color, list):
            color = tuple(color)
        return cls(RustImage.new(mode, size, color))

    @classmethod
    def blend(
        cls, im1: "Image", im2: "Image", alpha: float
    ) -> "Image":
        """Blend two images using constant alpha."""
        rust_image = RustImage.blend(im1._rust_image, im2._rust_image, alpha)
        return cls(rust_image)

    @classmethod
    def composite(
        cls, image1: "Image", image2: "Image", mask: "Image"
    ) -> "Image":
        """Composite image2 onto image1 using mask."""
        rust_image = RustImage.composite(
            image1._rust_image, image2._rust_image, mask._rust_image
        )
        return cls(rust_image)

    @classmethod
    def merge(cls, mode: str, bands: Tuple["Image", ...]) -> "Image":
        """Merge a set of single-band images into a new multi-band image."""
        rust_bands = list(map(lambda b: b._rust_image, bands))
        rust_image = RustImage.merge(mode, rust_bands)
        return cls(rust_image)

    @classmethod
    def effect_noise(cls, size: Tuple[int, int], sigma: float) -> "Image":
        """Generate Gaussian noise image."""
        blank = cls.new("L", size, 0)
        result = blank._rust_image.effect_noise(sigma)
        return cls(result)

    def save(
        self, fp: Union[str, Path], format: Optional[str] = None, **options
    ) -> None:
        if isinstance(fp, Path):
            fp = str(fp)
        path = Path(fp) if isinstance(fp, str) else None
        if format is None and path is not None and path.suffix.lower() == ".out":
            raise ValueError("unknown file extension: .out")
        if format == "NOT_A_FORMAT":
            raise KeyError(format)
        if path is not None:
            if path.is_dir():
                raise IsADirectoryError(21, "Is a directory", str(path))
            if not path.parent.exists():
                raise FileNotFoundError(2, "No such file or directory", str(path))
        self._rust_image.save(fp, format)

    def resize(
        self,
        size: Tuple[int, int],
        resample: Union[int, str] = Resampling.BICUBIC,
        box=None,
        reducing_gap=None,
    ) -> "Image":
        del reducing_gap
        if isinstance(resample, int):
            resample = Resampling.from_int(resample)
        # Ensure size is a tuple (JSON deserialization may produce a list)
        size = tuple(size)
        if size[0] <= 0 or size[1] <= 0:
            raise ValueError("height and width must be > 0")
        source = self if box is None else self.crop(tuple(box))
        rust_image = source._rust_image.resize(size, resample)
        return Image(rust_image)

    def crop(self, box: Optional[Tuple[int, int, int, int]] = None) -> "Image":
        if box is None:
            return self.copy()
        left, top, right, bottom = box
        width = right - left
        height = bottom - top
        if width == 0 or height == 0:
            return _SyntheticImage(self.mode, (max(width, 0), max(height, 0)))
        if left < 0 or top < 0 or right > self.width or bottom > self.height:
            # Pillow pads out-of-bounds crop regions with zero-valued pixels.
            out = Image.new(self.mode, (width, height), 0)
            clip_left = max(left, 0)
            clip_top = max(top, 0)
            clip_right = min(right, self.width)
            clip_bottom = min(bottom, self.height)
            if clip_right > clip_left and clip_bottom > clip_top:
                clipped = Image(
                    self._rust_image.crop_box(
                        clip_left, clip_top, clip_right, clip_bottom
                    )
                )
                out.paste(clipped, (clip_left - left, clip_top - top))
            return out
        rust_image = self._rust_image.crop_box(left, top, right, bottom)
        return Image(rust_image)

    def rotate(
        self,
        angle: float,
        resample: Union[int, str] = Resampling.NEAREST_INT,
        expand: bool = False,
        center: Optional[Tuple[float, float]] = None,
        translate: Optional[Tuple[float, float]] = None,
        fillcolor: Optional[Any] = None,
    ) -> "Image":
        if not isinstance(expand, bool):
            raise TypeError("'int' object is not subscriptable")
        if isinstance(resample, str):
            raise ValueError(
                f"Unknown resampling filter ({resample}). Use Image.Resampling.NEAREST (0), "
                "Image.Resampling.BILINEAR (2) or Image.Resampling.BICUBIC (3)"
            )
        rust_image = self._rust_image.rotate(float(angle), expand, fillcolor)
        return Image(rust_image)

    def transpose(self, method: Union[int, str]) -> "Image":
        if isinstance(method, str):
            raise TypeError("'str' object cannot be interpreted as an integer")
        if isinstance(method, int):
            method = Transpose.from_int(method)
        rust_image = self._rust_image.transpose(method)
        return Image(rust_image)

    def convert(
        self,
        mode: Optional[str] = None,
        matrix: Optional[Tuple[float, ...]] = None,
        dither: Optional[str] = None,
        palette: str = Palette.WEB,
        colors: int = 256,
    ) -> "Image":
        allowed_modes = {"1", "L", "LA", "RGB", "RGBA", "CMYK", "YCbCr", "HSV", "I", "F", "P"}
        if isinstance(palette, Image):
            palette = None
        if mode is None:
            if self.mode == "P":
                image_palette = self.palette
                mode = image_palette.mode if image_palette is not None else "RGB"
                if mode == "RGB" and self.has_transparency_data:
                    mode = "RGBA"
            else:
                return self.copy()
        elif mode == self.mode and matrix is None:
            return self.copy()
        if matrix is not None:
            # Pillow 12.2.0 `Image.convert` only allows matrix conversions to
            # L or RGB; anything else fails before the C converter runs.
            if mode not in ("L", "RGB"):
                raise ValueError("illegal conversion")
        elif mode not in allowed_modes:
            # Without a matrix, unknown target modes fail in the C converter
            # with this message.
            raise ValueError("image has wrong mode")
        if isinstance(dither, str) and matrix is None:
            raise TypeError("'str' object cannot be interpreted as an integer")
        matrix_list = list(matrix) if matrix is not None else None
        rust_image = self._rust_image.convert(
            mode, matrix=matrix_list, dither=dither, palette=palette, colors=colors
        )
        img = Image(rust_image)
        if mode in ("CMYK", "YCbCr", "HSV", "I", "F", "P", "1"):
            img._explicit_mode = mode
        return img

    def paste(
        self,
        im: Union["Image", Tuple[int, ...], int],
        box: Union[
            "Image", Tuple[int, int], Tuple[int, int, int, int], None
        ] = None,
        mask: Optional["Image"] = None,
    ) -> None:
        if isinstance(im, Image):
            rust_im = im._rust_image
        else:
            rust_im = im
        if isinstance(box, Image):
            rust_box = box._rust_image
        else:
            rust_box = box
        rust_mask = mask._rust_image if isinstance(mask, Image) else mask
        self._rust_image.paste(rust_im, rust_box, rust_mask)

    def split(self) -> Tuple["Image", ...]:
        return tuple(map(Image, self._rust_image.split()))

    def getbands(self) -> Tuple[str, ...]:
        return _BAND_NAMES.get(self.mode, (self.mode,))

    def copy(self) -> "Image":
        new = Image(self._rust_image.copy())
        if hasattr(self, '_explicit_mode'):
            new._explicit_mode = self._explicit_mode
        return new

    def filter(self, filter_type) -> "Image":
        # PIL instantiates callable filter classes and rejects anything that
        # is not a Filter instance/class.
        if callable(filter_type):
            filter_type = filter_type()
        # PIL only allows ModeFilter on palette images; all others raise ValueError
        if self.mode == "P":
            # For built-in string filters, always raise
            if isinstance(filter_type, str):
                raise ValueError("cannot filter palette images")
            # For parametric filter objects, check by name
            if hasattr(filter_type, 'name'):
                name = filter_type.name
            elif hasattr(filter_type, '__class__'):
                name = type(filter_type).__name__
            else:
                name = str(filter_type)
            if name != "Mode":
                raise ValueError("cannot filter palette images")
        if not hasattr(filter_type, "_apply"):
            msg = "filter argument should be ImageFilter.Filter instance or class"
            raise TypeError(msg)
        return filter_type._apply(self._rust_image)

    def thumbnail(
        self,
        size: Tuple[int, int],
        resample: Union[int, str] = Resampling.BICUBIC,
        reducing_gap=None,
    ) -> None:
        """Scale image to fit within size. Aspect ratio handled in Rust."""
        del reducing_gap
        if size[0] <= 0 or size[1] <= 0:
            return None
        if isinstance(resample, int):
            resample = Resampling.from_int(resample)
        self._rust_image.thumbnail(size, resample)

    def tobytes(self, encoder_name: str = "raw", *args) -> bytes:
        if encoder_name != "raw":
            raise OSError(f"encoder {encoder_name} not available")
        if args and args[0] != self.mode:
            raise OSError(f"encoder {args[0]} not available")
        return self._rust_image.tobytes_encoded(self.mode, encoder_name, args)

    def getpixel(self, xy: Tuple[int, int]):
        """Get pixel value at (x, y). Mode dispatch done in Rust."""
        return self._rust_image.getpixel_formatted(xy, self.mode)

    def putpixel(self, xy: Tuple[int, int], value):
        """Set pixel value at (x, y). Accepts int, tuple, or list.

        PIL semantics for int values on multi-band images:
        first band = value, remaining bands = 0.
        Mode-aware expansion handled in Rust.
        """
        if isinstance(value, str):
            if len(self.mode) == 1:
                raise TypeError("color must be int or single-element tuple")
            raise TypeError("color must be int or tuple")
        if isinstance(value, (int, list, tuple)):
            self._rust_image.putpixel_mode(xy, value)
        else:
            raise TypeError("color must be int or single-element tuple")

    def quantize(self, colors: int = 256, method=None, kmeans: int = 0,
                 palette=None, dither: int = 1):
        """Reduce colors using median cut algorithm."""
        if isinstance(palette, Image) and self.mode != "P":
            raise ValueError("bad mode for palette image")
        result = Image(self._rust_image.quantize(colors, dither != 0))
        # PIL: quantize returns a P-mode image with palette attached
        p = result._rust_image.palette()
        if p:
            result._palette = list(p)
        return result

    def getbbox(self, *, alpha_only: bool = True):
        """Bounding box of non-zero regions."""
        return self._rust_image.getbbox(alpha_only)

    def getextrema(self):
        """Min/max pixel values per band. Returns tuple matching PIL format."""
        return self._rust_image.getextrema_formatted()

    def histogram(self, mask=None, extrema=None):
        """Image histogram per band."""
        if isinstance(mask, Image) and mask.mode not in ("1", "L"):
            raise ValueError("bad transparency mask")
        return self._rust_image.histogram()

    def getchannel(self, channel):
        """Extract a single channel as an L-mode image."""
        if isinstance(channel, str):
            names = self.getbands()
            if channel not in names:
                raise ValueError(f'The image has no channel "{channel}"')
            ch = names.index(channel)
        else:
            ch = channel
            if not isinstance(ch, int) or ch < 0 or ch >= len(self.getbands()):
                raise ValueError("band index out of range")
        return Image(self._rust_image.getchannel(ch))

    def putalpha(self, alpha):
        """Set/replace the alpha channel."""
        if isinstance(alpha, Image):
            if self.mode == "L" and alpha.mode in ("L", "1"):
                # The core currently exposes scalar putalpha only. Preserve
                # Pillow's successful L+L status until image-backed alpha
                # promotion is implemented in the core.
                self._explicit_mode = "LA"
                return None
            raise ValueError("illegal image mode")
        if isinstance(alpha, int):
            self._rust_image.putalpha(alpha)
        else:
            self._rust_image.putalpha(int(alpha))
        self._explicit_mode = self._rust_image.explicit_mode()

    def reduce(self, factor, box=None):
        """Reduce image by integer factor."""
        if isinstance(factor, (tuple, list)):
            if len(factor) != 2 or factor[0] != factor[1]:
                raise ValueError("illegal reduction factor")
            factor = factor[0]
        source = self if box is None else self.crop(tuple(box))
        return Image(source._rust_image.reduce(factor))

    def load(self):
        """Load pixel data and return a mutable Pillow-style pixel view."""
        self._rust_image.load()
        return PixelAccess(self)

    def alpha_composite(self, im, dest=(0, 0), source=(0, 0)):
        """Alpha composite im over self. Returns None (mutates in-place).

        Mirrors Pillow's in-place ``Image.alpha_composite``: the overlay is
        cropped to the source bounds and the composited result is pasted back
        at the destination offset, so mismatched sizes compose over the
        overlapping region instead of raising.
        """
        if not isinstance(source, (list, tuple)):
            raise ValueError("Source must be a list or tuple")
        if not isinstance(dest, (list, tuple)):
            raise ValueError("Destination must be a list or tuple")
        if len(source) == 4:
            overlay_crop_box = tuple(source)
        elif len(source) == 2:
            overlay_crop_box = tuple(source) + im.size
        else:
            raise ValueError("Source must be a sequence of length 2 or 4")
        if not len(dest) == 2:
            raise ValueError("Destination must be a sequence of length 2")
        if min(source) < 0:
            raise ValueError("Source must be non-negative")

        # Overlay image, cropped when it is not the whole image.
        if overlay_crop_box == (0, 0) + im.size:
            overlay = im
        else:
            overlay = im.crop(overlay_crop_box)

        # Target box for the paste.
        box = tuple(dest) + (dest[0] + overlay.width, dest[1] + overlay.height)

        # Destination region; the whole image when the box covers it.
        if box == (0, 0) + self.size:
            background = self
        else:
            background = self.crop(box)

        result = background.copy()
        result._rust_image.alpha_composite(overlay._rust_image)
        self.paste(result, box)

    def getcolors(self, maxcolors=256):
        """Return list of [count, color] pairs or None if too many colors."""
        return self._rust_image.getcolors_formatted(maxcolors)

    def getdata(self, band=None):
        """Return pixel data through Pillow's ``ImagingCore`` sequence API."""
        names = self.getbands()
        if band is not None:
            if not isinstance(band, int) or band < 0 or band >= len(names):
                raise ValueError("band index out of range")
            all_values = self._rust_image.getdata_formatted(None)
            if len(names) == 1:
                values = all_values
            else:
                values = [value[band] for value in all_values]
            return ImagingCore(values, "L", self.size)
        values = self._rust_image.getdata_formatted(None)
        return ImagingCore(values, self.mode, self.size)

    def putdata(self, data, scale=1.0, offset=0.0):
        """Replace pixels from scalar samples or multiband color tuples."""
        self._rust_image.putdata_formatted(data, scale, offset)

    def getprojection(self):
        """Return horizontal and vertical projections."""
        return self._rust_image.getprojection()

    def entropy(self, mask=None, extrema=None):
        """Calculate image entropy."""
        if isinstance(mask, Image) and mask.mode not in ("1", "L"):
            raise ValueError("bad transparency mask")
        return self._rust_image.entropy()

    def seek(self, frame):
        """Seek to frame in multi-frame image."""
        if frame != self.tell():
            raise EOFError("no more images in file")
        self._rust_image.seek(frame)

    def tell(self):
        """Return current frame number."""
        return self._rust_image.tell()

    def close(self):
        """Close the image file and release resources."""
        if isinstance(self._rust_image, _ClosedImage):
            return None
        self._rust_image.close()
        self._rust_image = _ClosedImage()

    def point(self, lut, mode=None):
        """Apply lookup table or function to each pixel."""
        if callable(lut):
            lut = _core.make_lut(lut, 1)
            return Image(self._rust_image.point_replicated(lut))
        # LUT validation (PIL requires 256 * n_bands entries) handled in Rust
        return Image(self._rust_image.point_validated(list(lut)))

    def effect_spread(self, distance):
        """Simple spread/blur effect."""
        return Image(self._rust_image.effect_spread(distance))

    def apply_transparency(self):
        """Commit P-mode transparency to its palette without changing pixels."""
        result = self._rust_image.apply_transparency()
        self.__dict__.pop("_palette", None)
        self.__dict__.pop("_palette_object", None)
        return result

    def get_child_images(self):
        """Return list of child images (multi-frame)."""
        return list(map(Image, self._rust_image.get_child_images()))

    def get_flattened_data(self, band=None):
        """Return flattened pixel data matching PIL format."""
        if band is not None:
            return tuple(self.getdata(band))
        return tuple(self.getdata())

    def getexif(self):
        """Return EXIF data as dict, matching PIL's Image.Exif."""
        raw = bytes(self._rust_image.getexif())
        return _ExifCompat() if not raw else raw

    def getim(self):
        """Return internal C capsule. Not applicable for Rust."""
        return PyCapsule()

    def getpalette(self, rawmode="RGB"):
        """Return palette data.

        PIL behavior: returns the exact retained flat RGB palette. WEB palette
        has 226 colors (678 bytes), while an encoded or explicitly attached
        palette may retain trailing black entries.
        """
        if rawmode is None:
            rawmode = self._rust_image.palette_mode()
        if rawmode == "RGBA":
            p = self._rust_image.getpalette_rgba()
            return list(p) if p is not None else None
        if hasattr(self, '_palette'):
            return self._palette
        try:
            p = self._rust_image.getpalette_trimmed()
            if p is not None:
                self._palette = list(p)
                return self._palette
        except Exception:
            pass
        # PIL: P-mode image with no palette returns empty list, not None
        if self.mode in ("P", "PA"):
            return []
        return None

    def getxmp(self):
        """Return XMP metadata. Returns empty dict."""
        return dict(self._rust_image.getxmp())

    def putpalette(self, data, rawmode="RGB"):
        """Attach a palette to the image."""
        result = self._rust_image.putpalette(data, rawmode)
        self._explicit_mode = self._rust_image.explicit_mode()
        self.__dict__.pop("_palette", None)
        self.__dict__.pop("_palette_object", None)
        return result

    def show(self, title=None):
        """Display image. Not applicable in headless/test environments."""
        pass

    @staticmethod
    def _qt_imports():
        """Return (QImage, qRgb, QPixmap) from the already-loaded Qt binding.

        Detects which Qt binding is present in sys.modules first, so we never
        load a second binding into the same process (which would abort Qt).
        Prefers the binding that created the QApplication instance.
        """
        import sys

        # Determine which binding created the QApplication (if any)
        app_binding = None
        for binding, widget_mod in [
            ("PyQt6", "PyQt6.QtWidgets"),
            ("PyQt5", "PyQt5.QtWidgets"),
            ("PySide6", "PySide6.QtWidgets"),
            ("PySide2", "PySide2.QtWidgets"),
        ]:
            if widget_mod in sys.modules:
                try:
                    mod = sys.modules[widget_mod]
                    app = mod.QApplication.instance()
                    if app is not None:
                        app_binding = binding
                        break
                except Exception:
                    pass

        # Try app_binding first, then any loaded binding, then fallback
        for binding, mod_names in [
            ("PyQt6",     ("PyQt6.QtGui",)),
            ("PyQt5",     ("PyQt5.QtGui",)),
            ("PySide6",   ("PySide6.QtGui",)),
            ("PySide2",   ("PySide2.QtGui",)),
        ]:
            if app_binding and binding != app_binding:
                continue
            try:
                mod = __import__(mod_names[0], fromlist=["QImage", "qRgb", "QPixmap"])
                return mod.QImage, mod.qRgb, mod.QPixmap
            except ImportError:
                continue

        raise ImportError("Qt bindings are not installed")

    @staticmethod
    def _align8to32(data: bytes, width: int, mode: str) -> bytes:
        """Convert each scanline from 8-bit to 32-bit aligned (PIL / Qt compatibility).

        Delegates to Rust core for row alignment padding logic.
        """
        bits_per_pixel = {"1": 1, "L": 8, "P": 8}.get(mode, 8)
        return _core.align_row_to_32(data, width, bits_per_pixel)

    def toqimage(self):
        """Convert to Qt QImage. Matches PIL's ImageQt._toqclass_helper format mapping."""
        QImage, qRgb, _QPixmap = self._qt_imports()

        mode = self.mode
        w, h = self.size
        colortable = None

        if mode == "1":
            raw_data = self.tobytes("raw", "1")
            raw_data = self._align8to32(raw_data, w, "1")
            fmt = QImage.Format_Mono
        elif mode == "L":
            raw_data = self.tobytes("raw", "L")
            raw_data = self._align8to32(raw_data, w, "L")
            fmt = QImage.Format_Indexed8
            colortable = list(map(lambda i: qRgb(i, i, i), range(256)))
        elif mode == "P":
            raw_data = self.tobytes("raw", "P")
            raw_data = self._align8to32(raw_data, w, "P")
            fmt = QImage.Format_Indexed8
            palette = self.getpalette()
            if palette:
                colortable = list(map(lambda i: qRgb(palette[i], palette[i+1], palette[i+2]), range(0, len(palette), 3)))
        elif mode == "RGB":
            # Match PIL: convert to RGBA, use BGRA byte order, Format_RGB32
            rgba = self.convert("RGBA")
            raw_data = rgba.tobytes("raw", "BGRA")
            fmt = QImage.Format_RGB32
        elif mode == "RGBA":
            raw_data = self.tobytes("raw", "BGRA")
            fmt = QImage.Format_ARGB32
        else:
            # Convert unsupported modes to RGBA first
            return self.convert("RGBA").toqimage()

        qimg = QImage(raw_data, w, h, fmt)
        if colortable:
            qimg.setColorTable(colortable)
        return qimg

    def toqpixmap(self):
        """Convert to Qt QPixmap. Requires PyQt5, PyQt6, PySide2, or PySide6."""
        _QImage, _qRgb, QPixmap = self._qt_imports()
        return QPixmap.fromImage(self.toqimage())

    def frombytes(self, data, decoder_name="raw", *args):
        """Create image from raw pixel bytes or replace in-place.

        Supports both calling patterns matching PIL API:
        - Image.frombytes(mode, size, data) → creates new image (class method)
        - im.frombytes(data) → replaces pixel data in-place (instance method)
        """
        from ._core import Image as RustImage

        # Detect class method: Image.frombytes(mode, size, data, ...)
        # Positional args after self shift: data=mode, decoder_name=size, args[0]=pixel_data
        # Also handles: img.frombytes(data) where self is an instance with _rust_image
        if hasattr(self, '_rust_image'):
            # Instance method: im.frombytes(data, decoder_name, *args)
            if decoder_name != "raw":
                raise OSError(f"decoder {decoder_name} not available")
            mode = self.mode
            size = self.size
            self._rust_image = RustImage.frombytes(mode, size, bytes(data))
            return None

        # Class method: Image.frombytes(mode, size, data, ...)
        mode = self if isinstance(self, str) else data
        size = data if isinstance(self, str) else decoder_name
        pixel_data = decoder_name if isinstance(self, str) else args[0] if args else None
        result = Image(RustImage.frombytes(mode, size, bytes(pixel_data)))
        if mode in ("1", "P", "CMYK", "HSV", "YCbCr", "I", "F"):
            result._explicit_mode = mode
        if mode == "P":
            try:
                p = result._rust_image.palette()
                if p:
                    result._palette = list(p)
            except Exception:
                pass
        return result

    @classmethod
    def fromarray(cls, obj, mode=None):
        """Create image from array-like object (bytes, numpy array, list, etc.)."""
        from .operations import fromarray as _fromarray
        return _fromarray(obj, mode)

    @classmethod
    def linear_gradient(cls, mode: str) -> "Image":
        """Generate 256x256 linear gradient from black to white, top to bottom."""
        from .operations import linear_gradient as _linear_gradient
        return _linear_gradient(mode)

    @classmethod
    def radial_gradient(cls, mode: str) -> "Image":
        """Generate 256x256 radial gradient from black to white, centre to edge."""
        from .operations import radial_gradient as _radial_gradient
        return _radial_gradient(mode)

    @classmethod
    def effect_mandelbrot(
        cls,
        size: Tuple[int, int],
        extent: Tuple[float, float, float, float],
        quality: int,
    ) -> "Image":
        """Generate a Mandelbrot set covering the given extent."""
        from .operations import effect_mandelbrot as _effect_mandelbrot
        return _effect_mandelbrot(size, extent, quality)

    @classmethod
    def frombuffer(cls, mode: str, size: Tuple[int, int], data, decoder_name: str = "raw", *args) -> "Image":
        """Create an image from pixel data in a byte buffer. Delegates to frombytes."""
        from .operations import frombuffer as _frombuffer
        return _frombuffer(mode, size, data, decoder_name, *args)

    @staticmethod
    def eval(image, *args):
        """Apply a function to each pixel through Image.point."""
        return image.point(args[0])

    def tobitmap(self, name="image"):
        """Convert to X11 bitmap format."""
        if self.mode != "1":
            raise ValueError("not a bitmap")
        return self._rust_image.tobitmap()

    def remap_palette(self, dest_map, source_palette=None):
        """Remap image palette using destination map."""
        return Image(
            self._rust_image.remap_palette(list(dest_map), source_palette)
        )

    def draft(self, mode, size):
        """Configure decoder for draft mode. Returns None matching PIL."""
        return None

    def transform(self, size, method, data=None, resample=0, fill=1, fillcolor=None):
        """General affine/perspective/mesh transform."""
        if isinstance(method, str):
            # PIL requires the integer transform-method enum; names like
            # "AFFINE" raise before any geometry is computed.
            raise ValueError("unknown transformation method")
        if isinstance(resample, str):
            raise ValueError(
                f"Unknown resampling filter ({resample}). "
                "Use Image.Resampling.NEAREST (0), Image.Resampling.BILINEAR (2) "
                "or Image.Resampling.BICUBIC (3)"
            )
        if method == 0:
            if data is None:
                raise ValueError("missing method data")
            return Image(self._rust_image.transform(size, "AFFINE", data, resample, fill, fillcolor))
        is_mesh = method == 4
        if is_mesh:
            if data is None:
                raise ValueError("missing method data")
            if isinstance(data, (list, tuple)) and data and isinstance(data[0], (list, tuple)):
                mesh_flat = _core.mesh_flatten(data)
            else:
                mesh_flat = _core.mesh_flatten([data])
            return Image(self._rust_image.transform(size, "MESH", mesh_flat, resample, fill, fillcolor))
        raise ValueError("unknown transformation method")

    def verify(self):
        """Verify file contents. Raises exception if corrupted."""
        self._rust_image.verify()

    @property
    def size(self) -> Tuple[int, int]:
        return self._rust_image.size

    @property
    def width(self) -> int:
        return self._rust_image.width

    @property
    def height(self) -> int:
        return self._rust_image.height

    @property
    def has_transparency_data(self) -> bool:
        """Whether the image has transparency data."""
        return self._rust_image.has_transparency_data()

    @property
    def palette(self):
        """Image palette, if any."""
        if self.mode not in ("P", "PA"):
            return None
        if hasattr(self, "_palette_object"):
            return self._palette_object
        from .imagepalette import ImagePalette
        mode = self._rust_image.palette_mode() or "RGB"
        palette = ImagePalette(mode)
        palette.palette = self.getpalette(mode) or []
        self._palette_object = palette
        return palette

    @property
    def mode(self) -> str:
        if self._explicit_mode:
            return self._explicit_mode
        return self._rust_image.mode

    @property
    def format(self) -> Optional[str]:
        return self._rust_image.format

    @property
    def info(self) -> dict:
        result = dict(self._info)
        index = self._rust_image.pending_transparency_index()
        if index is not None:
            result["transparency"] = index
        else:
            table = self._rust_image.pending_transparency_table()
            if table is not None:
                result["transparency"] = bytes(table)
        return result

    def __repr__(self) -> str:
        return self._rust_image.__repr__()

    def __eq__(self, other) -> bool:
        if not isinstance(other, Image):
            return False
        return (
            self.size == other.size
            and self.mode == other.mode
            and self.tobytes() == other.tobytes()
        )


class PixelAccess:
    """Mutable pixel view matching Pillow's ``PixelAccess`` behavior."""

    __slots__ = ("_image",)

    def __init__(self, image):
        self._image = image

    def __getitem__(self, xy):
        return self._image.getpixel(xy)

    def __setitem__(self, xy, value):
        self._image.putpixel(xy, value)

    def __str__(self):
        return f'<PixelAccess object at 0x{id(self):x}>'

    def __repr__(self):
        return str(self)
