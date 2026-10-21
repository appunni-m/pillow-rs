# Grayscale YCbCr shared-gather guard — ARM64 — 2026-10-19

This is a focused regression guard for the GetChannel RGB output-allocation
candidate documented in
[`GetChannel output-allocation trial`](../../getchannel/getchannel-rgb-output-allocation-arm64-20261019/README.md).
The same `gather_channel::<3, 0>` helper is used by Grayscale's YCbCr SIMD
kernel, so this public operation is in the affected execution closure.

- Workload: `pipeline-op.grayscale.material-ycbcr-noise-1024x768`.
- Exact parity case: `PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`.
- Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7 ARM64, CPython 3.12.13,
  Pillow 12.2.0.
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty worktree.
- Build: release, default features, Parallel CPU disabled. The candidate
  extension identity is recorded in the linked GetChannel trial.
- Boundary: warm `call;observe-result`, 5 warmups, 20 iterations × 5 samples,
  concurrency 1.

## Results

| Run | Pillow median | SIMD median | Pillow / SIMD | SIMD p95 | Execution |
| --- | ---: | ---: | ---: | ---: | --- |
| Existing control | 118.646 µs | 50.146 µs | 2.366× | 56.709 µs | `neon-deinterleave`, 49,152 blocks |
| Candidate guard 1 | 140.855 µs | 53.042 µs | 2.656× | 154.416 µs | `neon-deinterleave`, 49,152 blocks |
| Candidate guard 2 | 129.646 µs | 51.312 µs | 2.527× | 146.791 µs | `neon-deinterleave`, 49,152 blocks |

Strict CPU and strict SIMD parity each passed 1/1. Both candidate benchmark
preflights also passed. The SIMD receipt shows the requested backend executed,
no fallback, terminal completion, one mode conversion, and one 786,432-byte
output allocation.

Candidate medians were 2.3–5.8% above the earlier control, while the matched
Pillow medians were 9–19% higher and candidate p95 values were much noisier.
This does not establish either a repeatable regression or a gain. The guard
remains inconclusive; do not promote a baseline from it. A source/build-matched
A–B–B–A run is required before accepting the shared-helper change.

The reports, raw samples, parity results, and execution attestations in this
directory retain both candidate repeats. The two benchmark-result observations
were imported into the local PerfDB; no baseline was changed.
