# ImageDraw.text terminal CPU receipt, ARM64, 2026-10-17

This is the accepted focused current-source CPU run after repairing terminal
receipt handling in the untimed benchmark proof path. It is a local dirty-tree
comparison, not a clean published snapshot or an operation-completion claim.

## Frozen scope

- Benchmark run: `migration-benchmark-dd65ed53cf2a47e08dc99e37927a986e`
- Workloads: `pil-imagedraw-imagedraw.text.standard` and
  `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768`.
- Backend: CPU only. The public operation manifest marks `ImageDraw.text` as a
  direct CPU operation; this run makes no SIMD, Parallel CPU, or GPU claim.
- Boundary: `call;observe-receiver`, warm cache, 5 warmups, 20 iterations × 5
  samples, concurrency 1 (100 timed samples per subject).
- Environment: macOS 15.7.7 ARM64, Pillow 12.2.0, CPython 3.12.
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty worktree).
- Target runtime stable identity: `ebddb4a466d407c0ce5b7acb31d5b3ce4ad18c93ce4072eab55015ffb2a6702a`.
- Loaded `_core.abi3.so` SHA-256:
  `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Benchmark producer attestation is complete and bound to the report. The
  benchmark report passed the local result validator.

## Results

| Workload | Pillow median | pillow-rs median | Verified ratio | Target execution |
|---|---:|---:|---:|---|
| `pil-imagedraw-imagedraw.text.standard` | 0.830646 ms | 0.500021 ms | 1.661× | CPU, 1 operation, terminal, no fallback |
| `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768` | 0.725833 ms | 0.492042 ms | 1.475× | CPU, 1 operation, terminal, no fallback |

Both rows are faster than Pillow on this host and remain below the 2× priority
target. Keep the issue open for optimization and for the missing SIMD, Parallel
CPU, and GPU category status. The timed public-call path contains no receipt
instrumentation; actual CPU execution is recorded by the separate untimed
proof child.

## Correctness and proof identity

The benchmark reused the matching parity report
`migration-parity-benchmark-gate-b4ff3842956e406c9f60c7a4aaf7ea0d` after
runtime and input identity checks. The two exact performance parity cases
passed 2/2. The existing focused ImageDraw guards passed 4/4 in
`migration-parity-256a655b3c50452188c8778f34bceb18` on the same target
extension:

- `PIL.ImageDraw.ImageDraw.text.mode.rgba`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-rgb-negative-position`
- `PIL.ImageDraw.ImageDraw.text.nuanced.embedded-bgra-rgb`
- `PIL.ImageDraw.ImageDraw.text.nuanced.embedded-bgra-rgba-context-rgb`

The accepted run's target proof receipts report `status=completed`,
`terminal_complete=true`, `actual_backend=cpu`, one operation, zero dispatches,
and no fallback. The previous receipt-enabled attempt,
`migration-benchmark-a42092da59104260beefd2f3c37fbb27`, is retained in
`../imagedraw-text-receipt-arm64-20261017/` but is diagnostic only: it exposed
an early-return bug that left the direct receipt non-terminal. The focused unit
test now covers the skipped-observation proof path.

## Artifacts

- `benchmark.json`: validated two-workload result.
- `benchmark-attestation.json` and `benchmark-attestation.json.children/`:
  execution identities and proof invocations.
- `benchmark-producer-attestation.json` and its bundle/children: producer
  identity bound to the report.
- `parity.json`: reused identity-matched parity result.
- `raw-samples.json`: ordered timing samples.
- `docs/evidence/performance-optimization-local-runs.csv`: appended Pillow and
  CPU rows for both selected workloads.
- `SHA256SUMS.txt`: checksums for this evidence directory, excluding this
  README and the checksum file itself.
