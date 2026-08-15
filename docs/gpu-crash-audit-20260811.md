# GPU crash and lazy-pipeline audit

Date: 2026-08-11  
Scope: `pillow-rs` GPU pool and the registered WGSL source tree

## Result

The audit found no unbounded loop in the 72 WGSL kernels embedded by the
active registry or in the two internal separable-blur pipelines compiled for
`BoxBlur`/`GaussianBlur`. The dynamic loops are bounded by host or shader
constants:

| Kernel family | Bound |
| --- | --- |
| Mandelbrot | `quality`, capped at 10,000 iterations |
| Box/Gaussian blur | radius, capped at 16 |
| Min/Max/Median/Rank filters | odd window, capped at 9 (`81` samples) |
| Median/Rank insertion sort | at most 81 items; the inner index strictly decreases |
| `Reduce` | x/y factors are capped at 64; zero factors normalize to one |
| `PutData`/`Pad` | at most four channels |

Finite work is bounded separately from termination: blur/filter sample counts
and Mandelbrot pixel×iteration work are preflighted against a conservative
128M inner-work budget. Requests above that budget use the CPU path, avoiding
driver-watchdog failures from a technically terminating but excessive kernel.

The audit also found and addressed these registered-path hazards:

- primary and output dimensions are checked before device initialization or
  image upload; zero-sized and over-capacity images route to the CPU;
- auxiliary images are checked for nonzero dimensions, capacity, and the
  shape required by the operation before a bind group is created;
- `AlphaComposite` uses its source auxiliary buffer and updates the current
  destination buffer in place, so ping-pong state remains correct;
- output-only generators use `[output, params]` bindings;
- transpose dispatch dimensions and source-coordinate formulas are explicit;
- `Reduce` carries both factors and host-computed output dimensions;
- scale, transform, enhancement, additive, autocontrast, and Mandelbrot
  numeric inputs are bounded before conversion to shader uniforms;
- inverted `Colorize` ramps use signed arithmetic rather than unsigned
  underflow;
- Mandelbrot uses the same 16x16 workgroup size assumed by host dispatch.
- dispatch grids are checked against the selected adapter's
  `max_compute_workgroups_per_dimension`, not only the aggregate pixel cap;
- `Reduce` returns safely for an empty accumulation block instead of dividing
  by zero if malformed factors reach an explicitly selected GPU backend;
- mutable ping-pong storage is allocated per batch and sized to that batch's
  high-water mark, so concurrent lazy batches cannot overwrite one another.
- large signed `Offset` values are normalized in WGSL before unsigned modular
  arithmetic, preventing parameter underflow;
- image storage allocation is checked against the selected adapter's
  `max_storage_buffer_binding_size` and `max_buffer_size` before any batch
  buffers are created;
- LA/PA `ExtractBand(1)` reads the alpha byte rather than the unused green
  byte;
- malformed operation/auxiliary counts are rejected before indexed access;
- the final storage-to-readback copy is recorded after the final compute pass
  in the same command buffer, removing the old second readback encoder and
  submit lifecycle;
- fractional Gaussian box radii whose integer part is zero retain their
  nonzero edge weight instead of being mistaken for `BoxBlur(0)` identity;
- blur and Min/Max watchdog estimates count all eagerly evaluated channel
  work, and the only admitted Sharpness endpoint returns before convolution;
- RGBA `ExtractBand(1)` reads green; only the two-band LA transport remaps
  channel 1 to the packed alpha byte;
- single-pass shaders that do not implement their complete public contract
  (`Autocontrast`, `Equalize`, `Quantize`, `Colorize`, unsupported resampling
  filters, incomplete mode conversion, and expanded rotation) are refused by
  the registry and use the CPU implementation.

`Cover`, `Fit`, `Contain`, and `Thumbnail` remain CPU fallbacks because their
full output-size/scale contract is not yet explicit in the GPU preparation
path. This is a reviewed safety boundary; no public input or coverage case was
removed.

## Lazy execution boundary

`PipelineOp` chains remain deferred until materialization. A normal GPU batch
does the following:

1. materializes nested auxiliary images as required by the operation contract;
2. uploads the primary image once;
3. records multiple ordered dispatches, split only by bounded operation or
   transient-arena limits;
4. submits chunks without an intermediate CPU readback;
5. records one final copy into a mapped staging buffer and performs one final
   readback.

Ping-pong storage and optional fallback bindings are execution-local and sized
to the batch high-water mark. This removes the old process-wide execution
mutex: independent lazy batches can record and submit concurrently while each
batch still preserves operation order through its own ping-pong resources and
the single device queue. A batch may still split into sequential submissions
when its operation count or transient arena exceeds the configured bound; it
does not read back or serialize through the CPU between those chunks.

GPU readback uses short polling intervals and a finite deadline. Device-lost
and uncaptured-error state poisons the process-local GPU singleton so automatic
routing stops selecting a failed device. Browser/WASM still needs an async
adapter and map/readback path; synchronous `pollster`/polling is not a valid
browser completion strategy.

## Dormant shader files

The following files are present but not embedded by the active registry and
were not treated as executable GPU coverage:

`autocontrast_cutoff.wgsl`, `autocontrast_histogram.wgsl`,
`autocontrast_remap.wgsl`, `color_3dlut.wgsl`, `effect_noise.wgsl`,
`equalize_cdf.wgsl`,
`equalize_histogram.wgsl`, `equalize_remap.wgsl`, `histogram_clear.wgsl`,
`merge.wgsl`, and `resize_nearest.wgsl`.

They remain classified as dormant implementation assets. They were not
deleted, registered, or used to improve a denominator. The two blur files are
not dormant: they are compiled as internal `BlurH`/`BlurV` pipelines and are
used for the lazy multi-pass blur expansion.

## Verification

- Managed `make test-core`: 112 passed, 0 failed.
- `make backend-support-matrix`: 87 CPU operations, 65 SIMD-pool operations,
  and 72 registered GPU shader source entries. The last number is a raw
  registry/source count, not a claim that all 72 public operations are
  eligible for GPU execution; contract and numeric-safety gates route
  incomplete or non-exact cases to CPU.
- Managed `make build-dev` rebuilt the Python extension with the embedded
  shaders before adapter-backed parity.
- Full GPU run `aa42c082-c48b-4197-aa90-9eac38508294` executed 3,673/3,673
  cases in 11.87 seconds: 3,672 passed, one shared font case failed, and no
  infrastructure error occurred. A second full run produced the same result
  in 11.44 seconds.
- After adding the distinct-channel RGBA `getchannel(1)` public regression,
  full GPU run `3ceb9f66-ac66-40e0-9118-f2bf0769b3bd` executed 3,674/3,674
  cases in 11.75 seconds: 3,673 passed, the same shared font case failed, and
  no hang, crash, timeout, not-run case, or infrastructure error occurred.
- Managed `make test` built once and ran the maintained combined campaign:
  CPU `3672/3673`, SIMD `3666/3673`, GPU smoke `1/1`, full GPU `3672/3673`,
  and JS/WASM passed. The aggregate correctly failed for visible parity
  mismatches; it did not hide or relabel them.
- Full GPU parity is requested by default after the smoke gate. Adapter
  absence alone is recorded as `skipped`/not proven; timeout, crash, kernel
  error, or parity mismatch is fatal. Every child is isolated in a process
  group, and the full GPU lane has an independent 300-second hard cap.
- Targeted rustfmt checks for the two touched Rust files and `git diff --check`
  passed. The repository-wide `make fmt` check still reports pre-existing
  formatting drift in unrelated dirty files.

## Remaining blockers

The application-level trigger is fixed and adapter-backed full-corpus runs are
stable, but the exact native mechanism inside the historical wgpu
`copy_buffer_to_buffer` call remains unproven. The evidence does not justify
claiming a particular Vulkan call or command-pool exhaustion. The external
process-group deadline remains necessary because an in-process deadline cannot
interrupt a wedged native copy, submit, or poll call.

One parity mismatch remains in the full GPU-selected corpus:
`PIL.ImageFont.FreeTypeFont.set_variation_by_axes.nuanced.variable-font-positive-axis-overflow`.
It reproduces in CPU and SIMD lanes and is classified as font behavior, not a
GPU backend failure. Reviewed CPU fallbacks also remain for public operations
whose shader arithmetic, mode, geometry, or complete Pillow contract is not
yet exact; no public input or coverage denominator was removed.

The shader safety policy follows the WGSL storage-access and dispatch rules in
the [WGSL specification](https://gpuweb.github.io/gpuweb/wgsl/) and the
resource/queue model documented by [wgpu](https://docs.rs/wgpu/latest/wgpu/).
