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

channels = ImageBatch.BatchExecutor(queue=True, backend="gpu")
channels.submit(rgba_a, ImageBatch.ExtractBand(3))
channels.submit(rgba_b, ImageBatch.ExtractBand(3))
alpha_a, alpha_b = channels.join()
```

With `queue=False` (the default), `submit` executes each operation immediately
through its ordinary single-image pipeline. With `queue=True`, submissions wait
until `join`, which returns results in submission order. A batch accepts
`ImageFilter.MedianFilter(3)` and `ImageBatch.ExtractBand(channel)` operations.
Jobs with the same operation, mode, and dimensions are grouped when GPU is the
selected backend. The initial native-mode group layouts are `L`, `LA`, `RGB`,
and `RGBA`; no image is converted to RGBA. A queued job that has no compatible
peer runs through the regular single-image operation at `join`.

The GPU group is a native-mode vertical stack. For `MedianFilter(3)`, one
replicated top and bottom row surrounds each image, so the filter cannot read
pixels from a neighbor at a group boundary. For `ExtractBand`, images are
stacked directly because each output pixel depends only on the corresponding
input pixel. The existing single-image operation processes each stack, and its
output is split back into ordinary per-image results. Split results use the
same operation result and metadata path as single-image calls. Other filter
sizes and unsupported modes continue through the existing single-image path.

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

### `ExtractBand` batch probe

`ImageBatch.ExtractBand(channel)` is available only through the explicit batch
API. It stacks compatible native `L`, `LA`, `RGB`, or `RGBA` sources, runs the
existing `Image.getchannel` pipeline once, and splits the `L` output back into
result images. Ordinary `Image.getchannel` calls and automatic backend routing
are unchanged. The parity lane checked every channel index for all four modes,
including LA alpha at index 1, submission order, returned `L` mode, source
`info`, eager `queue=False`, and a real one-dispatch GPU group with no fallback
or mode conversion.

The table reports the median full-window latency and GPU/SIMD pixel throughput
ratio. Each window creates and submits all images, executes the operation,
waits for GPU completion, splits results, and calls `tobytes()` on every output;
raw input buffers were generated before timing. Each cohort used 3 warmups and
12 measured windows, with Pillow processing the same number of images
sequentially. Large-cohort medians varied between runs, so read those rows as
observed evidence rather than a stable latency promise.

| Mode/channel | Images per join | Pillow p50 (ms) | SIMD p50 (ms) | GPU queued p50 (ms) | GPU/SIMD pixel throughput |
| --- | ---: | ---: | ---: | ---: | ---: |
| L / 0, 64×64 | 64 | 0.346 | 0.304 | 0.572 | 0.53× |
| LA / 1, 64×64 | 64 | 0.475 | 0.340 | 0.687 | 0.50× |
| RGB / 1, 64×64 | 64 | 0.455 | 0.440 | 0.711 | 0.62× |
| RGBA / 3, 64×64 | 64 | 0.421 | 0.418 | 0.755 | 0.55× |
| L / 0, 256×256 | 16 | 0.230 | 0.224 | 1.046 | 0.21× |
| LA / 1, 256×256 | 16 | 0.795 | 0.434 | 1.436 | 0.30× |
| RGB / 1, 256×256 | 16 | 0.749 | 0.507 | 1.723 | 0.29× |
| RGBA / 3, 256×256 | 16 | 0.525 | 0.663 | 1.849 | 0.36× |
| L / 0, 1024×768 | 4 | 0.454 | 0.687 | 2.414 | 0.28× |
| LA / 1, 1024×768 | 4 | 3.068 | 1.168 | 3.424 | 0.34× |
| RGB / 1, 1024×768 | 4 | 6.434 | 1.494 | 3.536 | 0.42× |
| RGBA / 3, 1024×768 | 4 | 1.530 | 2.756 | 5.026 | 0.55× |

At 64×64, grouping improved GPU throughput 18–24× relative to 64 sequential
GPU calls, but still reached only 0.50–0.62× single-thread SIMD pixel
throughput. It remained below SIMD at every tested size and mode. One dispatch
and exact native-mode parity therefore do not establish a useful GPU speedup
for this memory-bound operation. Keep this feature explicitly selected; do
not route ordinary `getchannel` calls through it or infer that other operations
will benefit. A future throughput attempt should remove full-frame packing and
splitting with shared buffers or fuse extraction into a later device-resident
consumer before tuning the shader.
