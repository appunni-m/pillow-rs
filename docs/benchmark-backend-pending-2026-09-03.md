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
  translation/axis-swap (`4db4d4981`, source `9885acb6`), general constant-
  denominator integer Perspective scale/shear/reflection/translation
  (`a735a563f`, source `51a5e110`), and full-output
  unit-scale Mesh translation/axis-swap (`413ed65ef`, source `10c9a49fd`),
  plus proof-certified f32-representable fractional Perspective and constant
  Quad/Mesh/PA maps (`549bc3e08`, source `51a5e110`), and proof-certified
  nonconstant-denominator Perspective nearest maps (`00696f1fb`, source
  `6f2a50886`). Fractional boundaries outside the exhaustive source-selection
  proof, nonconstant-denominator maps without that proof, filtered maps, and
  broader Quad/Mesh records remain on exact host semantic control pending a
  device arithmetic proof.
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
- [x] Extend the ordered F reducer through the proven 32768-tap marker-12
  envelope (`5eb257096`, source `e975f6831`) while retaining Pillow's
  vertical-first FLOAT32 intermediate for very tall F resizes. Native Pillow
  12.2.0 versus RSPIL direct boundary probes are 250/250 CPU and 250/250 GPU
  exact across 16385/16386/32768/32769 rows, two-axis shapes, all five
  non-nearest filters, finite/signed-zero/subnormal/largest-finite/special
  words; 180 rows through 32768 use native GPU receipts and 70 rows at 32769
  remain exact host semantic control. Filtered/nearest two- and three-stage
  chain probes are 36/36 exact on CPU and GPU. Focused ordered-F tests are
  10/10, the full pool-GPU group is 94/94, and CPU geometry tests are 12/12.
- [x] Extend the ordered F reducer through the proven 131072-tap marker-12
  envelope (`cc780e7b9`, source `8be644570`). The host proof and both WGSL
  guards now cover direct finite rows through 131072 taps while preserving
  Pillow 12.2.0's ordered f64/FMA and FLOAT32-store semantics. Native direct
  probes are 180/180 exact; 131073 remains terminal exact host semantic
  control. Focused ordered-F tests are 10/10 and the release/build-dev/format
  gates pass.
- [x] Extend the ordered F reducer through the proven 262144-tap marker-12
  envelope (`33535ab5d`, source `526515c81`). The host proof and both WGSL
  guards now cover direct rows through 262144 taps while preserving Pillow's
  ordered f64/FMA and FLOAT32-store semantics. Native Pillow 12.2.0 versus
  RSPIL direct probes are 160/160 exact across 262144/262145 boundary shapes,
  all five non-nearest filters, and finite/special words; 100 rows at 262144
  use native GPU receipts, 35 remain exact host semantic control, and at
  262145 the marker-9/host split is 20/20. Arithmetic-changing chains are
  100/100 exact without widening chain admission. Focused ordered-F tests are
  38/38 and the pool-GPU group is 98/98.
- [x] Extend the ordered F reducer through the proven 524288-tap marker-12
  envelope (`f77fbbc29`, source `cbfda102a`). The host proof and both WGSL
  guards now cover direct rows through 524288 taps while preserving Pillow's
  ordered f64/FMA and FLOAT32-store semantics. Native Pillow 12.2.0 versus
  RSPIL direct probes are 160/160 exact across 524288/524289 boundary shapes,
  all five non-nearest filters, and finite/special words; 125 rows use native
  GPU receipts and 35 remain exact host semantic control. Arbitrary
  finite/extreme/cancellation probes are 45/45 exact, and arithmetic-changing
  chains are 175/175 exact without widening chain admission. Focused
  ordered-F tests are 38/38 and pool-GPU tests are 99/99.
- [x] Extend the ordered F reducer through the proven 1048576-tap marker-12
  envelope (`5bc7f0786`, source `722b90638`). The host proof and both WGSL
  guards now cover direct finite rows through 1048576 taps while preserving
  Pillow's ordered f64/FMA and FLOAT32-store semantics. Native Pillow 12.2.0
  differential probes are 20/20 exact across 524289 and 1048576 one- and
  two-axis rows, all five non-nearest filters, and an over-bound 1048577 row;
  the 1048577 rows remain exact host semantic control. Focused ordered-F
  tests are 11/11 and the full pool-GPU group is 101/101.
- [x] Extend the ordered F reducer through the proven 4194304-tap marker-12
  envelope (`acefea1ce`, source `e90ce5355`). The host proof and both WGSL
  guards now cover direct finite rows through 4194304 taps while preserving
  Pillow's ordered f64/FMA and FLOAT32-store semantics. Native Pillow 12.2.0
  probes are 10/10 finite at four million taps, 20/20 special-value rows, and
  5/5 over-bound host-control rows; focused ordered-F tests are 11/11 and the
  full pool-GPU group is 104/104.
- [ ] Extend the exact F reducer beyond the currently proven domains,
  including f64-intermediate subnormal/overflow boundaries, vertical over-cap
  rows, non-Box filters, and arithmetic-changing chains. Forced generic WGSL
  f32 convolution still differs from Pillow's ordered host arithmetic by ULPs,
  so these rows require a separate device proof. The adapter-fitting marker-12
  envelope reaches 8,388,607 taps; the marker-13 compact repeated-coefficient
  path now covers only finite direct horizontal Box rows above that bound.
  Commit `8c92e95d8` closes the binding-size edge for the full table, while
  `d513cfa13` proves the compact one-pixel Box row without changing the ABI.
  Native subnormal/max-finite/special, vertical/tall, and adapter-buffer-limit
  rows remain exact host semantic control; the broader reducer bucket stays
  open.
- [x] Extend the filtered Quad/Mesh relocation proof (`cfa3b2690`, source
  `206bff9dfe82ab9eab5346931db2ddd0b11f4388`): correct non-square Quad
  axis-swap source extents, reject extra Mesh records, and admit only the
  exhaustive direct/axis-swapped filtered relocation envelope for ordinary
  packed L/RGB and palette-alpha pairs. Native Pillow 12.2.0 probes are
  16/16 exact for L/RGB and 8/8 exact for PA with terminal GPU receipts;
  filtered/scaled/partial/extra-record/fractional arithmetic outside the proof
  remains exact host semantic control.
- [x] Admit proof-certified constant integer nearest Quad/Mesh maps
  (`8b13f0c9b`, source `bff7976fa`) for packed byte modes and PA. Native
  Pillow 12.2.0 versus RSPIL is 180/180 exact with native GPU receipts,
  including 90/90 fill-boundary cases; interpolation is bypassed only after
  the exhaustive source-selection proof. Filtered, fractional, scaled,
  nonconstant-denominator maps without the source-selection proof,
  nonzero-weight, partial/multi-record maps and broader palette arithmetic
  remain exact host semantic control.
- [ ] Prove any broader arithmetic-changing projective/mesh/palette GPU domain.
  Constant-denominator integer Perspective nearest maps (including the new
  scale/shear/reflection envelope) and full-output
  unit-scale Mesh direct/axis-swap relocations for packed L/LA/RGB/RGBA, plus
  direct/axis-swapped Quad and Mesh nearest pair relocations for PA, the
  constant-integer nearest Quad/Mesh subfamily, signed unit-axis Perspective
  nearest relocations (including PA), the narrow
  L/LA/RGB/RGBA/PA filtered direct/axis relocation envelope, and raw
  interior-integer Bilinear/Bicubic constant-map envelopes are now admitted by
  exhaustive or bounded native proofs. Fractional boundaries outside the
  source-selection proof, scaled maps outside the integer proof,
  nonconstant-denominator maps without a source-selection proof, nonzero-weight,
  clipped/multi-record, filtered projective maps, other modes, and broader
  palette arithmetic remain on exact host semantic control until their device
  arithmetic is proven.

- [x] Admit exact partial unit-scale Mesh relocations
  (`c722e47b7`, source `c80f8fa9b`): integer in-output Mesh bboxes with direct
  or axis-swapped relocation are now admitted only after the exhaustive local
  bbox/source-selection proof, with explicit fills required for partial
  records. Native Pillow 12.2.0 differentials are 648/648, randomized
  3,240/3,240, and negative-translation 60/60 exact; ordinary byte tests are
  48/48 and P/PA palette-pair tests 12/12 with terminal native GPU receipts.
  Fractional, scaled, clipped, multi-record, and arithmetic-changing Mesh
  cases remain exact host semantic control.

- [x] Admit translated Quad relocations (`37baf748d`, source `e615a2e5d`):
  integral f32-exact source origins with direct or axis-swapped unit
  relocation now use the existing exhaustive source-selection proof and
  mirror Pillow Geometry.c's centered coordinate/FMA order. Native Pillow
  12.2.0 differentials are 288/288 ordinary L/LA/RGB/RGBA, 16/16 P, and
  16/16 PA exact with terminal native GPU receipts. Fractional, scaled,
  nonzero-weight, and broader Quad/Mesh arithmetic remains exact host semantic
  control.

- [x] Extend filtered projective relocation to LA/RGBA and Quad/Mesh
  (`a0fb33394`): the WGSL path now mirrors Pillow's premultiplied La/RGBa
  round trip for integral source samples and lowers non-square direct/axis
  Quad relocations to exact integer coordinates. Native Pillow 12.2.0 versus
  RSPIL probes are 224/224 exact across LA/RGBA Perspective, Quad, and Mesh
  cases with varied alpha/fill values; the focused pool-GPU group is 93/93,
  and fractional/scaled/non-dyadic maps remain exact host semantic control.
- [x] Admit signed unit-axis Perspective nearest relocations
  (`8caddc219`, source `423ebf445`): the WGSL path mirrors Pillow's
  center-plus-`COORD` truncation for reflected and axis-swapped unit maps,
  including palette-alpha index/alpha pairs. Native Pillow 12.2.0 versus
  RSPIL probes are 4,392/4,392 exact with terminal actual=GPU receipts,
  one dispatch, and no fallback. Fractional boundaries outside the proof,
  scaled maps outside the integer proof, nonconstant-denominator maps without
  the source-selection proof, filtered, partial, and multi-record maps remain
  exact host semantic control.
- [x] Admit constant-denominator integer Perspective nearest relocations
  (`a735a563f`, source `51a5e110`): the WGSL path mirrors centered destination
  coordinates and Pillow `COORD` truncation for proof-certified integer
  scale/shear/reflection/translation maps with `g=h=0`. Native Pillow 12.2.0
  versus RSPIL probes are 160/160 exact with terminal requested=actual GPU
  receipts; fractional, nonconstant-denominator maps without the proof,
  filtered, and arithmetic-changing maps remain exact host semantic control.
- [x] Admit constant half-pixel filtered projective maps
  (`730d6f5ee`, source `18df688e1`): f32-exact constant `n + 0.5` source
  coordinates for Bilinear/Bicubic Perspective, Quad, and complete one-record
  Mesh now lower to the integral sample reached after Pillow Geometry.c's
  filtered `-0.5` center shift. Native Pillow 12.2.0 differentials are
  480/480 exact on CPU and GPU across L/LA/RGB/RGBA/PA, with 240/240 terminal
  native GPU receipts; quarter-pixel, scaled/nonconstant, partial/multi-record,
  and other arithmetic-changing maps remain exact host semantic control.

- [x] Admit the interior-integer Bilinear projective envelope
  (`30e5aed11`, source `bd4bad16c`): for L/RGB/PA only, constant f32-exact
  integer source coordinates strictly inside the source bounds now mirror
  Pillow Geometry.c's filtered `-0.5` shift, four-neighbor sampling, and
  truncating byte conversion for Perspective, Quad, and complete one-record
  Mesh. Native Pillow 12.2.0 differentials are 1,620/1,620 exact across
  ordinary matrices and 7,203/7,203 exhaustive 2x2 value cases per mode;
  every admitted row has a terminal native GPU receipt. Edges, LA/RGBA
  premultiplied paths, Bicubic, fractional/scaled maps, and partial or
  multi-record geometry remain exact host semantic control.

- [x] Extend the interior-integer Bilinear proof to raw packed modes
  (`23b920fa1`, source `4242808f4`): CMYK, HSV, YCbCr, RGBX, and RGBa now use
  the same Geometry.c operation-order proof after their native physical
  channel layouts are checked. Native Pillow 12.2.0 versus RSPIL is exact for
  36,015 exhaustive 2x2 cases (seven-value tiles across five modes and three
  projective methods) plus 1,080 varied-size/coordinate cases, all with
  terminal requested=actual GPU receipts. Edge, fractional, and non-Bilinear
  cases remain exact host semantic control.
- [x] Add the interior-integer Bicubic projective proof for raw packed modes
  (`be74d45e9`, source `5cff60ee0`): Perspective, Quad, and complete one-record
  Mesh maps now use Pillow Geometry.c's `[-1,5,5,-1]/8` half-pixel weights in
  an integer `/64` shader path for L/RGB/CMYK/HSV/YCbCr/RGBX/RGBa. Native
  Pillow 12.2.0 differentials are 2,016/2,016 exact with terminal native GPU
  receipts; PA focused cases are 48/48 and rejected edge/fractional/LA/RGBA
  cases are 81/81 exact on host control. Scaled, nonconstant, clipped,
  partial, multi-record, and other arithmetic-changing maps remain host
  controlled.
- [x] Add the compact over-limit horizontal F Box proof (`d513cfa13`, source
  `547ccba56703d87240cd0d7815c22f17a9384585`): finite direct mode-F one-pixel
  horizontal Box rows above 8,388,607 taps transport one repeated f64
  coefficient, preserving ordered accumulation and FLOAT32 storage. Native
  Pillow 12.2.0 versus RSPIL is 3/3 exact with actual GPU receipts and no
  fallback; vertical/tall, non-finite/extreme, and adapter-limit rows remain
  exact host semantic control.

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
- [x] Preserve an observed completed-prefix terminal candidate when a later
  partial receipt arrives (`058b5e48b`, source `5a65691c9`). A later error
  receipt can carry useful attempt telemetry, but it must not replace the
  earlier meaningful `completed`/`cached` receipt or terminalize the partial
  record. The regression suite is 40/40 and the full 10,952-case replay has
  zero missing, partial, or indeterminate pipeline receipts; broader native/
  host partition reconciliation remains open.
- [x] Gate suite speed ratios on terminal requested=actual target receipts,
  matching latency samples, and empty fallback/error state (`1f49b7890`).
  Timing-complete rows without backend proof remain visible in independent
  coverage summaries but are explicitly `not_comparable` for speed claims.
- [x] Automatic SIMD layout-control receipt normalization (`8bb69acd0`):
  contextual SIMD-to-CPU handoffs for valid operations now publish an
  actionable `exact host semantic control: SIMD image-layout guard for ...`
  reason instead of an operation-level capability label. Strict explicit-SIMD
  capability errors remain unchanged. The full SIMD lane is 10,952/10,952
  exact with 6,847 native SIMD receipts, three terminal CPU controls, and no
  missing, partial, or indeterminate pipeline receipts.
  A next19 sidecar audit of the current replay independently revalidated all
  10,952 IDs: complete=6,832, not_applicable=4,120, and
  missing/partial/indeterminate=0. Terminal identity remains internally
  consistent (CPU 6,838 native; SIMD 6,847 native plus three explicit CPU
  layout controls; GPU 6,744 native plus 94 explicit CPU controls), with no
  empty-fallback host receipt or fallback on a native receipt. No actionable
  partition defect was found without relabeling intentional host controls.

### P2 — performance acceptance

- [ ] Obtain two consecutive fixed-ID, equal-receipt comparisons with zero
  budget violations. The Brightness factor-1 identity path and SIMD constant
  allocation plus the packed ExtractBand path are deterministic row-level
  wins, but an earlier current-HEAD pair still had seven timing-noise
  violations, so aggregate comparisons do not close this gate. Both runs
  measured 11/11 with 44/44 comparable records and 33/33 target receipts
  terminal with requested=actual and empty fallback reasons; a 40-sample GPU
  profile ranged 0.589083–42.261666 ms with stable 4 operations/9 dispatches,
  confirming timing bimodality rather than a source regression.
  A subsequent pair at the same source revision retained 11/11 workloads,
  44/44 comparable records, and 33/33 terminal requested=actual target
  receipts per run, but reported 23 timing violations with identical
  operation/resource fingerprints. The latest replay pair reduced that
  count to 20 while retaining the same 11/11, 44/44, and 33/33 receipt
  invariants and identical operation/dispatch/cache/resource/backend
  fingerprints. A fresh next17 pair on the same source revision again has
  11/11 workloads, 44/44 comparable records, and 33/33 terminal requested=actual
  target receipts per run with empty fallback/error state, but still reports
  six timing violations; all 44 execution fingerprints are identical after
  removing timing fields. The next18 campaign repeated the same 11/11,
  44/44, and 33/33 receipt invariants in two consecutive pairs; the first
  pair had 11 violations and the second six, with normalized execution
  fingerprint
  `7f443376fd0e6c5e65032b8df84e92bc5f16c5e34783f96bc6e8d807365e4c32`
  unchanged across all four runs. Violations moved among Pillow/CPU/SIMD/GPU
  rows and remained timing-only. P2 therefore remains timing noise rather than
  a receipt defect. A fresh next18b campaign at the current integrated
  revision repeated the same 11/11, 44/44, and 33/33 receipt invariants in
  two consecutive pairs; the pairs reported 10 and 15 violations, respectively,
  with the normalized execution fingerprint unchanged. Violations again moved
  across Pillow/CPU/SIMD/GPU rows, so no source or receipt fix is justified.
  A next19 four-run campaign at the same revision retained 11/11 selected and
  measured workloads, 44/44 comparable records, and 33/33 terminal
  requested=actual target receipts per run. All four normalized execution
  fingerprints remained
  `7f443376fd0e6c5e65032b8df84e92bc5f16c5e34783f96bc6e8d807365e4c32`; adjacent
  pairs reported 4, 12, and 6 violations, with varying row sets and no receipt
  or source divergence. The zero-violation gate remains open pending a pair at
  the newest integrated source revision.
  A fresh next20 pair at integrated revision
  `730d6f5ee4ef2fdf5fe2d84f8ea288fdfdc3de3b` again selected/measured 11/11
  workloads with 44/44 comparable records and 33/33 terminal
  requested=actual target receipts in each run. The normalized execution
  fingerprint stayed
  `7f443376fd0e6c5e65032b8df84e92bc5f16c5e34783f96bc6e8d807365e4c32`; run
  SHAs were `4e95c713932320ad30b06a8724387517354f2512633a9583e0e173dcea10dbf4`
  and `25c0a24c9b915eecda38761b7adc68d31ad1c732108f0b45fcb078678d7f5fee`.
  The budget artifact
  `3735f15f0f1ecab2f36f4bdc14a3d7fb5e76db74e15d732628c274772cef28b0`
  reported 11 timing-only violations; P2 remains open.
  A fresh next21 pair at pushed revision `27d7bda79ad08a3254a7276df2c9b400563dec9d`
  again measured 11/11 workloads with 44/44 comparable records and 33/33
  terminal requested=actual target receipts per run. Run SHAs were
  `e5c9f26a079aae3e306f118146910504efb58ecc8017170b379a40446e2c52ac` and
  `74246220d8ee446c3dbf8fbf1486c998de9dcd8fa20d8300ed2eb2edede9749f`; the
  budget artifact SHA was
  `c3d54d53cb18c22d88094e0e3e12de8cd6e61eb4cfefa4e337ea12351239037a` and it
  reported 11 timing-only violations. The normalized execution fingerprint
  remained unchanged, so the zero-violation gate is still open.
  A fresh next22 pair at pushed revision `d8cd0420483c7f73a2e359a8e4660820acfbd47e`
  again selected and measured 11/11 workloads with 44/44 comparable records
  and 33/33 terminal requested=actual target receipts in each run. Run SHAs
  were `be787c6518a62b9e7b28a27fe12119376220256532c35a990eec192a35231f9c` and
  `1e506e9d3362eb488ff2a405090a0ba5bfc112ef62d7114e2b069e091f21a8b9`; the
  budget artifact SHA was
  `07bfafa84b4ddd10ef40ce3e9db1b5db4e0d8a46adac51ffbd388b01d28e9f0a` and it
  reported 19 timing-only violations. The normalized fingerprint stayed
  unchanged; P2 remains open.

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
- [x] Proof-certified Quad/Mesh nearest GPU routing (`ecac88ac1`, source
  `de38b9cd7`): ordinary packed L/LA/RGB/RGBA direct and translated Quad plus
  complete one-record Mesh maps now use the exhaustive source-selection and
  finite-coordinate guards. Native Pillow 12.2.0 versus RSPIL is 24/24 exact;
  two bounded varied/f32-boundary sweeps are 20,000/20,000 exact, with 3,517
  native GPU receipts and the remainder exact host semantic control. Filtered,
  partial, multi-record, and proof-failing arithmetic remains host-controlled.
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

- [x] Ordered F Resize reducer through 32768 taps (`5eb257096`, source
  `e975f6831`): the host proof and both WGSL guards now cover the verified
  32768-tap envelope. Native direct CPU/GPU probes are 250/250 exact each,
  resize chains are 36/36 exact, and rows at 32769 remain exact host semantic
  control; no fixtures, thresholds, IDs, denominators, policy, or receipt
  taxonomy changed.

- [x] Ordered F Resize reducer through 131072 taps (`cc780e7b9`, source
  `8be644570`): the host proof and both WGSL guards now cover direct rows
  through 131072 taps while preserving Pillow's ordered f64/FMA and
  FLOAT32-store semantics. Native Pillow 12.2.0 versus RSPIL probes are
  180/180 exact; 131073 remains exact host semantic control. Focused
  ordered-F tests are 10/10 and release/build-dev/format gates pass.

- [x] Ordered F Resize reducer through 262144 taps (`33535ab5d`, source
  `526515c81`): the host proof and both WGSL guards now cover direct rows
  through 262144 taps while preserving Pillow's ordered f64/FMA and
  FLOAT32-store semantics. Native Pillow 12.2.0 versus RSPIL probes are
  160/160 exact across 262144/262145 boundary shapes, all five non-nearest
  filters, and finite/special words; 100 rows at 262144 use native GPU
  receipts, 35 remain exact host semantic control, and the 262145 marker-9/
  host split is 20/20. Arithmetic-changing chains are 100/100 exact without
  widening chain admission. Focused ordered-F tests are 38/38 and pool-GPU
  tests are 98/98.

- [x] Ordered F Resize reducer through 524288 taps (`f77fbbc29`, source
  `cbfda102a`): both WGSL guards and the host ordered proof now cover direct
  rows through 524288 taps. Native Pillow 12.2.0 versus RSPIL probes are
  160/160 exact across 524288/524289 boundary shapes, all five non-nearest
  filters, and finite/special words; the receipt matrix has 125 native GPU
  rows and 35 exact host semantic-control rows. Arbitrary
  finite/extreme/cancellation probes are 45/45 exact and a 175-case
  arithmetic-changing chain matrix is 175/175 exact without widening chain
  admission. Rows above 524288 remain exact host semantic control.

- [x] Signed unit-axis Perspective nearest GPU routing (`8caddc219`, source
  `423ebf445`): reflected and axis-swapped integer relocations, including PA
  raw index/alpha pairs, are admitted only after the source-selection proof.
  Native Pillow 12.2.0 probes are 4,392/4,392 exact with terminal native GPU
  receipts; fractional/scaled/nonconstant-denominator maps without the
  exhaustive proof, filtered/partial, and multi-record maps remain exact host
  semantic control.
- [x] Constant-denominator integer Perspective nearest GPU routing
  (`a735a563f`, source `51a5e110`): proof-certified integer scale/shear/
  reflection/translation maps use the centered source-selection proof before
  WGSL admission. Native Pillow 12.2.0 probes are 160/160 exact with terminal
  native GPU receipts; fractional boundaries outside the proof,
  nonconstant-denominator maps without the proof, filtered, and
  arithmetic-changing maps remain exact host semantic control.
- [x] Proof-certified f32-representable fractional Perspective and constant
  Quad/Mesh/PA nearest routing (`549bc3e08`, source `51a5e110`): the existing
  exhaustive per-output host f64 versus shader f32 source-selection proof now
  admits safe fractional Perspective maps and constant-coordinate Quad/Mesh
  and palette-alpha cases. Native Pillow 12.2.0 versus RSPIL probes are
  1,280/1,280 exact with terminal requested=actual GPU receipts; unsafe
  boundaries, filtered maps, and broader records remain exact host semantic
  control.
- [x] Proof-certified nonconstant-denominator Perspective nearest routing
  (`00696f1fb`, source `6f2a50886`): ordinary packed L/LA/RGB/RGBA nearest
  maps use the exhaustive source-selection proof without requiring `g=h=0`.
  Native Pillow 12.2.0 versus RSPIL is 12/12 exact across three matrices, and
  a bounded random stress sweep is 500/500 exact (179 native GPU, 321 exact
  host semantic control). The f32 non-finite-intermediate guard keeps
  overflow/NaN boundaries host-controlled; filtered, Quad/Mesh, palette, and
  proof-failing maps remain exact host semantic control.

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
  A subsequent four-run equal-receipt audit also kept 11/11 workloads,
  44/44 comparable records, and 33/33 terminal requested=actual target
  receipts in every run; adjacent budget comparisons still reported 12, 5,
  and 5 violations. The affected rows changed between runs despite stable
  operation/dispatch telemetry, so this remains host timing noise rather than
  a deterministic source regression.

- [x] Fresh P1/P2 receipt audit at the current HEAD found no identity or
  accounting defect. The full replay remains 10,952/10,952 exact with CPU
  6,838 native receipts, SIMD 6,847 native plus three explicit CPU controls,
  and GPU 6,744 native plus 94 explicit CPU controls; all pipeline partitions
  have zero missing, partial, or indeterminate receipts. A new fixed-ID pair
  measured 11/11 workloads, 44/44 comparable records, and 33/33 terminal
  requested=actual target receipts in both runs, but the pair still had seven
  budget violations. A 40-sample GPU profile ranged 0.589083–42.261666 ms
  with stable four-operation/nine-dispatch telemetry, so P2 remains open for
  host timing noise rather than a localized source regression. No benchmark
  scripts, fixtures, thresholds, IDs, denominators, policy, or receipt
  taxonomy changed.

- [x] Fresh combined all-backends replay at `ecac88ac1` completed all 10,952
  selected public cases exactly on CPU, SIMD, GPU, Node WASM, and browser WASM:
  every lane passed 10,952/10,952 with zero failed or not-run cases, and the
  GPU smoke gate passed 1/1. Terminal receipts are CPU 6,838; SIMD 6,847 SIMD
  plus three CPU layout controls; GPU 6,744 native plus 94 explicit host
  controls; and Node/browser WASM 6,951 each. Pipeline missing, partial, and
  indeterminate counts are zero. Result SHA-256 is
  `d5c7b3eacfcac74e4cfb9e7c212c0452a91a6851e1b85406d3dda77599d37331`, GPU
  execution is `6b6cabd0e1e37164e5238e714dfa2b64c486a4b2aa526f574d8a5bb8ce0e0c08`,
  and WGSL coverage is
  `f02ea46100424b88bceadaed5a6c5693417d7623db3bd34e0488c94894a7e494`.
  The aggregate status remains `passed_with_backend_gaps` solely for the
  explicit host-controlled partition; no fixtures, thresholds, IDs,
  denominators, policy, or receipt taxonomy changed.

- [x] Post-receipt-fix all-backends replay at `058b5e48b` again passed all
  10,952/10,952 value/error cases on CPU, SIMD, GPU, Node WASM, and browser
  WASM, with GPU smoke 1/1 and zero failed/not-run cases. Terminal receipt
  counts remain CPU 6,838; SIMD 6,847 plus three CPU controls; GPU 6,744 plus
  94 explicit host controls; and Node/browser WASM 6,951 each. The corrected
  terminal-candidate logic leaves pipeline missing, partial, and indeterminate
  counts at zero. Result SHA-256 is
  `95b77ab14a17342bd8ad1613d6332818941da0c6ee5fc3acbf2f52c9396f0529`, GPU
  execution is `47b196f6d680a9275e6251337d3e77b3259518838be0635dad9c4c558ac4a039`,
  and WGSL coverage remains
  `f02ea46100424b88bceadaed5a6c5693417d7623db3bd34e0488c94894a7e494`.

## Current integration state

`main` includes the parity fixes through `d513cfa13` (typed-F filtered
projective/mesh words and fill presence, filtered Rotate, near-zero Hamming
F/I parity, PA projective relocation, wide-row F admission guard, ordered F
reducer through 8388607 taps, tall-image F ordering, filtered
Perspective/Quad/Mesh relocation, arm64 wide-row F accumulation, unit-scale
Mesh relocation including exact partial bboxes, integer and signed-unit
Perspective nearest routing, translated Quad relocation, the
centered constant-denominator integer and proof-certified fractional
and nonconstant-denominator Perspective nearest envelopes, identity projective
routing, proof-certified Quad/Mesh nearest maps, receipt-proven suite cohorts,
  and packed SIMD ExtractBand, plus the 128-MiB F binding guard, compact
  over-limit horizontal Box path, and the interior-integer Bilinear/Bicubic
  projective envelopes across all proven raw packed modes).
The fresh combined replay at this revision is 10,952/10,952 value/error exact
with zero failed or not-run cases. It remains `passed_with_backend_gaps` only
because the explicit host-controlled partitions are still reported honestly:
CPU 6,838; SIMD 6,847 SIMD plus 3 CPU controls; GPU 6,744 GPU plus 94 CPU
controls; Node/browser WASM 6,951 each. Result SHA-256 is
`f0c0355fab5bc5951e285e29b00c44e5f543ef6197cbe7bb0a4a6af797587046`; the GPU
execution sidecar is `0c70a838b0f6ffcea6d0e80a7a2e0de2d943cb2405b1d1733d53863891e20780`,
with WGSL coverage `421d0643bc819e3641391b44cf22cf88b51eb34c9207b22cd32d8670bd0033bf`.
The focused list still has four open acceptance buckets: broader F device
arithmetic beyond the compact horizontal Box proof (including
f64-intermediate boundaries and arithmetic-changing chains), broader
arithmetic-changing projective/mesh/palette admission, native/host partition
reconciliation, and the two-consecutive-run zero-budget performance gate.
