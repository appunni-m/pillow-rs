# MedianFilter current GPU/SIMD latency pair — 2026-10-16

This is a source-matched, single-workload latency rebaseline after the L 3×3
SIMD selection-network candidate. It does not include a sustained GPU-batching
measurement and does not update the clean published matrix.

## Frozen case and timing boundary

- Workload: `pipeline-op.medianfilter.material-l-noise-1024x768`.
- Exact parity case: `PIL.ImageFilter.MedianFilter.nuanced.performance-material-l-noise-1024x768-size-3`.
- Profiles: `python-simd` and `python-gpu`; the runner also timed same-run Pillow.
- Boundary: `apply-filter` plus `observe-filter-result`; materialized public call,
  warm release build, five warmups, 20 iterations × five samples, concurrency 1.
- Exact parity completed for both target profiles (2/2 passed).

## Results

| Subject | Median | P95 | Throughput at concurrency 1 |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 42.796563 ms | 45.027917 ms | 23.366 ops/s |
| pillow-rs SIMD | 6.012688 ms | 6.084791 ms | 166.315 ops/s |
| pillow-rs GPU | 1.413146 ms | 1.601250 ms | 707.641 ops/s |

On this dirty ARM64 build, SIMD is 7.118× faster than Pillow and GPU call
latency is 4.255× lower than SIMD (30.285× faster than Pillow). The SIMD
receipt proves `MedianFilter/vector`, 49,152 blocks, zero scalar tail,
terminal completion, and no fallback. The GPU receipt proves requested and
actual GPU, one completed operation and dispatch, zero fallback, and one
786,432-byte upload plus one 786,432-byte readback. The measured boundary
includes that transfer and output materialization.

The report's concurrency-one throughput is the inverse of per-call latency; it
does **not** prove overlap or sustained batched throughput. A separate explicit
GPU-batch measurement is still required before claiming the GPU throughput
target.

## Identity and artifacts

- Run: `migration-benchmark-c0a7eb2c90f64c3a85720e0d5e5db701`.
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty worktree.
- Both target profiles loaded core SHA-256
  `315a2f3278395a2a2f9ae65221ad2fcbdb2cc463558931c0acd3ca35334ce198`.
- Host: macOS 15.7.7 ARM64; Pillow 12.2.0 / CPython 3.12. The benchmark
  report retains manifest/input and asset hashes, all ordered samples, parity
  results, and execution attestations.
- Benchmark and parity result schema validation passed.

The source/build identity differs from the clean 2026-10-13 report. Treat these
measurements as local diagnostic evidence, not a clean-matrix update or a claim
for other hosts, modes, sizes, or workloads. No tests, broad campaigns,
coverage, commit, or push ran.
