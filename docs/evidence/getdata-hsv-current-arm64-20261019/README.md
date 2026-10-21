# GetData HSV current-worktree CPU rebaseline — 2026-10-19

This refresh targets the historical macOS CPU miss for
`PIL.Image.Image.getdata` on HSV band 2. The selected workload is
`pipeline-chain.terminal-read.getdata.hsv-band2-1024x768`; the exact parity case
is `PIL.Image.Image.getdata.performance.material-hsv-band2-1024x768`.

- Host: macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0.
- Target: dirty revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`, release,
  default features, actual serial CPU, no fallback. Loaded `_core` SHA-256:
  `400c2ccc2b3644d4a87c9c604b112189cee53aa82b02426491cd77d623bb33e3`.
- Boundary: warm `band-data`, 5 warmups and 20 iterations × 5 samples, one
  concurrency lane. The report records one 786,432-byte host result buffer.
- Exact parity passed 1/1. Pillow median was 5.639667 ms; CPU median was
  2.593313 ms, or 2.175× Pillow.

The previously published 0.765807× CPU row is not reproduced on this dirty
current worktree. This is one profile/workload and does not complete GetData;
other modes, CPU workload coverage, SIMD/Pillow-SIMD, Parallel CPU, GPU, and
sustained throughput remain open.

Run: `migration-benchmark-af238f45122c4827a3e8d16d060fafb7`.
Parity: `migration-parity-benchmark-gate-13a413cdeef249f58efc7bbffcb5d7d2`.
Benchmark report SHA-256: `0adbf0bc5ea538650596701eaf7ef23750e02c9e1c7aec9fc3458ad18e26efd1`.
Parity report SHA-256: `7bcd5d9c10d4fe6554e2c48afbf6c9cb5ba83d94b93091c63f1b8ea3c22f9588`.

The benchmark, parity, and attestation artifacts were imported into the
generated PerfDB; its `history` now includes this run. The local results ledger
also records the same-report Pillow and CPU observations. No strict baseline
was promoted: the dirty-worktree report has no validated
`workflow-parity-link@2` cross-run context, which the baseline promoter
requires. Keep the pinned clean matrix row separate from this current-source
diagnostic until the source-build and cross-run proof are complete.
