# Active parity/backend checklist — 2026-09-03

This is the short, actionable queue. Historical probes and superseded runs
remain in the [exhaustive audit](benchmark-backend-exhaustive-audit-2026-08-30.md).

## Goal

Fix Pillow value and error parity first. Every public case remains executable
and exact; backend claims are accepted only when the receipt proves the backend
that ran. Exact host semantic control is a parity-preserving execution path,
not a closure claim. Keep fixtures, case IDs, thresholds, denominators, and
receipt rules unchanged.

## Pending work

### P0 — deterministic parity and native arithmetic

- [x] Correct varied CPU/SIMD Perspective and Quad sampling: destination
  centers, Pillow `COORD` truncation, filter edge clipping, and ordered
  interpolation are covered by the committed L/RGB regressions.
- [x] Finish Mesh filtered (bilinear/bicubic) parity for L/LA/RGB/RGBA,
  translated/clipped boxes, and explicit premultiplied modes. The final CPU
  path is exact across the six byte layouts; SIMD nearest is proven and SIMD
  filtered remains exact CPU semantic control.
- [x] Keep fractional Rotate angles on Pillow's affine path and reproduce the
  clipped wide-line bottom sentinel in Draw. CPU/SIMD Rotate and CPU/GPU Draw
  now match the bounded native matrices exactly.
- [x] Preserve source-aware Grayscale conversion for scalar and packed modes,
  including the segmented GPU terminal receipt after an exact host-controlled
  prefix.
- [ ] Extend exact native-GPU F reducers beyond the proven finite marker-9 /
  signed two-axis envelope and the new bounded marker-12 two-tap reducer.
  Marker 12 now covers direct Resize rows with at most two taps and finite
  normal f32 intermediates; the remaining inputs include heterogeneous and
  non-dyadic wider-tap values, mixed non-finite ordering, negative-zero
  cancellation, wider Box ratios, and arithmetic-changing chains. Forced
  generic WGSL f32 convolution already differs from Pillow's ordered host
  arithmetic by ULPs.
- [ ] Prove any broader arithmetic-changing projective/mesh/palette GPU domain.
  Until a device proof exists, the exact host path remains the required
  behavior for fractional and filtered geometry.

### P1 — backend identity and receipts

- [ ] Reconcile native versus host-controlled partitions without relabeling
  outcomes. The last full envelope is exact on all five public lanes, but GPU
  still has a host-controlled partition alongside native receipts. Preserve
  terminal actual-backend identity and make every fallback reason actionable.
- [x] Correct mixed SIMD/CPU terminal identity (`ddcff735c`): receipts now
  report the final successful segment (SIMD 6,847; CPU 3) while preserving
  per-operation handoff telemetry and exact values.
- [x] Preserve nonterminal host-control receipt prefixes in WASM evidence
  accounting (`385eeaab1`).

### P2 — performance acceptance

- [ ] Obtain two consecutive fixed-ID, equal-receipt comparisons with zero
  budget violations. The Brightness factor-1 identity path and SIMD constant
  allocation are deterministic row-level wins, but aggregate comparisons are
  still timing-noisy and do not close this gate.

## Verified changes already integrated

- [x] Degenerate thumbnail control flow (`dc6085f81`): expanded native probe
  reduced 21 mismatches to 0; all 172 thumbnail cases are exact.
- [x] F thumbnail no-reduction GPU admission (`b9a2d70e1`): focused
  heterogeneous five-filter case is exact and native on GPU; reducing F
  thumbnails remain host-controlled.
- [x] Perspective nearest CPU sampling (`ec546bc2a`): varied homography now
  matches Pillow's center evaluation and truncating `COORD` behavior.
- [x] Projective/Quad sampling and conservative GPU routing (`3320e2b22`):
  varied CPU/SIMD/GPU transform corpus is 130/130 per lane; fractional GPU
  transforms retain terminal CPU receipts with `exact host semantic control`.
- [x] Mesh filtered sampling and alpha/arithmetic ordering (`30ee05b29`,
  `1773f60b7`): CPU L/LA/RGB/RGBA bilinear/bicubic parity is exact, explicit
  RGBa/RGBX avoid double premultiplication, and compiled `Geometry.c` FMA/
  Horner order is preserved. SIMD filtered and fractional GPU paths retain
  exact host semantic control until device arithmetic is proven.
- [x] SIMD constant allocation (`d9b5cec0a`): removed the redundant full-frame
  zero-fill/copy pass with unchanged bytes and telemetry.
- [x] PA/F nearest relocation admissions and the bounded indexed projective
  proof remain exact within their documented envelopes.
- [x] CPU `ImageChops.constant` allocation (`2176ebfad`): construct the final
  L pixel directly, removing the redundant zero-fill/full-frame overwrite;
  focused parity is 11/11 exact and aggregate timing remains separately gated.
- [x] Heterogeneous F `ImageOps.pad` (`c5f03c6f3`): route the contain resize
  through exact f64-coefficient/f32-store semantics and admit only the proven
  marker-9 changed-axis GPU path (25/25 matrix exact; 23 native GPU).
- [x] Bounded ordered-f64 F Resize (`5cbbe7ff2`): marker 12 emulates Pillow's
  per-tap f64 FMA rounding with an integer U128 reducer for direct finite-normal
  rows whose coefficient ranges have at most two taps. The former host-
  controlled heterogeneous 3x1→2x1 Bilinear row is now native GPU and exact;
  wider/special/chain domains remain explicitly host-controlled.
- [x] Fractional Rotate routing (`7ca91ed47`): exact normalized right angles
  alone use transpose fast paths; the fixed 576-case CPU/SIMD matrix is
  576/576 exact.
- [x] Source-aware Grayscale and terminal GPU identity (`932ac964e`): native
  mode matrix is 6/6 exact and `Grayscale(F) -> Invert` is byte-exact with
  requested=actual GPU after a host-controlled prefix.
- [x] Draw wide-line bottom-edge parity (`ee2996057`): Pillow's sentinel
  scanline behavior is restored; bounded CPU/GPU Draw matrix is 240/240 exact.

## Bounded marker-12 evidence

- [x] The marker-12 candidate is exact on native GPU with requested=actual GPU,
  two dispatches, and no fallback. The host admission proof compares the
  integer ordered-FMA model with Pillow's direct f64 `mul_add` result before
  selecting the shader path.
- [x] A heterogeneous finite matrix of 4,270 direct F Resize cases (five
  filters, varied source/target sizes) had zero mismatches; 3,950 rows used
  native GPU and the remainder stayed on exact host semantic control. A
  1,175-case random finite-normal probe also had zero mismatches (428 native
  rows). These probes do not close the wider-tap, special-value, or chained
  arithmetic buckets above.
- [x] Fresh all-backends replay at `b7f2fadc9` passed 10,952/10,952 value/error
  comparisons on CPU, SIMD, GPU, Node WASM, and browser WASM with zero failed
  or not-run cases. Terminal receipts remain explicit (CPU 6,838; SIMD 6,847
  SIMD + 3 CPU controls; GPU 6,620 native + 218 host controls; WASM 6,951
  each); the recorded status is `passed_with_backend_gaps` because those
  intentional host/control partitions are not relabeled as native coverage.
  Replay hash: `49c0b07da8452284b454f23f26c43588af04e54f444308282bcd9fe4763a9f72`.

## Evidence refreshed at the final integrated revision (`ee2996057`)

- [x] Focused all-backends transform replay with terminal receipts (130/130
  source corpus; fractional GPU rows are host-controlled).
- [x] Full schema-v3 all-backends envelope at `ee2996057` and GPU SHA-256
  sidecar (10,952/10,952 exact on CPU, SIMD, GPU, Node WASM, and browser WASM;
  GPU native/host partitions remain explicit). Terminal receipts are CPU 6,838;
  SIMD 6,847 plus 3 exact Transform host controls; GPU 6,620 plus 218 explicit
  host controls; Node/browser WASM 6,951 each.
- [x] Standard benchmark and parity preflight at `ee2996057`: 744/744
  workloads measured, 0 not-run, 2,232/2,232 target subjects completed,
  202/202 parity cases exact, and zero budget comparison failures.
- [x] `make -C pillow-rs fmt`, `make build-dev`, focused Rust tests,
  `make migration-parity-receipt-test` (35/35), and
  `make migration-parity-evidence-check`.

Current evidence hashes: all-backends envelope
`2354185a8b4d2dbf12045a11d5904974c87e0d3d06868ecc85d3e2dea9a0abe7`, GPU
sidecar `3a1aad720667834e23980cab0e2f4da389333d17833f3c67da75287cbf08ecb0`,
benchmark `180f1d80bf1d0d197ce4a76c02c490dbcfbc5570a6d78a4afe318bddcfc211b3`,
and benchmark parity `1f54eceae77d7d81f42fcce5868ae8b3bc23ce50ed54d8524b545f854c63d965`.

Known environment blocker: `make -C pillow-rs clippy` still requires the
pinned `libavif 1.4.1` / `dav1d 1.5.3` / `libaom 3.13.2` toolchain.
