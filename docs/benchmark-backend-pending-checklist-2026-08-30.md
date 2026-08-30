# Benchmark/backend pending checklist

This is the focused follow-up list for the active audit goal. Historical audit
rows are not pending work. Parity is closed; the remaining work is performance
evidence and final delivery.

## Pending (in order)

- [x] **SIMD Equalize identity path.** The
  `native_lut_is_identity` short-circuit and fixed-band L/RGB histogram
  reduction are integrated. Two paired runs improved the uniform RGB
  `pipeline-matrix.expanded.equalize.256x256` workflow by 67.51% and 89.41%
  (backend by 73.22% and 93.03%); SIMD beat CPU in both final runs, controls
  stayed within 5%, and focused/full strict SIMD passed 6/6 and 10,952/10,952.
- [x] **Recompute the performance gate.** The post-change equal-ID comparison
  reports **203** violations (59 Pillow, 50 CPU, 66 SIMD, 28 GPU) across 2,976
  comparable rows. Thresholds, fixtures, and workload denominators were not
  edited.
- [ ] **Refresh the final parity receipt.** Run the maintained all-backends gate
  after the last source change and require CPU, SIMD, GPU, Node WASM, and browser
  WASM at 10,952/10,952 with terminal receipts.
- [ ] **Finalize and deliver.** Update the audit/checklist artifact hashes, run
  `make fmt`, `make clippy`, `make repo-map-check`, and `git diff --check`, then
  commit and push only intentional source/docs changes.

No parity lane is pending. If a performance candidate misses its acceptance
criteria, keep the measured violation and artifact visible rather than changing
the gate.

## Already closed (do not reopen)

- Strict SIMD parity: 10,952/10,952.
- Strict GPU parity: 10,952/10,952.
- Combined CPU/SIMD/GPU/Node/browser parity: 10,952/10,952 per lane.
- Standard benchmark correctness: 744/744 workloads, zero failures and
  not-run records.

## Current verified inputs

- SIMD focused benchmark: `/private/tmp/simd-5x5-direct-vector-benchmark.json`.
- SIMD strict parity: `/private/tmp/simd-strict-conv-direct.json`.
- GPU worker commit: `d5af68d3c66b6134c4277b2d77aa05fb3e20b8ec`.
- GPU readback commit: `b98431db71dc6f60e06c47838376a99783b7511b`.
- SIMD worker commit: `08b1fb4b7` (integrated as `b469848c9`).
- SIMD Equalize commit: `976567232c5086138339156d87b4cbaab2441fb8`.
- Latest standard benchmark: `build/migration-parity/final-standard-after-equalize-identity.json`
  (744/744; SHA-256 `2a7c9d0d7106dc60d6b1b2c4ffcd71a78c7f8cd2f6438b16e358022d0444c284`).
- Latest budget comparison: `build/migration-parity/pipeline-budget-check-after-equalize-identity.json`
  (203 violations; SHA-256 `3ba3206fc88f6dfb507130bfd94e2b18bdd5415437753289f1f69bd2c149f70b`).
