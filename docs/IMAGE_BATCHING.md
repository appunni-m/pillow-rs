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

maxima = ImageBatch.BatchExecutor(queue=True, backend="gpu")
maxima.submit(image_a, ImageFilter.MaxFilter(3))
maxima.submit(image_b, ImageFilter.MaxFilter(3))
max_a, max_b = maxima.join()

ranked = ImageBatch.BatchExecutor(queue=True, backend="gpu")
ranked.submit(gray_a, ImageFilter.RankFilter(3, rank=1))
ranked.submit(gray_b, ImageFilter.RankFilter(3, rank=1))
rank_a, rank_b = ranked.join()

channels = ImageBatch.BatchExecutor(queue=True, backend="gpu")
channels.submit(rgba_a, ImageBatch.ExtractBand(3))
channels.submit(rgba_b, ImageBatch.ExtractBand(3))
alpha_a, alpha_b = channels.join()

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
expanded.submit(rgba_a, ImageBatch.Expand(4, fill=(9, 17, 23, 31)))
expanded.submit(rgba_b, ImageBatch.Expand(4, fill=(9, 17, 23, 31)))
expanded_a, expanded_b = expanded.join()

lut = ImageFilter.Color3DLUT.generate(
    17, callback, channels=4, target_mode="RGBA"
)
shared_lut = ImageBatch.Color3DLUT(lut)
colors = ImageBatch.BatchExecutor(queue=True, backend="gpu")
colors.submit(rgba_a, shared_lut)
colors.submit(rgba_b, shared_lut)
color_a, color_b = colors.join()
```

With `queue=False` (the default), `submit` executes each operation immediately
through its ordinary single-image pipeline. With `queue=True`, submissions wait
until `join`, which returns results in submission order. A batch accepts
`ImageFilter.MedianFilter(3)`, `ImageFilter.MaxFilter(3)`, and native-L
`ImageFilter.RankFilter(3, rank=1)`,
`ImageBatch.ExtractBand(channel)`,
`ImageBatch.Invert()`, `ImageBatch.Brightness(factor)`,
`ImageBatch.Multiply(other_image)`, full-frame
`ImageBatch.Paste(source, mask)`, native-mode `ImageBatch.Expand(border, fill)`,
and a shared same-mode RGBA
`ImageBatch.Color3DLUT(filter)` operation. Jobs with the same operation and
compatible mode and dimensions are grouped when GPU is the selected backend;
Brightness also requires an exact GPU factor and L, LA, or RGB mode. LUT jobs
must share the same wrapper instance.
Multiply requires each secondary image to match its primary image's mode and
size. Batched Paste requires same-sized destination and source images in the
same native mode, plus a same-sized `L` mask; it pastes the source at `(0, 0)`.
The native-mode group layouts are `L`, `LA`, `RGB`, and `RGBA`; no image is
converted to RGBA. A queued job that has no compatible peer runs through the
regular single-image operation at `join`.

The GPU group is a native-mode vertical stack. For `MedianFilter(3)` and
`MaxFilter(3)`, and native-L `RankFilter(3, rank=1)`, one replicated top and
bottom row surrounds each image, so the filter cannot read pixels from a
neighbor at a group boundary. RankFilter batching is currently limited to the
packed-L second-minimum kernel; other modes, sizes, and ranks retain the
ordinary per-image path. For
`ExtractBand`, `Invert`, `Brightness`, `Multiply`, `Paste`, `Expand`, and
`Color3DLUT`,
images are stacked directly because each output pixel depends only on
corresponding input pixels. Brightness groups same-factor L, LA, and RGB images
with the existing native-byte kernel; LA alpha is preserved. RGBA and factors
the GPU cannot represent exactly continue
through the existing per-image route. `ImageOps.invert` groups only L and RGB
images, using its existing mode-specific pipeline; unsupported modes retain
the ordinary ImageOps validation behavior. For `Multiply`, primary and secondary
operands are each stacked in their native mode. For `Paste`,
destinations, sources, and L masks are stacked separately; the existing
full-frame masked paste runs at the origin and keeps each image independent.
For `Expand`, the batch requires equal borders and fill inputs. It inserts two
native fill rows between adjacent source images. The existing Expand operation
adds the outer border around the full stack, which gives every input its own
top/bottom border when the output is split into equal expanded-image slices.
LA fill keeps Pillow's two-component `(luma, alpha)` meaning; no mode is
converted. The GPU group planner checks both stacked input and expanded output
against pixel, native-byte storage, dispatch, shader-work, and current adapter
limits before allocating the combined images. An unsafe or incompatible group
uses ordinary single-image expansion.
For `Color3DLUT`, the batch
captures one immutable LUT and applies the existing RGBA-to-RGBA pipeline to
the stack; each job must reuse the same `ImageBatch.Color3DLUT` instance.
The existing single-image operation processes each stack, and its output is
split back into ordinary per-image results. Split results use the same
operation result and metadata path as single-image calls. Other filter sizes,
incompatible LUT objects, and unsupported modes continue through the existing
single-image path.

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

### `ImageFilter.MaxFilter(3)` batch probe

`BatchExecutor.submit(image, ImageFilter.MaxFilter(3))` reuses the existing
MaxFilter operation. With `queue=False`, each image follows the ordinary
single-image path immediately. With `queue=True`, equal-size native `L`, `LA`,
`RGB`, and `RGBA` images are stacked with one replicated top and bottom row per
input, processed by the existing MaxFilter pipeline, and split back in
submission order. Those halo rows preserve each image's edge behavior. Other
filter sizes and incompatible jobs retain the per-image path; the batch does
not convert inputs to RGBA.

The isolated parity run compared exact Pillow bytes, mode, size, metadata,
submission order, small incompatible-size fallbacks, and eager `queue=False`
results. CPU, SIMD, and GPU passed in all four modes for 64×64 × 64 and
256×256 × 16 images. GPU receipts confirmed one real MaxFilter dispatch per
compatible group, zero mode conversions, and no fallback.

The table reports full-call throughput on the same Apple M-series host. Each
window includes image creation, submission, operation execution, GPU transfer
and synchronization, result splitting, and `tobytes()` for every returned
image. Pillow runs sequentially; CPU and SIMD use `BatchExecutor(queue=False)`;
GPU eager uses `queue=False`; GPU queued uses `queue=True`. Every profile used
3 warmups and 12 measured windows.

| Mode | Cohort | Pillow (images/s) | CPU (images/s) | SIMD (images/s) | GPU eager (images/s) | GPU queued (images/s) | Queued GPU / SIMD | Queued GPU / Pillow |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L | 64 images @ 64×64 | 8,696 | 15,339 | 34,755 | 4,825 | 74,017 | 2.13× | 8.51× |
| LA | 64 images @ 64×64 | 4,505 | 13,945 | 19,058 | 5,013 | 65,330 | 3.43× | 14.50× |
| RGB | 64 images @ 64×64 | 1,777 | 6,658 | 12,468 | 2,904 | 57,605 | 4.62× | 32.41× |
| RGBA | 64 images @ 64×64 | 2,267 | 9,255 | 9,653 | 1,089 | 47,542 | 4.93× | 20.97× |
| L | 16 images @ 256×256 | 634 | 1,093 | 2,268 | 3,972 | 11,991 | 5.29× | 18.91× |
| LA | 16 images @ 256×256 | 351 | 973 | 1,333 | 3,780 | 9,164 | 6.87× | 26.11× |
| RGB | 16 images @ 256×256 | 236 | 736 | 869 | 2,998 | 6,761 | 7.78× | 28.65× |
| RGBA | 16 images @ 256×256 | 172 | 612 | 651 | 3,015 | 4,889 | 7.51× | 28.42× |

Grouped GPU throughput beats SIMD in every measured cohort and size; it also
beats Pillow by 8.5–32.4× at 64×64 and 18.9–28.7× at 256×256. The ordinary
serial CPU cohort beats Pillow in each case. SIMD is 2.1–7.0× faster than
Pillow at 64×64 and 3.6–3.8× at 256×256, so the 5× SIMD target is not proven
for most of these workloads. This is explicit batch throughput evidence only;
it does not change ordinary `ImageFilter.MaxFilter` routing.

### Native-L `ImageFilter.RankFilter(3, rank=1)` batch probe

Queued native-L rank-one jobs use the existing packed-L second-minimum
pipeline. Equal-size images share a vertical stack with one replicated row at
each image edge; `join()` issues one GPU dispatch for a compatible group and
returns independent L images. `queue=False`, other modes, other sizes, and
other ranks follow their normal per-image operation. This extends only the
explicit `ImageBatch` API and does not change ordinary `Image.filter` routing.

The isolated parity lane checks exact Pillow bytes for tie-heavy constant
images, an odd-width/height group, a single-column fallback, a material
256×256 group, and non-groupable L/LA ranks. It also verifies output mode,
dimensions, per-image metadata, eager execution, zero mode conversions, and
one packed-L shader dispatch for each compatible GPU group.

Two target-only fault-contract cases inject a grouped dimension failure and a
grouped memory failure. Each verifies that `join()` transparently falls back,
returns exact Pillow-equivalent bytes in submission order, and accepts a later
batch on the same executor. The faulted call cannot be sent to Pillow, so these
results are counted separately with `oracle=not_applicable`; the adjacent
normal-path cases remain the live Pillow parity checks. See the contributor
[command reference](COMMANDS.md) for the focused fault-contract gate and its
test-only build behavior.

On this Apple-silicon host, the following cohort processed 16 L images at
256×256 per call. Each value is the median of 12 full-call windows after 3
warmups; the window includes image construction, submission, execution,
transfer and synchronization, splitting, and `tobytes()` for every output.
Pillow is ordinary sequential Pillow. The CPU and SIMD columns use
`BatchExecutor(queue=False)`; the GPU columns use the same API with
`queue=False` and `queue=True` respectively.

| Profile | Full-call p50 (ms) | Images/s | Speedup over Pillow |
| --- | ---: | ---: | ---: |
| Pillow sequential | 26.050 | 614 | 1.00× |
| CPU | 7.263 | 2,203 | 3.59× |
| SIMD | 7.511 | 2,130 | 3.47× |
| GPU eager | 4.183 | 3,825 | 6.23× |
| GPU queued | 1.513 | 10,577 | 17.22× |

Queued GPU reached 4.97× the SIMD throughput and 2.77× eager-GPU throughput;
the parity run confirmed one real packed-L shader dispatch for the 16-image
group, with zero mode conversions. CPU was 3.59× faster than Pillow; SIMD was
3.47× faster, below the campaign's 5× SIMD target for this batch-driver
cohort. These values do not replace the ordinary per-image operation
benchmark.

A 1024×768 × 4 probe measured queued GPU at 1,238 images/s, above SIMD at
177 images/s and Pillow at 52 images/s. In that smaller large-image cohort,
eager GPU reached 1,586 images/s, so stack preparation and result splitting
outweighed the saved submissions. The queued path is intended for enough
compatible jobs to amortize that work; queueing does not guarantee a win over
eager GPU for every cohort size.

See the [command reference](COMMANDS.md) for isolated Pillow parity and
reproducible batch benchmark commands.

### Brightness batch probe

`ImageBatch.Brightness(factor)` queues the same operation as
`ImageEnhance.Brightness(image).enhance(factor)`. Equal-size L, LA, and RGB
images can share the existing native-byte GPU kernel when the factor is exact
for every byte. LA alpha remains unchanged. RGBA uses its ordinary
single-image route. `queue=False` does not alter normal image routing. The
Pillow parity lane compares bytes, mode, size, `info`, and the actual selected
backend; it also verifies one `brightness_native.wgsl` dispatch for each
compatible GPU group.

The trial below uses factor `0.5` and LA images on one Apple M-series host.
Each number is the median of 12 full-call windows after 3 warmups; one window
includes image creation, submission, execution, GPU transfers, result splitting,
and output materialization. The 1024×768 × 16 cohort was additionally checked
against a matching eager-GPU run. Ratios use the same workload size, so
GPU/SIMD is equivalent to SIMD latency divided by queued-GPU latency.

| Mode and cohort | Pillow p50 (ms) | CPU p50 (ms) | SIMD p50 (ms) | GPU eager p50 (ms) | GPU queued p50 (ms) | GPU queued / SIMD throughput | GPU queued speedup vs Pillow |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| LA 256×256 × 16 | 3.584 | 1.131 | 0.961 | 3.666 | 1.178 | 0.82× | 3.04× |
| LA 1024×768 × 4 | 11.426 | 3.384 | 2.752 | 2.812 | 3.103 | 0.89× | 3.68× |
| LA 1024×768 × 16 | 40.987 | — | 10.537 | 11.178 | 12.533 | 0.84× | 3.27× |
| LA 1024×768 × 4, after SIMD brightness shift | 11.878 | 3.028 | 1.019 | 2.900 | 3.257 | 0.31× | 3.65× |

Grouping clearly improves the small-image batch over eager GPU, but all three
queued results remain slower than SIMD. At the larger sizes, grouping also
loses to eager GPU, which indicates stack packing and splitting outweigh the
saved dispatch overhead for this operation. Treat this as a parity-complete,
performance-limited checkpoint; do not claim it meets the GPU throughput goal
or enable batching implicitly. The next useful optimization needs to reduce
host-side stack copies and per-result reconstruction, then rerun these same
workloads before expanding GPU grouping to larger images.

The added LA 1024×768 × 4 row is a follow-up after the SIMD brightness path
began using exact byte shifts. It used the same 12-window/3-warmup methodology
on the same host: serial CPU reached 1,321 images/s, SIMD 3,926 images/s,
eager GPU 1,379 images/s, and queued GPU 1,228 images/s. The queued executor
formed one compatible native-LA GPU group, but that did not offset stack
packing and result splitting; queued throughput was 0.31× SIMD and lower than
eager GPU. This is explicit `ImageBatch` throughput only and does not change
normal image routing. The normal build had Rayon disabled; no Parallel CPU
results are folded into these rows.

### `ImageOps.invert` batch probe

`ImageBatch.Invert()` queues the existing `ImageOps.invert` operation for
native `L` and `RGB` images. Other modes keep the existing single-image
validation and execution path. The parity check compares Pillow bytes and
metadata for both modes, and confirms that a compatible queued GPU group uses
one actual shader dispatch without fallback or mode conversion.

The RGB workload below processes four 1024×768 images per window. It ran on
2026-10-04 on an Apple M3 Pro with macOS 15.7.7, Python 3.12.13, and Rust
1.96.1. The source was `main` at `8bc163521` with the uncommitted
`ImageBatch.Invert` changes described here. Each result is the median of 12
windows after 3 warmups. The benchmark command was
`scripts/benchmark_imagebatch.py --operation invert --mode RGB --width 1024
--height 768 --images 4 --samples 12 --warmups 3`, run once per Pillow, CPU,
SIMD, and GPU profile, with `--queue` added for the final row. A window includes
image construction, submission, execution, any GPU transfer and
synchronization, output splitting, and output materialization. Pillow processes
the same four images sequentially. The eager GPU row runs four individual
operations, with one dispatch per image; the queued row uses one grouped
dispatch. The standard build has the `parallel` feature disabled.

| Profile | Queue | p50 per four-image window (ms) | Throughput (images/s) |
| --- | --- | ---: | ---: |
| Pillow 12.2.0 | Sequential reference | 7.352 | 544 |
| Serial CPU | Immediate single-image path | 1.834 | 2,181 |
| SIMD | Immediate single-image path | 2.745 | 1,457 |
| GPU | Immediate single-image path | 3.274 | 1,222 |
| GPU | Explicit queued group | 3.487 | 1,147 |

In this run, serial CPU was 4.01× faster than Pillow, SIMD was 2.68× faster,
and the queued GPU group was slower than both SIMD and eager GPU. A follow-up
eight-image cohort showed queued GPU improving on eager GPU, but it still
trailed CPU and SIMD; that run used 6 samples and 1 warmup rather than the
12/3 samples above. These results establish parity and one-dispatch grouping,
not a general batch speedup or the 5× SIMD / GPU-throughput goals. Keep this
feature explicit and benchmark the actual cohort size and device before
selecting it.

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

### RGBA `Color3DLUT` batch probe

`ImageBatch.Color3DLUT(filter)` snapshots a four-channel LUT with an RGBA
target. A queued group requires the same wrapper instance and equal-sized
RGBA inputs. It stacks those native RGBA bytes, runs the existing
`Color3DLUT` pipeline once, then splits the result images. `Image.filter`,
automatic backend selection, and other single-image routes are not redirected
through this batch path.

The first byte-for-byte Pillow comparison exposed a pre-existing arithmetic
parity defect shared by the CPU, SIMD, and GPU implementations: they prepared
table samples with four fractional bits, while Pillow prepares signed 10.6
samples. The shared implementation now uses six fractional bits for table
preparation and output rounding; pixel-coordinate interpolation remains
18.15. The focused regression also covers Pillow's negative and clamped LUT
values on CPU, SIMD, and GPU.

The parity run checks full output bytes, mode, dimensions, input order, and
per-image `info`. It exercises eager CPU, vector-eligible eager SIMD, eager
GPU, and queued GPU, and proves one actual GPU dispatch for compatible groups
at 64×64 × 64, 256×256 × 16, and 1024×768 × 4. It also verifies that distinct
LUT wrapper instances stay in separate groups. The command reference lists
the isolated comparison and benchmark commands.

These are full-window throughput measurements on the same Apple M-series host,
with 12 samples and 3 warmups. Every returned image is materialized to bytes
inside the timed window. Pillow runs sequentially through its ordinary filter;
CPU, SIMD, and GPU run through `BatchExecutor(queue=False)`, and the last
column is queued GPU. Values are million pixels per second. The queued GPU
preflight reported the requested backend with no fallback, and parity verified
one dispatch for each listed queued cohort.

| RGBA workload | Pillow | CPU | SIMD | GPU, queue off | GPU, queued | queued/SIMD | queued/Pillow |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 64×64 × 64 | 27.1 | 41.6 | 42.2 | 13.8 | 191.4 | 4.54× | 7.05× |
| 256×256 × 16 | 63.9 | 47.2 | 51.2 | 142.2 | 268.3 | 5.24× | 4.20× |
| 1024×768 × 4 | 67.6 | 50.7 | 50.3 | 338.1 | 200.3 | 3.98× | 2.96× |

Queueing pays off strongly for many small images and for the 256×256 cohort.
At 1024×768 × 4, the per-image GPU path is faster than stacking and splitting
the four results; queued GPU still exceeds SIMD throughput, but reaches only
59% of the queue-off GPU throughput. Shorter exploratory runs at 1024×768 × 8
and × 16 showed the crossover toward queued execution (286 versus 200 Mpix/s
at × 8; 363 versus 344 Mpix/s at × 16), but those smaller samples are not in
the table. This feature should remain explicit, and callers processing only a
few large images should compare both queue settings. The serial CPU and SIMD
profiles are also slower than Pillow in the two larger listed cohorts, so this
batch feature does not close the separate single-image performance gap.
There is no separate Parallel CPU result for this operation: the Color3DLUT
CPU kernel contains no Rayon work, so enabling the `parallel` feature does not
create a distinct execution path to benchmark.

For a reproducible profile, run `scripts/benchmark_imagebatch.py` with
`--operation color3dlut --mode RGBA`, the workload's width, height, and image
count, and `--samples 12 --warmups 3`. Use `--backend pillow`, `cpu`, `simd`,
or `gpu`; add `--queue` only for queued GPU. See the command reference for a
complete invocation.

### Full-frame masked `Paste` batch probe

`ImageBatch.Paste(source, mask)` groups only full-frame pastes at `(0, 0)` with
equal-size destination and source images in the same native `L`, `LA`, `RGB`,
or `RGBA` mode and a same-size `L` mask. The grouped path stacks destinations,
sources, and masks separately, then reuses the existing native masked-Paste
pipeline. It returns per-image results in submission order with each
destination's `info`; ordinary `Image.paste` and automatic routing are
unchanged. The GPU admission planner calls the same mode-specific dispatch
planner as the native kernel and applies the active adapter's buffer and
workgroup limits before building a group. Unsupported or unsafe groups use the
single-image path.

The isolated Pillow parity run checked exact bytes, mode, size, `info`, and
result order for L, LA, RGB, and RGBA, including queued pairs, 64-image
64×64 groups, and 16-image 256×256 groups. Eager `queue=False` Paste also
matched Pillow on CPU, SIMD, and GPU for all four modes; each requested backend
executed without fallback. Every queued cohort used one native-mode GPU
dispatch. The pure planner tests cover both the default 65,535-workgroup edge
and lower adapter workgroup limits without allocating boundary-sized images.

Target-only fault-contract cases exercise injected grouped dimension and
allocation failures. They check Pillow-exact GPU fallback outputs in submission
order, that submitted destinations remain unchanged, and that the same executor
accepts a successful follow-up job. These cases have no oracle execution
because Pillow cannot receive the internal fault injection; they remain
separate from the normal parity result count. The focused command is listed in
the [command reference](COMMANDS.md).

These corrected full-call medians were measured on one Apple M-series host at
256×256 × 16, with 3 warmups and 12 timed windows. The window includes image
construction, submission, execution, GPU upload/readback and synchronization,
result splitting, and `tobytes()` on each result. Pillow runs sequentially and
copies each destination before mutating it, matching ImageBatch's independent
result and input-preservation contract. The eager columns use
`BatchExecutor(queue=False)`; the queued column uses `BatchExecutor(queue=True)`.
All selected pillow-rs profiles executed their requested backend without
fallback. Times are milliseconds per complete window.

| Mode | Cohort | Pillow | Serial CPU | SIMD | GPU, eager | GPU, queued |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| L | 16 images @ 256×256 | 0.903 | 0.773 | 0.564 | 6.265 | 3.858 |
| LA | 16 images @ 256×256 | 2.573 | 0.933 | 1.098 | 6.169 | 3.543 |
| RGB | 16 images @ 256×256 | 3.068 | 1.515 | 2.401 | 6.994 | 3.645 |
| RGBA | 16 images @ 256×256 | 2.006 | 2.816 | 2.162 | 6.907 | 5.762 |

Serial CPU was slower than Pillow for RGBA (2.816 vs 2.006 ms). SIMD reached
1.60× Pillow for L, 2.34× for LA, 1.28× for RGB, and was slower for RGBA; no
mode approaches the 5× target. Queued GPU remained slower than both SIMD and
sequential Pillow in all four modes. These medians are observations for this
run, not stable performance guarantees. CPU measurements use the default build
with Rayon disabled; this cohort is below the 512×512-pixel threshold used by
the feature-gated masked-Paste row scheduler. ImageBatch Paste does not meet the
GPU latency or throughput target.

A mode-specific GPU arithmetic attempt replaced constant `/255` with an exact
integer reciprocal identity. Although the identity was exhaustive over the
rounded-byte numerator range, repeated full-call measurements were noisy and
did not show a reliable gain across modes, so the shader change was reverted.
This leaves host packing and transfer costs as the next measured target; do not
spend more time changing blend arithmetic until profiling shows it is dominant.

The next optimization should remove the input-atlas copies before touching
the shader. For N images of W×H pixels with C native bytes per pixel, masked
Paste must upload destination and source (`2CNWH` bytes), upload the L mask
(`NWH`), and read back the output (`CNWH`): `(3C + 1)NWH` logical payload
bytes across the device boundary, excluding alignment padding. The current batch builder additionally copies
`(2C + 1)NWH` bytes into separate contiguous host stacks before the GPU staging
write. Replace those redundant host stacks with direct writes from each
materialized native image slice into the final upload staging ranges. Then
measure output splitting/ownership copies; specialize the full-frame shader's
index arithmetic only if those data-movement costs are no longer dominant.
The traffic explanation follows the inspected call path; stage-level timings
have not yet confirmed its exact share. Keep this work inside `ImageBatch` and
leave ordinary `Image.paste` on its existing path.

Use the [command reference](COMMANDS.md) for the full-call benchmark and
isolated Pillow parity commands.

### Parallel CPU Paste profile

Masked Paste has a feature-gated Parallel CPU path when the clipped region
contains at least 262,144 pixels (the area of a 512×512 image). A batch with `queue=False` still executes
each image operation in submission order; when built with `parallel`, each
sufficiently large Paste distributes independent output rows through Rayon.
This is separately named **Parallel CPU** and is not SIMD or GPU work. The
isolated benchmark compares that build with ordinary, sequential Pillow.

The feature-enabled correctness lane added one
512×512 CPU parity input per native mode, exactly at the row-parallel
threshold. Pillow bytes, mode, size, and destination `info` matched for L, LA,
RGB, and RGBA, with the CPU backend selected and no fallback. A focused
feature-enabled Rust test also passed for clipped masked Paste.

The full-call benchmark used four 1024×768 images per window, 3 warmups, and
12 samples. The Rayon feature was enabled in the pillow-rs extension, and each
Paste covered 786,432 pixels, above the row-parallel threshold. Pillow ran its
ordinary sequential profile. Times are p50 milliseconds per complete window.

| Mode | Pillow | Parallel CPU | Pillow / Parallel CPU |
| --- | ---: | ---: | ---: |
| L | 1.977 | 1.944 | 1.02× |
| LA | 7.136 | 2.604 | 2.74× |
| RGB | 10.505 | 4.789 | 2.19× |
| RGBA | 6.713 | 5.131 | 1.31× |

Parallel CPU is materially faster for LA, RGB, and RGBA in this run, while L
is effectively tied with Pillow. Treat these as per-mode observations rather
than a promise that Rayon always wins. The measurements stay separate from the
default-build serial CPU rows above, and Pillow does not use this project's
parallel feature. Use the default pillow-rs installation when measuring ordinary
CPU, SIMD, or GPU profiles, as described in the command reference.

The command reference contains the exact Parallel CPU invocation.

### Native-mode `ImageOps.expand` batch

`ImageBatch.Expand(border, fill)` groups equal-size L, LA, RGB, and RGBA inputs
when their border and fill inputs match. It builds a vertical source stack with
native fill rows between images, then runs the existing Expand pipeline once.
The fill resolver preserves Pillow's mode rules, including `(luma, alpha)` for
LA. Outputs retain native mode and per-input ordering. Other modes and unsafe
group sizes use ordinary single-image Expand. The standard `ImageOps.expand`
route is unchanged.

The isolated parity lane compares bytes, mode, dimensions, and `info` with
Pillow 12.2.0 for queued groups and eager calls on CPU, SIMD, and GPU. It
covers all four modes at 7×5, 256×256 × 16, and 1024×768 × 4. Every queued GPU
cohort uses one `expand.wgsl` dispatch, reports zero mode conversions, and
executes on GPU without fallback. Larger output checks use SHA-256 digests of
the exact Pillow and pillow-rs byte buffers to keep the temporary oracle file
small.

Full-call measurements below were collected on one Apple M-series host with 3
warmups and 12 samples. Each window constructs inputs, submits the operations,
executes and materializes outputs, and reads every result's bytes. Pillow, CPU,
and SIMD use sequential per-image calls; GPU uses one queued batch. CPU and SIMD
were measured with the default non-Rayon build. Times are median milliseconds
per complete window. The RGBA 256×256 Pillow and serial-CPU values are medians
across three paired process runs because that comparison varied between runs;
the other cells show one process run each.

| Mode | Cohort | Pillow | Serial CPU | SIMD | Queued GPU |
| --- | --- | ---: | ---: | ---: | ---: |
| L | 16 images @ 256×256, border 7, fill 37 | 0.267 | 0.256 | 0.256 | 1.205 |
| LA | 16 images @ 256×256, border 7, fill 37 | 1.311 | 0.484 | 0.506 | 2.835 |
| RGB | 16 images @ 256×256, border 7, fill 37 | 1.561 | 0.664 | 0.664 | 2.388 |
| RGBA | 16 images @ 256×256, border 7, fill 37 | 1.196 | 0.999 | 1.010 | 3.212 |
| RGBA | 4 images @ 1024×768, border 7, fill 37 | 4.671 | 2.469 | 2.336 | 7.386 |

Serial CPU beat Pillow in each listed mode and cohort; the repeated medium RGBA
pair measured 1.20×. SIMD ranged from 1.0× to 2.6× Pillow on the medium cohort
and was 2.0× faster on the large RGBA cohort; it did not reach the overall 5×
target. Queued GPU was slower than SIMD in every row, so this Expand workload
does not meet the GPU latency or throughput target. There is no separate
Parallel CPU result: Expand's current CPU kernel does not use Rayon.

The queued RGBA 256×256 × 16 receipt measured 4,409,344 uploaded bytes,
4,665,600 readback bytes, one Expand dispatch, and zero mode conversions. This
is a native byte copy/pad operation, so the remaining GPU cost is data movement
and host ownership of independent output images rather than shader arithmetic.
The batch splitter now shares each owned output raster with its lazy result
instead of cloning the entire materialized image again. Further GPU gains need
to remove the contiguous host input-stack copy or let per-image outputs share
storage safely; changing the shader's pixel math would not address the measured
transfer volume. Treat this as a documented performance blocker for this
operation and keep the queued API separate from ordinary image routing.

The target-only fault-contract lane injects a grouped dimension error and a
grouped allocation error. For both, it verifies one exact GPU fallback per
image, preserved result order and metadata behavior, and a successful follow-up
join. These injected internal faults have no Pillow oracle case; the ordinary
parity cases establish their returned image values.
