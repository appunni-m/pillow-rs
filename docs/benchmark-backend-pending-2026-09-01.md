# Benchmark/backend pending checklist — 2026-09-01

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
- [ ] Extend the proof to the remaining coefficient/value domains. The open
  families are subnormal/nonfinite or negative-zero words, coefficient
  overflow/cancellation cases, Box ratios outside the proven row limits, and
  chains outside the cumulative intermediate proof. Those rows remain on
  exact host semantic control until their arithmetic and storage contracts are
  separately validated.
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

- [ ] Close the 15 genuine partial native receipts in each CPU, SIMD, and GPU
  lane; every claimed pipeline case needs a terminal-complete receipt. The
  prior 102-count included 101 public validation failures that occur before
  pipeline materialization; those receipts are now retained as operation
  telemetry but explicitly marked outside the deferred pipeline partition.
  The latest post-change envelope still has 15 genuine partials in each
  native lane (CPU 7,084 complete, SIMD 7,096, GPU 7,084), so this bucket is
  unchanged.
- [ ] Reconcile backend identity and fallback taxonomy. Current terminal
  counts include 405 SIMD-lane CPU receipts and 252 GPU-lane CPU receipts;
  the latest GPU lane has 6,832 native GPU receipts, 139 exact host semantic
  control records, and explicit logical-mode, dimension, Transform, and
  Contrast routes. These are visible evidence gaps, not value-parity
  exemptions.
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
- [x] The marker-9 native probe is exact for the heterogeneous lanes: the
  `(2,2) -> (1,2)` one-axis and `(2,2) -> (1,5)` two-axis Bilinear cases are
  byte-for-byte equal to Pillow and publish actual-GPU receipts. The rebuilt
  randomized probe covers 5,000 finite-F rows (269 actual GPU, 4,731 exact
  host control) with zero mismatches; the known `2x1 -> 4x1` false-proof
  counterexample remains on host control.
- [x] The receipt classifier changes are committed as `40c3e9860`,
  `635afb555`, and `cb1813bc8`. The latest guard proves step-bound
  pre-materialization validation errors, annotates retained setup telemetry
  with `pipeline_relevant=false`, and keeps prior deferred receipts
  conservative. `make migration-parity-receipt-test` passes 27/27; the full
  selected denominator remains 10,952 and public parity results retain their
  original schema without internal error fields.
- [x] The D-049 thumbnail control-flow fix is committed as `dc6085f81`:
  the expanded degenerate probe is 0 mismatches and all 172 thumbnail parity
  cases remain exact.
- [x] The focused post-merge checks pass: `make build-dev`, Rust F-resize
  tests 10/10, GPU-pool tests 21/21 (including Draw and indexed Fit), receipt tests 27/27, evidence/schema
  validation, and `make -C pillow-rs fmt`. Clippy remains blocked before
  compilation by the pre-existing pinned libavif 1.4.1/dav1d 1.5.3/libaom
  3.13.2 environment requirement.

## Closeout state

- [x] The source lane and receipt-partition correction are committed as
  `cb1813bc8`, the two-axis f64 GPU admission as `f17e1a7da`, the raw-color
  `ExtractBand` admission as `f55a770ad`, the raw-byte `EffectSpread`
  admission as `ebc7e765a`, raw-byte Draw admission as `7d1cc0af9`, and
  nearest indexed Fit admission as `0797e71f5`; the latest committed
  all-backend replay is
  schema-valid and value-exact for all 10,952 cases. Native lanes report 15
  genuine partial receipts and the GPU partition now has 6,832 native
  receipts plus 252 CPU receipts. No fixture, expected value,
  threshold, denominator, or case ID was changed.
- [ ] Do not mark the overall goal complete while P0, P1, or P2 remains open.
