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
- [x] Keep CPU projective BICUBIC distinct from BILINEAR
  (`9430e4ae8`, source `63fa09472`): the generic byte/PA path now uses
  Pillow Geometry.c's four-tap Horner/FMA filter for fractional Perspective
  maps. Native Pillow 12.2.0 versus Rust is 45/45 exact across L/RGB/PA;
  GPU admission remains restricted to the separately proven zero-weight
  relocation envelope.
- [x] Mirror Pillow's filtered CPU affine/projective byte semantics
  (`f08673da5`): LA/RGBA Perspective and Quad now use the native La/RGBa
  premultiplied round trip; affine coordinates and bilinear rows keep
  Geometry.c's fused and horizontal-first ordering; affine BICUBIC now has
  the four-tap clipped Horner path, including RGBX's non-alpha padding byte.
  Image.transform also preserves Pillow's exact errors for the known
  resize-only filters (LANCZOS, BOX, HAMMING) and unknown codes. Native
  Pillow 12.2.0 versus RSPIL probes are 2,400/2,400 affine and 1,920/1,920
  projective/Quad cases exact across L/LA/RGB/RGBA/RGBX/CMYK; the maintained
  migration parity replay is 10,952/10,952 exact. Filtered GPU alpha and
  broader arithmetic admissions remain on exact host semantic control.
- [x] Finish Mesh filtered (bilinear/bicubic) parity for L/LA/RGB/RGBA,
  translated/clipped boxes, and explicit premultiplied modes. The final CPU
  path is exact across the six byte layouts; SIMD nearest is proven and SIMD
  filtered remains exact CPU semantic control.
- [x] Preserve scalar FLOAT32 words for filtered Perspective, Quad, and Mesh
  transforms (`f36e1d1a7`). The CPU path mirrors Pillow Geometry.c's
  center/FMA map evaluation and FLOAT32 filter ordering, including overlapping
  mesh records and the omitted-versus-explicit `fillcolor` contract. Native
  Pillow 12.2.0 versus RSPIL is exact for the 60,000 finite/special projective
  probes, 10,000 overlapping Mesh probes, and fresh mixed typed/byte matrices;
  scalar and one-item tuple float fills are accepted only for mode F.
- [x] Keep fractional Rotate angles on Pillow's affine path and reproduce the
  clipped wide-line bottom sentinel in Draw. CPU/SIMD Rotate and CPU/GPU Draw
  now match the bounded native matrices exactly.
- [x] Preserve source-aware Grayscale conversion for scalar and packed modes,
  including the segmented GPU terminal receipt after an exact host-controlled
  prefix.
- [x] Admit proof-certified nearest projective/mesh relocations for packed
  L/LA/RGB/RGBA: exact-identity Perspective/Quad/complete one-record Mesh
  (`1c34fddd0`, source `d2690bf62`), constant-denominator integer Perspective
  translation/axis-swap (`4db4d4981`, source `9885acb6`), and full-output
  unit-scale Mesh translation/axis-swap (`413ed65ef`, source `10c9a49fd`).
  Fractional, scaled, nonconstant-denominator, and non-identity Quad/Mesh maps
  remain on exact host semantic control pending a device arithmetic proof.
- [x] Admit the separately proven filtered Perspective relocation envelope
  (`a826e1b8c`, source `bba3794bf`): ordinary packed L/RGB, direct or
  axis-swapped unit-scale maps with f32-exact integer translations, for
  Bilinear/Bicubic only. Native Pillow 12.2.0 versus GPU is 1,152/1,152 exact
  with terminal native receipts. The PA extension (`0082a900a`, source
  `b4df5d702`) proves the same zero-weight envelope for raw index/alpha pairs
  (120/120 exact, 12/12 native focused receipts). LA/RGBA alpha round trips,
  Mesh/Quad filters, fractional or scaled maps, and other filter arithmetic
  remain exact host semantic control.
- [x] Extend the exact native-GPU F reducer through the proven 256-tap
  marker-12 envelope (`c08dc378b`): direct finite-normal
  Resize rows model Pillow's
  arm64 split (scalar FMA through 15 horizontal taps, complete 16-tap product/
  ordered-add blocks, scalar FMA tail; vertical remains FMA). The deterministic
  scalar FMA tail; vertical remains FMA. The former 48-tap Lanczos cancellation
  divergence is now exact after matching Pillow's x/a-then-pi coefficient
  order. Native Pillow-vs-RSPIL direct F probes are 600/600 CPU and GPU exact,
  with 266 native GPU receipts; chained cases are 15/15 exact. Additional
  heterogeneous 65/96/128/192/256-tap rows, a two-axis row, and wide cancellation
  are exact with terminal native GPU receipts; a 257-tap row remains host-controlled.
  Rows over 256 taps and exceptional or arithmetic-changing inputs remain
  host-controlled.
- [x] Extend marker-9's IEEE special-value prepass beyond the finite 32-tap
  envelope (`9503aff04`): rows wider than 32 taps may use the existing exact
  reducer only when a special-product scan is present and its NaN/infinity
  bits match Pillow's ordered f64 result. Native 257-tap Bilinear/Bicubic/
  Lanczos/Hamming/Box cases (horizontal and vertical) are exact with terminal
  GPU receipts; finite wide rows remain on marker 12 or exact host semantic
  control.
- [x] Extend the ordered F reducer through the proven 1024-tap marker-12
  envelope (`b78014790`): the matched host/WGSL reducer can cover finite rows
  through 1024 taps when the ordered proof is representable, preserving
  Pillow's scalar-FMA and arm64 vector product/add ordering. Native Pillow
  12.2.0 matrices at 384, 512, 768, and 1024 taps are exact with terminal GPU
  receipts; the 1025-tap finite boundary remains exact host semantic control.
- [ ] Extend the exact F reducer beyond 1024 taps for finite/subnormal/overflow
  rows and arithmetic-changing chains. Forced generic WGSL f32 convolution
  still differs from Pillow's ordered host arithmetic by ULPs, so these rows
  require a separate device proof.
- [ ] Prove any broader arithmetic-changing projective/mesh/palette GPU domain.
  Constant-denominator integer Perspective nearest maps and full-output
  unit-scale Mesh direct/axis-swap relocations for packed L/LA/RGB/RGBA, plus
  direct/axis-swapped Quad and Mesh nearest pair relocations for PA, plus the
  narrow L/RGB/PA filtered Perspective relocation envelope, are now admitted
  by exhaustive or bounded native proofs. Fractional, scaled,
  nonconstant-denominator, partial/multi-record, and filtered projective maps,
  alpha/other-mode filtered rows, and palette arithmetic remain on exact host
  semantic control until their device arithmetic is proven.

### P1 — backend identity and receipts

- [ ] Reconcile native versus host-controlled partitions without relabeling
  outcomes. The latest full envelope is exact on all five public lanes, but GPU
  still has a host-controlled partition alongside native receipts. Preserve
  terminal actual-backend identity and make every fallback reason actionable.
  The stale `GPU does not support Transform` classification for the valid
  Perspective NaN-denominator fill edge is fixed by `8f440af60`: operation-only
  routing now defers Transform safety to image-aware preflight, which records
  `exact host semantic control` with terminal `actual_backend=cpu`. Broader
  native/host partition reconciliation remains open.
- [x] Correct mixed SIMD/CPU terminal identity (`ddcff735c`): receipts now
  report the final successful segment (SIMD 6,847; CPU 3) while preserving
  per-operation handoff telemetry and exact values.
- [x] Preserve nonterminal host-control receipt prefixes in WASM evidence
  accounting (`385eeaab1`).
- [x] Gate suite speed ratios on terminal requested=actual target receipts,
  matching latency samples, and empty fallback/error state (`1f49b7890`).
  Timing-complete rows without backend proof remain visible in independent
  coverage summaries but are explicitly `not_comparable` for speed claims.
- [x] Automatic SIMD layout-control receipt normalization (`8bb69acd0`):
  contextual SIMD-to-CPU handoffs for valid operations now publish an
  actionable `exact host semantic control: SIMD image-layout guard for ...`
  reason instead of an operation-level unsupported label. Strict explicit-SIMD
  capability errors remain unchanged. The full SIMD lane is 10,952/10,952
  exact with 6,847 native SIMD receipts, three terminal CPU controls, and no
  missing, partial, or indeterminate pipeline receipts.

### P2 — performance acceptance

- [ ] Obtain two consecutive fixed-ID, equal-receipt comparisons with zero
  budget violations. The Brightness factor-1 identity path and SIMD constant
  allocation plus the packed ExtractBand path are deterministic row-level
  wins, but the latest current-HEAD 11-ID pair still has nine
  timing-noise violations, so aggregate comparisons do not close this gate.
  Both runs measured 11/11 with 44/44 comparable records and 33/33 target
  receipts terminal with requested=actual and empty fallback reasons.

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
- [x] Typed-F projective filter parity and fill presence (`f36e1d1a7`):
  Perspective, Quad, and Mesh now operate on scalar FLOAT32 words instead of
  packed byte channels, mirror Pillow's center/FMA and bilinear/bicubic
  ordering, and preserve overlapping-mesh behavior for omitted versus
  explicit fills. Scalar and one-item tuple float fills match Pillow's mode-F
  contract; non-F float fills retain native type errors.
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
  direct finite-normal rows whose coefficient ranges have at most eight taps,
  then the arm64 wide-row extension (`31dfca10c`) models complete 16-tap
  product/add blocks through a 32-tap bound. The former host-controlled
  heterogeneous 3x1→2x1 Bilinear row, three-tap Lanczos row, and deterministic
  16/32-tap Bilinear rows are exact; rows over the bound, special values, and
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
- [x] Integer Perspective nearest GPU routing (`4db4d4981`, source
  `9885acb6`): proof-certified constant-denominator translation and axis-swap
  maps add 12/12 exact Pillow 12.2.0 versus CPU/GPU mode cases, with one
  native GPU dispatch and no fallback; the bounded Rust matrix is 24/24.
- [x] Mesh unit-relocation GPU routing (`413ed65ef`, source `10c9a49fd`):
  full-output one-record unit-scale direct/axis-swapped integer translations
  are admitted for L/LA/RGB/RGBA only after exhaustive source-selection proof.
  Native Pillow 12.2.0 versus RSPIL is 256/256 exact across four modes, four
  source sizes, four output shapes, and identity/positive/negative/axis-swap
  maps; every receipt is terminal requested=actual GPU with one dispatch and
  no fallback. Scaled, fractional, filtered, partial, and multi-record Mesh
  remains exact host semantic control.
- [x] Filtered Perspective relocation GPU routing (`a826e1b8c`, source
  `bba3794bf`): ordinary L/RGB direct or axis-swapped unit-scale maps with
  f32-exact integer translations are admitted for Bilinear/Bicubic after a
  bounded native proof (1,152/1,152 exact; 16/16 focused receipts native).
  Alpha modes, Mesh/Quad filters, and non-integral or scaled maps remain exact
  host semantic control; a clipped 1x1 Mesh relocation was rejected by the
  proof rather than admitted.
- [x] Palette-alpha Perspective nearest relocation (`683313494`, source
  `afc6e0eaf`): PA's native `(index, alpha)` transport is admitted only for
  direct or axis-swapped unit-scale integer Perspective maps. Native Pillow
  12.2.0 parity is 25/25 across varied/clipped cases with terminal GPU
  receipts; fractional, scaled, and filtered PA rows remain exact host
  semantic control.
- [x] Palette-alpha Quad/Mesh nearest pair relocation (`46c51e032`): PA's
  native `(index, alpha)` transport now admits direct and
  axis-swapped unit relocations for Quad and complete one-record Mesh after
  the shared source-selection proof. Native Pillow 12.2.0 probing is 40/40
  exact, with 37 native GPU receipts and three conservative host controls for
  non-square axis-swapped Quad dimensions. Ordinary packed bytes also admit
  the true direct Quad identity; filtered, scaled, partial, and multi-record
  geometry remains exact host semantic control.
- [x] Arm64 wide-row F Resize arithmetic (`31dfca10c`, source
  `68aa5472763`): CPU and marker-12 host/WGSL reducers mirror Pillow's
  horizontal >15-tap product/ordered-add split through a bounded 32-tap path,
  with a high-bit truncation guard for marker-6. The deterministic 16/32-tap
  Bilinear matrix improved 18/20→20/20; strict finite probes are 90/90 exact
  (87 native GPU, 3 host-control), and 6/6 special probes remain exact.
- [x] Wide F reducer admission guard (`f98859d07`, source
  `a77477179`): reject every coefficient row over 32 taps before marker-9
  exact-real reduction. A forced 48x1 Lanczos cancellation case that
  previously differed at the middle word now stays on exact host semantic
  control; the focused guard and full GPU tests pass.
- [x] Hamming near-zero kernel parity (`256c5a0b8`, source `e7647f692`):
  Pillow's `Resample.c` exact-zero branch is now mirrored in both pure-Rust
  F/I kernels, preserving near-zero sin/cos residuals. The bounded F matrix
  improved 4,188/4,200→4,200/4,200, with L/LA/RGB/RGBA and I matrices still
  100% exact.
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
- [x] Ordered F Resize reducer through 256 taps (`c08dc378b`): keep the
  host proof and both WGSL count guards aligned through 256 taps, covering
  heterogeneous finite Bilinear/Bicubic/Lanczos/Hamming/Box rows, a 65×65
  two-axis resize, and alternating wide cancellation. The 257-tap boundary
  remains exact host semantic control; mixed special/subnormal/overflow and
  arithmetic-changing chains remain pending. Focused GPU tests are 87/87 and
  Pillow 12.2.0 versus RSPIL CPU probes are 6/6 exact. No fixtures,
  thresholds, IDs, denominators, policy, or receipt taxonomy changed.

## Bounded marker-12 evidence

- [x] The marker-12 candidate is exact on native GPU with requested=actual GPU,
  two dispatches, and no fallback. The host admission proof compares the
  integer ordered-FMA model with Pillow's direct f64 `mul_add` result before
  selecting the shader path.
- [x] The arm64 wide-row extension preserves the same conservative proof:
  complete 16-tap horizontal blocks use rounded product/ordered-add state,
  scalar tails and all vertical rows use the FMA state, and special wide rows
  stay host-controlled. The focused wide-row native test and the full library
  suite pass; no rows over 32 taps or arithmetic-changing chains are admitted.
- [x] A heterogeneous finite matrix of 4,270 direct F Resize cases (five
  filters, varied source/target sizes) had zero mismatches; 3,950 rows used
  native GPU and the remainder stayed on exact host semantic control. A
  1,175-case random finite-normal probe also had zero mismatches (428 native
  rows) on the two-tap implementation. After the eight-tap extension,
  a 2,000-case native GPU probe spanning Bilinear, Bicubic, Lanczos, Hamming,
  and Box rows had zero mismatches, including wider-tap rows. The arm64
  16/32-tap regressions and 90-case finite matrix are also exact after
  `31dfca10c`. These probes do not close rows over the 32-tap bound,
  special-value, or chained arithmetic buckets above.
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
`2ab98459b5721d3a8b700d31bf7acf45dc2333ed8afdec6c7e2b14d6de6c9c75`, GPU
sidecar `382844e4457228047fb53a9522449ad341d549c14e299a58c86a4af7fafce1bb`,
WGSL coverage `3ec08641d0b6427a33b48ba982c90a9ea451c62bda134b6971019f8b316a591c`,
benchmark `180f1d80bf1d0d197ce4a76c02c490dbcfbc5570a6d78a4afe318bddcfc211b3`,
and benchmark parity `1f54eceae77d7d81f42fcce5868ae8b3bc23ce50ed54d8524b545f854c63d965`.

Known environment blocker: `make -C pillow-rs clippy` still requires the
pinned `libavif 1.4.1` / `dav1d 1.5.3` / `libaom 3.13.2` toolchain.

## Current performance evidence

- [x] Fresh current-HEAD equal-receipt pair (`59dcf26da`): both fixed-11 runs
  measured and passed 11/11 workloads, with 44/44 comparable records and
  33/33 target terminal requested=actual receipts with empty fallback reasons.
  The maintained checker reports nine bidirectional timing violations; the
  affected rows have unchanged operation/dispatch structure and no stable
  source regression. Run hashes are
  `c681dd91f4ce19108085857681902650846e06303c8aa1b8b7433b68f5ad61ec` and
  `ebb512b8b329c0fe91d6e1ed2309a31d615263df2f29cfbf20b0bcbfab12b71f`; the
  budget report hash is
  `604c6c08e92c3bd5377a7ca1d6bda1c92002e6cf19095a9723cc45da78eba87a`.
  This recheck leaves P2 open and did not modify benchmark scripts, fixtures,
  thresholds, IDs, denominators, policy, or receipt taxonomy.

## Current integration state

`main` includes the parity fixes through `f36e1d1a7` (typed-F filtered
projective/mesh words and fill presence, filtered Rotate, near-zero Hamming
F/I parity, PA projective relocation, wide-row F admission guard, filtered
Perspective relocation, arm64 wide-row F accumulation, unit-scale Mesh
relocation, integer Perspective nearest routing, identity projective routing,
receipt-proven suite cohorts, and packed SIMD ExtractBand).
The fresh combined replay at this revision is
10,952/10,952 value/error exact
with zero failed or not-run cases. It remains `passed_with_backend_gaps` only
because the explicit host-controlled partitions are still reported honestly:
CPU 6,838; SIMD 6,847 SIMD plus 3 CPU controls; GPU 6,741 GPU plus 97 CPU
controls; Node/browser WASM 6,951 each. Result SHA-256 is
`2ab98459b5721d3a8b700d31bf7acf45dc2333ed8afdec6c7e2b14d6de6c9c75`; the GPU
execution sidecar is `382844e4457228047fb53a9522449ad341d549c14e299a58c86a4af7fafce1bb`,
with WGSL coverage `3ec08641d0b6427a33b48ba982c90a9ea451c62bda134b6971019f8b316a591c`.
The only open acceptance item in this focused list is the two-consecutive-run
zero-budget performance gate, plus the explicitly bounded F device arithmetic
and broader projective admission work above.
