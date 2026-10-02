# Explicit image batching

<!-- release:summary -->
**Latest release: [12.2.0-alpha.5](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.0-alpha.5).**
<!-- /release:summary -->

`PIL.ImageBatch.BatchExecutor` is an opt-in API for callers that already have
independent image jobs to process together. It does not alter `Image` methods,
automatic backend routing, or ordinary CPU/SIMD/GPU execution.

```python
from PIL import ImageBatch, ImageFilter

batch = ImageBatch.BatchExecutor(queue=True, backend="gpu")
batch.submit(image_a, ImageFilter.MedianFilter(3))
batch.submit(image_b, ImageFilter.MedianFilter(3))
result_a, result_b = batch.join()
```

With `queue=False` (the default), `submit` executes each operation immediately
through its ordinary single-image pipeline. With `queue=True`, submissions wait
until `join`, which returns results in submission order. A batch currently
accepts `ImageFilter.MedianFilter` operations. `MedianFilter(3)` jobs with the
same mode and dimensions are grouped when GPU is the selected backend. The
initial native-mode group layouts are `L`, `LA`, `RGB`, and `RGBA`; no image is
converted to RGBA. A queued job that has no compatible peer runs through the
regular single-image operation at `join`.

The GPU group is a native-mode vertical stack. One replicated top and bottom
row surrounds each image, so the 3×3 filter cannot read pixels from a neighbor
image at a group boundary. The existing MedianFilter operation processes the
stack, and its output is split back into ordinary per-image results. The split
results use the same operation result and metadata path as single-image calls.
Other filter sizes and operations continue through the existing single-image
path.

If `backend` is omitted, automatic routing remains in effect; grouping is
attempted only when GPU is the preferred active backend. A backend can be
provided explicitly as `"cpu"`, `"simd"`, or `"gpu"`. Use the existing
pipeline telemetry to verify the actual backend and check for fallback when
measuring a new device or workload.

## Measured throughput

These measurements use the public `ImageBatch.BatchExecutor` API on one Apple
M-series host. Each row is the median of 12 timed windows after 3 warmups. Raw
input bytes were prepared before timing; image construction, submission,
operation execution, GPU transfer and synchronization, result splitting, and
returned-image materialization were inside the window. GPU work was explicitly
requested, and the parity lane confirmed one real GPU dispatch with no fallback
for each mode and size shown. SIMD ran single-threaded through the same public
filter API. The workload-size differs by image size so each join processes a
similar amount of input memory.

| Mode | Image size | Images per join | GPU queued (images/s) | SIMD single-image (images/s) | GPU / SIMD |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 64×64 | 64 | 83,481 | 23,051 | 3.62× |
| LA | 64×64 | 64 | 61,532 | 11,866 | 5.18× |
| RGB | 64×64 | 64 | 75,695 | 8,033 | 9.42× |
| RGBA | 64×64 | 64 | 40,272 | 5,914 | 6.81× |
| L | 256×256 | 16 | 9,976 | 1,536 | 6.49× |
| RGB | 256×256 | 16 | 6,467 | 527 | 12.27× |

This demonstrates throughput for a compatible queued workload, not a latency
win for one image. For comparison, the same 64-image L workload through
`queue=False` measured about 5,100 images/s on GPU; batching cuts repeated
submission and readback overhead by issuing one grouped operation. The parity
and backend checks cover exact Pillow bytes, per-image `info` values, eager and
queued execution, and one actual GPU dispatch for compatible groups in each
listed mode and size.
