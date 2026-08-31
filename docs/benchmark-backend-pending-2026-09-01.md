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
- Finite heterogeneous F bicubic arithmetic now matches Pillow's Horner/FMA
  evaluation and coefficient rounding: the focused matrix is 9,000/9,000,
  with the maintained F resize slice at 23/23 after the correction.
- F-mode nearest resize now follows Pillow's cumulative f64 affine stepping
  and copies the opaque sample word on GPU, preserving boundary selection,
  NaN, infinity, and signed zero. The focused F parity slice is 13/13, and
  the native GPU regression covers finite and special-value 1x2→1x7 cases.
- RGBa `ImageOps.fit` now uses the existing exact boxed coefficient path on
  GPU. The strict Fit matrix is 89/89 value-exact, with the formerly excluded
  RGBa case included in the 6/6 native-receipt subset.
- `ImageDraw.rounded_rectangle` now rejects reversed boxes at the public Rust
  boundary in Pillow's x-first/y-second order. Previously a radius-zero box
  with only a reversed y axis reached the empty-span rectangle kernel and was
  silently accepted; the focused regression and Python facade probe now match
  Pillow's `ValueError` messages.
- The proof-gated dyadic F lane is exact/native for the admitted Bilinear,
  narrow two-tap Bicubic/Lanczos/Hamming, one- or two-axis power-of-two Box,
  and chained all-Box cases; every admission is bounded by fixed/f64 row
  agreement and the source significand-span check. Heterogeneous/non-dyadic
  inputs remain on exact host control.

## Pending — do these in order

### P0 — exact F-mode GPU resize arithmetic

- [x] Implement and prove the bounded dyadic subset: fixed/f64 coefficient
  agreement, same-sign normal power-of-two F words, Bilinear, narrow
  two-tap Bicubic/Lanczos/Hamming rows, one- or two-axis power-of-two Box
  reductions through 64:1, and chained all-Box passes (with the cumulative
  significand-span bound). The direct native matrix is byte-exact with
  terminal `actual_backend=gpu` receipts and no fallback (9 admitted cases).
- [x] Keep the finite nonconstant Box-upscale copy lane admitted for arbitrary
  non-downscaling geometry; its one-tap relocation proof is separate from
  arithmetic-filter admission.
- [ ] Extend native-GPU exact arithmetic coverage to heterogeneous/non-dyadic
  Bilinear, broader Bicubic/Lanczos/Hamming rows, Box downscales outside the
  proven dyadic row limits, and chains outside the cumulative
  significand-span proof. CPU bicubic parity is fixed; keep every unproven
  device arithmetic input on exact host control, including NaN, infinity,
  and negative zero.

### P1 — honest backend-proof denominator

- [x] The receipt sidecars now emit schema `pipeline-execution-evidence@2`
  with one status for every selected case: `complete`, `partial_receipt`,
  `missing_receipt`, `not_applicable`, or `indeterminate`. The summary keeps
  the historical no-receipt counts and adds a partition whose total remains
  the fixed **10,952** public cases. Only high-confidence non-pipeline paths
  leave the backend-proof cohort; missing, partial, and indeterminate paths
  remain proof gaps. The all-backend envelope stays schema-v3, and old @1
  sidecars are diagnostic-only until regenerated.
- [x] Regenerate and review the schema-v3 all-backend artifact at the
  committed source `5cc713f99`. CPU/GPU report **7,090 complete + 102
  partial + 0 missing + 3,327 not applicable + 433 indeterminate**; SIMD
  reports **7,102 complete + 102 partial + 0 missing + 3,315 not applicable +
  433 indeterminate**. All six public lanes remain 10,952/10,952 and GPU
  smoke is 1/1; the aggregate is correctly `passed_with_backend_gaps`.
  Artifact SHA-256: `93eba42234b785614daf7f8cc8651fd04731607de6934bb5f46a74c78e808672`.
- [x] Fix the receipt boundary and classifier gaps without changing IDs or
  denominators: an observed final serialization may prove an earlier
  dispatch, while eager filter constructors/`ModeFilter`, source-backed
  conversion/no-op paths, and source-independent degenerate or out-of-bounds
  crops are not deferred pipeline work. Canonical Stat/getbbox/getdata
  observations and public crop validation now remove fixture-only prefixes;
  the final run keeps CPU/GPU at 102 partial + 0 missing and SIMD at 102
  partial + 0 missing, with 433 indeterminate cases in each native lane.
  Receipt/evidence regression tests pass (19/19).
- [ ] Keep the aggregate `passed_with_backend_gaps` until every claimed native
  cohort has complete terminal receipts, matching case-ID digests, requested
  actual backends, and an empty fallback taxonomy.

### P2 — performance acceptance

- [x] Bound GPU working-buffer reuse to four times the requested capacity;
  the controlled small-draw case dropped from about 2.4 ms with a 6.3 MiB
  retained pool to about 0.59 ms with a 19 KiB pool, with exact/native output.
- [x] Elide the factor-1.0 Brightness scan for native byte layouts. The
  mode-guarded identity path is exact across nine byte modes and the focused
  Brightness parity lane remains 7/7; CPU medians improved from about
  0.181/0.163 ms to 0.042/0.049/0.042 ms.
- [ ] Run the same equal-ID, equal-receipt cohort twice consecutively with
  **zero** budget violations. The Brightness optimization is a deterministic
  row-level improvement, but the aggregate cohort remains noise-sensitive:
  the latest fixed-11-ID comparisons retain 44/44 comparable subjects and
  report 3, 4, and 6 violations across repeated baselines/posts. Timing
  acceptance remains open.

## Required closeout

- [x] Run the maintained focused lanes, full strict all-backend parity, receipt
  and evidence validators, and format/core-lint checks. Push the corresponding
  source and checklist commits only after the final artifact is validated.

Last verified source: `5cc713f99` (full all-backend run; working tree has
pre-existing unrelated changes). The overall goal is intentionally **active**.
