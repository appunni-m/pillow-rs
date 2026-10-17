# YCbCr Grayscale paired CPU/SIMD rebaseline — ARM64, 2026-10-17

This focused run measured serial CPU and SIMD in one report against the same
Pillow baseline. It rechecks the retained four-block NEON Y-channel extraction
on the current extension; no implementation code changed for this run.

## Frozen workload and parity

- Workload: `pipeline-op.grayscale.material-ycbcr-noise-1024x768`.
- Main parity case: `PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`.
- Edge guard: `PIL.ImageOps.grayscale.nuanced.performance-ycbcr-odd-byte-tail` (YCbCr 3×1).
- Input asset SHA-256: `8d09ced0ef27917e84162a9bc924b41913252d85adfb87a2464a307c2a8cd110`.
- Boundary: warm public `call;observe-result`, 5 warmups, 20×5 samples,
  concurrency one; input setup is outside timing and result materialization is
  included.
- The benchmark preflight passed the material case for CPU and SIMD (2/2
  comparisons). Separate CPU and SIMD parity runs each passed the odd-tail
  guard (1/1 each). Both benchmark and parity result schemas validated.

## Results

Medians and p95 values are microseconds; ratios use the same report's Pillow
median.

| Profile | Median | P95 | Pillow / target | Execution proof |
| --- | ---: | ---: | ---: | --- |
| Pillow 12.2.0 | 163.480 | 345.458 | 1.000× | Pillow |
| Serial CPU | 52.417 | 187.083 | 3.119× | actual CPU, one Grayscale operation, complete, no fallback |
| SIMD | 52.687 | 158.708 | 3.103× | actual SIMD `neon-deinterleave`, 49,152 vector blocks, zero scalar tail, complete, no fallback |

Both target profiles clear the 2× triage gate on this workload. SIMD remains
below the final 5× goal. The CPU and SIMD medians are close in this one report;
the run does not establish a causal CPU-versus-SIMD regression or gain because
it is not an A–B–B–A experiment. Each target materialized one 786,432-byte
output allocation; peak live host bytes were 3,145,728, and one mode conversion
was recorded. No GPU path or sustained-throughput test ran.

The preceding B3 SIMD report measured 47.334 µs against an 87.521 µs Pillow
median (1.849×), while this report measured 52.687 µs against 163.480 µs
(3.103×). The target medians are of similar scale while the Pillow medians
diverge substantially. Treat each same-report ratio as a workload observation;
do not compare these ratios across runs as evidence that the kernel changed or
that its performance improved. This rebaseline resolves the unmatched CPU/SIMD
comparison but does not close the operation.

## Identity and receipts

- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty worktree).
- Release/default-feature build; `parallel` disabled.
- Loaded extension SHA-256:
  `3896b49bbf7375c727f1b3d65af7b527ba4f08d6ba7c5e8e078913fbd3e619ff`.
- YCbCr SIMD kernel SHA-256:
  `b09e1c0950b0c515dc72f50496d601ecbadd50361631edaa3f0bb5964d925b1e`.
- CPU color implementation SHA-256:
  `761c086118858975cbe5ad93457f97241ffedd5816452b1d9b3303d17e8265ca`.
- Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7 ARM64, CPython 3.12.13.
- Benchmark: `migration-benchmark-8627ed71ffcd45a5baba3b75d9cda50c`.
- Material-case parity gate: `migration-parity-benchmark-gate-51da76e3423e4cac83392470824e3e7d`.
- Odd-tail CPU parity: `migration-parity-03713b63a29e40f69234e849bd4651b0`.
- Odd-tail SIMD parity: `migration-parity-6d81bcdca0704e51ae5333a7f6bf3843`.
- Benchmark input SHA-256:
  `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`.
- Parity input SHA-256:
  `f9bc243391b4f5b3129319d8c792c59a12691df3363ff1aedb02ffb005923b2e`.

The complete reports, raw samples, execution/producer attestations, parity
proofs, and checksums are retained beside this file. Three subject rows were
added to [`../performance-optimization-local-runs.csv`](../performance-optimization-local-runs.csv).
No full campaign, coverage, commit, or push ran; this result completes no item
in the 33-item checkpoint batch.
