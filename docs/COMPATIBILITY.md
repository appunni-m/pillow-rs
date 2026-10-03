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
| Queue compatible independent GPU operations | `PIL.ImageBatch.BatchExecutor`; groups native `L`, `LA`, `RGB`, and `RGBA` `MedianFilter(3)`, `ExtractBand`, and `Multiply` jobs, plus same-instance RGBA `Color3DLUT` jobs; see [batching guide](IMAGE_BATCHING.md) |
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

pillow-rs targets Pillow 12.2.0 behavior where an API is implemented, and adds
the package, API, and execution capabilities below. The comparison is with the
upstream [Pillow 12.2.0 source](https://github.com/python-pillow/Pillow/tree/12.2.0/src/PIL)
and its first-party Python package. Third-party plugins and application code
are outside the comparison. “Not in Pillow” means the upstream package does not
provide the equivalent first-party integration or execution interface; it does
not mean users cannot build similar systems around Pillow.

The table describes the current source tree. Some additions postdate the latest
published `12.2.0-alpha.5`; check [installation](INSTALLATION.md) and the
[release notes](../CHANGELOG.md) before relying on a feature in a released
package.

| Addition | What pillow-rs adds | Difference from Pillow 12.2.0 | Availability and limits |
| --- | --- | --- | --- |
| Explicit multi-image queue | `PIL.ImageBatch.BatchExecutor` accepts independent jobs with `submit()` and returns results in submission order from `join()`. `queue=False` executes each job immediately; `queue=True` defers work until `join()`. | Pillow 12.2.0 has no first-party `ImageBatch` executor for queueing compatible image jobs. | In the current source; absent from published `12.2.0-alpha.5`. GPU grouping is implemented for `MedianFilter(3)`, `ExtractBand`, `Multiply`, and a shared RGBA `Color3DLUT` when mode and dimensions match. Incompatible jobs use their ordinary single-image path. See the [batching guide](IMAGE_BATCHING.md). |
| Runtime backend controls | Python, Rust, and JavaScript/WebAssembly expose `available_backends`, `active_backends`, `backend_enabled`, `enable_backend`, and `disable_backend` for routes compiled into each build: CPU, SIMD, and GPU where enabled. | Pillow 12.2.0 has no equivalent public controls for selecting its image-compute routes. | Included in the published alpha. `available_backends()` lists compiled implementations, not device readiness. Enabling or disabling a route changes automatic routing; an operation can still fall back when that route does not support it. |
| GPU compute backend | The Rust core can run registered image-operation kernels with `wgpu`. The core `gpu` Cargo feature is enabled by default and is inherited by the standard Python build. | Pillow 12.2.0 has no built-in GPU image-compute backend in its first-party package. | Device- and operation-dependent. The standard JavaScript/WebAssembly build disables core defaults and does not include GPU. Selecting GPU does not prove that a kernel ran; check actual-backend and dispatch evidence. See [compatibility limits](#partial-or-unsupported). |
| Optional Parallel CPU build | A default-off Cargo feature and companion Python distribution enable Rayon-backed CPU execution for supported work. The project reports this as **Parallel CPU**, separately from serial CPU, SIMD, and GPU. | Pillow 12.2.0 exposes no matching pillow-rs-style build profile or selector. This describes the explicit profile, not whether Pillow or its codecs use threads internally. | Configured in the current source but not included in published `12.2.0-alpha.5`. Install it with `pillow-rs[parallel]` after matching standard and companion wheels are published. Rayon is used only for CPU work; SIMD and GPU paths do not use it. See [Python installation](INSTALLATION.md#python). |
| Rust library | The `pillow-rs` crate exposes the implemented Pillow-style image surface directly to Rust callers, without requiring Python. | Pillow's first-party package does not provide a Rust crate API. | The crate is published; use the [Rust guide](RUST.md). Its API is a selected implementation surface, not a promise of complete Pillow coverage. |
| Node.js and browser WebAssembly | The `pillow-rs` npm package exposes image operations to Node.js and browsers through WebAssembly. | Pillow has no first-party JavaScript or WebAssembly binding. | The npm package is published. JavaScript has its own method names and WASM-owned objects must be freed; browser applications need a bundler or module server that serves the WASM asset. See the [JavaScript guide](../pillow-rs-js/README.md). |
| Pipeline execution telemetry | Rust and JavaScript/WebAssembly can opt in to a receipt for the latest completed pipeline, including requested and selected backend, operation paths, fallback reason, timing, and optional GPU dispatch/resource counters. | Pillow's public image API has no equivalent per-pipeline backend receipt. | Included in the published alpha. Diagnostic data, not an image-processing guarantee or whole-process metrics. Python helpers are available only under the internal `pillow_rs._core` module, not the public `PIL` namespace. Receipts are bounded to the latest sample on the executing thread. |

These are the system-level additions; the ordinary Pillow-compatible image
operations are reimplementations, not new Pillow features. For example,
[`ImageOps.cover`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2183)
and `ImageFilter.Color3DLUT` already exist in Pillow. SIMD is not unique to
pillow-rs either: Pillow has optimized native code. Native-mode processing is
an implementation choice, not evidence that Pillow lacks mode-aware paths.
None of the additions guarantees a speedup. Compare the operation, mode, and
backend you use in the [benchmark results](https://appunni-m.github.io/pillow-rs/benchmarks/).

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
