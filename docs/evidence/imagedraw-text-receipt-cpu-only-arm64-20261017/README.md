# ImageDraw.text CPU-only subject correction, ARM64, 2026-10-17

This focused run updates the two `ImageDraw.text` workload subject lists to
match the operation manifest: Pillow oracle plus serial CPU target only. The
cases, input assets, materialization boundary, sample counts, and thresholds
are unchanged. The corrected fixture SHA-256 is `e3d8a7907575eb4225cfa57de75323c4d9768aaef5daa3ca0f59991ee89d3e20`; the prior
overdeclared fixture SHA-256 was
`251a2fffbb20da7e26e850dbceb1adfb027f4972f8ad0100c72d8f02d40b305d`.

## Frozen scope

- Benchmark run: `migration-benchmark-f15c53df69354886b369d016cf105757`.
- Workloads: `pil-imagedraw-imagedraw.text.standard` and
  `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768`.
- Profile: CPU only, matching manifest requirement
  `PIL.ImageDraw.ImageDraw.text.performance.standard -> python-cpu`.
- Boundary: `call;observe-receiver`, warm cache, 5 warmups, 20 iterations × 5
  samples, concurrency 1 (100 timed samples per subject).
- Host: macOS-15.7.7-arm64-arm-64bit, Pillow 12.2.0, CPython 3.12.13.
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty worktree).
- Target `_core.abi3.so` SHA-256: `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Target runtime stable identity: `ebddb4a466d407c0ce5b7acb31d5b3ce4ad18c93ce4072eab55015ffb2a6702a`.
- Benchmark execution and producer attestations are complete; result schema
  validation passed.

## Results

| Workload | Pillow median | pillow-rs CPU median | Verified ratio | CPU receipt |
|---|---:|---:|---:|---|
| `pil-imagedraw-imagedraw.text.standard` | 0.869438 ms | 0.550271 ms | 1.580× | cpu, terminal, one operation, zero dispatches, no fallback |
| `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768` | 0.830874 ms | 0.515854 ms | 1.611× | cpu, terminal, one operation, zero dispatches, no fallback |

Both target rows remain below the user's 2× CPU priority target. These results
are faster than Pillow on this ARM64 host; no SIMD, Parallel CPU, or GPU claim
is made. Earlier run `migration-benchmark-dd65ed53cf2a47e08dc99e37927a986e`
remains in the ledger as historical timing, but its fixture falsely declared
SIMD and GPU subjects. Its contract status is superseded; its measurements are
not current profile coverage.

## Correctness and identity

- Reused parity run
  `migration-parity-benchmark-gate-b4ff3842956e406c9f60c7a4aaf7ea0d` after
  the runner validated case selection and runtime identities. Both exact
  ImageDraw performance cases passed (2/2).
- Target proof receipts show `status=completed`, `terminal_complete=true`,
  `actual_backend=cpu`, one operation, zero dispatches, and no fallback.
- `benchmark.json` records the corrected fixture hash and the same case IDs
  and measurement contract.

## Artifacts

- `benchmark.json`: validated focused result.
- `benchmark-attestation.json` and children: execution identity and proof.
- `benchmark-producer-attestation.json`, bundle, and children: producer proof.
- `parity.json`: reused identity-matched parity report.
- `raw-samples.json`: ordered timing samples.
- `docs/evidence/performance-optimization-local-runs.csv`: four current
  Pillow/CPU rows; previous rows are labeled pre-correction.
- `SHA256SUMS.txt`: checksums for evidence files, excluding this README and
  the checksum file itself.
