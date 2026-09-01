# Benchmark/backend pending checklist — 2026-09-01

This is the only active queue. The longer audit and the previous status
snapshot remain available for history:

- [exhaustive audit](benchmark-backend-exhaustive-audit-2026-08-30.md)
- [previous focused snapshot](benchmark-backend-pending-now-2026-08-31.md)

## Goal

Keep the overall parity goal **active**: exact Pillow values first, honest
native-backend evidence second, performance acceptance last. Do not remove,
rename, relabel, or weaken a case to make a gate green.

## Evidence already closed

- Public all-backend comparisons are **10,952/10,952** for CPU, SIMD, GPU,
  Node WASM, and browser WASM; GPU smoke is **1/1**.
- The three historical benchmark-only mismatch workloads reproduce as passes
  on current source: `pipeline-chain.loaded-10.rgba-png-512x384`,
  `pipeline-matrix.expanded.rotate.1x1`, and
  `pipeline-matrix.expanded.add.1x1`.
- Same-size filtered F resizes and bounded 2:1 Box F resizes have direct
  native-GPU byte proofs, including non-finite/negative-zero coverage where
  the proof admits it.
- Finite nonconstant F Box upscales have a separate exact copy proof for
  arbitrary non-downscaling geometry, including mixed `PutData(F)` plus Box
  chains (144/144 direct native-GPU samples).
- Finite heterogeneous F bicubic arithmetic now matches Pillow's Horner/FMA
  evaluation and coefficient rounding: the focused matrix is 9,000/9,000,
  with the maintained F resize slice at 23/23 after the correction.
- F-mode nearest resize now follows Pillow's cumulative f64 affine stepping
  and copies the opaque sample word on GPU, preserving boundary selection,
  NaN, infinity, and signed zero. The focused F parity slice is 13/13, and
  the native GPU regression covers finite and special-value 1x2→1x7 cases.
- RGBa `ImageOps.fit` now uses the existing exact boxed coefficient path on
  GPU. The strict Fit matrix is 89/89 value-exact, with the formerly excluded
  RGBa case included in the 6/6 native-receipt subset.
- `ImageDraw.rounded_rectangle` now rejects reversed boxes at the public Rust
  boundary in Pillow's x-first/y-second order. Previously a radius-zero box
  with only a reversed y axis reached the empty-span rectangle kernel and was
  silently accepted; the focused regression and Python facade probe now match
  Pillow's `ValueError` messages.
- `ImageDraw` geometry now keeps `I;16`, `I;16L`, `I;16B`, and `I;16N` on a
  native Luma16 canvas instead of widening them to RGBA8. Default shape ink,
  packed integer colors, declared byte order, and the small Pillow distinction
  between line/point and area/arc draw paths are covered by core and Python
  probes against all four modes.
- `Image.convert` and image-source `Image.paste` now materialize an `I;16*`
  destination through typed unsigned-16 samples. An `L` sample of 17 remains
  17 (not 4369), the declared byte order is retained, and the broader
  `L`/`LA`/`RGB`/`RGBA`/`CMYK`/`HSV`/`YCbCr`/`I`/`F`/`1`/`PA` source matrix
  matches Pillow's values and errors. CPU, SIMD, and GPU paste probes are
  exact; the maintained six-case I;16 paste slice passes on all three lanes.
- `ImageQt` row alignment now accepts zero-width rows and returns the original
  bytes, matching Pillow's already-aligned early return for empty and
  caller-provided buffers. The Rust helper regression and Python facade probe
  are exact for 1/L/P/I;16-width-zero inputs and ordinary padded rows.
- `ImageColor.getcolor` now validates Pillow's complete mode-descriptor set,
  raises `KeyError` for unknown mode names, and preserves mapped integer
  storage modes plus lowercase-`La` scalar behavior. Core tests and a broad
  native Python mode/error matrix are exact.
- `ImageEnhance.Contrast` now preserves valid zero-area CMYK images through
  the grayscale conversion and back to CMYK. The focused matrix is exact for
  `(0,0)`, `(0,3)`, and `(3,0)` across factors `0`, `.5`, `1`, and `2`.
- `Image.thumbnail` now preserves Pillow's zero-source control flow and error
  ordering. A 5-source × 20-request integer degenerate probe went from 21
  mismatches to 0; the maintained 7-case edge slice and all 172 thumbnail
  parity cases remain exact.
- `ImageOps.scale` now preserves Pillow's empty-image and factor-one control
  flow, including `inf * 0 -> NaN` validation order. The empty-image matrix is
  exact (0/72 mismatches after the fix), and the maintained D-002 cases remain
  3/3.
- `Image.putdata` now keeps Pillow's per-item order for mixed exact multiband
  values. A packed integer is committed before a later scalar float raises;
  the focused RGB/RGBA/CMYK prefix probe is exact (3/3), with no changes to
  the existing callback or oversized-input contracts.
- `Image.merge` now follows Pillow's typed and alias mode matrix. The native
  64-case matrix covering `1`, `I`, `F`, `P`, `La`, `PA`, `RGBX`, `RGBa`,
  `YCbCr`, `HSV`, and `LAB` went from 42 mismatches to 0; LAB A/B storage
  keeps Pillow's +128 byte bias while public reads decode it. First-band
  palette acceptance and later-band rejection are exact. `I;16`, `I;16L`,
  `I;16B`, and `I;16N` merge now preserve typed samples, declared byte order,
  exact mode spelling, and Pillow's single-band validation; focused core and
  Python probes are exact for all four variants.
- The backend receipt classifier now recognizes the public `ImageOps.scale`
  factor-one copy path. Pillow and Rust both return an eager copy before
  resample parsing, so the six factor-one scale workflows are correctly
  outside the deferred-pipeline denominator instead of being reported as
  missing receipts. The focused receipt suite is 19/19, the targeted
  all-backend parity slice is 6/6 on every parity lane, and its partition is
  `pipeline_not_applicable=6`, `pipeline_missing_receipt=0`. The full aggregate
  was then regenerated after the follow-up classifier correction recorded
  below.
- The receipt classifier now distinguishes an explicit public-call error at
  the first deferred-looking operation from a dependency-only `not_run`
  boundary. Pillow validates these arguments before constructing a lazy node;
  the dependent observation is therefore not evidence that pipeline work ran.
  Earlier deferred nodes and dependency-only failures remain conservative. The
  focused receipt suite is 19/19, and the committed full rerun reclassifies 90
  exact error cases while retaining 343 indeterminate native cases.
- `ImageFilter.Kernel` now preserves Pillow's raw `f32` scale and offset,
  including fractional, zero, negative, and non-finite values, and the 5x5
  GPU rows use Pillow's bottom-to-top kernel layout. The CPU matrix is exact
  (1,344/1,344 after 692 mismatches), SIMD is 180/180, GPU byte cases are
  180/180 plus 500 randomized cases, and the focused suite is 28/28. GPU
  admits only finite nonzero scales and integer-representable offsets; other
  values route through the exact host path. A follow-up GPU accumulation-order
  correction keeps 5x5 row additions as dependent `fma` steps; the full GPU
  lane is now 10,952/10,952. An arbitrary I-mode f32/i32 edge remains outside
  the public Kernel mode manifest and is explicitly tracked.
- The post-integration live-oracle parity gate at source `a900ec6f4` remains
  fully exact: 10,952/10,952 selected and passed, with zero failures,
  not-run cases, or infrastructure errors. The reproducible output is
  `/tmp/pillow-rs-after-a900-parity.json` (SHA-256
  `13463f29f7e8816c882f2a92e4e9735538a49061841bc65694cea7e6c99d0210`).
- The combined post-merge/kernel live-oracle gate is also exact: 10,952/10,952
  selected and passed, with zero failures, not-run cases, or infrastructure
  errors. Its temporary output is `/tmp/pillow-rs-post-merge-kernel-parity.json`
  (SHA-256 `fac150334b05965b4e662b1be4850c80509e42605b1ccfec968c4d148bb34f62`).
- The final schema-v3 all-backend envelope at source `7983d9406` is
  parity-green across CPU, SIMD, GPU, Node WASM, and browser WASM (each
  10,952/10,952), with GPU smoke 1/1. Its temporary output is
  `/tmp/all-backends-final-7983.json` (SHA-256
  `468a22e8a589d4a7a9dd9d7f7b53af43254ba7097d9529e85b7d9aa48b75c6ab`). The
  aggregate remains `passed_with_backend_gaps`: CPU/GPU each have 7,084
  terminal-complete receipts, 102 partial, 6 missing, 3,327 not-applicable,
  and 433 indeterminate cases; SIMD has 7,096 complete, 102 partial, 6
  missing, 3,315 not-applicable, and 433 indeterminate cases. GPU has 6,693
  device and 391 host-CPU receipts, with the recorded Transform, dimension,
  host-semantic, Contrast-midpoint, and logical-mode fallback categories.
- The proof-gated dyadic F lane is exact/native for the admitted Bilinear,
  narrow two-tap Bicubic/Lanczos/Hamming, one- or two-axis power-of-two Box,
  and chained all-Box cases; every admission is bounded by fixed/f64 row
  agreement and the source significand-span check. Heterogeneous/non-dyadic
  inputs remain on exact host control.

## Pending — do these in order

### P0 — exact F-mode GPU resize arithmetic

- [x] Implement and prove the bounded dyadic subset: fixed/f64 coefficient
  agreement, same-sign normal power-of-two F words, Bilinear, narrow
  two-tap Bicubic/Lanczos/Hamming rows, one- or two-axis power-of-two Box
  reductions through 64:1, and chained all-Box passes (with the cumulative
  significand-span bound). The direct native matrix is byte-exact with
  terminal `actual_backend=gpu` receipts and no fallback (9 admitted cases).
- [x] Keep the finite nonconstant Box-upscale copy lane admitted for arbitrary
  non-downscaling geometry; its one-tap relocation proof is separate from
  arithmetic-filter admission.
- [x] Re-run the broader heterogeneous/non-dyadic F arithmetic probes before
  changing admission. Forty finite cases (eight geometries × five filters)
  plus signed-zero/edge probes are byte-exact through the current
  host-controlled route. A disposable forced-generic-shader run diverges by
  ULPs on heterogeneous Bilinear/Bicubic/Lanczos/Hamming and non-dyadic Box;
  a 2×1→4×1 Bilinear counterexample also breaks a broad dyadic-source proof.
  Keep these inputs on exact host semantic control until a verified f64-
  equivalent device accumulator exists; no safe source admission change was
  found.
- [ ] Extend native-GPU exact arithmetic coverage to heterogeneous/non-dyadic
  Bilinear, broader Bicubic/Lanczos/Hamming rows, Box downscales outside the
  proven dyadic row limits, and chains outside the cumulative
  significand-span proof. CPU bicubic parity is fixed; keep every unproven
  device arithmetic input on exact host control, including NaN, infinity,
  and negative zero.

### P1 — honest backend-proof denominator

- [x] The receipt sidecars now emit schema `pipeline-execution-evidence@2`
  with one status for every selected case: `complete`, `partial_receipt`,
  `missing_receipt`, `not_applicable`, or `indeterminate`. The summary keeps
  the historical no-receipt counts and adds a partition whose total remains
  the fixed **10,952** public cases. Only high-confidence non-pipeline paths
  leave the backend-proof cohort; missing, partial, and indeterminate paths
  remain proof gaps. The all-backend envelope stays schema-v3, and old @1
  sidecars are diagnostic-only until regenerated.
- [x] Regenerate and review the schema-v3 all-backend artifact at the
  committed source `5cc713f99`. CPU/GPU report **7,090 complete + 102
  partial + 0 missing + 3,327 not applicable + 433 indeterminate**; SIMD
  reports **7,102 complete + 102 partial + 0 missing + 3,315 not applicable +
  433 indeterminate**. All six public lanes remain 10,952/10,952 and GPU
  smoke is 1/1; the aggregate is correctly `passed_with_backend_gaps`.
  Artifact SHA-256: `93eba42234b785614daf7f8cc8651fd04731607de6934bb5f46a74c78e808672`.
- [x] Fix the receipt boundary and classifier gaps without changing IDs or
  denominators: an observed final serialization may prove an earlier
  dispatch, while eager filter constructors/`ModeFilter`, source-backed
  conversion/no-op paths, and source-independent degenerate or out-of-bounds
  crops are not deferred pipeline work. Canonical Stat/getbbox/getdata
  observations and public crop validation now remove fixture-only prefixes;
  the final run keeps CPU/GPU at 102 partial + 0 missing and SIMD at 102
  partial + 0 missing, with 433 indeterminate cases in each native lane.
  Receipt/evidence regression tests pass (19/19).
- [x] Classify the six factor-one `ImageOps.scale` workflows as eager copies.
  The rule is argument-sensitive (`factor == 1.0` only); a non-identity scale
  remains deferred and still requires a terminal receipt. The targeted six-case
  all-backend gate is parity-green on CPU, SIMD, GPU, Node WASM, and browser
  WASM; the native receipt partition has no missing cases in this slice.
- [x] Regenerate the complete schema-v3 all-backend envelope after the
  factor-one classifier correction at source `1dc515445`. CPU/GPU now report
  **7,084 complete + 102 partial + 0 missing + 3,333 not applicable + 433
  indeterminate**; SIMD reports **7,096 + 102 + 0 + 3,321 + 433**. Node and
  browser WASM report **6,713 complete + 586 partial + 888 missing + 2,713
  not applicable + 52 indeterminate**. Every value lane remains 10,952/10,952
  and GPU smoke is 1/1; the aggregate correctly remains
  `passed_with_backend_gaps`. Artifact SHA-256:
  `56dcf71a65f169576a8bc077e630748bfc0415991f0d5696efea6670b4946c18`.
- [x] Regenerate the complete schema-v3 all-backend envelope after the
  explicit-error classifier correction at source `143ad86d9`. CPU/GPU now
  report **7,084 complete + 102 partial + 0 missing + 3,423 not applicable +
  343 indeterminate**; SIMD reports **7,096 + 102 + 0 + 3,411 + 343**. Node
  and browser WASM remain **6,713 complete + 586 partial + 888 missing +
  2,713 not applicable + 52 indeterminate**. Every value lane is still
  10,952/10,952 and GPU smoke is 1/1; the aggregate remains
  `passed_with_backend_gaps`. Artifact SHA-256:
  `e3edd78e6421aff1cd168fdf0931d1344c8382a1e19d3d05e73bb6043a114131`.
- [ ] Keep the aggregate `passed_with_backend_gaps` until every claimed native
  cohort has complete terminal receipts, matching case-ID digests, requested
  actual backends, and an empty fallback taxonomy.

### P2 — performance acceptance

- [x] Bound GPU working-buffer reuse to four times the requested capacity;
  the controlled small-draw case dropped from about 2.4 ms with a 6.3 MiB
  retained pool to about 0.59 ms with a 19 KiB pool, with exact/native output.
- [x] Elide the factor-1.0 Brightness scan for native byte layouts. The
  mode-guarded identity path is exact across nine byte modes and the focused
  Brightness parity lane remains 7/7; CPU medians improved from about
  0.181/0.163 ms to 0.042/0.049/0.042 ms.
- [ ] Run the same equal-ID, equal-receipt cohort twice consecutively with
  **zero** budget violations. The Brightness optimization is a deterministic
  row-level improvement, but the aggregate cohort remains noise-sensitive:
  the latest fixed-11-ID comparisons retain 44/44 comparable subjects and
  report 3, 4, and 6 violations across repeated baselines/posts. Timing
  acceptance remains open.

## Required closeout

- [x] Run the maintained focused lanes, full strict all-backend parity, receipt
  and evidence validators, and format/core-lint checks. Push the corresponding
  source and checklist commits only after the final artifact is validated.

Last committed all-backend artifact source: `143ad86d9` (the artifact above
was generated there; the working tree had pre-existing unrelated changes).
The latest integrated parity source is `efc734896` (I;16 merge parity on top
of the GPU accumulation-order fix `7983d9406`, D-048 Kernel fix `2c2b2d1ba`,
D-039 merge fix `5be0fd7a5`, and D-044 putdata fix `a900ec6f4`). The focused
69-test binding suite,
combined full live-oracle parity gate, and
final all-backend parity envelope are green; backend-proof completion, broader
`I;16*` backend receipt coverage, and timing acceptance remain required. The
overall goal is intentionally **active**.
