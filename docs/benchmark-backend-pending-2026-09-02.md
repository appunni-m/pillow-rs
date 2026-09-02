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

- [ ] Extend the proven F marker-9/signed two-axis reducers beyond their
  current finite, coefficient/value envelope. The remaining families are
  heterogeneous and
  non-dyadic values, mixed NaN/infinity ordering, negative-zero cancellation,
  wider Box ratios, and larger arithmetic domains. A forced generic WGSL
  f32-convolution diagnostic diverges from Pillow's ordered f64 path by ULPs;
  the current host-control guard is therefore required until a device-side
  f64-equivalent proof exists.
- [ ] Prove additional arithmetic-changing transform domains (fractional or
  non-identity projective/mesh geometry, palette transforms, and
  mixed-operation batches) before native admission. The bounded indexed
  identity/axis-swap subset is now native; keep the broader unproven rows on
  exact host semantic control.

### P1 — reconcile backend identity

- [ ] Reconcile the explicit fallback taxonomy and native identity claims.
  The current full envelope has zero partial, missing, or indeterminate
  pipeline receipts, but GPU still reports 6,744 native receipts and 94
  host-controlled receipts (30 exact host semantic-control, 62 unsafe-primary-
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
- [x] Signed two-axis F reduction admission: commit `a3d2c886b`. The
  integer-emulated path preserves ordered f64 accumulation and the f32
  horizontal boundary for the proven finite domain; broader heterogeneous
  and non-dyadic arithmetic remains queued.
- [x] Indexed `P`/`1` projective, quad, and one-record mesh nearest admission:
  commit `51b7070f7`. The bounded f32/f64 coordinate proof keeps exact
  identity/axis-swap mappings native with raw index bytes; fractional and
  arbitrary meshes remain host-controlled.
- [x] Palette-alpha (`PA`) affine-nearest pair relocation: commit `c7bb0a9a6`.
  The existing mode-1 transport preserves raw index/alpha pairs, so a bounded
  nearest/fixed-point proof now keeps this case native without palette
  expansion; broader palette arithmetic remains host-controlled.

### Arithmetic boundary retained

The two maintained `I;16*` filtered-resize rows remain exact host semantic
control. Native-vs-Pillow probes found ordered host `f64` accumulation landing
just below a half-integer while an exact integer reducer lands exactly on it;
the resulting 16-bit write differs. This is a real ordering/rounding boundary,
not a backend timing issue, so the current device admission guard stays in
place until ordered-`f64` behavior is reproduced on-device.

## Current evidence

- Full source revision: `c7bb0a9a6`.
- Full envelope: `build/migration-parity/all-backends-test-result.json`
  (SHA-256
  `df83aa837dad914ff07a46a71ffdd804a51fd1287b1f4b5527f6ce5305709e25`).
  CPU, SIMD, GPU, Node WASM, and browser WASM are each 10,952/10,952
  value/error exact; GPU smoke is 1/1. GPU has 6,744 native and 94
  host-controlled terminal receipts (6,838 complete), with zero partial,
  missing, or indeterminate pipeline receipts. Host-control partitions are
  30 exact semantic-control, 62 unsafe-primary-dimension, one
  unsafe/incomplete-dimension, and one Transform guard.
- GPU sidecar: `build/migration-parity/all-backends/parity-gpu-execution.json`
  (SHA-256
  `520854a8b0524022edf39a86350305e10adee838e5fac368e0fe3ebff77277c6`).
- Focused indexed projective envelope (archival replay; the incremental path is
  reused by later focused runs):
  `build/migration-parity/incremental/all-backends-test-result.json`
  (SHA-256
  `4d32646f78bbfe0c607855cd1cac9131fd08e92553303e92e717beb2eaa25d5f`).
  All five public lanes are 26/26 exact; GPU is native for all 26 with
  terminal receipts and no fallback. The focused GPU sidecar SHA-256 is
  `8043f145c5f26aee9c9c8393a6cab7a7a2c396b3156bdc22cc42a7a2bb9156e4`.
- Focused I/luma16 resize envelope (archival replay; the incremental path is
  reused by later focused runs):
  `build/migration-parity/incremental/all-backends-test-result.json`
  (SHA-256
  `55e8c825c3d78a7d020a0b25fa0c82a6e7bf0eacf9599a90893fe0e0f23e3976`).
  All five public lanes are 16/16 exact; GPU is native for 13/16, including
  all three maintained I convolution rows. The focused GPU sidecar SHA-256 is
  `658a8a75a24be13f55bd937a952162564e96baf16d4a1711500e8e2cfadbdde5`.
- Focused PA affine-nearest envelope (recorded immediately after the fix):
  `build/migration-parity/incremental/all-backends-test-result.json`
  (SHA-256
  `bc9bd8ba8eb6a659e6473200749a9f39fc35f6c29873d37d1bbe3699a3e9b4cd`).
  CPU, SIMD, GPU, Node WASM, and browser WASM are each 1/1 exact; GPU is
  actual GPU with one dispatch, a terminal receipt, and no fallback. Its GPU
  sidecar SHA-256 is
  `804c65ea3777e391986f8387a1e7ebb93df3312305ab308b8b020639c0c2bfde`.
  The incremental path is reusable and may be overwritten by a later focused
  run; these hashes are the evidence recorded for this replay.
- Verification: GPU pool tests 63/63; receipt-state tests 34/34;
  `make migration-parity-evidence-check`; the focused and full all-backend
  replays; `make build-dev`; `make -C pillow-rs fmt-fix`; and
  `make -C pillow-rs fmt`.
  Clippy remains blocked by the pre-existing pinned
  libavif 1.4.1/dav1d 1.5.3/libaom 3.13.2 environment requirement.

No fixtures, expected values, thresholds, IDs, denominators, public errors,
or receipt rules changed. Do not mark the overall goal complete while P0, P1,
or P2 remains open.
