# Grayscale RGB ARM64 strict baseline — 2026-10-20

This source-bound record covers one warm, fully materialized 1024×768 RGB
Grayscale call on ARM64 macOS. The worktree was dirty at `418d5f0c`; this run
records the current implementation and does not claim a code optimization.

- Workload: `pipeline-op.grayscale.material-rgb-noise-1024x768`
- Parity case: `PIL.ImageOps.grayscale.nuanced.performance-large-rgb-noise-1024x768`
- Profile: `python-cpu`; stable runner ID `lazys-macbook-pro-arm64`
- Boundary: `call;observe-result`, 5 warmups, 20 iterations × 5 samples,
  concurrency 1
- Oracle / target medians: Pillow 210.729 µs; CPU 92.875 µs (2.269× faster)
- Parity: 1/1 passed. Execution: actual CPU, terminal complete, no fallback;
  one Grayscale operation, one mode conversion, one 786,432-byte output
  allocation, 3,145,728 peak live host bytes
- Build witness: complete and source-stable across 297 files; loaded extension
  SHA-256 `fd6eb29222650095c88665ffe59d198fea8ebfea3eeff868cf090d805ecca0ce`
- PerfDB strict baseline: `grayscale-rgb-arm64-cpu-20261020`, baseline ID
  `d4973c33d205`; admitted in a disposable DuckDB. The generated database is
  not part of this evidence directory.

`benchmark.json`, `parity.json`, their attestations and producer bundles, raw
samples, the selected-run input closure, and the materialized build witness
are retained here. Their report and witness hashes are recorded in the
selected-run link. No SIMD/GPU profile, alternate mode, size, or architecture
is covered by this baseline.

The successful local macOS build carried
`RUSTFLAGS='-C link-arg=-undefined -C link-arg=dynamic_lookup'`. Future bound
runs on this macOS host must capture the same link setting; all selected-run
evidence needs a stable runner ID. See the reusable performance-parity
optimization skill for the observed admission requirements.
