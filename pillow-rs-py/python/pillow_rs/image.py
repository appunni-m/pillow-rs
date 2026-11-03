"""Internal Python Image class that wraps the Rust implementation."""
from abc import ABC, abstractmethod
from copy import deepcopy
from math import floor as _floor
from operator import index as _index
from pathlib import Path
from struct import iter_unpack as _iter_unpack
from sys import maxsize as _MAX_INDEX
from typing import Any, Optional, Protocol, Sequence, Tuple, Union

from . import _core
from ._core import Image as RustImage
from .enums import Palette, Resampling, Transform, Transpose


def _image_data_bands(mode):
    if mode in ("1", "L", "P", "I", "F", "I;16", "I;16L", "I;16B", "I;16N"):
        return 1
    if mode in ("LA", "La", "PA"):
        return 2
    if mode in ("RGB", "YCbCr", "HSV", "LAB"):
        return 3
    return 4


class _CompactImageDataSequence:
    """Lazy tuple view over compact interleaved byte samples."""

    __slots__ = ("_data", "_bands")

    def __init__(self, data, bands):
        self._data = data
        self._bands = bands

    def __len__(self):
        return len(self._data) // self._bands

    def __iter__(self):
        if len(self._data) % self._bands:
            # Preserve the previous behavior for malformed internal buffers:
            # keep the trailing partial tuple rather than letting
            # struct.iter_unpack raise.
            return (
                tuple(self._data[offset : offset + self._bands])
                for offset in range(0, len(self._data), self._bands)
            )
        return _iter_unpack(f"{self._bands}B", self._data)

    def __getitem__(self, index):
        if isinstance(index, slice):
            raise TypeError("sequence index must be integer, not 'slice'")
        try:
            index = _index(index)
        except TypeError:
            raise TypeError(
                f"sequence index must be integer, not '{type(index).__name__}'"
            ) from None
        if index > _MAX_INDEX or index < -_MAX_INDEX - 1:
            raise IndexError("cannot fit 'int' into an index-sized integer")
        if index < 0:
            index += len(self)
        if index < 0 or index >= len(self):
            raise IndexError("image index out of range")
        offset = index * self._bands
        return tuple(self._data[offset : offset + self._bands])


class _ImageDataSequence:
    """Pillow-style live sequence over one image's current pixel storage."""

    __slots__ = ("_image", "_bands", "_size", "_band")

    def __init__(self, image, mode, size, band=None):
        self._image = image
        self._bands = _image_data_bands(mode)
        self._size = size
        self._band = band

    def __len__(self):
        return self._size[0] * self._size[1]

    def _materialized_values(self):
        values = self._image.getdata_formatted(self._band)
        if isinstance(values, bytes) and self._bands > 1:
            return _CompactImageDataSequence(values, self._bands)
        return values

    def __iter__(self):
        return iter(self._materialized_values())

    def __getitem__(self, index):
        if isinstance(index, slice):
            raise TypeError("sequence index must be integer, not 'slice'")
        try:
            index = _index(index)
        except TypeError:
            raise TypeError(
                f"sequence index must be integer, not '{type(index).__name__}'"
            ) from None
        if index > _MAX_INDEX or index < -_MAX_INDEX - 1:
            raise IndexError("cannot fit 'int' into an index-sized integer")
        if index < 0:
            index += len(self)
        if index < 0 or index >= len(self):
            raise IndexError("image index out of range")
        x = index % self._size[0]
        y = index // self._size[0]
        value = self._image.getpixel_formatted((x, y))
        if self._band is not None and isinstance(value, (tuple, list)):
            return value[self._band]
        return value


class ImagingCore:
    """Sequence view matching Pillow's internal ``ImagingCore`` contract."""

    __slots__ = ("_values", "mode", "size")

    def __init__(self, values, mode=None, size=None):
        if isinstance(values, bytes) and mode is not None and _image_data_bands(mode) > 1:
            values = _CompactImageDataSequence(values, _image_data_bands(mode))
        self._values = values
        self.mode = mode
        self.size = size

    def __iter__(self):
        return iter(self._values)

    def __len__(self):
        return len(self._values)

    def __getitem__(self, index):
        if isinstance(index, slice):
            raise TypeError("sequence index must be integer, not 'slice'")
        try:
            index = _index(index)
        except TypeError:
            raise TypeError(
                f"sequence index must be integer, not '{type(index).__name__}'"
            ) from None
        if index > _MAX_INDEX or index < -_MAX_INDEX - 1:
            raise IndexError("cannot fit 'int' into an index-sized integer")
        length = len(self)
        if index < 0:
            index += length
        if index < 0 or index >= length:
            raise IndexError("image index out of range")
        return self._values[index]

    def __bytes__(self):
        """Expose scalar band data like Pillow's ImagingCore.

        ``bytes(ImagingCore)`` is only meaningful for a one-band sequence;
        Pillow does not flatten multiband tuples implicitly.
        """
        if isinstance(self._values, bytes):
            return self._values
        if isinstance(self._values, _CompactImageDataSequence):
            if not len(self._values):
                return self._values._data
            raise TypeError("'tuple' object cannot be interpreted as an integer")
        if isinstance(self._values, _ImageDataSequence):
            if self._values._bands > 1:
                if not len(self._values):
                    return b""
                raise TypeError("'tuple' object cannot be interpreted as an integer")
            values = self._values._materialized_values()
            if isinstance(values, bytes):
                return values
            return _core.imaging_core_to_bytes(values)
        return _core.imaging_core_to_bytes(self._values)

    def tobytes(self):
        return bytes(self)


class _ExifCompat:
    """Small Pillow ``Exif`` object shape for the public ``getexif`` result.

    The Rust core retains the encoded EXIF payload but deliberately does not
    expose Python's mutable ``Exif`` mapping type.  Keep the binding result
    shaped like Pillow's lazily-loaded object; tag decoding remains a future
    core concern and is not needed merely to expose the loaded record.
    """

    def __init__(self, raw=None, *, loaded_exif=True):
        self._data = {}
        self._hidden_data = {}
        self._ifds = {}
        self._info = None
        self.__dict__.update(_core.exif_compat_fields(raw, loaded_exif))


class UnidentifiedImageError(OSError):
    """Pillow-compatible class for bytes that no registered decoder accepts."""


class SupportsGetData(Protocol):
    """Protocol for Pillow's legacy transform-method data provider."""

    def getdata(self) -> Tuple[Transform, Sequence[int]]:
        """Return a transform method and the method-specific data."""
        ...


class ImageTransformHandler(ABC):
    """Base class for custom handlers passed to :meth:`Image.transform`."""

    @abstractmethod
    def transform(self, size: Tuple[int, int], image: "Image", **options: Any) -> "Image":
        """Return the transformed image using the provided Pillow options."""
        raise NotImplementedError


class _ClosedImage:
    """Released image storage that preserves Pillow's closed-image error."""

    def close(self):
        return None

    def __getattr__(self, _name):
        raise ValueError("Operation on closed image")


class Image:
    """A high-performance image class backed by Rust.

    The public ``PIL.Image.Image`` facade aliases this class.  Keeping the
    implementation independent of an installed Pillow package prevents the
    target facade from importing the oracle namespace while it is being
    initialized.
    """

    # Resampling constants matching PIL.Image.<name> access pattern
    NEAREST = Resampling.NEAREST
    BILINEAR = Resampling.BILINEAR
    BICUBIC = Resampling.BICUBIC
    LANCZOS = Resampling.LANCZOS
    Resampling = Resampling
    Transform = Transform
    Transpose = Transpose
    AFFINE = Transform.AFFINE
    EXTENT = Transform.EXTENT
    PERSPECTIVE = Transform.PERSPECTIVE
    QUAD = Transform.QUAD
    MESH = Transform.MESH
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
        self._native_info = None
        # Set only for Image.new results, whose Rust metadata is known empty.
        self._native_info_known_empty = False
        self._native_info_rebaseline = False
        self._native_info_omitted = frozenset()
        self._transpose_loads_info = False
        self._verify_encoded_source = False
        self._has_getdata_views = False

    def _sync_observed_info(self):
        # A caller may retain the dictionary itself across an in-place core
        # mutation. Refresh that same dictionary, while unobserved metadata
        # stays lazy.
        if self._native_info is not None:
            self.info

    def _copy_info_state_from(self, source):
        """Copy the public info mapping and its lazy-merge bookkeeping.

        The mapping is shallow-copied, so nested values keep Pillow's identity
        semantics. The native snapshot stays shared until the result rebases it.
        """
        self._info = source._info.copy()
        self._native_info = source._native_info
        self._native_info_rebaseline = True
        self._native_info_omitted = source._native_info_omitted

    def _new(self, rust_image):
        """Wrap a derived image with Pillow's shallow metadata-copy semantics."""
        if self._transpose_loads_info:
            self.load()
            self.info
        result = Image(rust_image)
        result._copy_info_state_from(self)
        return result

    def _ensure_materialized(self):
        """Ensure the underlying Rust image is materialized (not Paletted/Path)."""
        if hasattr(self._rust_image, 'materialize'):
            self._native_info_known_empty = False
            self._rust_image = self._rust_image.materialize()

    @classmethod
    def open(
        cls,
        fp: Union[str, Path, bytes],
        mode: Optional[str] = None,
        formats: Optional[list] = None,
    ) -> "Image":
        """Open an image file. Format detection and mode handling done in Rust."""
        # Keep validation outside the decode-error translation below; the
        # Rust core owns the rule, while Pillow exposes bad arguments directly
        # instead of relabeling them as an unidentified image.
        RustImage.validate_open_inputs(mode, formats)
        RustImage.validate_open_source(fp)
        try:
            rust_image = RustImage.open(fp, mode, formats)
        except (FileNotFoundError, IsADirectoryError):
            raise
        except Exception as exc:
            raise UnidentifiedImageError(f"cannot identify image file '{fp}'") from exc
        result = cls(rust_image)
        result._verify_encoded_source = True
        # WebP frame timing becomes public metadata during Pillow's load in
        # transpose. Cache that decoder requirement once when opening the
        # file; ordinary images never need a format lookup in transpose.
        result._transpose_loads_info = rust_image.format == "WEBP"
        return result

    @classmethod
    def new(
        cls,
        mode: str,
        size: Tuple[int, int],
        color: Union[int, Tuple[int, ...], str, None] = None,
    ) -> "Image":
        # Preserve omission across the Python facade so the Rust core owns
        # Pillow's mode-specific default-color path.
        result = cls(RustImage.new(mode, size, color))
        # Image.new has no source format, decoder fields, or pending metadata.
        result._native_info_known_empty = True
        return result

    @classmethod
    def effect_noise(cls, size: Tuple[int, int], sigma: float) -> "Image":
        """Generate Gaussian noise image."""
        return cls(RustImage.effect_noise_from_size(size, sigma))

    def save(
        self, fp: Union[str, Path], format: Optional[str] = None, **options
    ) -> None:
        # This Image owns one still frame, so save_all has no sequence to add.
        # Preserve PNG's public compression override used by ImageShow viewers.
        self._rust_image.save(fp, format, options.get("compress_level"))

    def resize(
        self,
        size: Tuple[int, int],
        resample: Union[int, str] = Resampling.BICUBIC,
        box=None,
        reducing_gap=None,
    ) -> "Image":
        palette = self.__dict__.get("_palette_object")
        if palette is not None and palette.dirty and self.mode in ("P", "PA"):
            # Pillow validates the filter and reducing gap, normalizes size,
            # then takes its identity-copy or source-load path.
            # TODO(pillow-rs#3ptb): parity-check custom geometry/gap objects
            # whose comparisons or numeric conversion raise at this boundary.
            filter_code = self._rust_image._resize_filter_code(resample)
            if reducing_gap is not None and reducing_gap < 1.0:
                raise ValueError("reducing_gap must be 1.0 or greater")
            if box is None:
                box = (0, 0) + self.size
            size = tuple(size)
            full_box = (0, 0) + self.size
            if self.size == size and box == full_box:
                # The identity copy loads but does not use reducing_gap.
                # Pass canonical geometry so Pillow-compatible identity inputs
                # avoid later integer/box coercion in the Rust binding.
                size = self.size
                box = full_box
                reducing_gap = None
            self._realize_dirty_palette()
            return self._new(
                self._rust_image.resize(size, filter_code, box, reducing_gap)
            )
        return self._new(self._rust_image.resize(size, resample, box, reducing_gap))

    def crop(self, box: Optional[Tuple[float, float, float, float]] = None) -> "Image":
        # Pillow rejects reversed coordinates before loading the source.
        if box is not None:
            if box[2] < box[0]:
                raise ValueError("Coordinate 'right' is less than 'left'")
            if box[3] < box[1]:
                raise ValueError("Coordinate 'lower' is less than 'upper'")
        self._realize_dirty_palette()
        return self._new(self._rust_image.crop(box))

    def rotate(
        self,
        angle: float,
        resample: Union[int, str] = Resampling.NEAREST_INT,
        expand: bool = False,
        center: Optional[Tuple[float, float]] = None,
        translate: Optional[Tuple[float, float]] = None,
        fillcolor: Optional[Any] = None,
    ) -> "Image":
        angle = float(angle)
        palette = self.__dict__.get("_palette_object")
        if palette is not None and palette.dirty and self.mode in ("P", "PA"):
            # Pillow validates Rotate inputs before loading. Prepare the typed
            # request once, commit the dirty palette, then queue against the
            # updated source metadata.
            plan = self._rust_image._prepare_rotate(
                angle, resample, expand, center, translate, fillcolor
            )
            self._realize_dirty_palette()
            rust_image = self._rust_image._rotate_prepared(plan)
        else:
            rust_image = self._rust_image.rotate(
                angle, resample, expand, center, translate, fillcolor
            )
        return self._new(rust_image)

    def transpose(self, method: Union[int, str]) -> "Image":
        # Pillow loads before the core transpose validates its method.
        self._realize_dirty_palette()
        if self._transpose_loads_info:
            self.load()
            self.info
        rust_image = self._rust_image.transpose(method)
        # Pillow's _new copies the mapping, while nested metadata values stay
        # shared. Native metadata remains lazy for images whose info was never
        # observed; the Rust transpose already retains that provenance.
        return self._new(rust_image)

    def convert(
        self,
        mode: Optional[str] = None,
        matrix: Optional[Tuple[float, ...]] = None,
        dither: Optional[str] = None,
        palette: str = Palette.WEB,
        colors: int = 256,
    ) -> "Image":
        # Pillow's convert loads before resolving or validating the target.
        self._realize_dirty_palette()
        rust_image = self._rust_image.convert(
            mode, matrix=matrix, dither=dither, palette=palette, colors=colors
        )
        img = self._new(rust_image)
        img._info.update(
            self._rust_image.converted_compatibility_info(img.mode)
        )
        return img

    def paste(
        self,
        im: Union["Image", Tuple[int, ...], int],
        box: Union[
            "Image", Tuple[int, int], Tuple[int, int, int, int], None
        ] = None,
        mask: Optional["Image"] = None,
    ) -> None:
        # Pillow handles Image-as-mask shorthand before normal box processing.
        if isinstance(box, Image):
            if mask is not None:
                raise ValueError(
                    "If using second argument as mask, third argument must be None"
                )
            mask = box
            box = None
        # Pillow inspects box length and expands a two-coordinate box before
        # resolving or loading the source, then passes coordinates after loads.
        if box is None:
            box = (0, 0)
        box_length = len(box)
        if box_length == 2:
            if isinstance(im, Image):
                size = im.size
            elif isinstance(mask, Image):
                size = mask.size
            else:
                raise ValueError("cannot determine region size; use 4-item box")
            box += (box[0] + size[0], box[1] + size[1])
            box_length = 4
        box_plan = self._rust_image._paste_prepare_box(
            box,
            mask is not None,
            isinstance(im, Image) or isinstance(mask, Image),
            box_length,
        )
        effective_mask = mask
        is_image_source = isinstance(im, Image)
        resolved_image_source = False
        if is_image_source:
            im.load()
            requires_conversion = im.mode != self.mode and (
                self.mode != "RGB" or im.mode not in ("LA", "RGBA", "RGBa")
            )
            if not requires_conversion:
                resolved_image_source = True
            else:
                convert_method = im.convert
                if (
                    type(im) is not Image
                    or "convert" in getattr(im, "__dict__", {})
                    or getattr(convert_method, "__func__", convert_method)
                    is not _BASE_IMAGE_CONVERT_METHOD
                ):
                    # Let Python overrides run before destination and mask
                    # callbacks, as Pillow does. The exact base Image path
                    # keeps its mode strategy in Rust.
                    # Pillow 12.3.0 compatibility: its paste wrapper trusts a
                    # convert() override's returned mode, while the native
                    # paste path accepts matching physical pixel sizes. Keep
                    # that observable behavior without promising that arbitrary
                    # modes are interchangeable through the public API.
                    im = convert_method(self.mode)
                    resolved_image_source = True
        # Image conversion and color-string resolution happen before the
        # destination load; raw numeric source coercion is retained for the
        # final core call, after destination and mask loading.
        if is_image_source and resolved_image_source:
            source_plan = self._rust_image._paste_prepare_resolved_image_source(
                im._rust_image
            )
        else:
            source_plan = self._rust_image._paste_prepare_source(
                im, effective_mask is not None, is_image_source
            )
        self.load()
        self._native_info_known_empty = False
        mask_is_active = bool(effective_mask) if effective_mask is not None else False
        if mask_is_active:
            effective_mask.load()
        self._rust_image._paste_apply_prepared(
            box_plan,
            source_plan,
            mask if mask_is_active else None,
        )

    def split(self) -> Tuple["Image", ...]:
        # Pillow loads pixel storage before splitting the bands.
        self.load()
        return tuple(self._new(image) for image in self._rust_image.split())

    def getbands(self) -> Tuple[str, ...]:
        return self._rust_image.getbands()

    def copy(self) -> "Image":
        self._realize_dirty_palette()
        return self._new(self._rust_image.copy())

    def filter(self, filter_type) -> "Image":
        # Pillow loads before validating the filter argument.
        self.load()
        return self._new(self._rust_image.filter(filter_type))

    def thumbnail(
        self,
        size: Tuple[int, int],
        resample: Union[int, str] = Resampling.BICUBIC,
        reducing_gap=2.0,
    ) -> None:
        """Scale image to fit within size. Aspect ratio handled in Rust."""
        provided_size = tuple(map(_floor, size))
        width, height = provided_size
        plan = self._rust_image._thumbnail_preflight((width, height))
        if plan is None:
            return
        box = None
        if reducing_gap is not None:
            draft = self.draft(
                None,
                (
                    int(size[0] * reducing_gap),
                    int(size[1] * reducing_gap),
                ),
            )
            if draft is not None:
                box = draft[1]
        # Pillow skips resize input conversion if draft changed the image to
        # the already selected target size.
        if self._rust_image.size == plan.output_size:
            return
        # Keep user-defined string conversion outside a PyImage borrow: a
        # custom filter can call back into this wrapper and mutate its image.
        filter_code = _core._thumbnail_filter_code(resample)
        if reducing_gap is not None and reducing_gap < 1.0:
            raise ValueError("reducing_gap must be 1.0 or greater")
        reducing_gap = _core._thumbnail_reducing_gap(reducing_gap)
        palette = self.__dict__.get("_palette_object")
        if palette is not None and palette.dirty and self.mode in ("P", "PA"):
            # Commit Python-owned palette bytes only after Pillow's no-op,
            # geometry, filter, and reducing-gap checks have succeeded.
            self._realize_dirty_palette()
        self._native_info_known_empty = False
        if box is not None:
            resized = self._rust_image.resize(
                plan.output_size, filter_code, box, reducing_gap
            )
            self._rust_image = resized
        elif self._rust_image.size == plan.output_size:
            # Filter or gap coercion may resize this wrapper reentrantly after
            # the earlier target-size check. Pillow still completes its
            # already-entered resize call and replaces the core with the
            # current mode's identity-resized copy.
            self._rust_image = self._rust_image.resize(
                plan.output_size, filter_code, None, reducing_gap
            )
        elif self._has_getdata_views:
            # Pillow replaces its core during thumbnail; previously returned
            # getdata sequences keep the old core alive. Detach this wrapper
            # only after the resize branch is reached.
            current = self._rust_image
            replacement = current._clone_handle()
            replacement._thumbnail_apply_prepared(plan, filter_code, reducing_gap)
            self._rust_image = replacement
        else:
            self._rust_image._thumbnail_apply_prepared(plan, filter_code, reducing_gap)

    def tobytes(self, encoder_name: str = "raw", *args) -> bytes:
        self._realize_dirty_palette()
        return self._rust_image.tobytes_encoded(encoder_name, args)

    def getpixel(self, xy: Tuple[int, int]):
        """Get pixel value at (x, y). Mode dispatch done in Rust."""
        # Pillow loads before converting the coordinate iterable to a tuple.
        self.load()
        return self._rust_image.getpixel_formatted(tuple(xy))

    def putpixel(self, xy: Tuple[int, int], value):
        """Set pixel value at (x, y). Accepts int, tuple, or list.

        Multiband integers contain packed channel bytes. RGB(A) lists are
        accepted for palette colors; ordinary component values use tuples.
        Negative coordinates count from the right or bottom edge.
        """
        # Pillow's _ensure_mutable() loads before parsing coordinates or the
        # pixel value. This also materializes any pending Rust pipeline before
        # the in-place write becomes visible.
        self.load()
        try:
            self._rust_image.putpixel_mode(xy, value, self._info)
        finally:
            # A retained public palette observes allocation, including when
            # coordinate or alpha validation subsequently raises.
            palette = self.__dict__.get("_palette_object")
            if palette is not None and isinstance(value, (tuple, list)) and len(value) in (3, 4):
                palette.palette = bytearray(self.getpalette(palette.mode) or [])

    def quantize(self, colors: int = 256, method=None, kmeans: int = 0,
                 palette=None, dither: int = 1):
        """Reduce colors using median cut algorithm."""
        # Pillow loads the source before defaulting or validating the method.
        self._realize_dirty_palette()
        if isinstance(palette, Image) and (
            self.mode != "RGBA" or method is None or method in (2, 3)
        ):
            # Pillow loads a reference palette only after its RGBA method
            # restriction, but before validating the reference/source modes.
            palette._realize_dirty_palette()
        return self._new(
            self._rust_image.quantize(colors, method, kmeans, dither != 0, palette)
        )

    def getbbox(self, *, alpha_only: bool = True):
        """Bounding box of non-zero regions."""
        # Pillow loads before coercing the native alpha_only argument.
        self.load()
        return self._rust_image.getbbox(alpha_only)

    def getextrema(self):
        """Min/max pixel values per band. Returns tuple matching PIL format."""
        # Pillow loads before querying sample extrema.
        self.load()
        return self._rust_image.getextrema_formatted()

    def histogram(self, mask=None, extrema=None):
        """Image histogram per band, optionally ranged for I/F samples."""
        # Pillow loads the source before checking mask truthiness, then loads
        # a truthy mask before handing both cores to the histogram operation.
        self.load()
        if mask:
            mask.load()
        else:
            mask = None
        return self._rust_image.histogram_with_input(mask, extrema)

    def getchannel(self, channel):
        """Extract a channel, preserving native single-band and scalar modes."""
        # Pillow loads before parsing or resolving the selector.
        self.load()
        return self._new(self._rust_image.getchannel(channel))

    def putalpha(self, alpha):
        """Set/replace the alpha channel."""
        # Pillow's _ensure_mutable() loads the image before coercing alpha.
        self.load()
        self._native_info_known_empty = False
        self._sync_observed_info()
        try:
            self._rust_image.putalpha_input(alpha)
        finally:
            # Pillow can promote the destination before rejecting an invalid
            # alpha input. Keep a retained public info mapping in sync even
            # when the core reports that later validation error.
            self._sync_observed_info()

    def reduce(self, factor, box=None):
        """Reduce image by integer factor through the Rust core."""
        # Pillow normalizes factor/box and takes the identity copy before
        # loading. Its non-identity P path loads immediately before reduction.
        if not isinstance(factor, (list, tuple)):
            factor = (factor, factor)
        if box is None:
            box = (0, 0) + self.size
        if factor == (1, 1) and box == (0, 0) + self.size:
            return self.copy()
        self._realize_dirty_palette()
        return self._new(self._rust_image.reduce(factor, box))

    def load(self):
        """Load pixel data and return a mutable Pillow-style pixel view."""
        self._native_info_known_empty = False
        self._sync_observed_info()
        self._realize_dirty_palette()
        self._rust_image.load()
        self._sync_observed_info()
        return PixelAccess(self)

    def alpha_composite(self, im, dest=(0, 0), source=(0, 0)):
        """Alpha composite im over self in-place through the Rust core."""
        self._native_info_known_empty = False
        self._rust_image.alpha_composite(im._rust_image, dest, source)

    def getcolors(self, maxcolors=256):
        """Return list of [count, color] pairs or None if too many colors."""
        # Pillow loads before coercing maxcolors in the ImagingCore call.
        self.load()
        return self._rust_image.getcolors_formatted(maxcolors)

    def getdata(self, band=None):
        """Return pixel data through Pillow's ``ImagingCore`` sequence API."""
        # Pillow loads before validating an explicit band index.
        self.load()
        if band is not None:
            values = self._rust_image.getdata_formatted(band)
            # Pillow keeps the original mode for single-band 1/L/P images;
            # selecting a channel from a multiband image produces an L core.
            result_mode = self.mode if self.mode in ("1", "L", "P") else "L"
            return ImagingCore(values, result_mode, self.size)
        mode, size = self.mode, self.size
        self._has_getdata_views = True
        values = _ImageDataSequence(self._rust_image, mode, size)
        return ImagingCore(values, mode, size)

    def putdata(self, data, scale=1.0, offset=0.0):
        """Replace pixels from scalar samples or multiband color tuples."""
        # Pillow's _ensure_mutable() loads before checking/coercing `data`.
        self.load()
        self._native_info_known_empty = False
        self._rust_image.putdata_formatted(data, scale, offset)

    def getprojection(self):
        """Return horizontal and vertical projections."""
        # Pillow loads before scanning projections.
        self.load()
        return self._rust_image.getprojection()

    def entropy(self, mask=None, extrema=None):
        """Calculate image entropy, optionally ranging native I/F samples."""
        # NO_PILLOW_SUPPORT: Pillow's Image.entropy accepts only a mask.
        # Pillow loads the source before checking mask truthiness, then loads
        # a truthy mask before handing both cores to the entropy operation.
        self.load()
        if mask:
            mask.load()
        else:
            mask = None
        return self._rust_image.entropy_with_input(mask, extrema)

    def seek(self, frame):
        """Seek to frame in multi-frame image."""
        self._native_info_known_empty = False
        self._rust_image.seek(frame)

    def tell(self):
        """Return current frame number."""
        return self._rust_image.tell()

    def close(self):
        """Close the image file and release resources."""
        if isinstance(self._rust_image, _ClosedImage):
            return None
        self._native_info_known_empty = False
        self._rust_image.close()
        self._rust_image = _ClosedImage()

    def point(self, lut, mode=None):
        """Apply lookup table or function to each pixel."""
        self._realize_dirty_palette()
        return self._new(self._rust_image.point(lut, mode))

    def effect_spread(self, distance):
        """Simple spread/blur effect."""
        return self._new(self._rust_image.effect_spread(distance))

    def apply_transparency(self):
        """Commit P-mode transparency to its palette without changing pixels."""
        if self.mode == "P" and "transparency" in self.info:
            self._realize_dirty_palette()
        self._native_info_known_empty = False
        self._sync_observed_info()
        result = self._rust_image.apply_transparency()
        self._sync_observed_info()
        self.__dict__.pop("_palette_object", None)
        return result

    def get_child_images(self):
        """Return list of child images (multi-frame)."""
        return list(map(Image, self._rust_image.get_child_images()))

    def get_flattened_data(self, band=None):
        """Return flattened pixel data matching PIL format."""
        if band is not None:
            return tuple(self.getdata(band))
        self.load()
        # Match getdata's existing view bookkeeping and eager load boundary,
        # but avoid routing an eager tuple result through ImagingCore and its
        # Python per-pixel slicing iterator.
        self._has_getdata_views = True
        values = self._rust_image.getdata_formatted(None)
        if not isinstance(values, bytes):
            return tuple(values)
        bands = _image_data_bands(self.mode)
        if bands == 1:
            return tuple(values)
        return tuple(_iter_unpack(f"{bands}B", values))

    def getexif(self):
        """Return EXIF data as dict, matching PIL's Image.Exif."""
        raw = bytes(self._rust_image.getexif())
        return _ExifCompat(raw, loaded_exif=self.format != "TIFF")

    def getim(self):
        """Return the live Rust-owned internal-image handle."""
        return self._rust_image.getim()

    def getpalette(self, rawmode="RGB"):
        """Return palette data.

        PIL behavior: returns the exact retained flat RGB palette. WEB palette
        has 226 colors (678 bytes), while an encoded or explicitly attached
        palette may retain trailing black entries.
        """
        # Pillow loads and realizes dirty palette metadata before reading it.
        self.load()
        return self._rust_image.getpalette_with_input(rawmode)

    def _realize_dirty_palette(self):
        """Commit Python ImagePalette bytes when Pillow would load the image."""
        # TODO(pillow-rs#3ptb): finish validation-ordered realization in save
        # and transform. Paste now realizes source, destination, and active mask
        # in Pillow's order; source-core aliasing and subclass convert hooks are
        # tracked at the Paste owner. Thumbnail preflights geometry and filter
        # before palette commit; explicit-source RemapPalette delays load until
        # after dest_map parsing.
        palette = self.__dict__.get("_palette_object")
        if palette is None or not palette.dirty or self.mode not in ("P", "PA"):
            return

        mode, data = palette.getdata()
        self._rust_image.putpalette(data, mode)
        palette.dirty = 0
        palette.rawmode = None
        has_transparency = "transparency" in self.info
        if mode in ("LA", "PA"):
            self._rust_image.commit_palette_alpha()
            # Pillow installs the raw palette before parsing transparency, so
            # keep its public metadata stable even if the later setter errors.
            self._native_info = deepcopy(self._rust_image.compatibility_info())
            self._native_info_omitted = self._native_info_omitted.difference(
                {"transparency"}
            )
            self._native_info_rebaseline = False
            if has_transparency:
                transparency = self.info["transparency"]
                if isinstance(transparency, int):
                    self._rust_image.putpalettealpha(transparency, 0)
                else:
                    self._rust_image.putpalettealphas(transparency)
                palette.mode = "RGBA"
            elif palette.mode != mode:
                palette.palette = bytearray(
                    self._rust_image.getpalette_with_input(palette.mode) or []
                )
        elif palette.mode != mode:
            palette.palette = bytearray(
                self._rust_image.getpalette_with_input(palette.mode) or []
            )

    def getxmp(self):
        """Return XMP metadata. Returns empty dict."""
        return dict(self._rust_image.getxmp())

    def putpalette(self, data, rawmode="RGB"):
        """Attach a palette to the image."""
        self._native_info_known_empty = False
        self._sync_observed_info()
        result = self._rust_image.putpalette(data, rawmode)
        self._sync_observed_info()
        self.__dict__.pop("_palette_object", None)
        return result

    def show(self, title=None):
        """Display the image using the registered :mod:`PIL.ImageShow` viewers."""
        from PIL import ImageShow

        ImageShow.show(self, title)

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
        colortable = list(
            map(
                lambda color: qRgb(*color),
                self._rust_image.indexed_color_table(),
            )
        )

        if mode == "1":
            raw_data = self.tobytes("raw", "1")
            raw_data = self._align8to32(raw_data, w, "1")
            fmt = QImage.Format_Mono
        elif mode == "L":
            raw_data = self.tobytes("raw", "L")
            raw_data = self._align8to32(raw_data, w, "L")
            fmt = QImage.Format_Indexed8
        elif mode == "P":
            raw_data = self.tobytes("raw", "P")
            raw_data = self._align8to32(raw_data, w, "P")
            fmt = QImage.Format_Indexed8
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
        if mode in ("L", "P"):
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
            mode = self.mode
            size = self.size
            pixel_data = data if isinstance(data, (bytes, bytearray)) else bytes(data)
            self._native_info_known_empty = False
            self._rust_image = RustImage.frombytes(
                mode, size, pixel_data, decoder_name
            )
            return None

        # Class method: Image.frombytes(mode, size, data, ...)
        mode = self if isinstance(self, str) else data
        size = data if isinstance(self, str) else decoder_name
        pixel_data = decoder_name if isinstance(self, str) else args[0] if args else None
        decoder = args[0] if isinstance(self, str) and args else "raw"
        pixel_data = (
            pixel_data
            if isinstance(pixel_data, (bytes, bytearray))
            else bytes(pixel_data)
        )
        result = Image(RustImage.frombytes(mode, size, pixel_data, decoder))
        return result

    @classmethod
    def fromarray(cls, obj, mode=None):
        """Create an image from an object implementing ``__array_interface__``."""
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
        self._realize_dirty_palette()
        return self._rust_image.tobitmap()

    def remap_palette(self, dest_map, source_palette=None):
        """Remap image palette using destination map."""
        if self.mode not in ("L", "P"):
            raise ValueError("illegal image mode")
        # Pillow loads P before reading the source palette, but delays its
        # load for an explicit source_palette until after dest_map processing.
        if self.mode == "P" and source_palette is None:
            self._realize_dirty_palette()
        return self._new(
            self._rust_image.remap_palette(list(dest_map), source_palette)
        )

    def draft(self, mode, size):
        """Configure decoder for draft mode. Returns None matching PIL."""
        return None

    def transform(self, size, method, data=None, resample=0, fill=1, fillcolor=None):
        """Apply a built-in transform or dispatch a Pillow method object."""
        if isinstance(method, ImageTransformHandler):
            if self.mode == "LA" and resample != Resampling.NEAREST:
                premultiplied = self.convert("La")
                return method.transform(
                    size, premultiplied, resample=resample, fill=fill
                ).convert("LA")
            if self.mode == "RGBA" and resample != Resampling.NEAREST:
                premultiplied = self.convert("RGBa")
                return method.transform(
                    size, premultiplied, resample=resample, fill=fill
                ).convert("RGBA")
            return method.transform(size, self, resample=resample, fill=fill)

        if hasattr(method, "getdata"):
            method, data = method.getdata()

        return self._new(
            self._rust_image.transform(size, method, data, resample, fill, fillcolor)
        )

    def verify(self):
        """Verify file contents. Raises exception if corrupted."""
        # Pillow's base Image.verify() is a no-op; file-backed ImageFile
        # instances provide the verification behavior. Derived and in-memory
        # images use the base contract, even when their Rust representation is
        # a deferred pipeline.
        if self._verify_encoded_source:
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
        return self._rust_image.mode

    @property
    def format(self) -> Optional[str]:
        return self._rust_image.format

    @property
    def info(self) -> dict:
        if self._native_info_known_empty:
            return self._info
        native = self._rust_image.compatibility_info()
        previous = self._native_info
        if previous is not None:
            if self._native_info_rebaseline:
                self._native_info_omitted = self._native_info_omitted.union(
                    previous.keys() - native.keys()
                )
            # A lazy geometry child can temporarily omit loaded decoder
            # fields. Retain their native baseline until they reappear, so a
            # later load also preserves user edits and deleted-key tombstones.
            self._native_info_omitted = self._native_info_omitted.difference(native)
            native.update(
                (key, previous[key]) for key in self._native_info_omitted
            )
        if previous is None:
            for key, value in native.items():
                self._info.setdefault(key, value)
        elif self._native_info_rebaseline:
            # Geometry retains the source's public metadata, even when the
            # child is a lazy pipeline and native decoder fields temporarily
            # disappear (for example loaded WebP frame timing). Existing
            # native keys also include tombstones deleted by the caller.
            for key, value in native.items():
                if key not in previous:
                    self._info.setdefault(key, value)
        elif native != previous:
            # Core mutations can add/remove transparency or load decoder
            # fields. Unchanged native values must not overwrite user edits
            # or restore keys the user deleted from the public mapping.
            for key in previous.keys() - native.keys():
                self._info.pop(key, None)
            for key, value in native.items():
                if key not in previous or value != previous[key]:
                    self._info[key] = value
        if previous is None or native != previous:
            # Lists exposed in info remain mutable. The comparison snapshot
            # must not share those values with the public dictionary.
            self._native_info = deepcopy(native)
        self._native_info_rebaseline = False
        return self._info

    @info.setter
    def info(self, value: dict) -> None:
        self._info = value
        self._native_info_known_empty = False
        self._native_info = self._rust_image.compatibility_info()
        self._native_info_rebaseline = False
        self._native_info_omitted = frozenset()

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


_BASE_IMAGE_CONVERT_METHOD = Image.convert


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
