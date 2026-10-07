# Supported APIs and limitations

<!-- release:summary -->
**Latest release: [12.2.1](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.1).**
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
implements. This section identifies capabilities of the pillow-rs system that
Pillow does not provide through an equivalent first-party interface. The
comparison uses the upstream [Pillow 12.3.0 source](https://github.com/python-pillow/Pillow/tree/12.3.0/src/PIL)
and its [first-party API reference](https://pillow.readthedocs.io/en/12.3.0/reference/index.html).
Pillow 12.3.0 was released on 2026-07-01 and was the latest stable Pillow
release on 2026-10-04 ([upstream release list](https://github.com/python-pillow/Pillow/releases)).
Third-party plugins and application code are outside the comparison. “Not in
Pillow” means upstream does not provide the equivalent first-party interface
or runtime integration; it does not mean users cannot build a similar system
around Pillow.

These are system additions around a selected Pillow-compatible image API, not
claims that ordinary Pillow operations are unique to this project. The table
separates application/runtime capabilities from contributor tooling and
describes the stable `v12.2.1` release and its matching source commit. For the
versions available to install, check the live
[crates.io](https://crates.io/crates/pillow-rs), [PyPI](https://pypi.org/project/pillow-rs/),
and [npm](https://www.npmjs.com/package/pillow-rs) listings. The
[registry release matrix](REGISTRY_RELEASE_MATRIX.md) records release evidence
and older artifacts, not a live version index. Each row states any platform or
operation limits that still apply.

**Application and runtime features added around Pillow's API:**

- A Rust-owned image, font, and codec runtime, a public Rust crate, and a
  Python `PIL` facade that delegates to the Rust core.
- Deferred chains for supported image operations.
- [Explicit GPU batching](IMAGE_BATCHING.md) streams existing lazy graphs with
  unique IDs and bounded native CPU/resident outputs. Unsupported contexts error
  at admission. It is included in 12.2.1; the earlier ImageBatch prototype was
  never part of the public API.
- Public CPU, architecture-specific SIMD, and GPU route controls, plus
  optional execution receipts that show the route actually used. Pillow may
  use native SIMD internally; the difference is that pillow-rs exposes route
  controls and a built-in GPU compute path.
- A published Node.js and browser WebAssembly package.
- An opt-in Rayon-backed **Parallel CPU** profile with a matching companion
  Python distribution, installable through `pillow-rs[parallel]` in 12.2.1.

**Contributor features added around the implementation:**

- A generated contract for the selected API surface, arguments, parity inputs,
  and benchmark workloads.
- An isolated Pillow oracle and differential parity runner with separate
  CPU, SIMD, GPU, and Parallel CPU lanes where applicable.
- Separate individual-operation and composed-pipeline benchmark reports,
  including mode and backend details.
- GPU shader-dispatch evidence that distinguishes an actual kernel run from a
  requested GPU route that fell back.

These additions do not make Pillow-style image operations unique to pillow-rs.
The Python `PIL` package delegates image work to the Rust core; Pillow is the
behavior reference, not a runtime backend. The availability column describes
12.2.1, with any remaining platform or operation limits called out explicitly.
Older registry artifacts are recorded in the
[registry release matrix](REGISTRY_RELEASE_MATRIX.md). Check
[installation](INSTALLATION.md) and the [release notes](../CHANGELOG.md) for
package details.

| Addition | What pillow-rs adds | Difference from Pillow 12.3.0 | Availability and limits |
| --- | --- | --- | --- |
| Rust image, font, and codec runtime | The image engine is the [`pillow-rs` Rust crate](RUST.md). Font parsing and rasterization use [`fontdone`](https://github.com/appunni-m/fontdone/tree/7c48b706482b8800e97b35cccc18671893c34e9f); supported image format parsing and encoding use [`image-slash-star`](https://github.com/appunni-m/image-slash-star/tree/70190214a0711223302c76ab58e76c097288d80b). Runtime image/font/codec work does not call Pillow, FreeType, or native codec libraries. | Pillow provides image, font, and codec functionality through its own implementation, but does not provide this Rust engine or a Rust crate API for its `PIL` surface. This is an implementation and integration difference, not a claim of additional Pillow-style image operations. | The 12.2.1 Rust crate is published; supported operations, fonts, and formats remain a selected subset. Its lockfile pins fontdone 2.14.3 and image-slash-star 0.1.2 at the revisions linked here. |
| Rust codec build selection | Rust consumers can disable default features and select codec features such as `image-jpeg`, `image-png`, `image-webp`, or `image-avif`; `image-codecs-all` enables the full codec set exposed through `image-slash-star`. | Pillow also supports optional codecs and reports installed codec support. The difference is the Cargo feature interface for Rust dependency builds, not exclusive format support or new image semantics. | Rust crate build configuration only. The core crate enables GPU and all supported codecs by default; selecting a smaller codec set requires disabling defaults and choosing the desired features. Python and npm package build configurations remain separately defined. See [`pillow-rs/Cargo.toml`](../pillow-rs/Cargo.toml) and the [Rust guide](RUST.md). |
| Deferred image-operation pipelines | The core `Image` handle can record supported operations as a chain and execute them when pixels are needed. Rust callers can force this with `Image::materialize()`, `Image::encode()`, or `Image::tobytes()`; Python image operations use the same deferred core where they map to a pipeline operation. Compatible sibling chains can reuse an already materialized, mode-preserving, non-palette prefix. | Pillow lazily decodes file-backed images after `Image.open()`, but methods that create a new image load the source pixels; for example, `resize()` returns a resized copy. Pillow does not expose an equivalent deferred operation-chain API or shared-prefix execution. See the [Pillow file lifecycle](https://pillow.readthedocs.io/en/12.3.0/reference/open_files.html) and [Image API](https://pillow.readthedocs.io/en/12.3.0/reference/Image.html). | Included in 12.2.1 for operations represented by its pipeline. Not every operation is deferred; validation, unresolved metadata queries, palette transitions, mode changes, or unsupported paths can materialize earlier or retain full-chain evaluation. Deferred errors may therefore surface at a later materialization boundary. |
| Explicit GPU batch scheduling | `GpuBatchExecutor` accepts existing lazy image graphs and returns identity-tagged, bounded streaming results. The earlier ImageBatch prototype was never part of the public API. | Pillow has no equivalent first-party queued executor. | Included in 12.2.1. Its admission contexts and measurements are documented separately; prototype measurements are historical and separate from current executor evidence. |
| Runtime backend controls | Python exports `PIL.available_backends()`, `active_backends()`, `backend_enabled()`, `enable_backend()`, and `disable_backend()`; Rust and JavaScript/WebAssembly expose corresponding controls. Rust also lets a caller attach a backend request to an `Image`. SIMD means architecture-specific vector implementations for supported operations; the serial CPU route is distinct from the optional Rayon Parallel CPU profile. | Pillow may use SIMD internally and [`PIL.features`](https://pillow.readthedocs.io/en/12.3.0/reference/features.html) reports compiled features and codecs, but Pillow has no equivalent public controls for enabling or disabling image-compute routes. | Included in 12.2.1. `available_backends()` lists compiled implementations, not device readiness. Enabling a route changes automatic routing; unsupported operations can still fall back. Python controls routes globally; it does not expose Rust's per-image backend request. |
| GPU compute | The Rust core runs registered image-operation kernels with `wgpu`; the `gpu` Cargo feature is enabled by default and is inherited by the standard Python build. | Pillow 12.3.0 has no built-in GPU image-compute backend in its first-party package. | Included in 12.2.1 for supported Rust and Python builds. Device and operation support vary. The standard JavaScript/WebAssembly build disables core defaults and does not include GPU. A GPU request does not prove that a kernel ran; inspect the actual backend and dispatch evidence. See [compatibility limits](#partial-or-unsupported). |
| Optional Parallel CPU profile | A default-off Cargo feature and companion Python distribution enable Rayon-backed CPU execution for supported work. pillow-rs reports this as **Parallel CPU**, separately from serial CPU, SIMD, and GPU. | Pillow has no equivalent pillow-rs build profile and runtime selector. This is a profile comparison, not a claim that Pillow or its codecs never use threads internally. | The matching `pillow-rs-parallel` 12.2.1 distribution is published for the supported wheel platforms. Install with `pip install 'pillow-rs[parallel]==12.2.1'`; see [Python installation](INSTALLATION.md#python) and the [registry release matrix](REGISTRY_RELEASE_MATRIX.md). Rayon is used only for CPU work; it is not part of SIMD or GPU execution. |
| Node.js and browser WebAssembly | The `pillow-rs` npm package exposes image operations to Node.js and browsers through WebAssembly. | Pillow has no first-party JavaScript or WebAssembly binding. | The npm package is published as 12.2.1; older registry artifacts are tracked in the [registry release matrix](REGISTRY_RELEASE_MATRIX.md). JavaScript has its own method names; callers must release WASM-owned objects, and browser applications need a bundler or module server that serves the WASM asset. See the [JavaScript guide](../pillow-rs-js/README.md). |
| Pipeline execution telemetry | Rust and JavaScript/WebAssembly can opt in to a receipt for the latest completed pipeline: requested and actual backend, per-operation paths, route/validation/backend timings, fallback reason, and available GPU dispatch/resource counters. Python exposes the hooks only through its internal binding. | Pillow's public image API has no equivalent per-pipeline backend receipt. | Included in 12.2.1. This is diagnostic data, not an image-processing guarantee or whole-process profiler. Python helpers are in `pillow_rs._core`, not the public `PIL` namespace. Receipts retain only the latest sample on the executing thread. |

### Parity and performance tooling in this repository

The repository also adds contributor tooling around the migration. It runs
alongside Pillow as a reference implementation; it is not a Pillow runtime
feature and does not imply complete API or mode coverage. Pillow maintains its
own [test suite](https://github.com/python-pillow/Pillow/tree/12.3.0/Tests) and
documents its own performance work in the
[12.3.0 release notes](https://pillow.readthedocs.io/en/12.3.0/releasenotes/12.3.0.html).
The addition here is the migration-specific comparison against a pinned Pillow
oracle, with results separated by operation, mode, and execution backend.

| Tooling | What it does | Scope |
| --- | --- | --- |
| Selected public API contract | A generated manifest records the Pillow endpoints, arguments, requirements, parity inputs, and benchmark workloads selected for this implementation. The [generated contract](generated/migration-parity-public-contract.md) shows the current declared inventory. | The contract is an explicit subset. Its operation and case counts describe the checked-in manifest, not all of Pillow. |
| Differential parity runner | Runs the pinned Pillow oracle and pillow-rs target in isolated processes, then compares results and errors for declared inputs. CPU, SIMD, GPU, and the separately built Parallel CPU profile have distinct lanes where applicable. | A passing lane proves only the selected inputs and target profile. GPU parity must also confirm an actual GPU dispatch; a request that falls back to CPU is not GPU evidence. See [coverage evidence](COVERAGE.md). |
| Operation and pipeline benchmark reports | Publishes separate individual-operation and composed-pipeline tables. Readers can filter and sort measured rows by operation, mode, workload, and backend. Parallel CPU is reported separately and reuses the matching ordinary Pillow measurement as its baseline; the workflow does not run a separate threaded Pillow baseline. | Pillow also publishes its own benchmarks, but not this repository's migration-specific, backend- and mode-aware comparisons. Results are tied to their source revision, host, workload, and measurement policy; they are not speed guarantees or proof that every operation/mode/backend combination is measured. See [benchmark methodology](BENCHMARKING.md) and [benchmark results](https://appunni-m.github.io/pillow-rs/benchmarks/). |
| GPU dispatch evidence collector | Internal Python binding hooks can record which embedded WGSL shader variants actually dispatched, along with dispatch and workgroup counts. | Pillow has no corresponding GPU shader execution evidence because its first-party image API has no built-in GPU compute route. | Contributor diagnostic only, not a public `PIL` API or proof of shader source-line coverage or image parity. Use the parity runner to verify output semantics and backend receipts to verify execution; see [coverage evidence](COVERAGE.md). |

The runtime rows describe system-level additions; they do not mean the image
operations themselves are unique. The default Python build keeps Rayon disabled
but includes the configured SIMD and GPU routes; automatic execution may use
those routes for supported operations. “Serial CPU” therefore describes the
CPU execution profile, not every operation selected by automatic routing.
Pillow-style operations such as
[`ImageOps.cover`](https://github.com/python-pillow/Pillow/blob/12.3.0/src/PIL/ImageOps.py)
and `ImageFilter.Color3DLUT` already exist in Pillow, so implementing them here
is compatibility work rather than a new feature. Pillow also documents native
C performance improvements in its [12.3.0 release notes](https://pillow.readthedocs.io/en/12.3.0/releasenotes/12.3.0.html),
so optimized native code is not unique to pillow-rs. The addition is explicit
backend selection and reporting, not SIMD as a general technique. Processing an
image in its native mode is an implementation choice, not evidence that Pillow
lacks mode-aware paths. None of the additions guarantees a speedup; compare the
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
| GPU acceleration | Build- and operation-dependent. The core and standard Python configuration enable the GPU feature by default; the standard WebAssembly build omits it. A compiled GPU route still depends on a usable device and per-operation support, and a request can fall back to CPU. |
| Release status | Version 12.2.1 is a stable, non-prerelease release. Its supported API remains the selected subset described in this guide; this does not imply complete Pillow compatibility. |

The 12.2.1 release pins image-slash-star 0.1.2 at
[revision 7019021](https://github.com/appunni-m/image-slash-star/tree/70190214a0711223302c76ab58e76c097288d80b)
and fontdone 2.14.3 at
[revision 7c48b70](https://github.com/appunni-m/fontdone/tree/7c48b706482b8800e97b35cccc18671893c34e9f).
The release source and dependency revisions are recorded in its lockfile.
Dependency project documentation may describe features that a particular
pillow-rs release does not include; use the dependency revision matching the
package or checkout you are using. Older registry artifacts and release checks
are documented in the [registry release matrix](REGISTRY_RELEASE_MATRIX.md).

## Before replacing Pillow

Try the operations, images, fonts, frame metadata, and expected errors your
application relies on in separate Pillow and pillow-rs environments.
[Migration steps](PYTHON.md#evaluate-an-existing-pillow-application) explain how.
For measured comparisons and open implementation work, see the contributor
[coverage evidence](COVERAGE.md) and [roadmap](PIPELINE_ROADMAP.md).
