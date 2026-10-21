# F-mode bicubic Resize current-source CPU rebaseline, native x86

## Frozen workload and source identity

- Workload: `pipeline-chain.resize-native-f32.bicubic-noise-1024x768`
- Parity case: `PIL.Image.Image.resize.nuanced.f-resize-bicubic-noise-1024x768`
- Mode and dimensions: F, 1024×768
- Boundary: `call;materialize` (observed steps), warm cache, release, 5 warmups, 20 iterations × 5 samples (100 samples), concurrency 1
- Target: revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty source; default CPU features
- Host: AMD Ryzen 5 2600X, Linux x86_64 (`x86_64-ryzen-5-2600x-native-01`), CPython 3.12.3
- Fixture identity: manifest and benchmark workload input hashes match the local source; this exact parity case object hash also matches (`3499bfbf9b922e4ee6c80b5f8bf9dcb182b224c8cb33ca6dd3bb463305a99762`).
- Source file hashes matched between local and runner for `pillow-rs/src/ops/pil_resize.rs` (`c2174fd05d57e27b06e5d22a8d7638cf8492f4949f2c4b65910bcbf39d0587da`) and `pillow-rs/src/compute/pool_cpu/ops/geometry.rs` (`8e2852743becd04ad63e607a897c9fcfc56cfef4b4c27a0dafb580fd71f89500`).
- Exact parity gate: `migration-parity-benchmark-gate-2e6a4484fe434ca29104126a3b5a3116`, selected/executed/passed 1/1, no failures.

## Result

| Subject | Median | P95 | Receipt | Speedup vs Pillow |
| --- | ---: | ---: | --- | ---: |
| Pillow | 4.305461 ms | 4.392972 ms | Pillow oracle | 1.000× |
| Serial CPU | 4.362252 ms | 4.560901 ms | actual `cpu`, terminal complete, no fallback | 0.987× |

Current source is 1.3% slower than Pillow on this runner, a near-tie. Do not revive the previously tested F32 accumulator-seeding change: its A–B–B–A result was parked as too small/noisy with possible SIMD/GPU regressions (Kata `pillow-rs#gm7b`). This rebaseline is not a new optimization and does not promote a performance floor.

## Artifacts

- Run: `migration-benchmark-290fb67c74c94821b392c8fb339af32b`
- Benchmark report SHA-256: `0b47bc0ce9808bab3b5194eecd072f6bdaa7d6d9860a2b4d6d9ef4b4d20c83bf`
- Raw samples SHA-256: `965f9766a8708cd6a910f42bb040e631b8d907d0a815da0f4216714f0b1b6ade`
- Benchmark attestation SHA-256: `f1bf6ca9c6d296d57c6c8af8911825a7011531d781475399ae3d8d62be302a0f`
- Producer attestation SHA-256: `7360d7511d1c2fd3f349365a539432a8fbda2038199e3aa62ac7276be996e24b`
- Parity report SHA-256: `203413026849bda6e0f4ae7d56370a33c52c2abf38252842723db73d8bdb1552` (full report remains on the runner; only its compact bound attestation was copied)

Only compact result, sample, and attestation artifacts were transferred from the runner. No repository checkout, target directory, or parity payload was copied or removed.
