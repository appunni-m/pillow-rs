# Benchmark/backend pending checklist — 2026-09-02

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
- [x] Extend marker 9 to a verified two-axis subset. Commit `f17e1a7da`
  proves the rounded horizontal f32 intermediate before the vertical reducer;
  the existing F-specific compute-pass boundary supplies the required device
  ordering. A heterogeneous `(2,2) -> (1,5)` Bilinear case now executes on
  native GPU with exact bytes and a terminal no-fallback receipt.
- [x] Extend marker 9 to finite subnormal F source/result words. Commit
  `b1962c6dd` decodes subnormals at the exact `2^-149` scale and adds
  integer-only ties-to-even subnormal rounding to both device reducers. A
  deterministic 1,050-row Pillow-vs-Rust probe had 0 mismatches (372 native
  GPU, 678 exact host semantic control); focused GPU tests are 23/23.
- [x] Extend marker 9 to pure finite filtered F resize chains. Commit
  `33e0f11ec` validates every stage and carries its rounded f32 words into the
  next stage's proof. The native Bicubic-to-Lanczos regression uses four
  dispatches; a 500-case deterministic chain probe had 0 mismatches (25 native
  GPU, 475 exact host semantic control).
- [x] Admit bounded finite-input overflow outputs. Commit `19acd29ab` extends
  the marker-9 integer reducer and both WGSL stores to encode a proven final
  ±infinity when Pillow's ordered f64 accumulation overflows the f32 store;
  NaN and ambiguous cancellation remain rejected. A native max/max/−max
  ringing-filter matrix is byte-exact for Bicubic, Lanczos, and Hamming with
  terminal GPU receipts.
- [x] Preserve proven signed-zero filtered outputs. Commit `19acd29ab` removes
  the blanket marker-9 rejection of `0x80000000` when the exact integer
  reducer agrees with Pillow's ordered f64 result. The native minimum-negative-
  subnormal Box regression is byte-exact with a terminal GPU receipt; rows
  whose ordered-f64 residual disagrees with the exact sum remain host-controlled.
- [ ] Extend the proof to the remaining coefficient/value domains. The open
  families are mixed NaN/infinity ordering and cancellation cases whose
  ordered f64 result does not match the device state machine, unproven
  negative-zero/cancellation cases whose ordered f64 result does not match
  the exact sum, Box ratios outside the proven row limits, chains containing
  non-Resize stages or outside the per-stage intermediate proof, and larger
  native-GPU arithmetic domains. Those rows remain on exact host semantic
  control until their arithmetic, ordering, and storage contracts are
  separately validated.
- [x] Guard the mixed geometry that combines horizontal upscaling with
  vertical downscaling. Commit `ea15ac316` rejects this schedule in marker 6,
  the dyadic chain proof, and the central router after a `(1,2) -> (2,1)` Box
  first divergence; the native regression and a 2,304-case sweep now have
  exact bytes with explicit host-control receipts.
- [x] Keep the newly proven signed two-axis subset integrated. Commit
  `a3d2c886b` adds host-verified two-limb signed integer accumulation, exact
  horizontal f32 intermediate boundaries, and two-axis proof checks. Focused
  F tests are 9/9; GPU-pool tests are 16/16; the post-merge native probe is
  byte-exact for 45/45 cases (7 native GPU, 38 exact host semantic control).
- [x] Preserve the conservative route for every unproven row. The forced
  generic-shader diagnostic diverged by ULPs on heterogeneous Bilinear,
  Bicubic, Lanczos, Hamming, and non-dyadic Box; a 2x1 -> 4x1 Bilinear
  counterexample also defeats a broad dyadic-source admission. No unsafe
  admission change was made; `f17e1a7da` admits only the separately proven
  two-axis rows.
- [x] Admit the proven typed `I;16`, `I;16L`, and `I;16B` filtered-resize
  subset. Commit `2ff9a6951` adds a marker-10 exact f64-coefficient reducer,
  declared-byte-order decoding, Pillow's native-u16 intermediate/store
  boundary, and separable intermediate-capacity accounting. A deterministic
  1,365-case matrix had 0 mismatches (926 rows admitted); `I;16N`, chains,
  mixed batches, and unproven rows remain exact host semantic control.
- [x] Admit the deterministic F special-value rows that the device can model
  exactly. Commit `bc8197617` adds an IEEE NaN/infinity state machine to the
  marker-9 host proof and both convolution shaders, preserves the first NaN
  payload/sign, canonicalizes invalid zero*infinity and opposite-infinity
  results, and quiets signaling NaNs in the Box-copy path. Native GPU tests
  cover NaN payloads, both infinity signs, invalid products, and Box copies;
  the focused `f_resize_f64` group is 8/8. A 2,800-row special-value probe
  (one-special and mixed-special patterns) had 0 mismatches; mixed orderings
  that do not match Pillow's ordered f64 result remain exact host semantic
  control.
- [x] Align Thumbnail reduction and typed I/F resampling with Pillow's native
  contracts. Commit `0013d013e` carries the final aspect-preserving
  dimensions once (no backend double-adjustment), routes byte reduction through
  `Reduce.c`'s fixed reciprocal, keeps RGBa/RGBX raw rather than alpha-
  premultiplied, mirrors 32bpc pair/quartet grouping, preserves INT32 typed-I
  intermediates, and uses the fractional post-reduce box. The canonical
  parity run and committed-source all-backend envelope are each 10,952/10,952
  exact with zero failures; focused CPU geometry is 6/6 and GPU pool is 36/36.

### P1 — complete native-backend receipt proof

- [x] Close the 1 genuine partial native receipt in each CPU, SIMD, and GPU
  lane; every observed pipeline boundary now has a terminal-complete receipt.
  Commit `70a92f4ca` marks the successfully observed `Filter5x5` result
  terminal even when a later, unrelated public call raises. The focused
  replay `/tmp/receipt-prefix-all-backends-70a92f4ca.json` is value-exact with
  zero partial cases in CPU, SIMD, and GPU (1/1 terminal receipt in each;
  GPU actual backend is GPU with no fallback). The later full-envelope
  accounting below retains that zero-partial result at the fixed denominator.
- [ ] Reconcile backend identity and fallback taxonomy. The current full
  envelope at `bc8197617` has exact public parity for all 10,952 IDs and no
  native partial/missing/indeterminate pipeline cases, but GPU still reports
  6,627 native GPU receipts plus 211 CPU receipts and 142 exact host-control
  fallbacks. CPU has 6,838 terminal receipts (6,832 pipeline-complete cases)
  and SIMD has 6,850 terminal receipts (6,844 pipeline-complete cases); the
  remaining terminal receipts are non-pipeline observations. Explicit
  logical-mode, dimension, Transform, and arithmetic guards remain visible
  evidence partitions, not value-parity exemptions.
- [x] Correct zero-operation observation accounting. Commits `2164e2226` and
  `2835ce29a` keep metadata-only empty pipelines telemetry-neutral, retain raw
  zero-operation observations with `pipeline_relevant=false`, preserve the
  last meaningful receipt candidate across Python/JS boundaries, and align
  WASM partial counts with their per-case classifier. The focused four-case
  PA replay is exact with 4/4 terminal receipts on CPU, SIMD, GPU, Node WASM,
  and browser WASM; the former three byte-`putpixel`→`PA` receipt gaps are
  closed without changing public values or the denominator.
- [x] Admit the raw-color `ExtractBand` subset on native GPU. Commit
  `f55a770ad` permits only `ExtractBand`/`PutPixel` batches for CMYK, HSV, and
  YCbCr, whose packed channel order is already preserved by the existing
  shader. The focused native regression and a 30-case filtered all-backend
  replay are byte-exact with 30 terminal GPU receipts and no fallback. The
  full envelope moves the GPU partition from 6,701 to 6,731 native receipts
  and from 383 to 353 CPU receipts; the remaining logical-mode routes still
  require separate proofs.
- [x] Admit the exact raw-byte `EffectSpread` lane on native GPU. Commit
  `ebc7e765a` adds the existing host-generated relocation-map/gather shader to
  the P/PA/1, RGBX/RGBa, HSV/YCbCr, CMYK, I, and F packed-byte mode guards while
  leaving typed I;16 storage on its separate path. The focused regression
  covers 13 byte-backed modes; a 34-case filtered all-backend replay is exact
  with 34/34 terminal GPU receipts and no fallback. The full envelope below
  records the same result at the fixed 10,952-case denominator.
- [x] Admit raw-byte ImageDraw batches on native GPU. Commit `7d1cc0af9`
  extends the exact host canvas plus packed-byte copy path to `1`, `P`, `PA`,
  `RGBX`, `RGBa`, `CMYK`, `HSV`, `YCbCr`, `I`, and `F`, while retaining typed
  `I;16` on its existing path. The focused native regression covers all ten
  modes, and the 73-case filtered replay is byte-exact across CPU, SIMD, GPU,
  Node WASM, and browser WASM: 72 GPU receipts are native and the single
  zero-height safety case remains host-controlled.
- [x] Admit nearest indexed `ImageOps.fit` batches on native GPU. Commit
  `0797e71f5` adds only `Fit(filter=NEAREST)` to the `P`/`PA` guard; filtered
  and interpolating indexed Fit rows remain on exact host semantic control.
  The focused regression covers both indexed modes. The fixed 15-case replay
  is byte-exact on every public lane with 14 terminal native GPU receipts;
  `pa-putpalette-expansion` intentionally remains host-controlled.
- [x] Admit the proven constant `F` `ImageOps.pad` row on native GPU. The
  source constant-resize marker now carries the scalar word through Pillow's
  contain pass, `gpu_pad_fill` preserves named/scalar fill bits, and the pad
  shader keeps mode-8 words opaque. The focused regression and filtered
  all-backend replay `/tmp/f-pad-all-backends.json` are value/error-exact with
  **1/1 native GPU** receipt and no fallback; non-constant, nearest, mixed,
  and invalid-color `F`/typed rows remain guarded.
- [x] Admit the proven nearest `F` `ImageOps.fit` row on native GPU. Commit
  `67f60f9b5` uses Pillow's boxed one-tap crop tables, marker 7 complete-word
  copies, and an explicit horizontal/vertical compute-pass boundary. The
  heterogeneous native regression and filtered replay
  `/tmp/f-fit-nearest-all-backends.json` are exact with **1/1 native GPU**
  receipt and no fallback; filtered Fit, mixed batches, and other logical
  modes remain guarded.
- [x] Admit the proven nearest `I` Cover→Pad chain on native GPU. Commit
  `544d0ebc1` carries signed int32 words through the nearest contain resize
  and raw Pad placement, uses scalar zero for omitted fills, and separates
  horizontal, vertical, and placement passes. The signed-word regression and
  `/tmp/i-cover-pad-all-backends.json` are exact with **1/1 native GPU**
  receipt, five dispatches, and no fallback; filtered Pad and other typed
  arithmetic remain guarded.
- [x] Admit the exact current-image `PutPixel -> Contrast` prefix on native
  GPU. Commit `5ed9f152e` mirrors one non-palette byte write only to compute
  Pillow's post-write midpoint, while the complete two-operation batch stays
  on the GPU. The 35-case L/LA/RGB/RGBA/CMYK replay is byte-exact across all
  public lanes with 35/35 terminal GPU receipts and no fallback; longer or
  palette-sensitive prefixes remain host-controlled.
- [x] Admit terminal CMYK `PutAlpha` and `PutAlphaData` on native GPU. The
  existing `put_alpha.wgsl`/`put_alpha_data.wgsl` paths already implement
  Pillow's integer CMYK-to-RGB promotion and alpha replacement exactly; the
  preflight had omitted this valid source mode. The focused two-case replay
  is byte-exact on every public lane with 2/2 terminal GPU receipts and no
  fallback. The admission is terminal-only because the result is RGBA and a
  following operation requires a segmented batch with updated mode metadata.
- [x] Close the WASM receipt gaps: commit `a2cf8c102` preserves JS setup/call
  errors for the evidence classifier and passes target results through the
  aggregator. The subsequent receipt-accounting commits `2164e2226` and
  `2835ce29a` remove zero-operation observation proofs and align the partial
  count with the classifier. The latest full envelope reports Node and
  browser value/error parity at 10,952/10,952, each with 6,945
  pipeline-complete cases, 4,007 explicit not-applicable boundaries, and zero
  missing, partial, or indeterminate pipeline cases. Backend/export identity
  reconciliation remains tracked separately below, so the aggregate status
  is still `passed_with_backend_gaps`.

### P2 — performance acceptance

- [ ] Produce two consecutive equal-ID/equal-receipt cohort comparisons with
  zero budget violations. The fixed 11-ID cohort has 44 comparable pairings
  and no receipt fallbacks; the latest consecutive checks report 11, 7, and 6
  violations. The factor-1.0 Brightness identity path remains a deterministic
  row-level improvement (CPU medians about 0.181/0.163 ms before versus
  0.042/0.049/0.042 ms after), but aggregate timing acceptance is still open.

## Evidence recorded in the current run

- [x] The latest schema-v3 all-backend envelope at committed source
  `f17e1a7da` is `/tmp/all-backends-post-f17e1a7da.json` (SHA-256
  `ee84c4c4f94aa0c81e1deeea6d712137e1b33299370da3866cacce66fe6c5a7f`).
  CPU, SIMD, GPU, Node WASM, and browser WASM are each 10,952/10,952
  value-exact; GPU smoke is 1/1; case-ID digest is
  `881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.
  Native CPU/GPU partitions are 7,084 complete + 15 partial + 3,853
  not-applicable; SIMD is 7,096 + 15 partial + 3,841 not-applicable.
- [x] The follow-up schema-v3 envelope at committed source `f55a770ad` is
  `/tmp/all-backends-post-f55a770ad.json` (SHA-256
  `7b97442f45ffe3f6db1128bd04cbc6dd438963f1aab900a374fcd2c46a943f4e`).
  CPU, SIMD, GPU, Node WASM, and browser WASM remain value-exact for all
  10,952 cases, with GPU smoke 1/1. The GPU lane reports 6,731 native GPU
  and 353 CPU receipts; its logical-mode fallback count is 117. Native
  receipt totals are 7,084 complete + 15 partial + 3,853 not-applicable;
  the aggregate remains `passed_with_backend_gaps`.
- [x] The latest schema-v3 envelope at committed source `ebc7e765a` is
  `/tmp/all-backends-post-ebc7e765a.json` (SHA-256
  `17326cec1fd5c70132aa21bb00af6f060b194e1d484491fbb5100f29c712beee`). CPU,
  SIMD, GPU, Node WASM, and browser WASM remain value-exact for all 10,952
  cases, with GPU smoke 1/1. The GPU lane now reports 6,744 native GPU and
  340 CPU receipts; the logical-mode preflight count is 104 (down from 117).
  Native receipt totals remain 7,084 complete + 15 partial + 3,853
  not-applicable, and the aggregate remains `passed_with_backend_gaps`.
- [x] The post-Draw/Fit schema-v3 envelope at committed source `0797e71f5` is
  `/tmp/all-backends-post-0797e71f5.json` (SHA-256
  `d95f880a7393ef078bbd09d7b0364cd0ee53836d31f232e2fa4754546369ba0f`). CPU,
  SIMD, GPU, Node WASM, and browser WASM remain value-exact for all 10,952
  cases, with GPU smoke 1/1. Native terminal receipts are CPU 7,084,
  SIMD 6,691 plus 405 CPU, and GPU 6,832 plus 252 CPU; each native lane still
  has 15 genuine partial receipts. The fixed case-ID digest remains
  `881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`, and
  the aggregate remains `passed_with_backend_gaps`.
- [x] The Draw filtered replay at committed source `7d1cc0af9` is
  `/tmp/draw-post-7d1cc0af9.json` (SHA-256
  `86f4adee48bd27cf6f53056dc4bee3d84a34a0fe7e1e27eba3d63cdbc193ef58`):
  73/73 values are exact on CPU, SIMD, GPU, Node WASM, and browser WASM;
  GPU receipts are 72 native and one host-controlled zero-height guard.
- [x] The indexed nearest Fit filtered replay at committed source
  `0797e71f5` is `/tmp/fit-indexed-nearest-0797e71f5.json` (SHA-256
  `8b8577b060f3b22001e0069017c4cc8584c7bee0e354128cd7dd90728274d83c`):
  15/15 values are exact on CPU, SIMD, GPU, Node WASM, and browser WASM;
  GPU receipts are 14 native and one host-controlled palette-expansion row.
- [x] The finite-subnormal marker-9 replay at committed source
  `b1962c6dd` is `/tmp/all-backends-post-b1962c6dd.json` (SHA-256
  `9a981a51e018cad9c65390311b6e38c58e40ee75861595c01e0c7baee48af5df`).
  CPU, SIMD, GPU, Node WASM, and browser WASM are each 10,952/10,952
  value-exact; GPU smoke is 1/1; the fixed case-ID digest remains
  `881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.
  Native receipt partitions remain CPU 7,084, SIMD 6,691 plus 405 CPU, and
  GPU 6,832 plus 252 CPU, with 15 genuine partials in each native lane.
- [x] The pure filtered F-chain replay at committed source `33e0f11ec` is
  `/tmp/all-backends-post-33e0f11ec.json` (SHA-256
  `d91175eb93e4580d3a40da029cc86ea6903d6b2bebeb46aa99c6d11a7700be4f`). CPU,
  SIMD, GPU, Node WASM, and browser WASM are each 10,952/10,952 value-exact;
  GPU smoke is 1/1; the fixed case-ID digest remains
  `881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`.
  Native receipt partitions remain CPU 7,084, SIMD 6,691 plus 405 CPU, and
  GPU 6,832 plus 252 CPU, with 15 genuine partials in each native lane.
- [x] The post-guard schema-v3 envelope at committed source `ea15ac316` is
  `/tmp/all-backends-post-ea15ac316.json` (SHA-256
  `8fef943b7e5a97188e4aa44ca4d34a54cf99acf7d9cffdf92f9506a1ade035cf`). CPU,
  SIMD, GPU, Node WASM, and browser WASM remain value-exact for all 10,952
  cases, with GPU smoke 1/1. Native receipt partitions remain CPU 7,084,
  SIMD 6,691 plus 405 CPU, and GPU 6,832 plus 252 CPU, with 15 genuine
  partials in each native lane; the mixed-axis Box route is now explicitly
  host-controlled and no longer an unguarded parity risk.
- [x] The setup-before-error receipt classifier at committed source
  `b867867ee` is covered by `make migration-parity-receipt-test` **28/28**.
  The fresh schema-v3 envelope is
  `/tmp/all-backends-post-b867867ee.json` (SHA-256
  `64690d9cdbf3415d69e742347a4410c523fcadc2ad4a4118d6c520a533ad754b`),
  revision `b867867ee5b52dd7674b524380233781b39952a5`. All five public lanes
  remain **10,952/10,952** value-exact with GPU smoke **1/1**. CPU and GPU
  report 7,084 complete + 1 genuine partial + 3,867 not-applicable; SIMD
  reports 7,096 + 1 + 3,855. Fourteen setup-before-error records are now
  outside the pipeline partition; the observed filter/invert prefix remains
  the single genuine partial. WASM remains 6,713 complete + 586 partial +
  888 missing + 2,738 not-applicable + 27 indeterminate.
- [x] The finite-overflow and proven signed-zero marker-9 extensions at
  committed source `19acd29ab` are covered by the native max/max/−max
  Bicubic, Lanczos, and Hamming matrix plus the minimum-negative-subnormal
  Box regression; focused F-resize tests are **17/17** and the serial
  GPU-pool group is **28/28**. The fresh schema-v3 envelope is
  `/tmp/all-backends-post-19acd29ab.json` (SHA-256
  `3e2f0c5bac51737de40e202ad993de64f28673379dc8bda4ada216631089c6ce`).
  Revision is `19acd29abdef41da22c3f3875c553e00c3d3c3be`. All five public
  lanes remain **10,952/10,952** value-exact with GPU smoke **1/1**; CPU and
  GPU report 7,084 complete + 1 genuine partial + 3,867 not-applicable, SIMD
  reports 7,096 + 1 + 3,855, and the WASM partitions remain 6,713 complete +
  586 partial + 888 missing + 2,738 not-applicable + 27 indeterminate.
- [x] The current-image Contrast prefix admission at committed source
  `5ed9f152e` is covered by `/tmp/contrast-prefix-all-backends.json` (SHA-256
  `2db9e1f47d3e2fce94ccfcf51162cb64ad194e0ccf6a794f462c9e66d3a640ca`). The
  fixed 35-case replay is schema-valid and byte-exact on CPU, SIMD, GPU, Node
  WASM, and browser WASM; each lane has 35 terminal-complete receipts, and
  the GPU lane has 35 native GPU receipts with no fallback. No fixtures,
  expected values, thresholds, IDs, denominators, or receipt taxonomy changed.
- [x] The terminal CMYK PutAlpha admission is covered by
  `/tmp/putalpha-cmyk-all-backends.json` (SHA-256
  `50995134050c39326b97158c17b8a9f358c8e6739d1667ebe1d43d1fac8055f7`).
  The fixed two-case replay is schema-valid and byte-exact on CPU, SIMD, GPU,
  Node WASM, and browser WASM; every lane has 2/2 terminal-complete receipts,
  and both GPU cases are native with no fallback. No fixtures, expected values,
  thresholds, IDs, denominators, or receipt taxonomy changed.
- [x] The typed I;16 filtered-resize replay at revision `2ff9a6951` is
  `build/migration-parity/all-backends-test-result.json` (SHA-256
  `58f1e1b3fcad066b5b9e82e2d5910fd502c593e53fa15759293fd312ac3c571c`). The
  three selected cases are value-exact with terminal-complete receipts on all
  five public lanes; GPU has 2 native receipts and 1 exact host semantic-control
  receipt for the intentionally guarded I;16N case, and GPU smoke is 1/1.
- [x] The marker-9 native probe is exact for the heterogeneous lanes: the
  `(2,2) -> (1,2)` one-axis and `(2,2) -> (1,5)` two-axis Bilinear cases are
  byte-for-byte equal to Pillow and publish actual-GPU receipts. The rebuilt
  randomized probe covers 5,000 finite-F rows (269 actual GPU, 4,731 exact
  host control) with zero mismatches; the known `2x1 -> 4x1` false-proof
  counterexample remains on host control.
- [x] The receipt classifier changes are committed as `40c3e9860`,
  `635afb555`, `cb1813bc8`, and `b867867ee`. The latest guard proves step-bound
  pre-materialization validation errors, annotates retained setup telemetry
  with `pipeline_relevant=false`, and keeps prior deferred receipts
  conservative. The zero-operation accounting follow-up is committed as
  `2164e2226` and `2835ce29a`; `make migration-parity-receipt-test` passes
  34/34, the full selected denominator remains 10,952, and public parity
  results retain their original schema without internal error fields.
- [x] The observed-prefix terminal-boundary fix is committed as `70a92f4ca`.
  `make migration-parity-receipt-test` passes 29/29. The focused replay
  `/tmp/receipt-prefix-all-backends-70a92f4ca.json` is schema-valid and
  value-exact; CPU, SIMD, and GPU each report 1/1 terminal-complete receipts
  and zero partial cases for `pipeline-composition.filter-rgba-5x5-invert`.
  The regenerated full envelope `/tmp/all-backends-post-2969b323.json`
  confirms zero native partial cases at the fixed denominator.
- [x] The D-049 thumbnail control-flow fix is committed as `dc6085f81`:
  the expanded degenerate probe is 0 mismatches and all 172 thumbnail parity
  cases remain exact.
- [x] The focused post-merge checks pass: `make build-dev`, Rust F-resize
  tests 17/17, GPU-pool tests 28/28 (including Draw, indexed Fit, finite
  subnormal marker-9 rows, filtered F chains, finite overflow and signed-zero
  rows),
  receipt tests 29/29, evidence/schema validation, and `make -C pillow-rs fmt`.
  `make -C pillow-rs clippy` remains blocked by the pre-existing pinned
  `image-slash-star` libavif 1.4.1/dav1d 1.5.3/libaom 3.13.2 environment
  requirement.

- [x] The post-receipt-fix full envelope at committed source `2969b323c` is
  `/tmp/all-backends-post-2969b323.json` (SHA-256
  `50e893989476cacee452f220e6f10e32166a2e0212058e9b5926360e42551d8f`). All
  five public lanes are value-exact at 10,952/10,952 with GPU smoke 1/1.
  CPU reports 7,085 complete + 0 partial; SIMD reports 7,097 complete + 0
  partial (6,698 SIMD + 405 CPU); GPU reports 7,085 complete + 0 partial
  (6,880 GPU + 211 CPU). Node and browser WASM remain 6,713 complete + 586
  partial + 888 missing + 2,738 not-applicable + 27 indeterminate, so the
  aggregate remains `passed_with_backend_gaps`.
- [x] The shared JS/WASM observed-boundary fix is committed as `d0ee51d9a`.
  `make test-wasm` passes the former-partial set 586/586 on Node and browser;
  485 receipts move to terminal-complete and 101 remain explicit
  pre-materialization errors. Full fixed-denominator artifacts are
  `/tmp/wasm-boundary-full-node-d0ee51d9a.json` (SHA-256
  `5d73ecbd6d6680fb65b7e0b91813ac30ac734c68a8c19522a76ffb7b7a8d0e06`) and
  `/tmp/wasm-boundary-full-browser-d0ee51d9a.json` (SHA-256
  `b2dca82a37a9332733783d8106373da82df2159c8320feb628fc8a85a8d40c9c`).
  Both hosts are value-exact at 10,952/10,952 and report 7,198 complete +
  101 partial + 888 missing + 2,738 not-applicable + 27 indeterminate.
- [x] The JS/WASM validation-boundary fix is committed as `a2cf8c102`.
  `make test-wasm` passes the former 20 indeterminate cases 20/20 on both
  Node and browser, and the 30-case receipt regression suite passes 30/30.
  Full artifacts are `/tmp/wasm-errorbound-full-node-a2cf8c102.json` (SHA-256
  `3998bd57b9a9ac3dd4ed679a70159957cbe855b6837ecbcdd825861a60d71780`) and
  `/tmp/wasm-errorbound-full-browser-a2cf8c102.json` (SHA-256
  `97535c5db628350aa3342ad1ee3fa44f5065019b13df7e87d6089e617876e9b7`).
  Both hosts report 7,198 complete + 3,754 not-applicable, with zero
  missing/partial/indeterminate pipeline cases; the public parity envelope
  remains free of the internal `execution_errors` field.
- [x] The post-zero-operation receipt envelope at committed source
  `2835ce29a` is `/tmp/all-backends-post-2835ce29.json` (SHA-256
  `9bd4bf29816f0923a5ef4fbfaf119fbc890a975e70b5c2c7ca5e177905cffc25`). The
  fixed case-ID digest remains
  `881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`, and
  all CPU, SIMD, GPU, Node WASM, and browser WASM public lanes are
  value/error-exact for 10,952/10,952 cases (GPU smoke 1/1). CPU reports
  6,838 terminal receipts with 6,832 pipeline-complete cases; SIMD reports
  6,850 with 6,844 pipeline-complete; GPU reports 6,627 native GPU plus 211
  CPU receipts, with the same 6,832 pipeline-complete cases and 142
  exact host-control fallbacks. Node and browser each report 6,951 terminal
  receipts, 6,945 pipeline-complete cases, and 4,007 not-applicable cases;
  all three native lanes and both WASM lanes have zero partial, missing, or
  indeterminate pipeline cases. The aggregate remains
  `passed_with_backend_gaps` because backend identity/fallback reconciliation
  and the P0/P2 buckets remain open.
- [x] The special-value marker-9 extension at committed source `bc8197617` is
  covered by `/tmp/all-backends-post-bc8197617.json` (SHA-256
  `0b75a5cdce922104f6d69b585ca5e0188c1d336c8d6029bf41378a4b755ab7fd`),
  revision `bc8197617bc0ba880f08aa251f294a51df788d95`. The fixed 10,952-case
  corpus remains value/error-exact in CPU, SIMD, GPU, Node WASM, and browser
  WASM (GPU smoke 1/1). CPU reports 6,838 terminal receipts and 6,832
  pipeline-complete cases; SIMD reports 6,850 and 6,844; GPU reports 6,627
  native GPU plus 211 CPU receipts and 6,832 pipeline-complete cases. Every
  lane has zero partial, missing, or indeterminate pipeline cases. The public
  corpus does not add a new special-value receipt partition, so backend
  identity reconciliation and the remaining arithmetic guards stay open.
- [x] The Thumbnail/typed-resampling parity fix at committed source
  `0013d013e` is covered by `build/migration-parity/all-backends-test-result.json`
  (SHA-256 `c1e45bd9951e7881aa4616d26c2a56984df3237ee22f49d3b9fea0e0344893fa`).
  The unchanged 10,952-case ID digest remains
  `881ae8494848c4528b57f43d38ab6b46935a12e743a8967edb263731d064c526`; all
  five public lanes are value/error-exact at 10,952/10,952 and GPU smoke is
  1/1. CPU reports 6,838 terminal receipts (6,832 pipeline-complete), SIMD
  6,850 (6,844 complete), GPU 6,628 native plus 210 CPU (6,832 complete),
  and Node/browser WASM 6,951 terminal (6,945 complete) each. No lane has
  partial, missing, or indeterminate pipeline cases; backend identity,
  broader F arithmetic, and P2 timing remain open.
- [x] The constant F `ImageOps.pad` native admission is covered by the
  focused `/tmp/f-pad-all-backends.json` replay: all five public lanes are
  value/error-exact for the selected case, and GPU has a terminal native
  receipt with no fallback. The source/shader change is intentionally scoped
  to a single non-nearest Pad on a finite constant F image; the remaining
  logical-mode and arithmetic guards are still active.
- [x] The nearest F `ImageOps.fit` native admission is covered by
  `/tmp/f-fit-nearest-all-backends.json` (SHA-256
  `1bc7a9b21f08554e84762714ebfbcb4f25d4117c9c512d91aef5c6f4059412ad`): the
  selected case is value/error-exact on all five public lanes and GPU has a
  terminal native receipt with no fallback. The admission remains limited to
  one nearest Fit with host-generated one-tap coefficients.
- [x] The nearest I Cover→Pad native admission is covered by
  `/tmp/i-cover-pad-all-backends.json` (SHA-256
  `a12189686480ea6883e157daf5906116ca3903aae0e13dea46d0bd6942a5b27e`): all
  five public lanes are value/error-exact for the selected chain, and GPU has
  a terminal native receipt with five dispatches and no fallback. The typed
  route remains limited to nearest word-copy geometry and scalar fill.

## Closeout state

- [x] The source lane and receipt-partition correction are committed as
  `cb1813bc8` plus setup-before-error classifier `b867867ee`, observed-prefix
  boundary fix `70a92f4ca`, the two-axis f64 GPU admission as `f17e1a7da`, the finite
  subnormal marker-9 admission as `b1962c6dd`, finite overflow and proven
  signed-zero marker-9 admission as `19acd29ab`, pure filtered F-chain admission
  as `33e0f11ec`, mixed-axis F scheduling guard as `ea15ac316`, the raw-color
  `ExtractBand` admission as `f55a770ad`, the raw-byte `EffectSpread` admission
  as `ebc7e765a`, raw-byte Draw admission as `7d1cc0af9`, nearest indexed Fit
  admission as `0797e71f5`, exact current-image Contrast-prefix admission as
  `5ed9f152e`, typed I;16 filtered-resize admission as `2ff9a6951`, the JS/WASM
  observed-boundary receipt fix as `d0ee51d9a`, the JS/WASM validation-boundary
  evidence fix as `a2cf8c102`, and the zero-operation receipt corrections as
  `2164e2226` and `2835ce29a`, plus the F special-value proof as `bc8197617`;
  the preceding full all-backend replay in that sequence is
  schema-valid and value/error-exact for all 10,952 cases.
  That preceding envelope has zero native or WASM partial/missing/indeterminate
  pipeline cases: CPU 6,838 terminal receipts (6,832 pipeline-complete), SIMD
  6,850 (6,844 pipeline-complete), and GPU 6,627 native GPU plus 211 CPU
  receipts (6,832 pipeline-complete). The GPU partition records 142 exact
  host semantic-control fallbacks plus explicit logical-mode, dimension,
  Transform, and arithmetic guards. Node and browser WASM each report 6,951
  terminal receipts, 6,945 pipeline-complete cases, and 4,007 not-applicable
  boundaries. No fixture, expected value, threshold, denominator, or case ID
  was changed. That preceding envelope is
  `/tmp/all-backends-post-bc8197617.json` (SHA-256
  `0b75a5cdce922104f6d69b585ca5e0188c1d336c8d6029bf41378a4b755ab7fd`) and
  remains schema-valid and value/error-exact at the fixed denominator.
- [x] The current source parity lane is committed as `0013d013e`.
  It aligns Thumbnail's final dimensions and reducing-gap arithmetic with
  Pillow, preserves raw RGBa/RGBX channels, and mirrors typed F/I reduction
  and INT32 resampling boundaries. The committed-source envelope is
  `build/migration-parity/all-backends-test-result.json` (SHA-256
  `c1e45bd9951e7881aa4616d26c2a56984df3237ee22f49d3b9fea0e0344893fa`),
  value/error-exact at 10,952/10,952 in all five public lanes with GPU smoke
  1/1. CPU/SIMD/GPU report 6,838/6,850/6,838 terminal receipts; GPU is
  6,628 native plus 210 CPU, and no lane has a partial, missing, or
  indeterminate pipeline receipt. The remaining P0, P1 identity, and P2
  timing items stay open.
- [ ] Do not mark the overall goal complete while P0, P1, or P2 remains open.
