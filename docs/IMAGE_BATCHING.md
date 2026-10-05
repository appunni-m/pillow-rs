# Explicit GPU batching and streaming

<!-- release:summary -->
**Latest release: [12.2.0](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.0).**
<!-- /release:summary -->

`GpuBatchExecutor` is implemented in the source checkout. It schedules existing
lazy image graphs on the pillow-rs GPU device, separately from normal Image routing.
The former ImageBatch experiment remains in the
[deprecated archive](../deprecated/imagebatch/README.md). No release is implied
by this source change.

## Submit the existing operation result

`ImageOps.invert(image)` returns an Image backed by the canonical Rust
[`Image::Pipeline`](../pillow-rs/src/image.rs) and
[`PipelineOp`](../pillow-rs/src/pipeline.rs). Submit that Image directly;
there is no additional operation class, conversion method, or callback API.

```python
from PIL import Image, ImageOps, GpuBatchExecutor

with GpuBatchExecutor(queue=True) as executor:
    job_id = executor.submit(ImageOps.invert(Image.new("L", (256, 256), 42)))
    for result in executor.join():
        assert result.job_id == job_id
        result.image.save("inverted.png")
```

`submit(pipeline)` validates the pending graph synchronously and returns a
monotonic unsigned 64-bit job ID. Rejected admissions consume an ID; a retry
receives a new ID. Overflow errors instead of wrapping. Manual admission at
capacity raises `QueueFull`. Drain the bounded window with `join()` before
submitting more work. Creating `join()` does not wait; iteration drives GPU
submission and completion. `queue=False` completes one GPU submission per job
before `submit()` returns; its undelivered results still occupy the window.

## Stream inputs and identify outputs

`run(jobs, output="cpu")` lazily transforms an iterator of
`(input_key, existing_lazy_image)` into materialized results. It fills only a
bounded window, groups jobs into submissions, and delivers one result at a
time. Neither inputs nor all results are collected internally.

```python
from PIL import Image, ImageOps, GpuBatchExecutor

paths = iter(["a-gray.png", "b-gray.png"])
jobs = ((path, ImageOps.invert(Image.open(path))) for path in paths)
with GpuBatchExecutor(gpu_bytes=512 << 20, host_bytes=256 << 20,
                      max_jobs=64, max_in_flight=2) as executor:
    with executor.run(jobs) as results:
        for result in results:
            result.image.save(result.input_key + ".inverted.png")
```

Every result contains `job_id`, `input_key`, and `image`. **Completion order is
unspecified.** Match by ID or key, never by iterator position or `zip` with the
input. Duplicate keys are allowed; job IDs distinguish them. Keys must be
strings or unsigned 64-bit integers; booleans and arbitrary objects are rejected.
Manual callers can retain a bounded ID-to-input map and remove entries when
they consume results.

Only one Results driver can own an executor. Concurrent `submit`, `join`, or
`run` during iteration is rejected, as is recursive/concurrent iteration.
Exhausting a stream allows another window on that executor. Closing or dropping
an unfinished Results cancels the executor; use its context manager for
predictable cleanup.

## Resource bounds and output ownership

Independent caps govern admission and how far execution can run ahead:

| Setting | Charged resources |
| --- | --- |
| `gpu_bytes` | Conservative per-graph working buffers, native uploads, parameters/LUTs, terminal staging, resident allocations, and their reserved download workspace |
| `host_bytes` | Conservative decode/upload temporaries, encoded source bytes, metadata, staging, and undelivered CPU pixels |
| `max_jobs` | Pending, executing, and undelivered jobs |
| `max_in_flight` | Outstanding GPU submissions |

Defaults are 512 MiB GPU, 256 MiB host, 64 jobs, and two submissions. These are
admission reservations, not measurements of process RSS or free VRAM. Shared
source owners are conservatively charged per graph. The Python producer may
retain one yielded `(key, Image)` while waiting for credits; this single
lookahead and memory retained by the producer itself are outside native byte
credits. Caller-owned collections of inputs or delivered CPU Images also remain
outside the executor's cap. Metadata accepts ordinary scalar, byte, and
container values and has a bounded nesting depth; custom objects/subclasses are
rejected. Nested metadata is snapshotted at admission and independently copied
for downloads, so later caller mutations do not grow the charged snapshot.

A job exceeding a hard cap fails before decode or GPU allocation. A valid job
that cannot fit beside live work raises `QueueFull`; `run()` drains work and
retries that input. Actual granted device buffer/binding, indexing, alignment,
workgroup, and cumulative shader-work bounds are also checked. Device limits
are not a free-memory guarantee; allocation/device failures remain errors.
The current workspace bounds each graph to 4096² peak pixels and each submission
to 256 operations and the GPU backend's shader-work bound.

Each graph has independent native pixel buffers, coordinates, borders, and
parameters. Several graph command buffers plus terminal copies share one
`queue.submit`; images are not stacked. There is no intermediate readback or
CPU image-operation fallback, and no Rayon or ThreadPoolExecutor scheduling.
Each dispatch gets stable parameters, so later queued writes cannot overwrite
an earlier operation's uniforms.

CPU outputs share a mapped staging arena for their submission. Only the next
result range is copied into its owning CPU Image. The whole arena stays charged
and unavailable to GPU work until its last ready range/view is released; it is
then unmapped and dropped. Per-job GPU buffers can retire after their terminal
copy completes. A slow consumer therefore stops admission when credits fill.
The executor retains counters, not an unbounded per-job history.

## Keep outputs on GPU

```python
from PIL import Image, ImageOps, GpuBatchExecutor, ResultBackpressure

jobs = ((i, ImageOps.invert(Image.new("L", (256, 256), i % 256)))
        for i in range(100))
with GpuBatchExecutor(max_jobs=8) as executor:
    with executor.run(jobs, output="gpu") as results:
        for result in results:
            cpu_image = result.image.download()
            cpu_image.save(f"result-{result.input_key}.png")
            del result  # release the previous GPU lease before pulling more
```

`GpuImage` is an opaque, immutable pillow-rs-owned resident handle exposing `mode`,
`size`, and `download()`. It currently has no public device, buffer, texture,
external-framework import, or resident-input resubmission API. Downloads copy
native bytes and leave the GPU allocation charged while its handle remains
alive. Returned handles stay valid after ordinary executor close while the
shared device remains healthy.

If caller-held resident outputs fill the budget, iteration raises
`ResultBackpressure`. Release those handles, then retry `next()` on the same
Results; this signal does not close the stream. The producer may already have
yielded the one pending lookahead before pressure is detected; that item is
retained for retry, and nothing beyond it is pulled.
Each resident lease retains a reservation for staging, one CPU pixel copy, and
cloned metadata, so later admission cannot starve its download. Downloads from
clones of the same handle serialize through that reserved workspace. Holding
resident outputs therefore retains host and GPU credits even before download.
`readback_bytes` counts scheduler CPU-output copies; explicit handle downloads
are separate and do not increment that counter.

## Admission and native modes

The stream planner accepts proven 8-bit layouts: L, LA, RGB, RGBA, CMYK, and
RGBX. It never inserts an RGBA conversion. GPU registry support alone does not
prove every operation/mode/parameter context; unresolved contexts fail at
admission. Current routes include:

| Operation family | Native contexts |
| --- | --- |
| Copy, invert, flip, mirror, transpose, crop, expand descriptor | The admitted byte layouts |
| Channel extraction | Valid sample index; output L, LA alpha index 1, RGBA alpha index 3 |
| Solarize, posterize, point | L/RGB; RGBA point tables where public semantics allow them |
| 3×3 median/min/max | L, LA, RGB, RGBA |
| Other rank/order filters | Proven L/LA parameter sets and bounded RGBA kernels |
| Gaussian blur | L/LA/RGB/RGBA with the existing exact, bounded radius calculation |
| Box blur | L/LA radius 1; bounded RGB/RGBA routes |
| Grayscale | All admitted byte layouts, including direct L/LA luma selection |
| Explicit conversion | Default L/RGB → RGBA without matrix/dither |
| Nearest resize | RGBA with output width + height ≤ 2048 for the stage-table reservation |
| Unmasked paste | Matching final native modes and the source's complete region, including lazy secondary graphs |
| Binary composition | Matching native modes/dimensions: multiply, screen, overlay, hard/soft light, difference, darker/lighter, modulo add/subtract, blend |
| Constant descriptor | Admitted byte sources; output L |
| Other RGBA descriptors | Exact brightness/color, convolution, and valid pixel updates admitted by the planner |

P/PA and 1-bit modes, typed scalar, premultiplied, masked-paste, and unresolved
host-controlled contexts are currently rejected. An explicit Convert operation
is allowed only when its native input/output layout is implemented.
A completed graph cache supplies pixels directly; already completed operations
are not replayed. An uncached encoded leaf uses the existing CPU codec decoder
then one native upload. CPU decoding is distinct from CPU image-operation
fallback. Current `getchannel()` materializes its input to validate bands before
recording ExtractBand; only operations still pending at submission are queued.

Native job-scoped failures expose `job_id`, `input_key`, `stage`, and `kind`.
Producer/adapter validation may raise ordinary Python errors before an ID
exists. Device-wide errors may have no identity; a failed submission may name
one representative job rather than report each affected job separately.
`Unsupported` includes contexts not proven within the planner's device/work
bounds; `Limit` identifies known configuration/admission-resource/identity
failures. Python's stream driver raises retryable `ResultBackpressure`; the
Rust core reports resident-credit pressure as `QueueFull` during submission.
An unsupported input
in `run()` stops reading later inputs; a generator's future inputs cannot be
validated in advance. Delivered CPU results remain valid; resident handles
require a healthy device. There is no automatic
replay. Close cancels image commands that have not been submitted and fences
queued uploads/in-flight commands before releasing their credits, discarding
unwanted images without downloading them. Timeout/device failure follows the
existing GPU backend failure policy.
Cancellation stops pulling from the input iterator; it does not call that
iterator's `close()`. If you retain a generator or another resource-owning
producer, close its resources yourself.

## Parity and performance evidence

The [focused validation guide](GPU_BATCH_VALIDATION.md) records exact live
Pillow comparisons, lifecycle checks, and completed-work timings. The benchmark
includes fresh graph construction, GPU upload/execution, materialization, and
terminal bytes. It uses normal sequential Pillow as its baseline.

Queue depth affects performance: grouping submissions does not guarantee
concurrent kernels or a speedup for every size/mode. Resident output skips
scheduler readback, while explicit downloads still transfer native bytes.

## Future codec destination integration

image-slash-star currently decodes into owned CPU bytes and has no public GPU
buffer API. Its `decode_into_with_policy` decodes into a Vec then copies;
using it would not establish direct decoding into GPU memory. This feature
therefore keeps existing decoding and removes intermediate GPU downloads.

A later integration can add a genuine codec destination API receiving only
`&mut [u8]`: Pillow owns a fresh mapped buffer, lends its native sample slice,
drops the view, and unmaps before GPU use on the same Device/Queue. CPU decoding
remains CPU decoding; driver transfers may still occur. Repeated mapping needs
legal usage/features, and a portable fallback uses mapped staging plus a GPU
copy. This is separate work with its own parity and measured benefit. See the
pinned [wgpu Buffer contract](https://docs.rs/wgpu/24.0.5/wgpu/struct.Buffer.html).
