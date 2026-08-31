# Benchmark/backend pending checklist — 2026-09-01

This is the only active queue. The longer audit and the previous status
snapshot remain available for history:

- [exhaustive audit](benchmark-backend-exhaustive-audit-2026-08-30.md)
- [previous focused snapshot](benchmark-backend-pending-now-2026-08-31.md)

## Goal

Keep the overall parity goal **active**: exact Pillow values first, honest
native-backend evidence second, performance acceptance last. Do not remove,
rename, relabel, or weaken a case to make a gate green.

## Evidence already closed

- Public all-backend comparisons are **10,952/10,952** for CPU, SIMD, GPU,
  Node WASM, and browser WASM; GPU smoke is **1/1**.
- The three historical benchmark-only mismatch workloads reproduce as passes
  on current source: `pipeline-chain.loaded-10.rgba-png-512x384`,
  `pipeline-matrix.expanded.rotate.1x1`, and
  `pipeline-matrix.expanded.add.1x1`.
- Same-size filtered F resizes and bounded 2:1 Box F resizes have direct
  native-GPU byte proofs, including non-finite/negative-zero coverage where
  the proof admits it.

## Pending — do these in order

### P0 — exact F-mode GPU resize arithmetic

- [ ] Implement and prove non-identity arithmetic filters and remaining Box
  ratios on changed geometry. The device path must match Pillow bit-for-bit,
  including NaN, infinity, and negative zero; keep unproven inputs on exact
  host control until a native proof exists.
- [ ] Add focused direct byte matrices and a regression test for every newly
  admitted native lane. Require terminal `actual_backend=gpu` receipts with
  no fallback for the admitted cohort.

### P1 — honest backend-proof denominator

- [ ] Reconcile the schema-v3 receipt gaps without hiding cases: CPU/GPU each
  currently have **3,562** no-receipt and **877** terminal-incomplete cases;
  SIMD has **3,550** and **884**. Distinguish explicitly non-pipeline cases
  from missing/partial receipts, while retaining every case in the public
  denominator.
- [ ] Keep the aggregate `passed_with_backend_gaps` until every claimed native
  cohort has complete terminal receipts, matching case-ID digests, requested
  actual backends, and an empty fallback taxonomy.

### P2 — performance acceptance

- [ ] Run the same equal-ID, equal-receipt cohort twice consecutively with
  **zero** budget violations. The latest comparable reports still show **8**
  and **9** violations (44 pairings each), so the speed gate remains open.

## Required closeout

- [ ] Run the maintained focused lane, full strict all-backend parity, receipt
  and evidence validators, format/lint checks, then commit and push only after
  the corresponding evidence changes.

Last verified source: `920efe009` (targeted benchmark run; working tree has
pre-existing unrelated changes). The overall goal is intentionally **active**.
