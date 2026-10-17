# I32 Resize current-source baseline — 2026-10-16

This is a focused current-tree diagnostic for the 1024×768 I-mode Bicubic
Resize workload. It is not a clean release-matrix row or an optimization
claim. The run freezes one workload and compares serial CPU, single-thread
SIMD, and GPU against the same Pillow 12.2.0 oracle.

## Frozen workload and parity

- Workload: `pipeline-chain.resize-native-i32.bicubic-noise-1024x768`
- Exact parity case: `PIL.Image.Image.resize.nuanced.i-resize-bicubic-noise-1024x768`
- Boundary: warm public `call;materialize`; input setup is outside timing.
- Policy: five warmups, 20 iterations × five samples, concurrency one.
- Parity: 3 selected, 3 executed, 3 passed, 0 failed (CPU, SIMD, GPU).
- Benchmark validation: benchmark and parity result schemas both valid.

## Results

| Profile | Median | P95 | Pillow / target | Actual backend | Receipt |
| --- | ---: | ---: | ---: | --- | --- |
| Pillow 12.2.0 | 2.279062 ms | 2.553250 ms | 1.000× | Pillow | oracle |
| Serial CPU | 1.123250 ms | 1.357834 ms | 2.028× | CPU | complete, no fallback; one 786,432-byte output allocation |
| Single-thread SIMD | 1.531000 ms | 2.688250 ms | 1.488× | SIMD | complete, no fallback; `neon-f64-resize`, 73,728 vector blocks, no scalar tail |
| GPU | 12.310959 ms | 13.175625 ms | 0.185× | GPU | complete, no fallback; two dispatches, 3,145,728 bytes uploaded and 786,432 bytes read back |

GPU latency is 8.04× the measured SIMD latency on this row. The benchmark
includes both transfer and materialization costs. It records one full-frame
copy and 6,423,308 retained cache bytes for the GPU call; these counters do
not by themselves identify the dominant phase.

## Identity and limits

- Benchmark run: `migration-benchmark-9bfa6efe0de341e0bbceaa86e9f6f716`.
- Parity run: `migration-parity-benchmark-gate-6c7d59ffab104e01bf1f0467ccba573d`.
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty worktree.
- Loaded `pillow_rs._core` SHA-256: `7e6fced7ae0a16ea94847861e99c91a3d8be8c5a791613a91f2de525fffbd57c`.
- `geometry.rs` SHA-256: `d02910803282d6db6ca38a2afff249cdc4a25dd2577d852637b3f72a15ae1bbd`.
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`.
- Benchmark-input SHA-256: `de981adb0ee12bd7bcb1d99d5b8779fe83e86598b9a786efcfa4d405c7d278dd`.
- Parity-input SHA-256: `5f6c75f43f50c227a6cc920f25c6532f3be2ff55db81462ae1f367bdb74e4508`.
- Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7 ARM64; CPython 3.12.13.

The serial CPU row clears 2× on this input only. SIMD remains below 2× Pillow,
and GPU is substantially slower than SIMD. The old I32 Resize experiments used
other source identities; this run cannot attribute gains or regressions to
those candidates and does not complete Resize across modes, sizes, hosts, or
backends. Keep this row diagnostic until a clean matching cohort is published.

Raw benchmark samples, execution attestations, and the parity report remain in
the local build directory `build/migration-parity/i32-resize-current-source-20261016/`.
Kata tracking is `pillow-rs#b9n6`; read-only backend audits are `#95h0`,
`#y1ba`, and `#2sev`. No source file changed for this rebaseline. No full test,
parity, benchmark, or coverage campaign, commit, or push ran.
