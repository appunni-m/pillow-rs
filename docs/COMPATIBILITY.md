# Supported APIs and limitations

<!-- release:summary -->
**Latest release: [12.2.0-alpha.5](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.0-alpha.5).**
<!-- /release:summary -->

pillow-rs implements a subset of Pillow. Use the tables below to decide whether
it fits your application. A supported operation can still reject unsupported
modes, options, or input formats.

## Supported API families

| What you want to do | APIs to start with |
| --- | --- |
| Create, open, save, and inspect images | `PIL.Image`: `new`, `open`; image `save`, `size`, `mode`, pixel access |
| Resize, crop, rotate, or convert | Image `resize`, `crop`, `rotate`, `convert` |
| Flip, mirror, fit, or apply image operations | `PIL.ImageOps` and `PIL.ImageChops` |
| Queue compatible independent GPU operations | `PIL.ImageBatch.BatchExecutor`; groups native `L`, `LA`, `RGB`, and `RGBA` `MedianFilter(3)`, `ExtractBand`, `Multiply`, and masked `Paste` jobs, plus same-instance RGBA `Color3DLUT` jobs; see [batching guide](IMAGE_BATCHING.md) |
| Draw shapes and text | `PIL.ImageDraw` |
| Adjust color or apply filters | `PIL.ImageColor`, `PIL.ImageEnhance`, `PIL.ImageFilter` |
| Load fonts and measure text | Selected `PIL.ImageFont` APIs; see font limits below |
| Work with palettes, frames, or statistics | `PIL.ImagePalette`, `PIL.ImageSequence`, `PIL.ImageStat` |

Start with the [Python recipes](PYTHON.md), [Rust guide](RUST.md), or
[JavaScript guide](../pillow-rs-js/README.md). JavaScript uses its own API names
and initialization; Python examples cannot be pasted directly into JavaScript.
The [complete selected-operation inventory](https://appunni-m.github.io/pillow-rs/api-support/)
is available when you need to check a specific public path.

## pillow-rs additions beyond Pillow

pillow-rs uses Pillow 12.2.0 as its behavior-parity target for the operations it
implements. The feature comparison below checks against the current upstream
[Pillow 12.3.0 source](https://github.com/python-pillow/Pillow/tree/12.3.0/src/PIL)
and [first-party API reference](https://pillow.readthedocs.io/en/12.3.0/reference/index.html),
released on 2026-07-01. Third-party plugins and application code are outside
the comparison. “Not in Pillow” means upstream does not provide the equivalent
first-party interface; it does not mean users cannot build a similar system
around Pillow.

These are the first-party system features in this checkout that do not have an
equivalent in Pillow: job batching, selectable compute backends, built-in GPU
execution, an opt-in Parallel CPU profile, Rust and JavaScript/WebAssembly
integrations, and per-pipeline execution receipts. This is not a list of image
operations pillow-rs implements; operations shared with Pillow belong to the
compatibility surface. The table marks features that are only in this checkout
separately from those in the latest published pillow-rs alpha,
`12.2.0-alpha.5`. Check [installation](INSTALLATION.md) and the
[release notes](../CHANGELOG.md) before relying on a source-only feature in a
released package.

| Addition | What pillow-rs adds | Difference from Pillow 12.3.0 | Availability and limits |
| --- | --- | --- | --- |
| Explicit image-job queue | `PIL.ImageBatch.BatchExecutor` accepts independent jobs with `submit()` and returns results in submission order from `join()`. `queue=False` executes each operation immediately; `queue=True` defers it until `join()`. | Pillow has no first-party `ImageBatch` executor for submitting independent image jobs and joining compatible work. | Current source only; absent from published `12.2.0-alpha.5`. Queued GPU grouping supports `MedianFilter(3)`, `ExtractBand`, `Multiply`, full-frame masked `Paste`, and shared-instance RGBA `Color3DLUT`. Compatible groups require native `L`, `LA`, `RGB`, or `RGBA` layouts and equal dimensions; Paste also requires an equal-sized `L` mask, and Color3DLUT jobs must reuse the same wrapper instance. Unsupported or incompatible jobs use the ordinary single-image path. See the [batching guide](IMAGE_BATCHING.md). |
| Runtime backend controls | Python, Rust, and JavaScript/WebAssembly expose `available_backends`, `active_backends`, `backend_enabled`, `enable_backend`, and `disable_backend` for the routes compiled into that build: CPU, SIMD, and GPU where available. | Pillow's public API has no equivalent controls for selecting these image-compute routes. | Included in `12.2.0-alpha.5`. `available_backends()` lists compiled implementations, not device readiness. Selecting a route changes automatic routing; unsupported operations can still fall back. |
| GPU compute | The Rust core runs registered image-operation kernels with `wgpu`; the `gpu` Cargo feature is enabled by default and is inherited by the standard Python build. | Pillow 12.3.0 has no built-in GPU image-compute backend in its first-party package. | Included in `12.2.0-alpha.5` for supported Rust and Python builds. Device and operation support vary. The standard JavaScript/WebAssembly build disables core defaults and does not include GPU. Requesting GPU does not prove that a kernel ran; inspect the selected backend and dispatch evidence. See [compatibility limits](#partial-or-unsupported). |
| Optional Parallel CPU profile | A default-off Cargo feature and companion Python distribution enable Rayon-backed CPU execution for supported work. pillow-rs reports this as **Parallel CPU**, separately from serial CPU, SIMD, and GPU. | Pillow has no equivalent pillow-rs build profile and runtime selector. This is a profile comparison, not a claim that Pillow or its codecs never use threads internally. | Configured in the current source but not included in published `12.2.0-alpha.5`. When matching standard and companion wheels are published, install with `pip install 'pillow-rs[parallel]'`. Rayon is used only for CPU work; it is not part of SIMD or GPU execution. See [Python installation](INSTALLATION.md#python). |
| Rust library | The `pillow-rs` crate exposes the implemented Pillow-style image surface directly to Rust callers, without requiring Python. | Pillow's first-party package does not provide a Rust crate API. | Published; see the [Rust guide](RUST.md). The crate implements a selected surface and does not promise complete Pillow coverage. |
| Node.js and browser WebAssembly | The `pillow-rs` npm package exposes image operations to Node.js and browsers through WebAssembly. | Pillow has no first-party JavaScript or WebAssembly binding. | Published. JavaScript has its own method names; callers must release WASM-owned objects, and browser applications need a bundler or module server that serves the WASM asset. See the [JavaScript guide](../pillow-rs-js/README.md). |
| Pipeline execution telemetry | Rust and JavaScript/WebAssembly can opt in to a receipt for the latest completed pipeline: requested and actual backend, per-operation paths, route/validation/backend timings, fallback reason, and available GPU dispatch/resource counters. | Pillow's public image API has no equivalent per-pipeline backend receipt. | Included in `12.2.0-alpha.5`. This is diagnostic data, not an image-processing guarantee or whole-process profiler. Python helpers are only in the internal `pillow_rs._core` module, not the public `PIL` namespace. Receipts retain only the latest sample on the executing thread. |

These are system-level additions. Pillow-style image operations such as
[`ImageOps.cover`](https://github.com/python-pillow/Pillow/blob/12.3.0/src/PIL/ImageOps.py)
and `ImageFilter.Color3DLUT` already exist in Pillow, so implementing them here
is compatibility work rather than a new feature. Pillow also documents native
C performance improvements in its [12.3.0 release notes](https://pillow.readthedocs.io/en/12.3.0/releasenotes/12.3.0.html),
so optimized native code is not unique to pillow-rs. Processing an image in
its native mode is an implementation choice, not evidence that Pillow lacks
mode-aware paths. None of the additions guarantees a speedup; compare the
operation, mode, and backend in the
[benchmark results](https://appunni-m.github.io/pillow-rs/benchmarks/).

## Partial or unsupported

| Area | Limitation |
| --- | --- |
| Full Pillow replacement | APIs outside the selected operation inventory are not promised |
| Image modes and options | Support varies by operation; verify the combinations your application uses |
| Codecs | JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO/CUR have selected supported paths; metadata, frames, and encoding differ by format |
| AVIF | Partial and outside the current browser codec contract |
| Fonts | Selected font loading, masks, and metrics; full shaping, color-font behavior, and FreeType replacement are not promised |
| GUI, screen capture, external color management | Outside the supported public contract |
| GPU acceleration | Optional and operation-dependent; choosing GPU can fall back to CPU |
| API stability | Alpha releases may introduce breaking changes |

The release uses image-slash-star 0.1.2 and fontdone 2.14.3-alpha.10.
Dependency projects' newer documentation can describe features this release
does not include. Consult their versioned
[codec reference](https://docs.rs/image-slash-star/0.1.2/image_slash_star/) and
[font reference](https://docs.rs/fontdone/2.14.3-alpha.10/fontdone/) when needed.

## Before replacing Pillow

Try the operations, images, fonts, frame metadata, and expected errors your
application relies on in separate Pillow and pillow-rs environments.
[Migration steps](PYTHON.md#evaluate-an-existing-pillow-application) explain how.
For measured comparisons and open implementation work, see the contributor
[coverage evidence](COVERAGE.md) and [roadmap](PIPELINE_ROADMAP.md).
