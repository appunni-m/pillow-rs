# Benchmark/backend pending checklist — 2026-09-01

This is the short, active queue. The complete investigation log is in the
[exhaustive audit](benchmark-backend-exhaustive-audit-2026-08-30.md); the
superseded snapshot is [here](benchmark-backend-pending-now-2026-08-31.md).

## Goal

Keep the parity goal active: first make every Pillow value and error exact,
then prove which native backend executed, then accept performance only from
equal-ID, equal-receipt measurements. Never delete, relabel, or weaken a case,
fixture, denominator, or threshold to improve a report. A host-controlled
execution is a parity-preserving fallback, not a parity completion claim.

## Remaining work — only these three buckets

### P0 — broader exact F-mode device arithmetic

- [x] Add the verified marker-9 f64-equivalent device reducer for a single
  changed axis. Commit `4fe5535ff` transports the complete f64 coefficient
  dyadic parts, performs exact integer products/sums, and rounds once at the
  f32 store. The host proof admits only finite normal F words whose ordered
  f64 result matches the reducer; unchanged axes copy their words exactly.
- [ ] Extend the proof to two changed axes and the remaining coefficient/value
  domains. The open families are changed-horizontal-plus-vertical F resizes,
  subnormal/nonfinite or negative-zero words, coefficient overflow/cancellation
  cases, Box ratios outside the proven row limits, and chains outside the
  cumulative intermediate proof. The current two-axis shape stays on exact
  host semantic control until its device intermediate/storage contract is
  separately validated.
- [x] Keep the newly proven signed two-axis subset integrated. Commit
  `a3d2c886b` adds host-verified two-limb signed integer accumulation, exact
  horizontal f32 intermediate boundaries, and two-axis proof checks. Focused
  F tests are 9/9; GPU-pool tests are 16/16; the post-merge native probe is
  byte-exact for 45/45 cases (7 native GPU, 38 exact host semantic control).
- [x] Preserve the conservative route for every unproven row. The forced
  generic-shader diagnostic diverged by ULPs on heterogeneous Bilinear,
  Bicubic, Lanczos, Hamming, and non-dyadic Box; a 2x1 -> 4x1 Bilinear
  counterexample also defeats a broad dyadic-source admission. No unsafe
  admission change was made.

### P1 — complete native-backend receipt proof

- [ ] Close the 102 partial native receipts in each CPU, SIMD, and GPU lane;
  every claimed pipeline case needs a terminal-complete receipt. The current
  all-backend artifact has no missing or indeterminate native receipts, but
  the 102 partial cases remain proof gaps.
- [ ] Reconcile backend identity and fallback taxonomy. Current terminal
  counts include 405 SIMD-lane CPU receipts and 386 GPU-lane CPU receipts;
  the GPU lane has 6,698 native GPU receipts and 142 exact host semantic
  control receipts, plus explicit logical-mode, dimension, Transform, and
  Contrast routes. These are visible evidence gaps, not value-parity
  exemptions.
- [ ] Close the WASM receipt gaps: each Node/browser lane is value-exact but
  currently reports 6,713 complete, 586 partial, 888 missing, 2,738
  not-applicable, and 27 indeterminate cases. Keep the aggregate status
  `passed_with_backend_gaps` until these receipts are resolved or explicitly
  bounded by maintained backend evidence.

### P2 — performance acceptance

- [ ] Produce two consecutive equal-ID/equal-receipt cohort comparisons with
  zero budget violations. The fixed 11-ID cohort has 44 comparable pairings
  and no receipt fallbacks; the latest consecutive checks report 11, 7, and 6
  violations. The factor-1.0 Brightness identity path remains a deterministic
  row-level improvement (CPU medians about 0.181/0.163 ms before versus
  0.042/0.049/0.042 ms after), but aggregate timing acceptance is still open.

## Evidence recorded in the current run

- [x] The schema-v3 all-backend envelope at source `4fe5535ff` is
  `/tmp/all-backends-post-4fe5535.json` (SHA-256
  `b915cb2f93c172241a2bbe911ba418414aa5cabbd08c39302367a9080e580946`).
  CPU, SIMD, GPU, Node WASM, and browser WASM are each 10,952/10,952
  value-exact; GPU smoke is 1/1; case-ID digest is
  `881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.
- [x] The marker-9 native probe is exact for the new heterogeneous lane:
  the `(2,2) -> (1,2)` Bilinear case is byte-for-byte equal to Pillow and
  publishes an actual-GPU receipt. The rebuilt randomized probe covers 5,000
  finite-F rows (269 actual GPU, 4,731 exact host control) with zero
  mismatches; the known `2x1 -> 4x1` false-proof counterexample remains on
  host control.
- [x] The receipt classifier changes are committed as `40c3e9860` and
  `635afb555`. `make migration-parity-receipt-test` passes 24/24; the full
  selected denominator remains 10,952 and public parity results retain their
  original schema without internal error fields.
- [x] The D-049 thumbnail control-flow fix is committed as `dc6085f81`:
  the expanded degenerate probe is 0 mismatches and all 172 thumbnail parity
  cases remain exact.
- [x] The focused post-merge checks pass: `make build-dev`, Rust F-resize
  tests 9/9, GPU-pool tests 16/16, receipt tests 24/24, evidence/schema
  validation, and `make -C pillow-rs fmt`. Clippy remains blocked before
  compilation by the pre-existing pinned libavif 1.4.1/dav1d 1.5.3/libaom
  3.13.2 environment requirement.

## Closeout state

- [x] The source lane is committed as `4fe5535ff` and the committed-revision
  all-backend envelope is schema-valid. No fixture, expected value, threshold,
  denominator, or case ID was changed.
- [ ] Do not mark the overall goal complete while P0, P1, or P2 remains open.
