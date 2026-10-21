# GaussianBlur RGBA SIMD current-source rebaseline — ARM64 — 2026-10-20

## Frozen workload and result

- Workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Exact parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- Context: RGBA 1024×768, warm release, one warmup, 3 iterations × 2 samples, concurrency 1
- Boundary: complete public workflow through terminal materialization
- Oracle: Pillow 12.2.0; host: macOS-15.7.7-arm64-arm-64bit; Python 3.12.13
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf` (dirty worktree)
- Benchmark run: `migration-benchmark-c73e9b5abb194d7c806f7beaf295d361`
- Parity gate: `migration-parity-benchmark-gate-23c599990b554eedafda9428554fa585` (pass)

| Subject | Median latency | p95 latency | Throughput |
| --- | ---: | ---: | ---: |
| Pillow | 6.624521 ms | 6.718708 ms | 150.965 ops/s |
| pillow-rs SIMD | 6.377979 ms | 6.621167 ms | 156.796 ops/s |

The current target is 1.039× Pillow for this selected case. Exact parity passed 1/1. The execution receipt reports requested=actual `simd`, `terminal_complete=true`, no fallback, path `vector`, 589,824 vector blocks, and zero scalar tail. The receipt also records one host allocation (3,145,728 bytes), two host buffers (6,291,456 bytes total), and no mode conversion. SIMD target is still below the repository's 5× Pillow goal.

The untimed execution receipt's backend phase was 5.900 ms; the end-to-end latency above includes workflow setup and terminal materialization. Most measured work remains inside the blur/materialization phase, so the next SIMD optimization should target the kernel path rather than shorten the family wrapper.

## Build and source identity

- Release extension: `pillow-rs-py/python/pillow_rs/_core.abi3.so`
- Extension SHA-256: `eb109268953b1acc925e753d55a100f9df869ca3d319d74fb1e85016344bd344`
- `adapters.rs` SHA-256: `8baecb092e5c1b70f48084d4db5fe487cc19b059d831b7fe34a902827e1af5f5`
- GaussianBlur adapter SHA-256: `27266e4b05ca016f7628f059356d03c18a37b8abeb9a8959e4b3bfc7dc042c29`
- ARM64 RGBA kernel SHA-256: `7649aa692c011ed1d76d90f6845db5bffc56008cd7bb6a2296359871de2927ec`
- Features: default profile; Parallel CPU/Rayon disabled

The family orchestrator move preserves its validation, mode dispatch, and calls to the shared blur implementation. This run exercises the moved route end to end. It is a current-source rebaseline, not a controlled before/after estimate of the file move.

## Retained artifacts

This directory keeps the benchmark report and raw samples, compressed exact-parity result, execution attestations, and producer attestations/bundles. `SHA256SUMS` records every retained file. The same selected-run measurement is also in `docs/evidence/performance-optimization-local-runs.csv`.
