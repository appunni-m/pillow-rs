# Current-source YCbCr Grayscale rebaseline — 2026-10-16

This is one focused `PIL.ImageOps.grayscale` rebaseline on the current dirty
ARM64 checkout. It checks the YCbCr path locally after the 2026-10-13 published
Ubuntu ARM64 row measured 1.934× Pillow. It is a control only; no algorithm
change is attributed.

The selected workload is
`pipeline-op.grayscale.material-ycbcr-noise-1024x768`; its exact case is
`PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`.
The workflow is warm, release, `call` plus `observe-result`, five warmups,
20×5 samples, concurrency one. Only `python-cpu` was selected. Exact parity
passed 1/1 against Pillow 12.2.0.

| Subject | Median | P95 | Throughput | Execution |
| --- | ---: | ---: | ---: | --- |
| Pillow | 0.112417 ms | 0.144125 ms | 8,895 ops/s | Pillow |
| pillow-rs serial CPU | 0.046375 ms | 0.050125 ms | 21,563 ops/s | actual CPU, one Grayscale, no fallback |

This current-source Mac ARM64 comparison is 2.424× Pillow. The selected
Grayscale operation reports one mode conversion, no vector blocks, one host
allocation of 786,432 bytes, and completed result materialization. The host
differs from the Ubuntu ARM64 published matrix, so this does not replace or
retire its 1.934× row. It is dirty-tree diagnostic evidence; the separate RGB
Grayscale row and other modes/backends are not addressed.

The implementation path is unchanged in the current tree at
[`imageops.rs`](../../../pillow-rs/src/compute/pool_cpu/ops/imageops.rs). The
target source identity is dirty revision
`9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`; Pillow and the target both ran on
`Lazys-MacBook-Pro.local`, macOS 15.7.7 ARM64, CPython 3.12.13. A same-host
current-source run on Ubuntu ARM64 is still required before updating that
published comparison.

## Evidence

- Benchmark run: `migration-benchmark-9927a2fd0d464d3f84f9948daa85979d`.
- Parity gate: `migration-parity-benchmark-gate-c8754973062543bb91dcb9bc38d8642f`.
- Benchmark report SHA-256: `fb169ec215d5e7be72fd03dd025961a2b52a070c2bc83bd6aab5706a78d98695`.
- Compressed parity result SHA-256: `aa3c154630e2ac143362f9a6c0ac198b3698f5c49d44b96e76f88c72dec53c5d`.
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`.
- Benchmark-input SHA-256: `7eaf80ff5b3b32123c066d7e52f8ce7dcbbd359ff716c116df0256c694bae838`.
- Parity-input SHA-256: `b7b3d903f1c0fdcb2e03347ef46b2f19b62e36891daf3da4ebd1dc040e6bd028`.

Both report schema validators passed. The local-run ledger contains the two
Pillow/CPU subject rows. No other workload, coverage, commit, or push ran.
