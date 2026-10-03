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

pillow-rs targets Pillow 12.2.0 behavior where an API is implemented, and also
adds the following first-party interfaces and execution options. This list
describes the current repository source; a published alpha may predate a main
branch addition. Check [installation](INSTALLATION.md) and the
[release notes](../CHANGELOG.md) for package availability. The comparison is
against the upstream [Pillow 12.2.0 source](https://github.com/python-pillow/Pillow/tree/12.2.0/src/PIL),
not third-party plugins or application code.

| Addition | What pillow-rs provides | Availability and limits |
| --- | --- | --- |
| Explicit multi-image queue | `PIL.ImageBatch.BatchExecutor` accepts independent image jobs through `submit()` and returns results in submission order from `join()`. Compatible jobs can share one GPU operation. | Opt-in API; GPU grouping is operation-, mode-, and size-specific. Current groups cover `MedianFilter(3)`, `ExtractBand`, and `Multiply` for native `L`, `LA`, `RGB`, and `RGBA`; shared `Color3DLUT` is RGBA-only. See the [batching guide](IMAGE_BATCHING.md). |
| Backend controls | Python exposes `PIL.available_backends()`, `PIL.active_backends()`, `PIL.backend_enabled()`, `PIL.enable_backend()`, and `PIL.disable_backend()` for the `cpu`, `simd`, and `gpu` routes. | Availability depends on the build and host. Backend eligibility does not mean every operation has a kernel there; unsupported work can fall back to CPU. |
| GPU compute | The Rust core includes a `wgpu` backend behind its `gpu` Cargo feature, which is enabled by the core crate's default feature set. Python builds can use GPU kernels for operations that register support. | Operation- and device-dependent. A GPU request is not proof that the operation ran on the GPU; check the [compatibility limits](#partial-or-unsupported) and measure the backend that actually ran. |
| Parallel CPU profile | A separate, opt-in Rayon-backed CPU build processes supported work in parallel. It is reported as **Parallel CPU**, separately from serial CPU, SIMD, and GPU. | The Cargo feature is default-off. The repository configures the `pillow-rs[parallel]` companion wheel, but the published `12.2.0-alpha.5` package predates that extra; see [installation](INSTALLATION.md#python) for the current package status. |
| Rust and JavaScript/WebAssembly integrations | The same Rust image core is available as the `pillow-rs` Rust crate and through an npm package for Node.js and browsers using WebAssembly. | These are first-party pillow-rs interfaces, not Pillow's Python API. JavaScript uses JavaScript method names, and browser use requires a WebAssembly-capable bundler or module server; see [Rust](RUST.md) and [JavaScript](../pillow-rs-js/README.md). |

These additions extend pillow-rs; they do not imply that every Pillow operation
is supported. In particular, SIMD is not claimed as unique to pillow-rs—Pillow
and its codec dependencies also use optimized native code. Pillow's
`ImageFilter.Color3DLUT` and ordinary mode-aware image operations are compatible
features; the extra is the `ImageBatch` wrapper that can reuse a shared LUT for
compatible queued work. Processing native image modes directly is an
implementation choice in pillow-rs, not a claim that Pillow lacks mode-specific
processing.

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
