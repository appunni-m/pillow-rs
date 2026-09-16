# Changelog

All notable user-facing changes are recorded here. The first package release
is intentionally a compatibility-development release; the parity status and
backend limitations in the documentation remain part of its contract.

## 0.2.0 - Unreleased

- Remove obsolete Rust pixel trait methods (`channels4`, `from_channels`,
  generic `get_pixel_mut`, and `blend_pixel`), `Image::transform_affine`, and
  the deferred `Quantize`, `PointOp`, `LinearGradient`, `RadialGradient`, and
  `EffectMandelbrot` variants. See the [migration guide](https://appunni-m.github.io/pillow-rs/rust/#upgrading-to-020).
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
