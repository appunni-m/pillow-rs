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
- [x] Align F resampling arithmetic in commit `a83fb9244`. Pillow's Hamming
  cancellation row now uses the native `sincos`/fused-window ordering, SIMD
  F resize preserves observable negative-zero words, and the marker-9 host
  proof models the shader's unsigned four-limb reducer without admitting
  same-sign overflow. The focused 11-case F cohort is exact on CPU, SIMD,
  GPU, Node WASM, and browser WASM, with 11 terminal native-GPU receipts.
- [ ] Prove additional arithmetic-changing transform domains (fractional or
  non-identity projective/mesh geometry, palette transforms, and
  mixed-operation batches) before native admission. The bounded indexed
  identity/axis-swap subset is now native; keep the broader unproven rows on
  exact host semantic control.

### P1 — reconcile backend identity

- [ ] Reconcile the explicit fallback taxonomy and native identity claims.
  The current full envelope has zero partial, missing, or indeterminate
  pipeline receipts at `74ceca899`, but GPU still reports 6,746 native
  receipts and 92 host-controlled receipts (28 exact host semantic-control,
  62 unsafe-primary-dimension, one unsafe/incomplete-dimension, and one
  Transform capability guard).
  CPU has 6,838 terminal receipts (6,832 pipeline-complete); SIMD has 6,850
  (6,844 pipeline-complete). The maintained typed `I;16B` and `I;16N`
  filtered-resize rows replay exactly but remain host-controlled because the
  ordered-f64 versus device-integer boundary proof rejects native admission.
  The remaining host-controlled partitions need policy/identity reconciliation;
  no row may be relabeled to improve counts.
- [x] Receipt-history accounting now retains nonterminal host-control prefixes
  when building the WASM fallback taxonomy (commit `385eeaab1`). This closes
  the evidence-writer discrepancy without changing terminal backend identity;
  the aggregate GPU identity gap above remains open.

### P2 — performance acceptance

- [ ] Produce two consecutive equal-ID/equal-receipt cohort comparisons with
  zero budget violations. The fixed 11-ID cohort has 44 comparable pairings;
  eight fresh runs at `a83fb9244` report 9, 4, 5, 6, 5, 11, and 14
  violations for the seven adjacent comparisons. The
  factor-1.0 Brightness identity path is a deterministic row-level
  improvement (CPU medians about 0.181/0.163 ms before versus
  0.042/0.049/0.042 ms after). Commit `d9b5cec0a` also removes the redundant
  zero-fill/full-frame copy in `simd_constant`; the fixed-11 strict cohort
  stayed 11/11 exact with 44/44 requested=actual terminal receipts, and the
  paired SIMD row median improved from 0.418604 ms to 0.3965205 ms. Aggregate
  acceptance is still open.

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
- [x] F affine-nearest scalar-word relocation: commit `6203ec533`. A bounded
  per-destination proof compares Pillow's f64 coordinate/truncation source
  selection with the uploaded signed-16.16 walk before admitting the mode-8
  raw-word shader path. The maintained F EXTENT transform is native GPU with
  exact bytes; filtered F transforms and bilinear F rotate remain exact host
  semantic control because they interpolate scalar values.
- [x] F nearest rotate scalar-word relocation: commit `3ebf2cd5c`. The same
  fixed-point source-selection proof now admits raw F nearest rotations while
  preserving complete f32 words, including signed zero, NaN payload, infinity,
  and subnormal values. Filtered F rotations remain exact host semantic control
  because they interpolate scalar values rather than relocating words.
- [x] PA nearest rotate raw pair relocation: commit `74ceca899`. The bounded
  fixed-point nearest proof now admits palette-alpha index/alpha pairs on the
  native GPU path. The lowered rotate fill is normalized from the public
  `(gray, gray, gray, alpha)` tuple to Transform's `(gray, alpha, 0, 0)`
  contract, preserving custom fill alpha exactly; filtered PA chains remain
  exact host semantic control.
- [x] SIMD constant allocation pass: commit `d9b5cec0a`. `simd_constant`
  now allocates the final byte value directly, preserving output and vector
  telemetry while removing the redundant zero-fill and block-copy traversal.
  Strict SIMD parity is 1/1; this is a safe row-level improvement, not closure
  of the aggregate P2 timing gate.

### Arithmetic boundary retained

The two maintained `I;16*` filtered-resize rows remain exact host semantic
control. Native-vs-Pillow probes found ordered host `f64` accumulation landing
just below a half-integer while an exact integer reducer lands exactly on it;
the resulting 16-bit write differs. This is a real ordering/rounding boundary,
not a backend timing issue, so the current device admission guard stays in
place until ordered-`f64` behavior is reproduced on-device.

## Current evidence

- Full source revision: `74ceca899c5b943caa6397916ce5507dcd213a0d`.
- Full envelope: `build/migration-parity/all-backends-test-result.json`
  (SHA-256
  `c693587e96149b6e09e992ad5cff666387dced986c2a7db2d195ef9aa370b350`).
  CPU, SIMD, GPU, Node WASM, and browser WASM are each 10,952/10,952
  value/error exact; GPU smoke is 1/1. GPU has 6,746 native and 92
  host-controlled terminal receipts (6,838 complete), with zero partial,
  missing, or indeterminate pipeline receipts. Host-control partitions are
  28 exact semantic-control, 62 unsafe-primary-dimension, one
  unsafe/incomplete-dimension, and one Transform capability guard.
- GPU sidecar: `build/migration-parity/all-backends/parity-gpu-execution.json`
  (SHA-256
  `404a0671115c0bc82bf8b1e45a3e07e6559305dfdefeda20f698c76d51697d19`).
- Focused F nearest-rotate regression: `float_nearest_rotate_native_gpu_preserves_words`
  passes in the GPU pool suite (67/67), with exact CPU/GPU bytes and requested
  backend equal to actual GPU with no fallback. The regression includes signed
  zero, a NaN payload, infinity, and a subnormal word. The public full corpus
  contains no F nearest-rotate row, so this F closure does not itself change
  aggregate counts; the PA nearest-rotate closure changes the full envelope to
  6,746 native versus 92 host-controlled receipts.
- Focused PA nearest-rotate replay:
  `build/migration-parity/incremental/pa-nearest-all-backends-test-result.json`
  (SHA-256
  `b58d18f9919432088bd098d3f275fe95b4f32591acec089cd34d19c4ba2eb422`).
  Both PA cases are value/error-exact on CPU, SIMD, GPU, Node WASM, and browser
  WASM (2/2 each); GPU is native for the nearest-fill case and exact host
  semantic control for the filtered resize/rotate chain. The focused GPU
  execution sidecar SHA-256 is
  `2939b6d6166b010243e86b9828124ffeb2e50170e5255c0a1e0d3d68f1ebbf91`.
- Post-change strict SIMD constant parity: `build/migration-parity/simd-constant-
  strict-post.json` (SHA-256
  `ddb47c78ec218d35b6cc9ce83bde4091bcc6abfe16c5b816ca872ec3712235f7`),
  selected/executed/passed `1/1`.
- Focused F affine-nearest replay: `build/migration-parity/incremental/all-
  backends-test-result.json` (SHA-256
  `375828ecbd2dc091054ba1f691019b1983a0f052a46b6fbd9e6ff1a1c90725b5`).
  CPU, SIMD, GPU, Node WASM, and browser WASM are each 2/2 exact. GPU is
  native for the F EXTENT transform (one dispatch, no fallback) and remains
  exact host semantic control for the bilinear F rotate row. The focused GPU
  sidecar SHA-256 is
  `9366a58403f7400d172da70a15240eeac98ec7837d1977d640824a6a1207e744`.
- Focused F arithmetic replay after `a83fb9244`: the selected 11-case cohort
  is exact on CPU, SIMD, GPU, Node WASM, and browser WASM (11/11 each), with
  11/11 terminal native-GPU receipts and no fallback. The envelope SHA-256 is
  `c003d02c3b7e09624ed1840fa5ce59abe954ed6edc7a20d486854d0fe7f71c05`, and
  the GPU execution sidecar SHA-256 is
  `b2197b9afec17e9d32f19b4842a5ed8110052f53ef2e64119fea31f9f2b9b19f`.
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
- Verification: GPU pool tests 67/67; receipt-state tests 35/35; the focused
  11-case F replay at `a83fb9244` is 11/11 exact on every public lane with
  11/11 native-GPU receipts;
  `make migration-parity-receipt-test`; `make migration-parity-evidence-check`;
  the focused and full all-backend replays; `make build-dev`;
  `make -C pillow-rs fmt-fix`; and
  `make -C pillow-rs fmt`.
  `make -C pillow-rs clippy` remains blocked by the pre-existing pinned
  libavif 1.4.1/dav1d 1.5.3/libaom 3.13.2 environment requirement.

No fixtures, expected values, thresholds, IDs, denominators, public errors,
or receipt rules changed. Do not mark the overall goal complete while P0, P1,
or P2 remains open.
