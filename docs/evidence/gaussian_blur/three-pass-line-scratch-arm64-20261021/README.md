# GaussianBlur three-pass line scratch trial — ARM64 — 2026-10-21

Disposition: rejected; no implementation change is retained.

## Frozen contract

- Workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- RGBA noise, 1024×768, radius 7.5; full public workflow through materialization.
- Warm cache, one warmup, three iterations × two samples, concurrency one.
- Runner: `lazys-macbook-pro-arm64`; manifest SHA-256: `761907d11b42a73e4ce1ac9495ce1d9f295a48a457a79ad0b774c4a701f48147`.
- Benchmark and parity input hashes match the saved current-source baseline: `9f0a8fdbef7c4eb921ac9897a7ddd1810c213df4fe962fb8e7d9870f541202a8` and `fffed899cc0391dfdc2e33d89a5059263b5b3410520e252835ffe1ac31b942d9`.

## Formal A–B–B–A results

| Cohort | Pillow median | SIMD median | Pillow / SIMD | Parity |
| --- | ---: | ---: | ---: | --- |
| A1 control | 7.131271 ms | 6.311875 ms | 1.1298× | 1/1 pass |
| B1 candidate | 6.588167 ms | 6.233896 ms | 1.0568× | 1/1 pass |
| B2 candidate | 6.583042 ms | 6.716584 ms | 0.9801× | reused B1 proof |
| A2 control | 6.591104 ms | 6.220376 ms | 1.0596× | 1/1 pass |

All timing reports used the frozen manifest, workload, input, policy, and runner.
Every SIMD receipt completed terminally on actual `simd`, reported no fallback,
589,824 vector blocks, and zero scalar tails. The candidate’s two SIMD medians
average 3.3% slower than the two control medians; B2 is slower than both A
controls and Pillow in its cohort. The candidate does not show a repeatable
full-call improvement, so its measured traffic reduction does not justify
retaining the extra row-buffer allocations and adapter path.

The focused kernel equivalence test passed 1/1 over short, odd, clamped-edge,
and vector-boundary widths. The candidate passed the selected Pillow parity
case in both fresh proof runs. Parity is intact, but performance acceptance
failed.

## Source and artifact identity

- Control extension: `_core.abi3.so` SHA-256 `83541d6d8c53d510a916005c66d13daced41148651aef6c0c9378750bc98484d`.
- Candidate extension: `_core.abi3.so` SHA-256 `20d33806894f17d31f35a66dc5022b5ef215860799b09c0b5405374ef87a41db`.
- Candidate adapter files and the rejected kernel snapshot are preserved under the ignored local directory `build/migration-parity/gaussian-three-pass-rows-arm64-20261021/candidate-snapshot/` for audit. The checked-out source and loaded extension were restored to the A control after A2; the candidate files are not compiled or dispatched.
- Full reports, raw samples, parity proofs, attestations, producer receipts, and both A/B binaries remain in the ignored local directory `build/migration-parity/gaussian-three-pass-rows-arm64-20261021/`. They are not added to Git because the selected parity reports embed large image observations.

The older 2026-10-21 source-bound rebaseline used a different manifest snapshot;
it was retained as historical evidence and excluded from this formal cohort.
The current operation remains open: CPU, remaining SIMD modes/sizes/architectures,
Pillow-SIMD comparison, GPU latency and sustained throughput still require
their own results.
