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
  families are NaN/invalid special-value arithmetic, unproven negative-zero
  and cancellation cases whose ordered f64 result does not match the exact
  sum, Box ratios outside the proven row limits, chains containing non-Resize
  stages or outside the per-stage intermediate proof, and larger native-GPU
  arithmetic domains. Those rows remain on exact host semantic control until
  their arithmetic, ordering, and storage contracts are separately validated.
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

### P1 — complete native-backend receipt proof

- [ ] Close the 1 genuine partial native receipt in each CPU, SIMD, and GPU
  lane; every claimed pipeline case needs a terminal-complete receipt. The
  latest classifier separates fourteen setup-mutation receipts that precede
  public validation errors before materialization from the one real observed
  filter/invert prefix. The fresh envelope reports CPU 7,084 complete + 1
  partial, SIMD 7,096 + 1, and GPU 7,084 + 1; the setup-only records remain
  operation telemetry outside the deferred pipeline partition.
- [ ] Reconcile backend identity and fallback taxonomy. Current terminal
  counts in the last full envelope include 405 SIMD-lane CPU receipts and 252
  GPU-lane CPU receipts; that GPU lane has 6,832 native GPU receipts and 139
  exact host semantic-control records, with explicit logical-mode, dimension,
  Transform, and Contrast routes. The current 35-case Contrast replay removes
  that route for the proven prefix, but a new full-denominator envelope is
  still needed before changing the aggregate counts. These are visible
  evidence gaps, not value-parity exemptions.
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
- [x] Admit the exact current-image `PutPixel -> Contrast` prefix on native
  GPU. Commit `5ed9f152e` mirrors one non-palette byte write only to compute
  Pillow's post-write midpoint, while the complete two-operation batch stays
  on the GPU. The 35-case L/LA/RGB/RGBA/CMYK replay is byte-exact across all
  public lanes with 35/35 terminal GPU receipts and no fallback; longer or
  palette-sensitive prefixes remain host-controlled.
- [ ] Close the WASM receipt gaps: each Node/browser lane is value-exact but
  currently reports 6,713 complete, 586 partial, 888 missing, 2,738
  not-applicable, and 27 indeterminate cases. Keep the aggregate status
  `passed_with_backend_gaps` until these receipts are resolved or explicitly
  bounded by maintained backend evidence.

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
  conservative. `make migration-parity-receipt-test` passes 28/28; the full
  selected denominator remains 10,952 and public parity results retain their
  original schema without internal error fields.
- [x] The D-049 thumbnail control-flow fix is committed as `dc6085f81`:
  the expanded degenerate probe is 0 mismatches and all 172 thumbnail parity
  cases remain exact.
- [x] The focused post-merge checks pass: `make build-dev`, Rust F-resize
  tests 17/17, GPU-pool tests 28/28 (including Draw, indexed Fit, finite
  subnormal marker-9 rows, filtered F chains, finite overflow and signed-zero
  rows),
  receipt tests 28/28,
  evidence/schema validation, and `make -C pillow-rs fmt`. Clippy remains
  blocked before
  compilation by the pre-existing pinned libavif 1.4.1/dav1d 1.5.3/libaom
  3.13.2 environment requirement.

## Closeout state

- [x] The source lane and receipt-partition correction are committed as
  `cb1813bc8` plus setup-before-error classifier `b867867ee`, the two-axis f64 GPU admission as `f17e1a7da`, the finite
  subnormal marker-9 admission as `b1962c6dd`, finite overflow and proven
  signed-zero marker-9 admission as `19acd29ab`, pure filtered F-chain admission
  as `33e0f11ec`, mixed-axis F scheduling guard as `ea15ac316`, the raw-color
  `ExtractBand` admission as `f55a770ad`, the
  raw-byte `EffectSpread` admission as `ebc7e765a`, raw-byte Draw admission as
  `7d1cc0af9`, nearest indexed Fit admission as `0797e71f5`, and exact
  current-image Contrast-prefix admission as `5ed9f152e`; the latest
  committed all-backend replay is
  schema-valid and value-exact for all 10,952 cases. Native lanes report one
  genuine partial receipt each and the GPU partition has 6,832 native receipts
  plus 252 CPU receipts. No fixture, expected value, threshold, denominator,
  or case ID was changed. The latest envelope is
  `/tmp/all-backends-post-19acd29ab.json` and remains schema-valid and
  value-exact at the fixed denominator.
- [ ] Do not mark the overall goal complete while P0, P1, or P2 remains open.
