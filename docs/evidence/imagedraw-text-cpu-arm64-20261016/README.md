# ImageDraw.text CPU baseline — 2026-10-16

Dirty-tree diagnostic baseline for the two declared 1024×768 RGB workloads. This is not an accepted optimization: the benchmark report marks Rust execution `not_proven` with `actual_backend: null`.

## Measurements

| Workload | Pillow median | Rust profile median | Observed ratio |
|---|---:|---:|---:|
| `pil-imagedraw-imagedraw.text.standard` | 772.229 µs | 498.354 µs | 1.5496× |
| `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768` | 752.084 µs | 471.271 µs | 1.5959× |

Each used 5 warmups, 20 iterations × 5 samples (100 timed samples), warm cache, concurrency 1, and `call` + `observe-receiver`. The benchmark parity gate passed. The six CPU parity cases are summarized in `parity-summary.json`; 6/6 passed.

## Backend attribution

The binding calls `PyDraw.text` → `Draw::text_with_optional_font_input` → `text_with_options_inner`, which rasterizes the mask and composites into the host raster. This direct path has no compute-adapter dispatch, but the benchmark emitted no positive backend receipt. Keep the ratios diagnostic until the reporting contract can represent the direct CPU path. SIMD/GPU execution and latency were not measured and must not be inferred from this result.

## Identity and scope

- Pillow 12.2.0; target package 12.2.1; CPython 3.12.13.
- Host: Lazys-MacBook-Pro.local, macOS-15.7.7-arm64-arm-64bit, arm64.
- Source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty worktree); loaded core extension SHA-256 `315a2f3278395a2a2f9ae65221ad2fcbdb2cc463558931c0acd3ca35334ce198`.
- `make build-parity`, six selected CPU parity cases, and two selected CPU benchmark workloads only. No code changes, full suite, full campaign, or coverage.

The full compressed parity report remained at measurement time at `/tmp/imagedraw-text-arm64-20261016-parity.json.gz`; its canonical and container hashes remain in `parity-summary.json` and the sidecar attestation. Output images are not duplicated here.
