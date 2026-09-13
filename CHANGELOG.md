# Changelog

All notable user-facing changes are recorded here. The first package release
is intentionally a compatibility-development release; the parity status and
backend limitations in the documentation remain part of its contract.

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
