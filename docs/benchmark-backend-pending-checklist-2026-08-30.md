# Benchmark/backend pending checklist

This is the focused checklist for the active audit goal. No operation is
excluded; parity is closed. The only open gate is measured performance budget
work.

## Pending

- [ ] **Close the remaining speed budget.** The latest guarded comparison has
  **139** violations across **2,976** comparable rows: 52 CPU, 40 SIMD, 29
  Pillow, and 18 GPU. Continue only with paired, actual-backend measurements;
  do not change thresholds, fixtures, or workload denominators.
- [ ] **Recompute the budget after each accepted optimization.** Keep the
  latest benchmark, comparison, and hash in this checklist so the count is
  reproducible and historical timing snapshots do not become the work list.
- [ ] **Deliver the committed evidence.** Push the source and documentation
  commits after the final review; leave unrelated generated/user files
  unstaged.

## Closed in this pass

- [x] CPU BoxBlur now returns an exact clone for constant images up to 64x64;
  the 32x32 target improved 67.53% whole-workflow and 83.02% in the CPU
  backend. BoxBlur and GaussianBlur parity remained 63/63.
- [x] SIMD Reduce serializes outputs below 1,024 pixels to remove row-task
  overhead. The 16x16, 32x24, and 32x32 targets improved to 0.014229,
  0.014646, and 0.015229 ms; focused strict SIMD parity passed 14/14.
- [x] GPU point fusion now handles explicit native L/LA/RGB/RGBA byte modes;
  the 1,024-dispatch L invert chain became one dispatch and improved 36.6-48.8%
  in paired Metal runs. Focused strict GPU parity passed 1/1.
- [x] SIMD Equalize identity, grouped SIMD convolution, direct RGB upload,
  and bounded readback polling are integrated with exact parity receipts.
- [x] The committed wasm feature gate for the SIMD threshold was verified by
  `make build-wasm-core`; Node and browser lanes then passed the aggregate gate.
- [x] The final all-backends receipt passes CPU, SIMD, GPU smoke/full, Node WASM,
  and browser WASM at **10,952/10,952** each.
- [x] Maintained checks pass: `make fmt`, `make clippy`,
  `make repo-map-check`, `make migration-parity-fixtures-check`, and
  `git diff --check`.

## Current receipts

- Standard benchmark: `build/migration-parity/final-standard-after-latest.json`
  (744/744; SHA-256
  `300c4dc3597b7034317a7d3f68e5335986083df1ec98b242f5d30a4d9df621fe`).
- Benchmark parity sidecar: `build/migration-parity/final-standard-after-latest-parity.json`
  (SHA-256
  `79bec9b9890f308fd608c46089334207ae1eaf9adffe4b7417ae1fd320bfe1c5`).
- Budget comparison: `build/migration-parity/pipeline-budget-check-after-latest.json`
  (139 violations; SHA-256
  `21bc30a5c6cecfc28f1e6b6e40f7410dd40a4685e02111a4acd2f3bbaefcbec0`).
- Performance report: `build/migration-parity/pipeline-performance-report-after-latest.json`
  (SHA-256
  `14c1df36f815c1637d82cca066bd24a1698671eac4177ab53cc944b9dcaf5166`).
- Coverage/roadmap receipts:
  `pipeline-benchmark-coverage-after-latest.json` (SHA-256
  `42a11382f48b3977f8240e1cb89d787b1ace8e8c06fe0774606ab1b0fd4af857`) and
  `pipeline-roadmap-status-after-latest.json` (SHA-256
  `eab2f2d054c2612d17db98069e522549d33970e0275b3a9627f132efb6eff6d9`).
- All-backends receipt: `build/migration-parity/all-backends-after-latest.json`
  (all six lanes passed; SHA-256
  `138b37213c107a4d8149ae13a3f58dc5c0f0064a08c28ce4f650f391cde7fe94`).

The active goal remains open only because the 139-row performance budget is
not yet closed. Do not reopen parity work unless a new exact byte/value/error
divergence is observed.
