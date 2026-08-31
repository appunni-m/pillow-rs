# Exact parity-first pending checklist — 2026-08-31

This is the short active queue. The exhaustive historical audit remains in
[`benchmark-backend-exhaustive-audit-2026-08-30.md`](benchmark-backend-exhaustive-audit-2026-08-30.md);
closed waves and row-by-row history stay there.

## Goal

**Active — exact Pillow parity first, honest native-backend proof second,
performance third.** No input, denominator, expected value, threshold, or
backend label may be changed to shorten this list. A host-control path is a
diagnostic implementation state, not a completed native-backend claim.

## Current evidence

- Public all-backend values are green: CPU, SIMD, GPU, Node WASM, and browser
  WASM each compare **10,952/10,952**, and the GPU smoke gate is **1/1**.
  The latest schema-v3 receipt is
  `build/migration-parity/all-backends-test-result.json`, generated at
  `e72971f21` (SHA-256
  `363133834538efaad80ed444ea5716e66d6407c5d7f936fbb1e0c78798eaf2`).
- Schema-v3 correctly reports `passed_with_backend_gaps`, not plain `passed`:
  the full lanes have 937 terminal-complete pipeline receipts, thousands of
  non-pipeline or incomplete cases, SIMD has 43 CPU receipts, and GPU has 41
  CPU receipts plus recorded host-control/fallback reasons. This is an
  evidence gap, not a parity failure or an excuse to remove cases.
- The maintained 70-row GPU cohort is value-exact and currently classified as
  70 native GPU / 0 host-control / 0 failures. Constant-F lowering is exact;
  the small-frame CPU Gaussian path now has a focused terminal benchmark after
  `888f1bba5`.

## Pending — only these items remain

### P0. General exact F-mode GPU lowering

- [ ] Extend filtered F-mode resize beyond finite constant samples and the
  bounded Box upsample copy. Nonconstant and mixed-F inputs still require the
  exact device accumulator/rounding path. Keep every such input value-exact
  against Pillow while the native proof is incomplete.
- Acceptance: direct byte checks covering finite, negative-zero, and non-finite
  values; terminal actual-GPU receipts with no fallback for the admitted native
  cohort; focused and full strict parity remain green.

### P1. Close the backend-proof denominator honestly

- [ ] Reconcile the 10,952 public cases with pipeline applicability. Every case
  admitted to a CPU/SIMD/GPU native cohort must have a terminal-complete
  receipt, the requested actual backend, and an empty fallback taxonomy. Cases
  that do not enter the pipeline remain explicitly counted outside that cohort;
  they are not relabeled or silently dropped.
- [ ] Regenerate the schema-v3 all-backends artifact after the final source
  commit and require the validator to keep `passed_with_backend_gaps` visible
  until the denominator is proven.
- Acceptance: reproducible receipt sidecars, equal case-ID digests, exact
  requested/actual backend counts, and no false plain-`passed` aggregate.

### P2. Finish the performance contract

- [ ] Rerun the same 11-ID equal-receipt cohort after the CPU filter fix, then
  classify any remaining nonzero budget rows by operation/backend. The former
  repeated CPU `draw-filter-invert` violation is the first target; its focused
  benchmark is now `0.055396 ms` median on CPU.
- [ ] Produce **two consecutive zero-violation** budget reports on the same
  source with equal workload IDs and terminal no-fallback receipts. Until both
  exist, the speed gate stays open.

## Goal tracking

The Codex goal remains **active** because P0, P1, and P2 are unfinished. This
file is the current status record; update it only with reproducible evidence.
