"""Explicit queued image batching for compatible operations.

The ordinary :class:`PIL.Image.Image` methods keep their existing routing.
This module provides an opt-in executor for callers that already have a set of
independent image jobs to join together.
"""

from copy import deepcopy

from . import _core
from .image import Image


class BatchExecutor:
    """Submit built-in image filters and join their results in input order.

    ``queue=False`` (the default) executes each submitted operation
    immediately through its usual single-image path. With ``queue=True``,
    ``join()`` groups compatible ``ImageFilter.MedianFilter(3)`` jobs on the
    GPU when possible. The first grouped formats are native ``L``, ``LA``,
    ``RGB``, and ``RGBA`` images of equal size. Incompatible jobs use the
    ordinary per-image operation.

    Example::

        from PIL import ImageBatch, ImageFilter

        batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
        batch.submit(image_a, ImageFilter.MedianFilter(3))
        batch.submit(image_b, ImageFilter.MedianFilter(3))
        results = batch.join()

    The current batch operation is intentionally limited to MedianFilter.
    Inputs retain their mode; the executor does not convert them to RGBA.
    """

    def __init__(self, queue=False, backend=None):
        self._executor = _core.BatchExecutor(queue=queue, backend=backend)
        self._metadata = []

    def submit(self, image, operation):
        """Submit one image and a built-in ``ImageFilter.MedianFilter``.

        Returns the zero-based submission index. In nonqueued mode the
        operation has completed before ``submit`` returns. In queued mode it
        runs during ``join``.
        """
        if not isinstance(image, Image):
            raise TypeError("batch input must be a PIL.Image.Image instance")
        if type(operation).__name__ != "MedianFilter":
            raise TypeError(
                "batch operation must be an ImageFilter.MedianFilter instance"
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
