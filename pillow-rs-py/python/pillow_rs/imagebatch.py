"""Explicit queued image batching for compatible operations.

The ordinary :class:`PIL.Image.Image` methods keep their existing routing.
This module provides an opt-in executor for callers that already have a set of
independent image jobs to join together.
"""

from copy import deepcopy
import operator

from . import _core
from .image import Image


class ExtractBand:
    """Request zero-based channel extraction in an explicit image batch.

    Example::

        batch.submit(image, ImageBatch.ExtractBand(3))  # RGBA alpha
    """

    __slots__ = ("channel",)

    def __init__(self, channel):
        try:
            self.channel = operator.index(channel)
        except TypeError as error:
            raise TypeError("channel must be an integer") from error


class Invert:
    """Request ``ImageOps.invert(image)`` in an explicit image batch.

    Equal-size L or RGB jobs can share one native-mode GPU dispatch. Other
    modes use their ordinary ImageOps validation and execution path.
    """

    __slots__ = ()


class Brightness:
    """Request ``ImageEnhance.Brightness(image).enhance(factor)`` in a batch.

    Equal-size L, LA, or RGB jobs with the same GPU-exact factor can share one
    native-mode GPU dispatch. Other inputs use their ordinary image path.
    """

    __slots__ = ("factor",)

    def __init__(self, factor):
        self.factor = factor


class Multiply:
    """Request ``ImageChops.multiply(image, other)`` in an explicit batch.

    Example::

        batch.submit(image_a, ImageBatch.Multiply(image_b))
    """

    __slots__ = ("image",)

    def __init__(self, image):
        if not isinstance(image, Image):
            raise TypeError("multiply operand must be a PIL.Image.Image instance")
        self.image = image


class Paste:
    """Request full-frame ``Image.paste(source, mask=mask)`` in a batch.

    Grouped GPU execution currently requires equal-sized native-mode source
    and destination images and an equal-sized L mask. The operation pastes at
    the origin and returns a new result without mutating the submitted
    destination. Other combinations keep their ordinary single-image path.
    """

    __slots__ = ("source", "mask")

    def __init__(self, source, mask):
        if not isinstance(source, Image):
            raise TypeError("paste source must be a PIL.Image.Image instance")
        if not isinstance(mask, Image):
            raise TypeError("paste mask must be a PIL.Image.Image instance")
        self.source = source
        self.mask = mask


class Expand:
    """Request ``ImageOps.expand(image, border, fill)`` in an explicit batch.

    Equal-size native L, LA, RGB, or RGBA images with matching borders and
    fills can share one GPU Expand dispatch. The fill is resolved against each
    image's native mode, including LA alpha selection.
    """

    __slots__ = ("border", "fill")

    def __init__(self, border=0, fill=0):
        try:
            border = operator.index(border)
        except TypeError as error:
            raise TypeError("border must be an integer") from error
        if border < 0:
            raise ValueError("border must be non-negative")
        self.border = border
        self.fill = fill


class Color3DLUT:
    """Snapshot a Pillow ``ImageFilter.Color3DLUT`` for shared batch use.

    Reuse the same ``ImageBatch.Color3DLUT`` instance for compatible RGBA
    images. The lookup table is captured once and remains immutable while
    queued jobs are pending.
    """

    __slots__ = ("_prepared",)

    def __init__(self, lut):
        from .imagefilter import Color3DLUT as Color3DLUTFilter

        if not isinstance(lut, Color3DLUTFilter):
            raise TypeError("lut must be a PIL.ImageFilter.Color3DLUT instance")
        self._prepared = _core.BatchColor3DLUT(lut)


class BatchExecutor:
    """Submit built-in image filters and join their results in input order.

    ``queue=False`` (the default) executes each submitted operation
    immediately through its usual single-image path. With ``queue=True``,
    ``join()`` groups compatible ``ImageFilter.MedianFilter(3)`` and
    ``ImageFilter.MaxFilter(3)``, and native-L
    ``ImageFilter.RankFilter(3, rank=1)``,
    ``ImageBatch.ExtractBand(channel)``, ``ImageBatch.Invert()``,
    ``ImageBatch.Brightness(factor)``,
    ``ImageBatch.Multiply(image2)``, or
    full-frame ``ImageBatch.Paste(source, mask)``, or native-mode
    ``ImageBatch.Expand(border, fill)`` jobs, plus jobs using one shared
    RGBA-to-RGBA ``ImageBatch.Color3DLUT``, on the GPU when possible.
    Grouped operands use native ``L``, ``LA``, ``RGB``, or ``RGBA`` storage
    with equal dimensions; batched Paste requires an L mask. Incompatible jobs
    use the ordinary per-image operation.

    Example::

        from PIL import ImageBatch, ImageFilter

        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        batch.submit(image_a, ImageFilter.MedianFilter(3))
        batch.submit(image_b, ImageFilter.MedianFilter(3))
        results = batch.join()

        channels = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        channels.submit(rgba_image, ImageBatch.ExtractBand(3))
        alpha = channels.join()[0]

        inversions = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        inversions.submit(gray_a, ImageBatch.Invert())
        inversions.submit(gray_b, ImageBatch.Invert())
        inverted_a, inverted_b = inversions.join()

        brightness = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        brightness.submit(luma_a, ImageBatch.Brightness(0.5))
        brightness.submit(luma_b, ImageBatch.Brightness(0.5))
        darker_a, darker_b = brightness.join()

        products = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        products.submit(image_a, ImageBatch.Multiply(image_b))
        products.submit(image_c, ImageBatch.Multiply(image_d))
        product_a, product_c = products.join()

        pastes = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        pastes.submit(destination_a, ImageBatch.Paste(source_a, mask_a))
        pastes.submit(destination_b, ImageBatch.Paste(source_b, mask_b))
        pasted_a, pasted_b = pastes.join()

        expanded = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        expanded.submit(image_a, ImageBatch.Expand(4, fill=(9, 17, 23, 31)))
        expanded.submit(image_b, ImageBatch.Expand(4, fill=(9, 17, 23, 31)))
        expanded_a, expanded_b = expanded.join()

        lut = ImageFilter.Color3DLUT.generate(
            17, callback, channels=4, target_mode="RGBA"
        )
        shared_lut = ImageBatch.Color3DLUT(lut)
        colors = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        colors.submit(rgba_a, shared_lut)
        colors.submit(rgba_b, shared_lut)
        color_a, color_b = colors.join()

    Batched ``MaxFilter(3)`` reuses the ordinary max-filter pipeline over a
    native-mode stack with one replicated edge row on each side of every
    image, so neighboring jobs cannot affect one another. The native-L
    ``RankFilter(3, rank=1)`` batch follows the same halo layout and reuses its
    packed-L rank pipeline. It is groupable only for L images with that exact
    size and rank; other RankFilter jobs retain the ordinary per-image path.
    Batched extraction
    reuses ``Image.getchannel`` over a same-mode vertical
    stack and returns one ``L`` image per input. Batched multiplication stacks
    each primary and secondary operand separately, then reuses the existing
    ``ImageChops.multiply`` pipeline. Batched Paste stacks full-frame
    destinations, sources, and L masks, then reuses ``Image.paste`` at the
    origin. A Color3DLUT batch snapshots one shared LUT and applies the
    existing RGBA pipeline to a vertical stack. Invert groups L and RGB images
    through the existing ``ImageOps.invert`` pipeline. Expand inserts native
    fill rows between vertically stacked inputs, then uses one existing
    ``ImageOps.expand`` pipeline; slicing the expanded stack returns one
    independently bordered result per input. Inputs retain their mode; the
    executor does not convert them to RGBA.
    """

    def __init__(self, queue=False, backend=None):
        self._executor = _core.BatchExecutor(queue=queue, backend=backend)
        self._metadata = []

    def submit(self, image, operation):
        """Submit one image and a supported filter or ImageBatch operation.

        Returns the zero-based submission index. In nonqueued mode the
        operation has completed before ``submit`` returns. In queued mode it
        runs during ``join``.
        """
        if not isinstance(image, Image):
            raise TypeError("batch input must be a PIL.Image.Image instance")
        if isinstance(operation, Color3DLUT):
            operation = operation._prepared
        if type(operation).__name__ not in (
            "MedianFilter",
            "MaxFilter",
            "RankFilter",
            "ExtractBand",
            "Invert",
            "Brightness",
            "Multiply",
            "Paste",
            "Expand",
            "BatchColor3DLUT",
        ):
            raise TypeError(
                "batch operation must be an ImageFilter.MedianFilter or "
                "ImageFilter.MaxFilter or ImageFilter.RankFilter, "
                "ImageBatch.ExtractBand, ImageBatch.Invert, ImageBatch.Multiply, "
                "ImageBatch.Brightness, "
                "ImageBatch.Paste, ImageBatch.Expand, or "
                "ImageBatch.Color3DLUT instance"
            )
        if isinstance(operation, (Brightness, Expand)):
            # Pillow's Brightness and ImageOps.expand create fresh outputs,
            # whose public info mapping does not inherit the source mapping.
            # Retain the native snapshot so lazy compatibility fields do not
            # reappear when Image.info is first read.
            metadata = (
                {},
                deepcopy(image._rust_image.compatibility_info()),
                False,
                frozenset(),
            )
        else:
            metadata = (
                image._info.copy(),
                deepcopy(image._native_info),
                image._native_info_rebaseline,
                image._native_info_omitted,
            )
        index = self._executor.submit(image._rust_image, operation)
        if index != len(self._metadata):
            raise RuntimeError("batch executor returned an unexpected submission index")
        self._metadata.append(metadata)
        return index

    def join(self):
        """Execute queued jobs and return images in submission order."""
        metadata, self._metadata = self._metadata, []
        images = self._executor.join()
        if len(images) != len(metadata):
            raise RuntimeError("batch executor returned an unexpected result count")
        results = []
        for rust_image, image_metadata in zip(images, metadata, strict=True):
            image = Image(rust_image)
            (
                image._info,
                image._native_info,
                image._native_info_rebaseline,
                image._native_info_omitted,
            ) = image_metadata
            results.append(image)
        return results
