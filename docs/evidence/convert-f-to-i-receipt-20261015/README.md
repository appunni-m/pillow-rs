# Eager F→I CPU receipt repair — 2026-10-15

The serial-CPU `F`→`I` fast path now wraps the existing `f_to_i` operation in the eager image-operation receipt helper only when telemetry is enabled, the selected active backend is CPU, and the parallel feature is disabled. With telemetry off, it continues to call `f_to_i` directly. The converter, result mode, validation, and pixel output are unchanged. SIMD/GPU and Parallel CPU receive no new execution credit.

## Verification

- Focused default-feature release extension build: `make build-parity PYTHON=.venv/bin/python` (pass).
- Exact parity case `PIL.Image.Image.convert.nuanced.mode-audit-F-to-I-17x3`: 1 selected, 1 passed on CPU. Run `migration-parity-9eddeae6026e46248c60823e69849e5a`.
- Workload `pipeline-op.convert-mode-f-to-i-1024x768`: whole workflow `setup-image;call;materialize`, mode F 1024×768 → I, warm cache, five warmups, one iteration × 100 samples, concurrency 1. Run `migration-benchmark-ad393f0966374e56a05f3676546381f6`.
- Current ARM64 medians: Pillow 1.059375 ms; target CPU 0.432271 ms; target/Pillow speedup 2.4507×. This is a current-tree measurement, not an attributed before/after optimization gain, and it does not update the clean published matrix.
- Target receipt: requested/actual CPU, one operation, terminal complete, no fallback; 2 host buffers / 6,291,456 bytes at the operation boundary, one output allocation / 3,145,728 bytes. The Python `tobytes()` materialization is inside measured latency but outside this core buffer receipt. The logical F→I mode change is not a physical DynamicImage color-type conversion, so the physical conversion count remains zero.
- The benchmark workload gate is `successful_execution`, and its generated parity report selected 0 cases. The separate mode-audit parity above confirms F→I behavior on a varied 17×3 input, but is not the benchmark's constant-color input. No performance claim is inferred from that separate parity.

## Identities

- Source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty); whole tracked diff SHA-256 `782149f5e023567f0f29c7cfd308e3b1e381d468e52b90659b18003bfbc39667`.
- `pillow-rs/src/ops/convert.rs` SHA-256 `56153d55e80ddd21965bbcabfe4ab299ad6ee76f2e7551df8f842b46fbe1efca`.
- Target extension SHA-256 `bb8f7ebdb351d32193666b5bdfcf197a5c5e301258628859aabafbf33f074fe2`.
- Benchmark report SHA-256 `e6eac0fd760922cb385e960c23737a60688b5bad67d676a866fb1b7b89591bc3`; parity report SHA-256 `0a4402286f107bd70923d0af6a18e7120520f326a91a8258a01505d844f3bb7d`.

All reports and execution/producer sidecars for this focused run are retained in this directory. No full campaign, coverage, commit, or push ran.
