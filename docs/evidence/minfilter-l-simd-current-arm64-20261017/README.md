# MinFilter L size-3 current-source SIMD baseline — 2026-10-17

## Result

The current ARM64 build does not reproduce the published MinFilter SIMD gap on
the selected 1024×768 L/noise workload. The full public-call boundary was
`apply-filter;observe-filter-result`, warm cache, 5 warmups, 20 iterations × 5
samples, concurrency 1. It includes filter execution and terminal result
observation.

| Subject | Median | p95 | Samples |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 32.106792 ms | 33.516125 ms | 100 |
| pillow-rs SIMD | 4.676334 ms | 4.793584 ms | 100 |

The SIMD/Pillow median speedup is 6.865804×. The receipt reports
`requested=actual=simd`, terminal completion, no fallbacks, one MinFilter
operation, vector path, 98,304 16-pixel vector blocks, and zero scalar-tail
pixels. The benchmark's main parity preflight passed 1/1. Strict SIMD parity
also passed the selected 1×1 and 33×35 L edge cases (2/2).

This is evidence for this workload, mode, size, source, and runner. It does not
close the MinFilter operation matrix: LA/RGB and other sizes, other filter
sizes, Parallel CPU, Pillow-SIMD on a matched x86 runner, GPU latency and
sustained throughput remain unmeasured here. No SIMD kernel change was tried;
the current-source admission gate already exceeds 5× versus Pillow.

## Identity and selectors

- Workload: `pipeline-op.minfilter.material-l-noise-1024x768`
- Main parity case: `PIL.ImageFilter.MinFilter.nuanced.performance-material-l-noise-1024x768-size-3`
- Edge parity cases: `PIL.ImageFilter.MinFilter.nuanced.backend-noise-l-1x1-size-3`; `PIL.ImageFilter.MinFilter.nuanced.backend-noise-l-33x35-size-3`
- Benchmark run: `migration-benchmark-c27e9eab53f74869b14c6b943a937581`
- Main parity run: `migration-parity-benchmark-gate-8c9f86530f284e2790188e71d046197f`
- Edge parity run: `migration-parity-1bf2a165c72547db9285e2fe59e10d01`
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty worktree)
- Loaded target core SHA-256: `af985e7b98977920ed0ec21ba59092b9ee7e4b7e0d4a241cab0703ec9dd874bb`
- SIMD adapter SHA-256: `96dcd31d15a047b09fe693d30407feada64be083de2186185e43a4eff5f19070`
- Native filter kernel SHA-256: `7858618ddc40e4e89bee5ecf911d4d603a66b2f7a2b38c0b4728ca7df098fa37`
- Benchmark fixture SHA-256: `5518e26b243dc3708095e09b8998cc6f5976f6c6cbc1b047529c5dc3d2e46350`
- Parity fixture SHA-256: `bf48f3b32721bb955075668f42afd6c7b94538828547773d282e63ca6da2b9e2`
- Main noise asset SHA-256: `42cbcbd0a9898a06c301a48137fd39668a3aa69aa2963a99a0c19eb2b21a6629`
- Host: macOS 15.7.7 arm64, CPython 3.12.13; Pillow 12.2.0
- Cargo profile: release defaults; `parallel` feature disabled

The clean 2026-10-13 snapshot's MinFilter L SIMD row was 4.218428× Pillow but
did not include positive vector-work evidence. This current dirty-source run is
separate, matched to its loaded core, and demonstrates actual vector execution
for the selected case. It is not a matched Pillow-SIMD comparison.

## Artifacts

- `A1-benchmark.json` and `A1-benchmark.raw-samples.json`
- `A1-benchmark.attestation.json` and its child receipts
- `A1-benchmark.producer.json`
- `A1-parity.json` and `A1-parity.attestation.json`
- `edge-parity.json`, its attestation, and producer receipt
- Two Pillow/SIMD rows appended to `docs/evidence/performance-optimization-local-runs.csv`

No source code, tests, fixtures, or thresholds changed. No coverage, commit, or
push ran; the operation batch remains 0/33.
