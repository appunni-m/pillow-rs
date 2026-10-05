"""Explicit queued image batching for compatible operations.

The ordinary :class:`PIL.Image.Image` methods keep their existing routing.
This module provides an opt-in executor for callers that already have a set of
independent image jobs to join together.
"""

from copy import deepcopy
import operator

from . import _core
from .image import Image
from .imagefilter import MaxFilter, MedianFilter, RankFilter


class ExtractBand:
    """Request zero-based channel extraction in an explicit image batch.

    Example::

        batch.submit(ImageBatch.PipelineOp(image, ImageBatch.ExtractBand(3)))  # RGBA alpha
    """

    __slots__ = ("channel",)

    def __init__(self, channel):
        try:
            self.channel = operator.index(channel)
        except TypeError as error:
            raise TypeError("channel must be an integer") from error


class Grayscale:
    """Request ``ImageOps.grayscale(image)`` in an explicit image batch.

    Equal-size native L, LA, RGB, RGBA, or YCbCr jobs can share one GPU
    dispatch. Inputs stay in their native modes and return as L images.
    """

    __slots__ = ()


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

        batch.submit(ImageBatch.PipelineOp(image_a, ImageBatch.Multiply(image_b)))
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


class Composite:
    """Request ``Image.composite(foreground, background, mask)`` in a batch.

    Equal-size L, LA, RGB, or RGBA foreground/background pairs with an
    equal-size L mask can share one queued GPU schedule. Every image remains
    in its native mode. Other combinations keep the ordinary single-image
    Composite behavior.

    Example::

        batch.submit(ImageBatch.PipelineOp(foreground, ImageBatch.Composite(background, mask)))
    """

    __slots__ = ("background", "mask")

    def __init__(self, background, mask):
        if not isinstance(background, Image):
            raise TypeError("composite background must be a PIL.Image.Image instance")
        if not isinstance(mask, Image):
            raise TypeError("composite mask must be a PIL.Image.Image instance")
        self.background = background
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


class PipelineOp:
    """Bind an input image and its pipeline for explicit submission.

    Supply the input image and a callable that builds a lazy image using
    normal PIL methods, or an operation parameter object such as
    ``ImageFilter.MedianFilter(3)``. The callable receives the bound image.
    Its returned Rust descriptors are checked for GPU support during
    ``submit(pipeline_op)``, before the executor accepts the job.
    """

    __slots__ = ("_image", "_operation")

    def __init__(self, image, operation):
        if not isinstance(image, Image):
            raise TypeError("pipeline input must be a PIL.Image.Image instance")
        if not callable(operation) and not isinstance(operation, (
            MedianFilter, MaxFilter, RankFilter, ExtractBand, Grayscale,
            Invert, Brightness, Multiply, Paste, Composite, Expand, Color3DLUT,
        )):
            raise TypeError("PipelineOp requires a lazy pipeline builder or operation parameters")
        self._image = image
        self._operation = operation


class BatchExecutor:
    """Submit image operations or lazy GPU pipelines and join in input order.

    ``submit(PipelineOp(image, ...))`` schedules normalized GPU operations.
    Unsupported GPU operation descriptors are rejected before enqueueing.
    General pipelines use independent GPU plans; built-in wrappers can group
    compatible images into a shared dispatch.

    ``queue=False`` (the default) executes each submitted operation
    immediately through its usual single-image path. With ``queue=True``,
    ``join()`` groups compatible ``ImageFilter.MedianFilter(3)`` and
    ``ImageFilter.MaxFilter(3)``, and native-L
    ``ImageFilter.RankFilter(3, rank=1)``,
    ``ImageBatch.ExtractBand(channel)``, ``ImageBatch.Grayscale()``,
    ``ImageBatch.Invert()``,
    ``ImageBatch.Brightness(factor)``,
    ``ImageBatch.Multiply(image2)``, or
    full-frame ``ImageBatch.Paste(source, mask)``, same-mode
    ``ImageBatch.Composite(background, mask)``, or native-mode
    ``ImageBatch.Expand(border, fill)`` jobs, plus jobs using one shared
    RGBA-to-RGBA ``ImageBatch.Color3DLUT``, on the GPU when possible.
    Grouped operands use native ``L``, ``LA``, ``RGB``, or ``RGBA`` storage
    with equal dimensions; Grayscale also accepts native ``YCbCr`` triples.
    Batched Paste requires an L mask. Incompatible groups use the individual
    operation on the selected backend. GPU-unsupported descriptors raise
    before enqueueing.

    Example::

        from PIL import ImageBatch, ImageFilter

        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        batch.submit(ImageBatch.PipelineOp(image_a, ImageFilter.MedianFilter(3)))
        batch.submit(ImageBatch.PipelineOp(image_b, ImageFilter.MedianFilter(3)))
        results = batch.join()

        channels = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        channels.submit(ImageBatch.PipelineOp(rgba_image, ImageBatch.ExtractBand(3)))
        alpha = channels.join()[0]

        grayscales = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        grayscales.submit(ImageBatch.PipelineOp(rgb_a, ImageBatch.Grayscale()))
        grayscales.submit(ImageBatch.PipelineOp(rgb_b, ImageBatch.Grayscale()))
        gray_a, gray_b = grayscales.join()

        inversions = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        inversions.submit(ImageBatch.PipelineOp(gray_a, ImageBatch.Invert()))
        inversions.submit(ImageBatch.PipelineOp(gray_b, ImageBatch.Invert()))
        inverted_a, inverted_b = inversions.join()

        brightness = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        brightness.submit(ImageBatch.PipelineOp(luma_a, ImageBatch.Brightness(0.5)))
        brightness.submit(ImageBatch.PipelineOp(luma_b, ImageBatch.Brightness(0.5)))
        darker_a, darker_b = brightness.join()

        products = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        products.submit(ImageBatch.PipelineOp(image_a, ImageBatch.Multiply(image_b)))
        products.submit(ImageBatch.PipelineOp(image_c, ImageBatch.Multiply(image_d)))
        product_a, product_c = products.join()

        pastes = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        pastes.submit(ImageBatch.PipelineOp(destination_a, ImageBatch.Paste(source_a, mask_a)))
        pastes.submit(ImageBatch.PipelineOp(destination_b, ImageBatch.Paste(source_b, mask_b)))
        pasted_a, pasted_b = pastes.join()

        composites = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        composites.submit(ImageBatch.PipelineOp(foreground_a, ImageBatch.Composite(background_a, mask_a)))
        composites.submit(ImageBatch.PipelineOp(foreground_b, ImageBatch.Composite(background_b, mask_b)))
        composite_a, composite_b = composites.join()

        expanded = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        expanded.submit(ImageBatch.PipelineOp(image_a, ImageBatch.Expand(4, fill=(9, 17, 23, 31))))
        expanded.submit(ImageBatch.PipelineOp(image_b, ImageBatch.Expand(4, fill=(9, 17, 23, 31))))
        expanded_a, expanded_b = expanded.join()

        lut = ImageFilter.Color3DLUT.generate(
            17, callback, channels=4, target_mode="RGBA"
        )
        shared_lut = ImageBatch.Color3DLUT(lut)
        colors = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        colors.submit(ImageBatch.PipelineOp(rgba_a, shared_lut))
        colors.submit(ImageBatch.PipelineOp(rgba_b, shared_lut))
        color_a, color_b = colors.join()

    Batched ``MaxFilter(3)`` reuses the ordinary max-filter pipeline over a
    native-mode stack with one replicated edge row on each side of every
    image, so neighboring jobs cannot affect one another. The native-L
    ``RankFilter(3, rank=1)`` batch follows the same halo layout and reuses its
    packed-L rank pipeline. It is groupable only for L images with that exact
    size and rank; other GPU-supported RankFilter jobs use individual GPU plans.
    Batched extraction
    reuses ``Image.getchannel`` over a same-mode vertical
    stack and returns one ``L`` image per input. Batched multiplication stacks
    each primary and secondary operand separately, then reuses the existing
    ``ImageChops.multiply`` pipeline. Batched Paste stacks full-frame
    destinations, sources, and L masks, then reuses ``Image.paste`` at the
    origin. Batched Composite stacks foregrounds, backgrounds, and L masks,
    then reuses ``Image.composite`` over that stack. Grayscale applies the
    existing pipeline to a native-mode stack and returns one ``L`` image per
    input. A Color3DLUT batch snapshots one shared LUT and applies the
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

    def submit(self, pipeline_op):
        """Submit one bound PipelineOp; reject unsupported GPU operations.

        The operation's builder runs here to normalize its descriptors. In
        queued mode the accepted pipeline executes during ``join``.
        """
        if not isinstance(pipeline_op, PipelineOp):
            raise TypeError("batch operation must be an ImageBatch.PipelineOp instance")
        image = pipeline_op._image
        specification = pipeline_op._operation
        if callable(specification):
            return self._submit_pipeline(specification(image))
        return self._submit_grouped(image, specification)

    def _submit_grouped(self, image, operation):
        """Submit normalized parameters through an existing group implementation.

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
            "Grayscale",
            "Invert",
            "Brightness",
            "Multiply",
            "Paste",
            "Composite",
            "Expand",
            "BatchColor3DLUT",
        ):
            raise TypeError(
                "batch operation must be an ImageFilter.MedianFilter or "
                "ImageFilter.MaxFilter or ImageFilter.RankFilter, "
                "ImageBatch.ExtractBand, ImageBatch.Grayscale, "
                "ImageBatch.Invert, ImageBatch.Multiply, "
                "ImageBatch.Brightness, "
                "ImageBatch.Paste, ImageBatch.Composite, ImageBatch.Expand, or "
                "ImageBatch.Color3DLUT instance"
            )
        metadata_source = (
            operation.background if isinstance(operation, Composite) else image
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
                metadata_source._info.copy(),
                deepcopy(metadata_source._native_info),
                metadata_source._native_info_rebaseline,
                metadata_source._native_info_omitted,
            )
        index = self._executor.submit_grouped(image._rust_image, operation)
        if index != len(self._metadata):
            raise RuntimeError("batch executor returned an unexpected submission index")
        self._metadata.append(metadata)
        return index

    def _submit_pipeline(self, image):
        """Schedule a lazy image pipeline built with normal PIL methods.

        Requires a GPU executor and GPU-supported operation descriptors.
        ``queue=True`` defers execution until ``join``; ``queue=False``
        completes the pipeline here. Image-dependent GPU restrictions raise
        an error at execution rather than falling back to CPU. Each general
        pipeline uses its own existing GPU dispatch plan.
        """
        if not isinstance(image, Image):
            raise TypeError("batch input must be a PIL.Image.Image instance")
        metadata = (
            image._info.copy(),
            deepcopy(image._native_info),
            image._native_info_rebaseline,
            image._native_info_omitted,
        )
        index = self._executor.submit_prepared_pipeline(image._rust_image)
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
