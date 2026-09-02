# Active parity/backend checklist — 2026-09-03 (focused)

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
- [x] Admit exact-identity nearest Perspective, Quad, and complete one-record
  Mesh transforms for L/LA/RGB/RGBA (`1c34fddd0`, source `d2690bf62`).
  Fractional, scaled, filtered, and broader projective/mesh maps remain on
  exact host semantic control pending a device arithmetic proof.
- [ ] Extend exact native-GPU F reducers beyond the proven finite marker-9 /
  signed two-axis envelope and the bounded marker-12 reducer. Marker 12 now
  covers direct Resize rows with at most eight taps and finite normal f32
  intermediates; the remaining inputs include rows over the eight-tap bound,
  mixed non-finite ordering, negative-zero cancellation, f64 subnormal or
  overflowing intermediates, and arithmetic-changing chains. Forced generic
  WGSL f32 convolution already differs from Pillow's ordered host arithmetic
  by ULPs. A fresh 20-row probe is exact on 18/20 CPU and GPU rows, but the
  deterministic failures are `F(16,1) -> (1,1)` and `F(32,1) -> (1,1)`
  Bilinear (Pillow words `c8be3d3d`/`baafc8bb`, Rust words
  `c9be3d3d`/`b9afc8bb`). Local arm64 Pillow disassembly confirms the source
  behavior changes at horizontal tap counts over 15 from scalar FMA to vector
  multiply followed by ordered adds; the current reducer models only the FMA
  envelope, so widening it without a separate >15-tap proof is unsafe.
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
- [x] Gate suite speed ratios on terminal requested=actual target receipts,
  matching latency samples, and empty fallback/error state (`1f49b7890`).
  Timing-complete rows without backend proof remain visible in independent
  coverage summaries but are explicitly `not_comparable` for speed claims.

### P2 — performance acceptance

- [ ] Obtain two consecutive fixed-ID, equal-receipt comparisons with zero
  budget violations. The Brightness factor-1 identity path and SIMD constant
  allocation plus the packed ExtractBand path are deterministic row-level
  wins, but aggregate comparisons are still timing-noisy and do not close this
  gate.

## Verified changes already integrated

- [x] Degenerate thumbnail control flow (`b0c154b33`, source `dc6085f81`):
  expanded native probe reduced 21 mismatches to 0; all 172 thumbnail cases
  are exact.
- [x] F thumbnail no-reduction GPU admission (`96aeeb2be`, source
  `b9a2d70e1`): focused heterogeneous five-filter case is exact and native on
  GPU; reducing F thumbnails remain host-controlled.
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
- [x] Bounded ordered-f64 F Resize (`5cbbe7ff2`, `9762c2af5`): marker 12
  emulates Pillow's per-tap f64 FMA rounding with an integer U128 reducer for
  direct finite-normal rows whose coefficient ranges have at most eight taps.
  The former host-controlled heterogeneous 3x1→2x1 Bilinear row and a native
  three-tap Lanczos row are exact; rows over the bound, special values, and
  chains remain explicitly host-controlled.
- [x] Fractional Rotate routing (`7ca91ed47`): exact normalized right angles
  alone use transpose fast paths; the fixed 576-case CPU/SIMD matrix is
  576/576 exact.
- [x] Source-aware Grayscale and terminal GPU identity (`932ac964e`): native
  mode matrix is 6/6 exact and `Grayscale(F) -> Invert` is byte-exact with
  requested=actual GPU after a host-controlled prefix.
- [x] Draw wide-line bottom-edge parity (`ee2996057`): Pillow's sentinel
  scanline behavior is restored; bounded CPU/GPU Draw matrix is 240/240 exact.
- [x] Identity projective nearest GPU routing (`1c34fddd0`, source
  `d2690bf62`): 12/12 native L/LA/RGB/RGBA Perspective/Quad/Mesh cases match
  CPU bytes and publish requested=actual GPU receipts; non-identity and
  filtered geometry remains exact host semantic control.
- [x] Receipt-aware suite aggregation (`1f49b7890`): a fresh 744-workload
  benchmark remains 744/744 measured, while suite comparisons move from
  276/324 status-only comparable cells to 180/324 receipt-proven cells;
  144 cells are explicit `not_comparable`. Artifact SHA-256:
  `b4b2438cfc19b48b740d676483bcd1f053f300f9bf324c1bd3d8073bb3dbffd4`.
- [x] Packed RGBA-family SIMD ExtractBand (`f35002e1c`): replace the
  per-block byte shuffle with explicit little-endian `u32x4` shift/mask while
  preserving exact bytes and vector/tail telemetry. Automatic getchannel
  parity is 128/128; the fixed equal-receipt row improved whole median
  0.166375→0.089396 ms and backend median 137084→60084 ns. Aggregate budget
  acceptance remains open because unrelated timing noise produced two
  violations.

## Bounded marker-12 evidence

- [x] The marker-12 candidate is exact on native GPU with requested=actual GPU,
  two dispatches, and no fallback. The host admission proof compares the
  integer ordered-FMA model with Pillow's direct f64 `mul_add` result before
  selecting the shader path.
- [x] A heterogeneous finite matrix of 4,270 direct F Resize cases (five
  filters, varied source/target sizes) had zero mismatches; 3,950 rows used
  native GPU and the remainder stayed on exact host semantic control. A
  1,175-case random finite-normal probe also had zero mismatches (428 native
  rows) on the two-tap implementation. After the eight-tap extension,
  a 2,000-case native GPU probe spanning Bilinear, Bicubic, Lanczos, Hamming,
  and Box rows had zero mismatches, including wider-tap rows. These probes do
  not close rows over the eight-tap bound, special-value, or chained arithmetic
  buckets above.
- [x] Fresh all-backends replay at `9762c2af5` passed 10,952/10,952
  value/error comparisons on CPU, SIMD, GPU, Node WASM, and browser WASM with
  zero failed or not-run cases. Terminal receipts remain explicit (CPU 6,838;
  SIMD 6,847 SIMD + 3 CPU controls; GPU 6,620 native + 218 host controls;
  WASM 6,951 each); the recorded status is `passed_with_backend_gaps` because
  those intentional host/control partitions are not relabeled as native
  coverage. Replay hash:
  `3515a246cc14e6cd2a271d611dc7f53133de852ae40b2b0b5525d44340cd727c`; GPU
  execution sidecar hash:
  `7ab888e2d5dd9c5f2ff9119d07668ae84fdce7e9e5d2899c2dd67733396fdf62`.

## Historical full-envelope evidence (`ee2996057`)

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

## Current integration state

`main` includes the parity fixes through `f35002e1c` (identity projective
nearest routing, receipt-proven suite cohorts, and packed SIMD ExtractBand).
The fresh combined replay at this revision is 10,952/10,952 value/error exact
with zero failed or not-run cases. It remains `passed_with_backend_gaps` only
because the explicit host-controlled partitions are still reported honestly:
CPU 6,838; SIMD 6,847 SIMD plus 3 CPU controls; GPU 6,707 GPU plus 131 CPU
controls; Node/browser WASM 6,951 each. Result SHA-256 is
`3db4e5c3543816325ab9ac3bea0e5d821c0cc23a25716386b78d3bafb6beb336`; the GPU
execution sidecar is `6d639b0ed60e191212f1975352231f9911880bd932ff1f8a0c2d489a445efbbe`.
The only open acceptance item in this focused list is the two-consecutive-run
zero-budget performance gate, plus the explicitly bounded F device arithmetic
and broader projective admission work above.
