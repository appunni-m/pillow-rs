# Active parity/backend checklist — 2026-09-02

This is the focused queue. Historical investigations remain in the
[exhaustive audit](benchmark-backend-exhaustive-audit-2026-08-30.md) and the
[previous checklist](benchmark-backend-pending-2026-09-01.md).

## Goal

Fix Pillow value/error parity first. Then prove the backend that executed each
pipeline. Accept performance only from equal-ID, equal-receipt comparisons.
Keep every case, fixture, denominator, and threshold unchanged. Host semantic
control is a parity-preserving execution path, not a claim that the native
backend is complete.

## Pending — only these three buckets

### P0 — broaden exact native-GPU arithmetic

- [ ] Extend the proven F marker-9 reducer beyond its current finite,
  coefficient/value envelope. The remaining families are heterogeneous and
  non-dyadic values, mixed NaN/infinity ordering, negative-zero cancellation,
  wider Box ratios, and larger arithmetic domains. A forced generic WGSL
  f32-convolution diagnostic diverges from Pillow's ordered f64 path by ULPs;
  the current host-control guard is therefore required until a device-side
  f64-equivalent proof exists.
- [ ] Prove additional arithmetic-changing transform domains (projective,
  mesh, palette, and mixed-operation batches) before native admission. Keep
  unproven rows on exact host semantic control.

### P1 — reconcile backend identity

- [ ] Reconcile the explicit fallback taxonomy and native identity claims.
  The current full envelope has zero partial, missing, or indeterminate
  pipeline receipts, but GPU still reports 6,713 native receipts and 125
  host-controlled receipts (61 exact host semantic-control, 62 unsafe-primary-
  dimension, one unsafe/incomplete-dimension, and one Transform guard).
  CPU has 6,838 terminal receipts (6,832 pipeline-complete); SIMD has 6,850
  (6,844 pipeline-complete). The remaining host-controlled partitions need
  policy/identity reconciliation; no row may be relabeled to improve counts.

### P2 — performance acceptance

- [ ] Produce two consecutive equal-ID/equal-receipt cohort comparisons with
  zero budget violations. The fixed 11-ID cohort has 44 comparable pairings;
  the latest consecutive checks report 11, 7, and 6 violations. The
  factor-1.0 Brightness identity path is a deterministic row-level
  improvement (CPU medians about 0.181/0.163 ms before versus
  0.042/0.049/0.042 ms after), but aggregate acceptance is still open.

## Recently closed in this queue

- [x] D-049 degenerate thumbnail control flow: commit `dc6085f81`, expanded
  probe 0 mismatches, all 172 thumbnail cases exact.
- [x] Typed `I;16*` affine-nearest GPU admission: commit `614d4cd90`.
- [x] Typed `I;16N` filtered-resize GPU admission: commit `cdce9b98c`.
  Native upload/readback now follows Pillow's declared byte order.

## Current evidence

- Full source revision: `cdce9b98c`.
- Full envelope: `build/migration-parity/all-backends-test-result.json`
  (SHA-256
  `69863881a1dbb193da6be48ea6e39c0b4b49de8a8df83ab003116251ccd251e1`).
  CPU, SIMD, GPU, Node WASM, and browser WASM are each 10,952/10,952
  value/error exact; GPU smoke is 1/1.
- GPU sidecar: `build/migration-parity/all-backends/parity-gpu-execution.json`
  (SHA-256
  `56c2e16af00df0facb646156a53533e0f95a478bc44d31b02855d43d78bc1990`).
- Focused `i16n-frombytes-bilinear` envelope:
  `build/migration-parity/incremental/all-backends-test-result.json`
  (SHA-256
  `e2626ad621d7b893c91761cac2dca2d1bd29d7008d5fdfc8f77ec12fdf6dd984`).
  All five public lanes are 1/1 exact; GPU is native with no fallback. The
  focused GPU sidecar SHA-256 is
  `005f91deaba074c4019ad0f6cae726c6173e346de0f632bf6dc89561d0537f15`.
- Verification: GPU pool tests 57/57; receipt-state tests 34/34;
  `make build-dev`; `make -C pillow-rs fmt-fix`; and `make -C pillow-rs fmt`.
  Clippy remains blocked by the pre-existing pinned
  libavif 1.4.1/dav1d 1.5.3/libaom 3.13.2 environment requirement.

No fixtures, expected values, thresholds, IDs, denominators, public errors,
or receipt rules changed. Do not mark the overall goal complete while P0, P1,
or P2 remains open.
