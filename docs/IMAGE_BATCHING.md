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

products = ImageBatch.BatchExecutor(queue=True, backend="gpu")
products.submit(image_a, ImageBatch.Multiply(image_b))
products.submit(image_c, ImageBatch.Multiply(image_d))
product_a, product_c = products.join()
```

With `queue=False` (the default), `submit` executes each operation immediately
through its ordinary single-image pipeline. With `queue=True`, submissions wait
until `join`, which returns results in submission order. A batch accepts
`ImageFilter.MedianFilter(3)`, `ImageBatch.ExtractBand(channel)`, and
`ImageBatch.Multiply(other_image)` operations. Jobs with the same operation,
mode, and dimensions are grouped when GPU is the selected backend. Multiply
also requires each secondary image to match its primary image's mode and size.
The native-mode group layouts are `L`, `LA`, `RGB`, and `RGBA`; no image is
converted to RGBA. A queued job that has no compatible peer runs through the
regular single-image operation at `join`.

The GPU group is a native-mode vertical stack. For `MedianFilter(3)`, one
replicated top and bottom row surrounds each image, so the filter cannot read
pixels from a neighbor at a group boundary. For `ExtractBand`, images are
stacked directly because each output pixel depends only on the corresponding
input pixel. For `Multiply`, primary images and secondary images are each
stacked in their native mode, then passed to the existing Multiply pipeline
once. The existing single-image operation processes each stack, and its output
is split back into ordinary per-image results. Split results use the same
operation result and metadata path as single-image calls. Other filter sizes
and unsupported modes continue through the existing single-image path.

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

### `Multiply` batch probe

`ImageBatch.Multiply(other_image)` keeps both operands in native `L`, `LA`,
`RGB`, or `RGBA` storage and reuses the existing `ImageChops.multiply`
pipeline. The isolated Pillow parity lane checked byte-for-byte results and
source `info` in all four modes, reversed submission order, a 64-image group,
`queue=False`, and one actual `multiply.wgsl` dispatch per compatible queued
group with no fallback or mode conversion. Pillow's LA and RGBA behavior
multiplies the stored alpha byte too; the native batch path preserves it.

Each timing is the median of 12 full-call windows after 3 warmups, on the same
Apple M-series host. A window creates both operands, submits the images,
executes the operation, waits for GPU completion, splits results, and calls
`tobytes()` on every result. The Pillow profile processes image pairs
sequentially. The parity lane checked every input in the 64×64 × 64 and
256×256 × 16 cohorts byte-for-byte; GPU telemetry confirmed one real dispatch
for each queued group. Values are milliseconds per full window.

| Mode | Size × images | Pillow | CPU | SIMD | GPU, queue off | GPU, queued | GPU/SIMD throughput |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| L | 64×64 × 64 | 0.646 | 0.418 | 0.457 | 12.694 | 0.720 | 0.63× |
| LA | 64×64 × 64 | 1.344 | 0.534 | 0.531 | 13.060 | 0.895 | 0.59× |
| RGB | 64×64 × 64 | 1.431 | 0.621 | 0.693 | 15.848 | 1.104 | 0.63× |
| RGBA | 64×64 × 64 | 1.274 | 0.667 | 1.557 | 20.368 | 2.399 | 0.65× |
| L | 256×256 × 16 | 0.895 | — | 0.443 | — | 1.053 | 0.42× |
| LA | 256×256 × 16 | 3.695 | — | 0.773 | — | 2.016 | 0.38× |
| RGB | 256×256 × 16 | 3.680 | — | 0.877 | — | 2.616 | 0.34× |
| RGBA | 256×256 × 16 | 2.949 | — | 1.371 | — | 3.214 | 0.43× |

The command reference contains the exact invocation for each backend and both
workload sizes. Each benchmark profile uses 12 samples and 3 warmups.

For 64 small images, grouping makes GPU execution about 8–18× faster than
issuing the same GPU operation once per image, but queued GPU throughput still
reaches only 0.59–0.65× SIMD. At 256×256 it reaches 0.34–0.43× SIMD. The
larger exploratory 1024×768 × 4 measurements for L and LA also remained below
SIMD; the RGB/RGBA cohort was stopped because the complete four-mode results
at both smaller sizes already showed no crossover. This is a working GPU
batch operation, not evidence that GPU is the fastest backend. Keep it
explicitly selected; ordinary `ImageChops.multiply` routing is unchanged.
The performance blocker is the cost of packing two full image stacks and
splitting the readback around a simple byte-wise kernel. A follow-up should
change the separate batch transport/scheduling design, not the ordinary
single-image Multiply path.
