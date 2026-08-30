# Benchmark/backend pending checklist

This is the focused follow-up list for the active audit goal. Historical audit
rows are not pending work. Parity is closed; the remaining work is performance
evidence and final delivery.

## Pending (in order)

- [ ] **SIMD Equalize identity path.** Finish the isolated
  `native_lut_is_identity` short-circuit for the uniform RGB
  `pipeline-matrix.expanded.equalize.256x256` case. Accept only with two paired
  A/B runs showing at least 50% whole-workflow and 70% backend improvement,
  SIMD no slower than CPU, no fallback, and no more than 5% regression in the
  nonidentity LUT/equalize controls; then run strict SIMD parity.
- [ ] **Recompute the performance gate.** Regenerate the equal-ID comparison
  after any accepted change. The current guarded count is **247** violations
  (12 GPU, 93 CPU, 79 SIMD, 63 Pillow). Thresholds, fixtures, and workload
  denominators must not be edited to change this number.
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
- Latest standard benchmark: `build/migration-parity/final-standard-after-gpu-readback.json`
  (744/744; SHA-256 `c54198ca909a32e0e24ed7bc0229dbee6b8788f6527fd2bbd690ed2674a80a7b`).
- Latest budget comparison: `build/migration-parity/pipeline-budget-check-after-gpu-readback.json`
  (247 violations; SHA-256 `ca7ddf0c4d48b0a0da6222e048d96344fc2e841fa5a963b9e57889b4d405fbd4`).
