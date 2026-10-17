# RGBA fused Transpose current-source A1 baseline

This one-workload baseline checks the historical Transpose CPU concern on the current dirty ARM64 source. The older Linux timings were execution-only; this run has a passing source-to-target comparison and current execution receipts. It does not establish an x86 result or complete the Transpose API.

## Frozen workload and identity

- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`.
- Exact parity case: `PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`; RGBA input asset SHA-256 `f9f99c9f98e812616f070e76349a768ff948a3c1c151902e53ee1d366c7a7d2e`.
- Boundary: observed `transpose-0;transpose-1;materialize`; warm release, 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Oracle: Pillow 12.2.0 / CPython 3.12.13. Target: serial CPU, default Cargo features, Parallel CPU disabled.
- Host: macOS-15.7.7-arm64-arm-64bit; architecture `arm64`.
- Benchmark run: `migration-benchmark-c3a769162d2e4694bb3d6c6d90d2a5ed`; exact case gate: `migration-parity-benchmark-gate-81f96f5e1f1946e2b56d04dd2d7a22bb` (1/1 passed).
- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.
- Benchmark input SHA-256: `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`; parity input SHA-256: `c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`.
- Loaded `_core.abi3.so` SHA-256: `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Source hashes: CPU geometry `geometry.rs` `f6b2603ec1c712f2274d7b02c7e162a30d7cc33d63cb50d7b439a108796721d8`; registry `1d37d30579e8a2563c804ae5271e1ca118ade42ca6e4c3720ba45bb0d010690f`; public `ops/transpose.rs` `b88d4cf1c6c18652fc2e88b2fe308892090f400282d8a0430823db0efa2ae959`; Python `image.py` `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`.

## Result and disposition

| Subject | Median | P95 | Pillow / target |
| --- | ---: | ---: | ---: |
| Pillow | 0.966875 ms | 1.843750 ms | 1.000× |
| Serial CPU | 0.461834 ms | 0.771583 ms | 2.094× |

Both Transpose operations reported the actual CPU path, the terminal is complete, and no fallback occurred. The resource receipt reports 2 host buffers / 4,743,168 bytes, one tracked host allocation / 2,371,584 bytes, no mode conversion, and 2 fused operations. On this ARM64 workload, current CPU median is just above the 2× checkpoint, so the historical CPU miss does not reproduce here. The result does not settle x86 performance; defer source changes until a current x86 receipt can resolve that host-specific gap.

## Artifacts

The exact parity report is gzip-compressed. Decompressing `parity.json.gz` reproduces the raw report with SHA-256 `bc44935d5b4dddfcdc842cdf240704aecb3d1b62ba0cff5eae2f8f2d62f17819`. Verify the retained artifacts with:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

No broad campaign, coverage, commit, or push was run.
