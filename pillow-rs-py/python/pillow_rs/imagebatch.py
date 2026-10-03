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


class BatchExecutor:
    """Submit built-in image filters and join their results in input order.

    ``queue=False`` (the default) executes each submitted operation
    immediately through its usual single-image path. With ``queue=True``,
    ``join()`` groups compatible ``ImageFilter.MedianFilter(3)``,
    ``ImageBatch.ExtractBand(channel)``, or ``ImageBatch.Multiply(image2)``
    jobs on the GPU when possible. Grouped operands use native ``L``, ``LA``,
    ``RGB``, or ``RGBA`` storage with equal dimensions. Incompatible jobs use
    the ordinary per-image operation.

    Example::

        from PIL import ImageBatch, ImageFilter

        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        batch.submit(image_a, ImageFilter.MedianFilter(3))
        batch.submit(image_b, ImageFilter.MedianFilter(3))
        results = batch.join()

        channels = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        channels.submit(rgba_image, ImageBatch.ExtractBand(3))
        alpha = channels.join()[0]

        products = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        products.submit(image_a, ImageBatch.Multiply(image_b))
        products.submit(image_c, ImageBatch.Multiply(image_d))
        product_a, product_c = products.join()

    Batched extraction reuses ``Image.getchannel`` over a same-mode vertical
    stack and returns one ``L`` image per input. Batched multiplication stacks
    each primary and secondary operand separately, then reuses the existing
    ``ImageChops.multiply`` pipeline. Inputs retain their mode; the executor
    does not convert them to RGBA.
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
        if type(operation).__name__ not in ("MedianFilter", "ExtractBand", "Multiply"):
            raise TypeError(
                "batch operation must be an ImageFilter.MedianFilter, "
                "ImageBatch.ExtractBand, or ImageBatch.Multiply instance"
            )
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
