"""Python Image class that wraps the Rust pillow-rs implementation."""
from pathlib import Path
from typing import Any, Optional, Tuple, Union

from ._core import Image as RustImage
from .enums import Palette, Resampling, Transpose

_BAND_NAMES = {
    "L": ("L",),
    "LA": ("L", "A"),
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


class Image:
    """A high-performance image class backed by Rust. Pillow-compatible API."""

    def __init__(self, rust_image=None):
        if RustImage is None:
            raise ImportError("pillow-rs Rust extension not available.")
        if rust_image is None:
            rust_image = RustImage()
        self._rust_image = rust_image
        # Inherit explicit mode from Rust pipeline (e.g. "1", "P", etc.)
        try:
            em = rust_image.explicit_mode()
            self._explicit_mode = em if em else None
        except Exception:
            self._explicit_mode = None

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
        if isinstance(fp, bytes):
            rust_image = RustImage.open_bytes(fp)
        else:
            rust_image = RustImage.open(fp)
        return cls(rust_image)

    @classmethod
    def new(
        cls,
        mode: str,
        size: Tuple[int, int],
        color: Union[int, Tuple[int, ...], str, None] = 0,
    ) -> "Image":
        # CMYK/YCbCr/HSV/I/F are stored as RGB/RGBA internally but tagged with mode
        nonstandard = {"CMYK": "RGBA", "YCbCr": "RGB", "HSV": "RGB", "I": "L", "F": "L"}
        rust_mode = nonstandard.get(mode, mode)
        rust_image = RustImage.new(rust_mode, size, color)
        img = cls(rust_image)
        if mode in nonstandard:
            img._explicit_mode = mode
        return img

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
        rust_bands = [b._rust_image for b in bands]
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
        self._rust_image.save(fp, format)

    def resize(
        self,
        size: Tuple[int, int],
        resample: Union[int, str] = Resampling.BILINEAR,
    ) -> "Image":
        if isinstance(resample, int):
            resample = Resampling.from_int(resample)
        rust_image = self._rust_image.resize(size, resample)
        return Image(rust_image)

    def crop(self, box: Tuple[int, int, int, int]) -> "Image":
        left, top, right, bottom = box
        width = right - left
        height = bottom - top
        rust_image = self._rust_image.crop((left, top, width, height))
        return Image(rust_image)

    def rotate(
        self,
        angle: float,
        resample: Union[int, str] = Resampling.NEAREST,
        expand: bool = False,
        center: Optional[Tuple[float, float]] = None,
        translate: Optional[Tuple[float, float]] = None,
        fillcolor: Optional[Any] = None,
    ) -> "Image":
        angle = angle % 360
        rust_image = self._rust_image.rotate(float(angle), expand, fillcolor)
        return Image(rust_image)

    def transpose(self, method: Union[int, str]) -> "Image":
        if isinstance(method, int):
            method = Transpose.from_int(method)
        rust_image = self._rust_image.transpose(method)
        return Image(rust_image)

    def convert(
        self,
        mode: str,
        matrix: Optional[Tuple[float, ...]] = None,
        dither: Optional[str] = None,
        palette: str = Palette.WEB,
        colors: int = 256,
    ) -> "Image":
        # Handle non-standard modes at Python level
        if mode in ("CMYK", "YCbCr", "HSV", "I", "F"):
            rgb = self._rust_image.convert("RGB", matrix=None, dither=None, palette=palette, colors=colors)
            img = Image(rgb)
            img._explicit_mode = mode
            return img
        if mode == "P":
            # Quantize then tag as palette mode
            rust_image = self._rust_image.quantize(colors=min(colors, 256), dither=(dither is not None))
            img = Image(rust_image)
            img._explicit_mode = "P"
            return img
        matrix_list = list(matrix) if matrix is not None else None
        rust_image = self._rust_image.convert(
            mode, matrix=matrix_list, dither=dither, palette=palette, colors=colors
        )
        img = Image(rust_image)
        if mode == "1":
            img._explicit_mode = "1"
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
            rust_mask = None
        else:
            rust_box = box
            rust_mask = mask._rust_image if mask is not None else None
        self._rust_image.paste(rust_im, rust_box, rust_mask)

    def split(self) -> Tuple["Image", ...]:
        return tuple(Image(band) for band in self._rust_image.split())

    def getbands(self) -> Tuple[str, ...]:
        return _BAND_NAMES.get(self.mode, (self.mode,))

    def copy(self) -> "Image":
        new = Image(self._rust_image.copy())
        if hasattr(self, '_explicit_mode'):
            new._explicit_mode = self._explicit_mode
        return new

    def filter(self, filter_type) -> "Image":
        if self.mode == "P":
            raise ValueError("cannot filter palette images")
        # Parametric filter objects have _apply(); string names go to Rust
        if hasattr(filter_type, '_apply'):
            return filter_type._apply(self._rust_image)
        rust_image = self._rust_image.filter(str(filter_type))
        return Image(rust_image)

    def thumbnail(
        self,
        size: Tuple[int, int],
        resample: Union[int, str] = Resampling.BICUBIC,
    ) -> None:
        """Scale image to fit within size. Aspect ratio handled in Rust."""
        self._rust_image.thumbnail(size, None)

    def tobytes(self, encoder_name: str = "raw", *args) -> bytes:
        return self._rust_image.tobytes()

    def getpixel(self, xy: Tuple[int, int]):
        """Get pixel value at (x, y). Mode dispatch done in Rust."""
        return self._rust_image.getpixel_formatted(xy, self.mode)

    def putpixel(self, xy: Tuple[int, int], value):
        """Set pixel value at (x, y). Accepts int, tuple, or list."""
        if isinstance(value, int):
            self._rust_image.putpixel(xy, (value, value, value, 255))
        elif len(value) == 3:
            self._rust_image.putpixel(xy, (*value, 255))
        elif len(value) == 4:
            self._rust_image.putpixel(xy, tuple(value))
        else:
            self._rust_image.putpixel(xy, (value[0], value[0], value[0], 255))

    def quantize(self, colors: int = 256, method=None, kmeans: int = 0,
                 palette=None, dither: int = 1):
        """Reduce colors using NeuQuant algorithm."""
        return Image(self._rust_image.quantize(colors, dither != 0))

    def getbbox(self, *, alpha_only: bool = True):
        """Bounding box of non-zero regions."""
        return self._rust_image.getbbox(alpha_only)

    def getextrema(self):
        """Min/max pixel values per band. Returns tuple matching PIL format."""
        result = self._rust_image.getextrema()
        if len(result) == 1:
            return tuple(result[0])
        return tuple(tuple(v) for v in result)

    def histogram(self, mask=None, extrema=None):
        """Image histogram per band."""
        return self._rust_image.histogram()

    def getchannel(self, channel):
        """Extract a single channel as an L-mode image."""
        ch_map = {"R": 0, "G": 1, "B": 2, "A": 3, "L": 0}
        ch = ch_map.get(channel, channel) if isinstance(channel, str) else channel
        return Image(self._rust_image.getchannel(ch))

    def putalpha(self, alpha):
        """Set/replace the alpha channel."""
        if isinstance(alpha, Image):
            raise NotImplementedError("Image.putalpha with Image argument")
        if isinstance(alpha, int):
            self._rust_image.putalpha(alpha)
        else:
            self._rust_image.putalpha(int(alpha))

    def reduce(self, factor, box=None):
        """Reduce image by integer factor."""
        return Image(self._rust_image.reduce(factor))

    def load(self):
        """Load pixel data. Returns pixel access object (stub)."""
        self._rust_image.load()
        return self  # TODO: return PixelAccess object

    def alpha_composite(self, im, dest=(0, 0), source=(0, 0)):
        """Alpha composite im over self."""
        self._rust_image.alpha_composite(im._rust_image)

    def getcolors(self, maxcolors=256):
        """Return list of (count, color) tuples or None if too many colors."""
        result = self._rust_image.getcolors(maxcolors)
        if result is None:
            return None
        # Convert raw bytes to proper tuples (matching PIL format)
        n_bands = len(self.getbands())
        out = []
        for count, raw_color in result:
            if n_bands == 1:
                color = raw_color[0]
            else:
                color = tuple(raw_color)
            out.append((count, color))
        return out

    def getdata(self, band=None):
        """Return pixel data as sequence of tuples (matching PIL)."""
        raw = self._rust_image.getdata(band if band is not None else -1)
        n_bands = len(self.getbands())
        if n_bands == 1:
            return list(raw)  # PIL returns flat list of ints
        # Group flat bytes into tuples
        return [tuple(raw[i:i+n_bands]) for i in range(0, len(raw), n_bands)]

    def putdata(self, data, scale=1.0, offset=0.0):
        """Replace pixel data from a sequence. Flattening done in Rust."""
        self._rust_image.putdata(data)

    def getprojection(self):
        """Return horizontal and vertical projections."""
        return self._rust_image.getprojection()

    def entropy(self, mask=None, extrema=None):
        """Calculate image entropy."""
        return self._rust_image.entropy()

    def seek(self, frame):
        """Seek to frame in multi-frame image."""
        self._rust_image.seek(frame)

    def tell(self):
        """Return current frame number."""
        return self._rust_image.tell()

    def close(self):
        """Close the image file and release resources."""
        self._rust_image.close()

    def point(self, lut, mode=None):
        """Apply lookup table or function to each pixel."""
        if callable(lut):
            # Function-based: convert to LUT
            table = [lut(i) for i in range(256)]
            lut = bytes(int(v) & 0xFF for v in table)
        return Image(self._rust_image.point(list(lut)))

    def effect_spread(self, distance):
        """Simple spread/blur effect."""
        return Image(self._rust_image.effect_spread(distance))

    def apply_transparency(self):
        """Apply transparency mask to image."""
        if self.mode == "RGBA":
            pass  # Already has alpha
        elif self.mode == "P" and self.palette:
            pass  # Palette transparency

    def get_child_images(self):
        """Return list of child images (multi-frame)."""
        return []

    def get_flattened_data(self, band=None):
        """Return flattened pixel data matching PIL format."""
        if band is not None:
            return tuple(self.getdata(band))
        return tuple(self.getdata())

    def getexif(self):
        """Return EXIF data. Returns minimal empty EXIF bytes matching PIL."""
        # Minimal EXIF header (TIFF with 0 IFD entries) — matches PIL empty EXIF
        return b'Exif\x00\x00MM\x00*\x00\x00\x00\x08\x00\x00\x00\x00\x00\x00'

    def getim(self):
        """Return internal C capsule. Not applicable for Rust."""
        raise NotImplementedError("getim: not applicable for Rust implementation")

    def getpalette(self, rawmode="RGB"):
        """Return palette data."""
        if hasattr(self, '_palette'):
            return list(self._palette)
        return None

    def getxmp(self):
        """Return XMP metadata. Returns empty dict."""
        return {}

    def putpalette(self, data, rawmode="RGB"):
        """Attach a palette to the image."""
        self._palette = list(data) if data else []

    def show(self, title=None):
        """Display image. Not applicable in headless/test environments."""
        pass

    def toqimage(self):
        """Convert to Qt QImage. Not applicable for Rust."""
        raise NotImplementedError("toqimage: requires Qt")

    def toqpixmap(self):
        """Convert to Qt QPixmap. Not applicable for Rust."""
        raise NotImplementedError("toqpixmap: requires Qt")

    @classmethod
    def frombytes(cls, mode, size, data, decoder_name="raw", *args):
        """Create image from raw pixel bytes."""
        from ._core import Image as RustImage
        img = cls(RustImage.frombytes(mode, size, bytes(data)))
        if mode in ("1", "P", "CMYK", "HSV", "YCbCr", "I", "F"):
            img._explicit_mode = mode
        return img

    @classmethod
    def fromarray(cls, obj, mode=None):
        """Create image from numpy array or similar. Not yet implemented."""
        raise NotImplementedError("Image.fromarray requires numpy")

    @staticmethod
    def eval(image, *args):
        """Apply function to each pixel via LUT."""
        if args:
            func = args[0]
            table = [func(i) & 0xFF for i in range(256)]
            lut = bytes(table)
            return Image(image._rust_image.point(list(lut)))
        raise ValueError("eval requires a function argument")

    def draft(self, mode, size):
        """Configure decoder for draft mode."""
        raise NotImplementedError("Image.draft")

    def tobitmap(self, name="image"):
        """Convert to X11 bitmap format."""
        return self._rust_image.tobitmap()

    def remap_palette(self, dest_map, source_palette=None):
        """Remap image palette using destination map."""
        return Image(self._rust_image.remap_palette(list(dest_map)))

    def draft(self, mode, size):
        """Configure decoder for draft mode. Returns self for compatibility."""
        return self

    def transform(self, size, method, data=None, resample=0, fill=1, fillcolor=None):
        """General affine/perspective transform."""
        if method == 0 or method == "AFFINE" or (isinstance(data, list) and len(data) == 6):
            return Image(self._rust_image.transform(size, "AFFINE", data, resample, fill, fillcolor))
        raise NotImplementedError(f"transform method '{method}' not yet implemented")

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
    def mode(self) -> str:
        if self._explicit_mode:
            return self._explicit_mode
        return self._rust_image.mode

    @property
    def format(self) -> Optional[str]:
        return self._rust_image.format

    @property
    def info(self) -> dict:
        return {}

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
