"""Explicit, bounded GPU scheduling of existing lazy Image results.

Completion order is unspecified. Match a result by job_id or input_key.
There is no operation adapter and no CPU image-operation fallback.
"""
from __future__ import annotations

from typing import NamedTuple
import sys
import weakref
import threading
from functools import wraps
from copy import deepcopy
from . import _core
from .image import Image


class BatchError(RuntimeError):
    """A failed admission or stream, with job_id, input_key, stage and kind."""

    def __init__(self, message, *, job_id=None, input_key=None, stage="driver", kind="State"):
        super().__init__(message)
        self.job_id, self.input_key, self.stage, self.kind = job_id, input_key, stage, kind


class QueueFull(BatchError):
    """Drain admitted work before submitting another job."""


class ResultBackpressure(BatchError):
    """Release held GPU outputs, then retry next() on the same Results."""


def _translate(error):
    kind = getattr(error, "kind", "Execution")
    cls = {"QueueFull": QueueFull, "ResultBackpressure": ResultBackpressure}.get(kind, BatchError)
    return cls(str(error), job_id=getattr(error, "job_id", None),
               input_key=getattr(error, "input_key", None),
               stage=getattr(error, "stage", "execution"), kind=kind)


def _metadata_bytes(value, seen=None, depth=0):
    if depth > 64:
        raise ValueError("GPU batch metadata exceeds 64 container levels")
    seen = set() if seen is None else seen
    identity = id(value)
    if identity in seen:
        return 0
    seen.add(identity)
    size = sys.getsizeof(value)
    if type(value) is dict:
        return size + sum(_metadata_bytes(k, seen, depth + 1) + _metadata_bytes(v, seen, depth + 1) for k, v in value.items())
    if type(value) in (tuple, list, set, frozenset):
        return size + sum(_metadata_bytes(v, seen, depth + 1) for v in value)
    if value is None or type(value) in (bool, int, float, str, bytes):
        return size
    raise TypeError("GPU batch metadata must contain ordinary scalar/byte/container values")


def _metadata(image):
    # Validate before deepcopy so custom objects cannot execute copy hooks.
    # Snapshot nested containers too; admitted metadata cannot grow through
    # caller mutation after its host reservation has been measured.
    metadata = (image._info, image._native_info, image._native_info_omitted)
    _metadata_bytes(metadata)
    return deepcopy(metadata)


def _wrap(native, metadata):
    image = Image(native)
    info, image._native_info, image._native_info_omitted = deepcopy(metadata)
    image._info = info
    image._native_info_rebaseline = True
    return image


class GpuImage:
    """Immutable native GPU pixels; download() leaves the GPU lease alive."""
    __slots__ = ("_native", "_metadata")

    def __init__(self, native, metadata):
        self._native, self._metadata = native, metadata

    @property
    def mode(self):
        return self._native.mode

    @property
    def size(self):
        return self._native.size

    def download(self):
        """Return a normal CPU Image, retaining this device allocation."""
        return _wrap(self._native.download(), self._metadata)


class BatchResult(NamedTuple):
    """A stable identity and materialized CPU Image or owning GpuImage."""
    job_id: int
    input_key: object
    image: object


def _exclusive(method):
    @wraps(method)
    def guarded(self, *args, **kwargs):
        executor = self if isinstance(self, GpuBatchExecutor) else self._executor
        if not executor._lock.acquire(blocking=False):
            raise BatchError("executor is already being driven by another thread")
        try:
            return method(self, *args, **kwargs)
        finally:
            executor._lock.release()
    return guarded


class GpuBatchExecutor:
    """Schedule existing lazy Images on GPU with independent resource caps.

    queue=False eagerly executes one submission per job. queue=True encodes
    independent jobs together. run() consumes its keyed input iterator lazily;
    join() drains a bounded manual window. Exactly one Results driver may be
    active. GPU-unsupported contexts fail during admission.
    """
    def __init__(self, *, queue=True, gpu_bytes=512 << 20, host_bytes=256 << 20,
                 max_jobs=64, max_in_flight=2):
        try:
            self._native = _core.GpuBatchExecutor(queue=queue, gpu_bytes=gpu_bytes,
                host_bytes=host_bytes, max_jobs=max_jobs, max_in_flight=max_in_flight)
        except _core.GpuBatchError as error:
            raise _translate(error) from error
        self._lock = threading.RLock()
        self._driver = None
        self._templates = {}
        self._closed = False
        self._max_jobs = max_jobs

    def _check_idle(self):
        if self._closed:
            raise BatchError("executor is closed")
        if self._driver is not None and self._driver() is not None:
            raise BatchError("an active Results iterator owns this executor")

    def _submit(self, pipeline, key=None):
        if not isinstance(pipeline, Image):
            raise TypeError("submit accepts the existing lazy Image returned by an image operation")
        metadata = _metadata(pipeline)
        try:
            job_id = self._native.submit(pipeline._rust_image, key, _metadata_bytes(metadata))
        except _core.GpuBatchError as error:
            raise _translate(error) from error
        self._templates[job_id] = metadata
        return job_id

    @_exclusive
    def submit(self, pipeline):
        """Admit one existing lazy Image and return its unique job ID."""
        self._check_idle()
        if not self._templates:
            try:
                self._native.set_resident_output(False)
            except _core.GpuBatchError as error:
                raise _translate(error) from error
        return self._submit(pipeline)

    @_exclusive
    def join(self):
        """Stream materialized results from the admitted manual window."""
        self._check_idle()
        results = Results(self, None, "cpu")
        self._driver = weakref.ref(results)
        return results

    @_exclusive
    def run(self, jobs, *, output="cpu"):
        """Transform an iterator of (compact key, lazy Image) into results."""
        self._check_idle()
        if self._templates:
            raise BatchError("drain manual submissions before starting run()")
        if output not in ("cpu", "gpu"):
            raise ValueError("output must be 'cpu' or 'gpu'")
        try:
            self._native.set_resident_output(output == "gpu")
        except _core.GpuBatchError as error:
            raise _translate(error) from error
        results = Results(self, iter(jobs), output)
        self._driver = weakref.ref(results)
        return results

    @property
    @_exclusive
    def stats(self):
        """Bounded native counters, including bytes pinned by GPU handles."""
        return self._native.stats()

    @_exclusive
    def close(self):
        """Stop consuming input and cancel unwanted work without readback."""
        if self._closed:
            return
        self._closed = True
        self._native.close()
        self._templates.clear()
        if self._driver is not None and self._driver() is not None:
            driver = self._driver()
            driver._source = None
            driver._pending = None
            driver._closed = True
        self._driver = None

    def __enter__(self):
        if self._closed:
            raise BatchError("executor is closed")
        return self

    def __exit__(self, *exc):
        self.close()


class Results:
    """Exclusive lazy stream; caller-held GPU lease pressure is retryable."""
    def __init__(self, executor, source, output):
        self._executor, self._source, self._output = executor, source, output
        self._pending = None
        self._closed = False
        self._advancing = False

    def __iter__(self):
        return self

    @_exclusive
    def __next__(self):
        if self._advancing:
            raise BatchError("Results iterator cannot be entered recursively")
        self._advancing = True
        try:
            return self._next()
        finally:
            self._advancing = False

    def _next(self):
        if self._closed:
            raise StopIteration
        executor = self._executor
        try:
            # At most one producer-owned lookahead exists. Neither the input
            # iterable nor the completed outputs are converted into lists.
            while self._source is not None and len(executor._templates) < executor._max_jobs:
                if self._pending is None:
                    try:
                        self._pending = next(self._source)
                    except StopIteration:
                        self._source = None
                        break
                try:
                    key, pipeline = self._pending
                except (TypeError, ValueError) as error:
                    raise TypeError("run() inputs must be (input_key, lazy Image) pairs") from error
                try:
                    executor._submit(pipeline, key)
                except QueueFull as error:
                    if not executor._templates:
                        raise ResultBackpressure(str(error), job_id=error.job_id,
                            input_key=key, stage="delivery", kind="ResultBackpressure") from error
                    break
                self._pending = None
            native = executor._native.next_result()
            if native is None:
                self._closed = True
                executor._driver = None
                raise StopIteration
            job_id, key, image = native
            metadata = executor._templates.pop(job_id)
            if self._output == "gpu":
                image = GpuImage(image, metadata)
            else:
                image = _wrap(image, metadata)
            return BatchResult(job_id, key, image)
        except _core.GpuBatchError as error:
            self.close()
            raise _translate(error) from error
        except (StopIteration, ResultBackpressure):
            raise
        except BaseException:
            self.close()
            raise

    @_exclusive
    def close(self):
        """Stop this stream and its executor; do not pull more producer items."""
        if not self._closed:
            self._executor.close()
        self._source = None
        self._pending = None
        self._closed = True

    def __del__(self):
        if hasattr(self, "_closed") and not self._closed:
            self.close()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
