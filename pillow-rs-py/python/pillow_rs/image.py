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
    "I": ("I",),
}


class Image:
    """A high-performance image class backed by Rust. Pillow-compatible API."""

    def __init__(self, rust_image=None):
        if RustImage is None:
            raise ImportError("pillow-rs Rust extension not available.")
        if rust_image is None:
            rust_image = RustImage()
        self._rust_image = rust_image

    @classmethod
    def open(
        cls,
        fp: Union[str, Path, bytes],
        mode: Optional[str] = None,
        formats: Optional[list] = None,
    ) -> "Image":
        if isinstance(fp, Path):
            fp = str(fp)
        rust_image = RustImage.open(fp)
        return cls(rust_image)

    @classmethod
    def new(
        cls,
        mode: str,
        size: Tuple[int, int],
        color: Union[int, Tuple[int, ...], str, None] = 0,
    ) -> "Image":
        rust_image = RustImage.new(mode, size, color)
        return cls(rust_image)

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
        if angle not in [0, 90, 180, 270]:
            raise NotImplementedError(
                f"Arbitrary angle rotation ({angle}°) not yet implemented."
            )
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
        matrix_list = list(matrix) if matrix is not None else None
        rust_image = self._rust_image.convert(
            mode, matrix=matrix_list, dither=dither, palette=palette, colors=colors
        )
        return Image(rust_image)

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
        return Image(self._rust_image.copy())

    def filter(self, filter_type) -> "Image":
        rust_image = self._rust_image.filter(str(filter_type))
        return Image(rust_image)

    def thumbnail(
        self,
        size: Tuple[int, int],
        resample: Union[int, str] = Resampling.BICUBIC,
    ) -> None:
        if isinstance(resample, int):
            resample = Resampling.from_int(resample)
        current_width, current_height = self.size
        max_width, max_height = size
        width_ratio = max_width / current_width
        height_ratio = max_height / current_height
        scale = min(width_ratio, height_ratio)
        new_width = int(current_width * scale)
        new_height = int(current_height * scale)
        self._rust_image = self._rust_image.resize(
            (new_width, new_height), resample
        )

    def tobytes(self, encoder_name: str = "raw", *args) -> bytes:
        return self._rust_image.tobytes()

    def getpixel(self, xy: Tuple[int, int]):
        """Get pixel value at (x, y). Returns tuple matching image mode."""
        r, g, b, a = self._rust_image.getpixel(xy)
        if self.mode == "L":
            return r
        elif self.mode == "LA":
            return (r, a)
        elif self.mode == "RGB":
            return (r, g, b)
        elif self.mode == "RGBA":
            return (r, g, b, a)
        return (r, g, b)

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

    def close(self):
        """Close the image file and release resources."""
        self._rust_image.close()

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
