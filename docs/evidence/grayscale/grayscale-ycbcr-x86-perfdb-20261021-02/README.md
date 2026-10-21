# YCbCr Grayscale x86 PerfDB baseline — 2026-10-21

This is the source/build-bound selected x86_64 cohort for `ImageOps.grayscale`
on a materialized 1024×768 YCbCr noise image. Pillow is compared from this same
report. Timing covers the public call and `observe-result` boundary for 100
samples, five warmups, 20 iterations per sample, warm cache, concurrency one.

## Results

| Profile | Median / p95 (µs) | Throughput (ops/s) | Pillow / target | Baseline policy | Executed backend |
| --- | ---: | ---: | ---: | --- | --- |
| pillow | 1192.824 / 1215.184 | 838.346 | 1.000× | oracle | pillow |
| python-cpu | 859.773 / 878.374 | 1163.098 | 1.387× | strict | cpu |
| python-simd | 854.003 / 877.213 | 1170.955 | 1.397× | tracking | simd |
| python-gpu | 1115.104 / 1238.104 | 896.778 | 1.070× | tracking | gpu |

CPU clears the serial-CPU target for this row. SIMD is only 1.397× faster
than Pillow, below the 5× target. GPU is 1.070× faster than Pillow, but its
median latency is 1.306× SIMD latency and its measured throughput is 896.778
ops/s versus SIMD at 1,170.955 ops/s. SIMD and GPU therefore remain tracking
channels. The GPU receipt confirms GPU execution and terminal completion but
does not name the selected physical adapter.

## Evidence identity

- Benchmark run: `{now['run_id']}` (`{now['started_at']}` to `{now['finished_at']}`).
- Workload: `pipeline-op.grayscale.material-ycbcr-noise-1024x768`.
- Parity case: `PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`.
- Selected parity passed for all three profiles (3/3); every target recorded its
  requested backend as actual, completed materialization, and had no fallback.
- Source revision: `{target['revision']}` with a dirty source snapshot. Loaded
  `_core.abi3.so` SHA-256:
  `7b7433a32cd1a4c49dd4019d328b673f5cf960c16a6cef2133a6123ce118a066`.
- Manifest SHA-256: `{now['manifest']['sha256']}`. Benchmark report SHA-256:
  `592b7ec79d9f753f8eba7a5df3fca2c6530294392af89f8f9d81b0b81a9eb79f`.
  Parity report SHA-256:
  `27252d9a2f692afd5f2365507dff8ce8977618b508fb8ca5467bfe75907f15f1`.
- PerfDB channels: `grayscale-ycbcr-x86-20261021`; CPU strict baseline
  `62520383b315`, SIMD tracking baseline `3cdc3979b2b5`, GPU tracking baseline
  `33c9e7e1a454`. DuckDB remains generated local state.

## Findings

The manifest generator marked the Grayscale performance requirement as CPU-only
while the selected workload and parity case include CPU, SIMD, and GPU. I fixed
profile ownership in `scripts/build_migration_parity_manifest.py`; the PerfDB
validator stayed strict and no workloads, parity assertions, or thresholds
were changed. The regenerated manifest admitted all three backends.

PerfDB also falsely rejected higher throughput during baseline promotion because
its promotion guard compared throughput in the latency direction. The guard
now rejects lower throughput and accepts higher throughput. Focused Rust tests
passed:

- `cargo test baseline_promotion_` — 2 passed.
- `cargo test gpu_throughput_regression_is_caught_even_when_latency_improves` — 1 passed.

Only this workload and its three parity profiles ran. No full benchmark or
parity campaign, coverage, commit, or push was performed.

## Retained files

Selected-run packages live under `selected-run/` and include the report, parity,
manifest, source/build witness, and hashed input files required to reconstruct
the three active channels. `SHA256SUMS` covers every file in this directory.
