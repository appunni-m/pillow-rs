# Changelog

All notable user-facing changes are recorded here. The first package release
is intentionally a compatibility-development release; the parity status and
backend limitations in the documentation remain part of its contract.

## 0.1.2 - 2026-09-15

### Changed

- Publish only through GitHub OIDC after exact main CI, using synchronized
  fontdone alpha.9 and image-slash-star 0.1.1 dependency pins.
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

- The public Python namespace remains `PIL`; `RSPIL` stays a deprecated
  compatibility bridge.
- GPU and browser capability gaps, benchmark budget review, and source
  coverage limitations remain explicit release evidence.

## 0.1.1 - 2026-09-13

### Changed

- Made `PIL` the public Python namespace so `from PIL import Image` is the
  direct replacement for the former `RSPIL` import.
- Kept `pillow_rs` as the internal binding namespace and `RSPIL` as a
  deprecated compatibility bridge.
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
