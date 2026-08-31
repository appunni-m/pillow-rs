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

- [x] The receipt sidecars now emit schema `pipeline-execution-evidence@2`
  with one status for every selected case: `complete`, `partial_receipt`,
  `missing_receipt`, `not_applicable`, or `indeterminate`. The summary keeps
  the historical no-receipt counts and adds a partition whose total remains
  the fixed **10,952** public cases. Only high-confidence non-pipeline paths
  leave the backend-proof cohort; missing, partial, and indeterminate paths
  remain proof gaps. The all-backend envelope stays schema-v3, and old @1
  sidecars are diagnostic-only until regenerated.
- [ ] Regenerate the schema-v3 all-backend artifact and review the resulting
  partition. A classification of the last @1 CPU/GPU artifact predicts
  **6,513 complete + 877 partial + 20 missing + 2,530 not applicable + 1,012
  indeterminate**; SIMD predicts **6,518 + 884 + 20 + 2,519 not applicable +
  1,011 indeterminate**. These
  are planning counts, not replacement evidence; the rerun must retain every
  case and preserve requested/actual backend and fallback receipts.
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
