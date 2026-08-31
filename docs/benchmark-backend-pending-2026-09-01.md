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
- Finite nonconstant F Box upscales have a separate exact copy proof for
  arbitrary non-downscaling geometry, including mixed `PutData(F)` plus Box
  chains (144/144 direct native-GPU samples).
- The new proof-gated dyadic F lane is also exact/native for the admitted
  Bilinear and one- or two-axis power-of-two Box cases; the two-axis proof is
  bounded by the source significand-span check. Heterogeneous/non-dyadic
  inputs remain on exact host control.

## Pending — do these in order

### P0 — exact F-mode GPU resize arithmetic

- [x] Implement and prove the bounded dyadic subset: fixed/f64 coefficient
  agreement, same-sign normal power-of-two F words, Bilinear, and one- or
  two-axis power-of-two Box reductions through 64:1 (with the two-axis
  significand-span bound). The direct native matrix is byte-exact with
  terminal `actual_backend=gpu` receipts and no fallback.
- [x] Keep the finite nonconstant Box-upscale copy lane admitted for arbitrary
  non-downscaling geometry; its one-tap relocation proof is separate from
  arithmetic-filter admission.
- [ ] Extend exact arithmetic coverage to heterogeneous/non-dyadic Bilinear,
  Bicubic/Lanczos/Hamming, Box downscales outside the proven 2:1 and
  one-/two-axis power-of-two limits, and two-axis reductions outside the
  significand-span proof. Keep every unproven arithmetic input on exact host
  control, including NaN, infinity, and negative zero.

### P1 — honest backend-proof denominator

- [x] The receipt sidecars now emit schema `pipeline-execution-evidence@2`
  with one status for every selected case: `complete`, `partial_receipt`,
  `missing_receipt`, `not_applicable`, or `indeterminate`. The summary keeps
  the historical no-receipt counts and adds a partition whose total remains
  the fixed **10,952** public cases. Only high-confidence non-pipeline paths
  leave the backend-proof cohort; missing, partial, and indeterminate paths
  remain proof gaps. The all-backend envelope stays schema-v3, and old @1
  sidecars are diagnostic-only until regenerated.
- [x] Regenerate and review the schema-v3 all-backend artifact. At source
  `a053a7422`, CPU/GPU report **6,924 complete + 466 partial + 0 missing +
  2,550 not applicable + 1,012 indeterminate**; SIMD reports
  **6,936 + 466 + 0 + 2,539 + 1,011**. All six public lanes remain
  10,952/10,952 and GPU smoke is 1/1; the aggregate is correctly
  `passed_with_backend_gaps`. Artifact SHA-256:
  `6f9be35154e337a90b4b8be2bd0251cd8ea89b1253b8c3b2aa0aa2346dd0c9e4`.
- [x] Fix the receipt boundary and classifier gaps without changing IDs or
  denominators: an observed final serialization may prove an earlier
  dispatch, while eager `ModeFilter` and source-independent degenerate or
  out-of-bounds crops are not deferred pipeline work. The focused partitions
  moved CPU/GPU from 877 partial + 20 missing to 466 partial + 0 missing, and
  SIMD from 884 + 20 to 466 + 0; receipt/evidence regression tests pass.
- [ ] Keep the aggregate `passed_with_backend_gaps` until every claimed native
  cohort has complete terminal receipts, matching case-ID digests, requested
  actual backends, and an empty fallback taxonomy.

### P2 — performance acceptance

- [x] Bound GPU working-buffer reuse to four times the requested capacity;
  the controlled small-draw case dropped from about 2.4 ms with a 6.3 MiB
  retained pool to about 0.59 ms with a 19 KiB pool, with exact/native output.
- [ ] Run the same equal-ID, equal-receipt cohort twice consecutively with
  **zero** budget violations. The current post-fix pair has all 44 pairings
  comparable but **18** violations; the earlier ratio-bounded cohort reported
  **5** and **6** (44 pairings each). Timing acceptance remains open.

## Required closeout

- [ ] Run the maintained focused lane, full strict all-backend parity, receipt
  and evidence validators, format/lint checks, then commit and push only after
  the corresponding evidence changes.

Last verified source: `a053a7422` (full all-backend run; working tree has
pre-existing unrelated changes). The overall goal is intentionally **active**.
