# Rust comparison: pillow-rs and image-rs

<!-- release:summary -->
**Latest release: [12.2.3](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.3).**
<!-- /release:summary -->

Updated 2026-10-25. This page compares the `pillow-rs` 12.2.3 Rust crate
with the `image` 0.25.10 crate from the [image-rs project](https://github.com/image-rs/image).
The projects overlap on raster operations, but have different API goals and
should not be treated as drop-in replacements.

## Feature comparison

| Area | pillow-rs | image-rs (`image` 0.25.10) |
| --- | --- | --- |
| Main goal | Rust core for Pillow-compatible image behavior, also used by the Python `PIL` and JavaScript/WASM packages. Compatibility is operation- and mode-specific; see the [compatibility guide](COMPATIBILITY.md). | Rust library for common image decoding, encoding, and image processing. It does not aim to implement Pillow's Python API. See the [crate documentation](https://docs.rs/image/0.25.10/image/). |
| Image model | `pillow_rs::Image` carries Pillow mode names, metadata, palette state, encoded-byte input, and lazy operation pipelines. Callers can also use the crate's typed raster buffers. | `ImageBuffer<P, Container>` stores a compile-time pixel type; `DynamicImage` is an enum for common channel and sample combinations. See [`ImageBuffer`](https://docs.rs/image/0.25.10/image/struct.ImageBuffer.html) and [`DynamicImage`](https://docs.rs/image/0.25.10/image/enum.DynamicImage.html). |
| Modes and samples | Pillow-style modes include `1`, `L`, `LA`, `P`, `PA`, `RGB`, `RGBA`, `CMYK`, `HSV`, `LAB`, and integer/float modes. Support depends on the operation and backend. | `DynamicImage` covers common Luma, Luma-alpha, RGB, and RGBA buffers at 8 and 16 bits, plus RGB and RGBA 32-bit floats. It does not model Pillow's palette, CMYK, or named mode semantics as equivalent `DynamicImage` variants. |
| Operation surface | Pillow-style image operations, filters, drawing, font rendering, palette handling, statistics, and compatibility-oriented argument/error behavior are exposed from the Rust core. | Typed raster operations such as crop, flips, rotations, resize, and filters are available through [`imageops`](https://docs.rs/image/0.25.10/image/imageops/), alongside format I/O. |
| File formats and I/O | The core accepts byte buffers. Codec support is selected through `image-slash-star` features; filesystem and host-object I/O belongs to the Python and JavaScript bindings. | Readers and writers are part of the Rust crate. Format features are independently selectable; in 0.25.10 the default set includes common formats and the `rayon` feature. See the [feature list](https://docs.rs/crate/image/0.25.10/features). |
| Execution choices | Named CPU, SIMD, and GPU paths are selected per operation where supported. The `parallel` feature is a separate, opt-in Parallel CPU profile; it is not SIMD. | The crate exposes a `rayon` feature, enabled by its default feature set in 0.25.10. It does not expose pillow-rs-style named CPU/SIMD/GPU backend selection. |
| Host integrations | The same core powers the public Python `PIL` namespace and the JavaScript/WASM package. | A Rust crate API; application-specific Python or JavaScript bindings are outside its scope. |

The comparison describes the pinned releases above, not an exhaustive API
matrix. For projects that need Pillow's modes and behavior, check the exact
operation and mode in the compatibility guide. For projects that need typed
Rust pixel buffers and image codecs, compare the `image` crate's enabled
features with the application's format and deployment needs.

## Rust CPU benchmark against image-rs

This is a native Rust comparison, separate from the Python-versus-Pillow and
Pillow-SIMD results in the [benchmark guide](BENCHMARKING.md). It compares
three operations whose output bytes matched exactly before timing. Image setup
is outside the timed region; each output is materialized into an owned image.

| Workload | pillow-rs CPU median (µs) | image-rs median (µs) | image-rs / pillow-rs |
| --- | ---: | ---: | ---: |
| RGB crop, 1024×768 → 832×672 | 200–342 | 401–692 | 1.88–2.03× |
| RGB horizontal flip, 1024×768 | 571–932 | 837–1,646 | 1.44–1.77× |
| RGB 90° counter-clockwise rotation, 1024×768 | 663–750 | 901–954 | 1.27–1.39× |

These ranges are the medians from three separate process runs on an Apple M3 Pro,
macOS 15.7.7, with rustc 1.96.1. Each process warmed each implementation four
times, then took 21 alternating samples of 20 calls per implementation. The
table gives the range of the two process medians, not a confidence interval.
Absolute timings moved between runs, while pillow-rs was faster in all three
paired comparisons. A ratio above 1 means the image-rs time was higher.

The benchmark uses a fixed patterned RGB buffer and the direct Rust APIs. It
builds pillow-rs with `--no-default-features`, locks each operation to
`Backend::Cpu`, and pins image-rs to 0.25.10 with default features disabled.
This excludes Python, codecs, GPU, Rayon, and input construction. The harness
checks byte equality before it records timings and refuses to compare a
workload whose outputs differ. Its current coverage is intentionally narrow;
these measurements do not predict performance for other operations, modes,
image sizes, architectures, or feature sets. Reproduction steps are in the
[contributor benchmark guide](../CONTRIBUTING.md#performance-work).

## Choosing between them

Choose pillow-rs when the application needs Pillow-compatible behavior through
Rust, or wants to share the same core across its Python or WebAssembly-facing
interfaces. Choose image-rs when its typed pixel model, codec features, and
Rust-native operation surface fit the application. If speed is the deciding
factor, benchmark the same operation, mode, dimensions, output contract, and
execution profile on the deployment target.
