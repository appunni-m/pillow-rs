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
  pipeline receipts, but GPU still reports 6,717 native receipts and 121
  host-controlled receipts (57 exact host semantic-control, 62 unsafe-primary-
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
- [x] Typed `I` filtered-resize GPU admission: commit `b8cd50207`.
  Marker-11 keeps signed INT32/f64 two-pass rounding exact for the proven
  pure-resize domain; mixed and unproven arithmetic chains remain queued.

## Current evidence

- Full source revision: `b8cd50207`.
- Full envelope: `build/migration-parity/all-backends-test-result.json`
  (SHA-256
  `cd5bd577ebff9d92e1babd0b11a19a38559d7f408e35c0310a35827b0ca63965`).
  CPU, SIMD, GPU, Node WASM, and browser WASM are each 10,952/10,952
  value/error exact; GPU smoke is 1/1. GPU has 6,717 native and 121
  host-controlled terminal receipts (6,838 complete), with zero partial,
  missing, or indeterminate pipeline receipts.
- GPU sidecar: `build/migration-parity/all-backends/parity-gpu-execution.json`
  (SHA-256
  `b360f2efd75346c7d66ac5e5d35158156e25d75cd68f3319353d4043856fda30`).
- Focused I/luma16 resize envelope:
  `build/migration-parity/incremental/all-backends-test-result.json`
  (SHA-256
  `55e8c825c3d78a7d020a0b25fa0c82a6e7bf0eacf9599a90893fe0e0f23e3976`).
  All five public lanes are 16/16 exact; GPU is native for 13/16, including
  all three maintained I convolution rows. The focused GPU sidecar SHA-256 is
  `658a8a75a24be13f55bd937a952162564e96baf16d4a1711500e8e2cfadbdde5`.
- Verification: GPU pool tests 62/62; receipt-state tests 34/34;
  `make build-dev`; `make -C pillow-rs fmt-fix`; and `make -C pillow-rs fmt`.
  Clippy remains blocked by the pre-existing pinned
  libavif 1.4.1/dav1d 1.5.3/libaom 3.13.2 environment requirement.

No fixtures, expected values, thresholds, IDs, denominators, public errors,
or receipt rules changed. Do not mark the overall goal complete while P0, P1,
or P2 remains open.
