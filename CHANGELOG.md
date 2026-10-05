# Changelog

All notable user-facing changes are recorded here. Stable release scope,
parity status, and backend limitations are described in the documentation.

## 12.2.0 - 2026-10-05

- Add exact rolling-row CPU and vectorized SIMD paths for uniform native-L
  5x5 filtering, with byte-rounding, border, tail, and Pillow-parity checks.
- Pin stable fontdone 2.14.3, including the fix for charmap metadata on reused
  faces.

### 12.2.0-alpha.6 candidate

Prepared on 2026-10-05. Source parity and exact-main CI passed, but the
release is not accepted: publication was partial, and the published Rust crate
resolves an older fontdone implementation that fails font-error and thread
teardown checks. See the [release evidence](docs/REGISTRY_RELEASE_MATRIX.md#alpha6-partial-publication-2026-10-05).

- Add explicit GPU pipeline batching with bounded admission and streaming CPU
  or resident GPU results, stable job IDs and caller keys, and immediate
  rejection of unsupported pending contexts. Reuse existing lazy image graphs;
  document cancellation, ownership, limits, and measured throughput.
- Archive the experimental ImageBatch API and its implementation under
  `deprecated/imagebatch`; remove its runtime exports.
- Add native-mode composed GPU execution and retain derived-image metadata.
- Read native packed L samples correctly in GPU geometry-table transforms.
- Keep SIMD `I`/`F` sample widening separate from L/LA-to-RGBA expansion;
  restore all 19 typed-conversion parity cases without changing their inputs.
- Restore generator support for existing font variation style and reused-face
  observations without removing any committed parity input.
- Optimize native LA/L filters, byte putdata, RGBA transpose, RGB merge,
  composite, conversions, and terminal reads while preserving the covered
  Pillow contracts. Per-operation performance gaps remain documented.
- Prepare the opt-in Parallel CPU companion wheel through `pillow-rs[parallel]`;
  keep Rayon disabled in the standard wheel.
- Correct the documentation dashboard's stale workload-count assertions to
  the existing catalog of 657 workloads: 195 operations and 462 pipelines.
- Gate future Rust crate uploads on a public font consumer of the normalized
  archive, including malformed-font errors and cached-face thread teardown.
  This guard is a recovery change after the alpha.6 tag, not part of its artifacts.

## 12.2.0-alpha.5 - 2026-10-01

- Preserve zero-valued fourth samples for omitted and scalar CMYK/RGBX
  `ImageOps.expand` fills in the Node and browser WASM parity adapter.
- Add native CPU and SIMD CMYK masked-paste paths and regression coverage.
- Keep Rayon opt-in and report its correctness-gated benchmarks in a distinct
  Parallel CPU profile.

## 12.2.0-alpha.4 - 2026-10-01

- Avoid intermediate copies in native `I`/`F` to `RGB` conversion and reuse
  native RGB samples for HSV conversion while preserving Pillow output bytes.

## 12.2.0-alpha.3 - 2026-10-01

- Preserve native fill-channel order in `ImageOps.expand` for formats such as
  CMYK and RGBX across the Python and Node/browser WASM parity adapters.
- Add native-mode expand parity cases and fix the mismatches that blocked the
  post-alpha.2 main CI run.

## 12.2.0-alpha.2 - 2026-09-30

- Add mode-specific CPU, SIMD, and GPU paths across image operations, keeping
  native samples for formats such as L, LA, RGB, CMYK, RGBX, and RGBA instead
  of routing those operations through RGBA intermediates.
- Optimize paste, crop, expand, pad, filters, effects, and channel operations;
  refine transpose, flips, and right-angle rotations while retaining Pillow
  parity on the covered workloads.
- Pin fontdone 2.14.3-alpha.12 to include reused-face charmap metadata fixes.
- Preserve native CMYK and RGBX fill bytes in the Node/browser WASM parity
  adapter for `ImageOps.expand`.
- Keep Rayon `parallel` default-off and expose it explicitly through the Python
  binding; keep SIMD row scheduling and GPU result readback serial even in
  parallel-enabled builds.
- Add exact integer SIMD filtering for native Sharpness modes and a contiguous
  shuffled lane load for L images, preserving CPU parity across modes and tails.

## 12.2.0-alpha.1 - 2026-09-16

- Align Cargo, npm, Python manifests and runtime, and current documentation on
  one version targeting Pillow 12.2.0. Verify synchronization in CI and handle
  Python artifact normalization and GitHub prerelease status automatically.

- Remove obsolete Rust pixel trait methods (`channels4`, `from_channels`,
  generic `get_pixel_mut`, and `blend_pixel`), `Image::transform_affine`, and
  the deferred `Quantize`, `PointOp`, `LinearGradient`, `RadialGradient`, and
  `EffectMandelbrot` variants. Current integrations start from the [Rust guide](https://appunni-m.github.io/pillow-rs/rust/).
- Route internal LUT fusion through `Eval`; eager quantization, gradient,
  and Mandelbrot constructors remain supported. Keep all existing benchmark operation
  workloads (85 canonical families) and the full public parity inventory.
- Ship Python through `PIL` and its internal `pillow_rs` implementation only.
  Remove the former import bridge; use `from PIL import Image`.
- Use current PyO3 capsule/class conversion APIs and reject deprecated API usage
  in the workspace and CI.
- Route the JavaScript `transform(size, matrix)` convenience method through
  the public affine entry point. Invalid sizes now raise `TypeError`; exposed
  output pixels use Pillow's zero-fill default, including transparent alpha.
- Remove retired test runners, generated oracle outputs, unused shaders, and
  first-release-only tooling. Keep the frozen inventory authority byte-for-byte.
- Refresh pinned GitHub Actions for CI, Pages, and trusted publication.
- Include the complete project license in Cargo and Python archives, use current
  Python license metadata, and verify license contents before release.
- Keep WASM optimization enabled for published npm artifacts; the parity
  runner's faster correctness build remains separate.

## 0.1.3 - 2026-09-16

- Disable automatic package-manager caching with `package-manager-cache: false`
  in the npm publisher. `cache: false` incorrectly selected an unsupported
  package manager and stopped Node setup before authentication in 0.1.2.
- Validate setup-node cache inputs in `make release-tools-test` and the main
  documentation/input CI gate, so this configuration error fails before tagging.
- Keep the successful 0.1.2 Cargo and PyPI uploads and its tag immutable; use
  synchronized 0.1.3 metadata for the corrected three-registry release.
- Preserve the released fontdone alpha.10 and image-slash-star 0.1.2 pins.
  Runtime implementation and parity inputs are unchanged.

## 0.1.2 - 2026-09-15

### Changed

- Publish only through GitHub OIDC after exact main CI, using synchronized
  fontdone alpha.10 and image-slash-star 0.1.2 dependency pins.
- Fix Windows font binding compilation by converting native FreeType integer
  widths at the Rust boundary, with parity inputs for Unicode bearings,
  fractional metrics, and embedded bitmap bounds.
- Build and install-test portable ABI3 wheels for Linux, macOS, and Windows;
  publish the Python source distribution and one shared Node/browser npm package.
- Bind every upload to verified checksums, stage only absent PyPI files, and
  prevent GitHub asset recovery when a required registry publication failed.
- Pin Rust coverage to nightly-2026-07-16 and verify its source-bound receipts.

- Synchronized the Cargo, Python, and WebAssembly package metadata for the
  next patch release.
- Corrected release and parity tooling so installed Pillow remains the source
  oracle, large WASM envelopes stream safely, and current coverage reports are
  accepted without changing measured thresholds.

### Compatibility and release notes

- The public Python namespace remains `PIL`; legacy import compatibility remains deprecated.
- GPU and browser capability gaps, benchmark budget review, and source
  coverage limitations remain explicit release evidence.

## 0.1.1 - 2026-09-13

### Changed

- Made `PIL` the public Python namespace so `from PIL import Image` is the
  documented replacement entry point.
- Kept `pillow_rs` as the internal binding namespace and retained a deprecated
  compatibility bridge.
- Synchronized the Rust, Python, and WebAssembly package versions for the
  release candidate.

### Compatibility and release notes

- The active parity corpus compares the public PIL replacement with Pillow at
  runtime using input-only cases.
- GPU and browser capability gaps remain explicit in generated evidence.
- The release depends on registry versions of `image-slash-star` and
  `fontdone`; publication is ordered after those dependencies are visible.

## 0.1.0

### Added

- Pure-Rust image operations with Python and WASM bindings.
- Manifest-driven Pillow parity, coverage, and correctness-gated benchmark
  workflows across CPU, SIMD, GPU, Node WASM, and browser WASM lanes.
- Locked CI and release checks for crates.io, PyPI, and npm packaging.

### Compatibility and release notes

- The active parity corpus is recorded in
  `docs/benchmark-backend-pending-2026-09-03.md`.
- GPU and browser capability gaps remain explicit in generated evidence.
- The first release depends on registry versions of `image-slash-star` and
  `fontdone`; publication is ordered after those dependencies are visible.
