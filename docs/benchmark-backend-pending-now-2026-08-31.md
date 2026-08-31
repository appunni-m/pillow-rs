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
  `build/migration-parity/all-backends-test-result.json`, generated at pushed
  source `2fdc6bb57` (SHA-256
  `2c3b8ce4575de3b243fce2fc9ba85c55b958c9be52bd5ac9019f9633a6c437fa`).
- Schema-v3 correctly reports `passed_with_backend_gaps`, not plain `passed`:
  CPU and GPU each have 6,513 terminal-complete pipeline receipts; SIMD has
  6,518.  CPU has 3,562 no-receipt and 877 terminal-incomplete cases; SIMD
  has 3,550 no-receipt and 884 terminal-incomplete cases.  SIMD has 300
  terminal CPU receipts, and GPU has 389 terminal CPU receipts plus recorded
  host-control/fallback reasons. This is an evidence gap, not a parity
  failure or an excuse to remove cases.
- The receipt-boundary correction in `d0821989d` keeps a successful pipeline
  receipt as the terminal candidate when observation serialization emits no
  separate telemetry, then marks it only after every public observation is
  successful. A one-case CPU/SIMD/GPU check proved 1/1 terminal receipts per
  lane; the full run confirms the larger counts above.
- The maintained 70-row GPU cohort is value-exact and currently classified as
  70 native GPU / 0 host-control / 0 failures. Constant-F lowering is exact;
  finite nonconstant Box upscales add a 144/144 native-GPU exact matrix, while
  the bounded one- or two-axis F Box 2:1 downscale lane now has direct native
  byte checks after `2fdc6bb57`; the small-frame CPU Gaussian path has a
  focused terminal benchmark after `888f1bba5`.

## Pending — only these items remain

### P0. General exact F-mode GPU lowering

- [x] Added a bounded pure-Rust/WGSL lane for one- or two-axis 2:1 Box
  downscales: finite, same-sign F samples at or above `2^-20` use a two-tap
  half-before-add shader branch and copy unchanged axes opaquely. Direct
  Pillow byte checks covered 2,000 one-axis finite extreme cases (all matched;
  1,179 native GPU and 821 deliberate negative-zero host-control) plus 3,000
  two-axis cases (all matched; 2,500 native GPU and 500 deliberate
  negative-zero host-control).
- [ ] Extend filtered F-mode resize beyond finite constant samples and the
  bounded finite Box upsample copy and the new 2:1 Box lane. Arithmetic
  filters, other Box ratios, and nonfinite/negative-zero samples still require
  an exact device accumulator/rounding path. Keep every such input value-exact
  against Pillow while the native proof is incomplete.
- Acceptance: direct byte checks covering finite, negative-zero, and non-finite
  values; terminal actual-GPU receipts with no fallback for the admitted native
  cohort; focused and full strict parity remain green.

### P1. Close the backend-proof denominator honestly

- [ ] Reconcile the 10,952 public cases with pipeline applicability. The
  receipt boundary is now correct, but CPU/GPU still have 877 terminal-
  incomplete cases and 3,562 no-receipt cases; SIMD has 884 and 3,550.
  Every case admitted to a CPU/SIMD/GPU native cohort must have a
  terminal-complete receipt, the requested actual backend, and an empty
  fallback taxonomy. Cases that do not enter the pipeline remain explicitly
  counted outside that cohort; they are not relabeled or silently dropped.
- [x] Regenerated the schema-v3 all-backends artifact after final source
  commit `2fdc6bb57`; the validator keeps `passed_with_backend_gaps` visible
  until the denominator is proven.
- Acceptance: reproducible receipt sidecars, equal case-ID digests, exact
  requested/actual backend counts, and no false plain-`passed` aggregate.

### P2. Finish the performance contract

- [x] Rerun the same 11-ID equal-receipt cohort after the CPU filter fix. Runs
  6→7 and 7→8 retained 44 comparable pairings but reported 8 and 9 timing
  violations; the former repeated CPU `draw-filter-invert` row is no longer a
  CPU violation in the focused run (`0.055396 ms` median). Remaining rows are
  variance/regressions to classify, not a closed speed gate. Receipts:
  `stable-cohort-budget-7-vs-6.json` and
  `stable-cohort-budget-8-vs-7.json` (8 and 9 violations respectively; 44
  comparable pairings in each).
- [ ] Produce **two consecutive zero-violation** budget reports on the same
  source with equal workload IDs and terminal no-fallback receipts. Until both
  exist, the speed gate stays open.

## Goal tracking

The Codex goal remains **active** because P0, P1, and P2 are unfinished. This
file is the current status record; update it only with reproducible evidence.
