# Rust comparison: pillow-rs and image-rs

<!-- release:summary -->
**Latest release: [12.2.3](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.3).**
<!-- /release:summary -->

Updated 2026-10-26. This page compares the `pillow-rs` 12.2.3 Rust crate
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

## Rust CPU and SIMD benchmarks against image-rs

This is a native Rust comparison, separate from the Python-versus-Pillow and
Pillow-SIMD results in the [benchmark guide](BENCHMARKING.md). It compares
four operations using three subjects: pillow-rs locked to serial CPU, pillow-rs
locked to SIMD, and `image` with default features disabled. Each subject's
output bytes must match exactly before timing. Input construction is outside
the timed region; output allocation and materialization remain inside it.

| Workload | pillow-rs CPU median (µs) | pillow-rs SIMD median (µs) | SIMD proof | image-rs median (µs) | image-rs / CPU | image-rs / SIMD |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| RGB crop, 1024×768 → 832×672 | 167.88–184.12 | — | `native-copy`, 0 vector blocks; timing withheld | 379.57–388.46 | 2.11–2.28× | — |
| RGB horizontal flip, 1024×768 | 535.02–540.13 | 258.39–267.92 | `native-copy`, 147,456 vector blocks | 796.52–811.97 | 1.49–1.50× | 3.01–3.08× |
| RGB 90° counter-clockwise rotation, 1024×768 | 581.15–603.05 | 371.99–417.43 | `native-copy`, 196,608 vector blocks | 861.04–872.87 | 1.45–1.48× | 2.09–2.31× |
| RGB 180° rotation, 1024×768 | 437.34–479.62 | 257.70–276.72 | `native-copy`, 147,456 vector blocks | 749.12–774.36 | 1.61–1.71× | 2.80–2.91× |

These ranges are medians from three separate process runs on an Apple M3 Pro,
macOS 15.7.7, with rustc 1.96.1. Each process used four warmups, then 21
alternating samples of 20 calls per measured subject. Ratios are calculated from
paired medians within each process; the ranges are not confidence intervals.
Absolute timings varied between runs. Ratios above 1 mean image-rs took longer.

The benchmark uses a fixed patterned RGB buffer and the direct Rust APIs. It
builds pillow-rs with `--no-default-features` and pins image-rs to 0.25.10 with
default features disabled. This excludes Python, codecs, GPU, Rayon, and input
construction. Before timing, the harness checks exact byte equality and an
untimed execution receipt for each pillow-rs backend. SIMD timings require the
requested and actual backend to be SIMD, no fallback, and positive vector-block
evidence for the operation. Crop runs under the SIMD backend but records a
`native-copy` path with zero vector blocks, so its SIMD latency is withheld.
Telemetry is disabled during timed samples. The measured SIMD rows use
`native-copy` kernels with positive vector-block counts. The coverage is
intentionally narrow; these measurements do not predict performance for other
operations, modes, image sizes, architectures, or feature sets. Reproduce with
the command in the [contributor performance guide](../CONTRIBUTING.md#performance-work).

## Choosing between them

Choose pillow-rs when the application needs Pillow-compatible behavior through
Rust, or wants to share the same core across its Python or WebAssembly-facing
interfaces. Choose image-rs when its typed pixel model, codec features, and
Rust-native operation surface fit the application. If speed is the deciding
factor, benchmark the same operation, mode, dimensions, output contract, and
execution profile on the deployment target.
