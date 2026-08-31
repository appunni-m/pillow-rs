# Active parity-first checklist — benchmark/backend goal (2026-08-31)

This is the short execution queue for the active benchmark/backend goal. The
row-by-row evidence remains in
[`benchmark-backend-exhaustive-audit-2026-08-30.md`](benchmark-backend-exhaustive-audit-2026-08-30.md)
and the historical classification register remains in
[`benchmark-backend-pending-checklist-2026-08-30.md`](benchmark-backend-pending-checklist-2026-08-30.md).

## Goal state

**Active — parity first, then performance.** A row reported as
`NotImplemented`, `unsupported`, `not_proven`, or a failed terminal receipt is
not a completed backend result. It may be classified for diagnosis, but it
cannot close this goal. Do not remove inputs, lower denominators, relabel a
backend, or weaken a correctness/performance gate.

Current fixed evidence (revision `907c7a1ef`):

- Standard benchmark: 744/744 measured; 202/202 parity preflight passed.
- All-backend parity: CPU/SIMD/GPU/Node/browser 10,952/10,952; GPU smoke 1/1.
  GPU execution recorded 14,601 actual GPU receipts; host-control and typed
  mode rows remain open below.
- Final standard backend receipts: CPU 4,285; SIMD 4,385 actual SIMD plus
  6 CPU samples for `matrix-058`; GPU 4,267 actual GPU plus 18 exact-host
  samples across the three remaining GPU rows.
- The two same-tree standard comparisons recorded 112 and 56 timing
  violations; their stable intersection is 11 rows (the noisy union is not a
  performance conclusion).
- Merged source fixes: `e4cf004ad` (SIMD rank-window selector),
  `91fa214b2` (native-byte nearest resize), `e2dca2cf5` (native masked
  entropy/histogram), `4329e3e8e` (F-mode rank cutoff separation),
  `5c542cafb` (exact F-nearest GPU resize admission), `67a8375d2`
  (identity Fit lowered to a GPU Duplicate dispatch), and `907c7a1ef`
  (retain exact I-mode resize routing after the full-corpus regression gate).

## P0 — exact parity blockers (must close)

### CPU/SIMD completion mismatch (three exact IDs)

- [x] `pipeline-chain.loaded-10.rgba-png-512x384` — the stale logical-mode
  bug was fixed in ancestor `9ed4dadee`; the current focused run completes
  CPU/SIMD/GPU with exact RGB output (hash
  `7cdb79780776686f81239d2d0591cf13367fc064ac0b5277160c367d65b32d52`).
- [x] `pipeline-matrix.expanded.rotate.1x1` — current standard and focused
  runs complete actual SIMD with a scalar tail and exact parity.
- [x] `pipeline-matrix.expanded.add.1x1` — current standard and focused runs
  complete actual SIMD through the padded bytewise tail with exact parity.

The old audit's three completion-mismatch rows therefore do not reproduce on
the current revision; keep the exact cases as regression checks.

Acceptance for this block is met on the current revision: the three IDs have
equal source/target value contracts and the focused receipts prove actual
CPU/SIMD/GPU execution. Re-run them after every backend change.

### GPU exact implementation/routing block

The historical audit recorded 70 GPU failures. Most no longer reproduce after
the intervening GPU work, so do not carry that stale count into a new gate.
The focused post-fix receipts now close two of the five concrete GPU routing
rows; the remaining rows must be repaired or made exact before closure:

- [x] `pipeline-op.fit.benchmark-materialized` — identity Fit now lowers to
  one GPU `Duplicate` dispatch; focused receipt is 6/6 actual GPU with strict
  RGB parity.
- [ ] `pipeline-op.fit.matrix-32x24` — fractional crop plus default bicubic
  remains an exact-host path; implement a filter-exact GPU Fit shader before
  changing this disposition.
- [x] `pipeline-chain.resize-typed.simd-f-resize-transpose` — F-mode nearest
  resize plus transpose now uses two GPU dispatches; focused strict output is
  byte-exact (hash `af4d75fa7a8a71c71205096640167ab230ff32c2a81846f8ed54f41575715eed`).
- [ ] `pipeline-chain.resize-typed.simd-i-resize-transform` and
  `pipeline-chain.resize-cache.f64-identical-geometry` — still complete
  through CPU with `exact host semantic control`; implement exact typed GPU
  geometry or retain the explicit open blocker.

- [ ] Re-run the historical 70-row GPU matrix against the current revision
  after the current GPU changes; reopen only IDs that still produce a failed
  terminal workflow or a host-control fallback. The exact IDs and prior
  evidence remain in audit section 12, but they are not current failures until
  reproduced.

Acceptance for each current GPU row: focused exact case parity, a completed
terminal actual-GPU receipt with no fallback, strict all-backend parity, and a
regression case in the maintained generator. Historical rows must be rerun
against the current revision before being reopened.

### SIMD typed-conversion block

- [ ] `pipeline-chain.matrix-058` — the standard benchmark completes the
  requested SIMD profile through actual CPU because the RGBA→PA→RGB palette
  transition is not admitted by the SIMD converter. Add a typed palette plan
  (or a real vectorized palette-table path) and prove actual SIMD; do not call
  a CPU fallback a SIMD result.

## P1 — benchmark correctness and evidence (do not defer)

- [ ] Persist the exact exception, failing step, and requested/actual backend
  for every timed failure; a missing error is not `not_proven` evidence.
- [ ] Add terminal-completeness to receipts. A drained one-dispatch prefix is
  not a successful terminal workflow (currently affects Thumbnail and
  `pipeline-chain.matrix-021`).
- [ ] Compare suite timings only on equal workload-ID intersections and print
  the intersection/union/symmetric difference for every subject pair.
- [ ] Classify all 48 all-subject not-run inputs from the audit. Keep their
  matched-error/API coverage visible; only a generator-backed decision can
  remove an input from the performance denominator.
- [ ] Require every default performance workload to preflight to a successful
  Pillow value. Matched expected errors remain parity tests, not performance
  passes.

## P2 — stable performance cohorts after parity

Do not chase the noisy one-run union. The current stable intersection contains
11 rows and is the only timing cohort to investigate next:

- `pillow / pipeline-matrix.expanded.darker.256x256`
- `pillow / pipeline-matrix.expanded.equalize.1024x768`
- `pillow / pipeline-matrix.expanded.equalize.256x256`
- `pillow / pipeline-op.effectmandelbrot.benchmark-materialized`
- `python-cpu / pil-image.open.standard`
- `python-cpu / pil-imageops.mirror.standard`
- `python-cpu / pipeline-op.fit.benchmark-materialized`
- `python-simd / pil-image.open.standard`
- `python-simd / pipeline-chain.matrix-024`
- `python-simd / pipeline-chain.reviewed.draw-filter-invert`
- `python-simd / pipeline-op.remappalette.benchmark-materialized`

The GPU resident RGB multiply/screen +17%/+58% row remains a cached-read/setup
variance disposition, not proof of a measured multiply/screen regression.

The masked-analysis row is now materially faster in a targeted CPU run
(3.929 ms median versus 4.759 ms baseline) while the four exact masked cases
pass. The SIMD rank worker measured an exact 9×9 kernel improvement (~42%) and
strict Rank/Median parity 129/129. These are candidates until the paired full
reports confirm them.

## Verification and publication

- [x] Finish and validate the post-GPU-fix all-backend run (all six lanes pass;
  the pre-I-guard regression receipt is retained as historical evidence).
- [x] Commit the native masked entropy/histogram change after strict CPU,
  full all-backend, fixture, and evidence checks pass (`e2dca2cf5`).
- [x] Restore F-mode 9×9 SIMD RankFilter coverage after the 5×5 byte cutoff
  change (`4329e3e8e`); strict SIMD now passes 10,952/10,952.
- [x] Re-run the full standard benchmark after the GPU/I-mode fixes, then
  capture a same-tree repeat against the fixed baseline; retain both JSON
  receipts, parity sidecars, budget reports, hashes, and actual-backend fields.
- [ ] Require two consecutive budget reports with zero violations before
  marking the performance gate complete. Until then the goal status is
  **active**.
- [x] Run `make fmt`, `RUSTC_WRAPPER= make clippy-core`,
  `make repo-map-check`, `make migration-parity-fixtures-check`,
  `make migration-parity-evidence-check`, and `git diff --check`.
- [ ] Commit only verified source/docs changes and push `main`.

## Closed in this wave

- [x] `e4cf004ad` — SIMD 7×7/9×9 rank windows use the exact byte-domain
  selector; strict Rank/Median parity 129/129 and direct materialized 9×9
  measurement pass.
- [x] `91fa214b2` — nearest resize copies native L8/LA8/RGB8/RGBA8 bytes while
  preserving cumulative Pillow coordinates; direct parity passes and the CPU
  resize boundary improved 75.9% in the matched profile.
- [x] `09fe72ee8` — tiny CPU Reduce workloads avoid Rayon overhead while large
  source images remain row-parallel.
- [x] `5c542cafb` — F-mode nearest resize plus transpose is admitted to the
  GPU only for the byte-preserving nearest path; strict focused output is
  exact.
- [x] `67a8375d2` — identity Fit lowers to one GPU `Duplicate` dispatch while
  fractional/filter-specific Fits remain exact-host.
- [x] `907c7a1ef` — I-mode Resize stays on the exact host path after the full
  corpus caught four raw-shader convolution mismatches.

## Live receipts

- Baseline: `build/migration-parity/final-standard-after-native-geometry.json`.
- Pre-F-mode receipts: `final-standard-after-mask-resize-rank.json` (744/744;
  two SIMD F-rank rows failed), budgets 77 and 280.
- Pre-GPU-fix post-F-mode standard: `build/migration-parity/final-standard-after-f-rank.json`
  (744/744 measured; all subjects completed; SIMD F-rank fixed, one SIMD
  palette CPU fallback and five GPU host-control fallbacks remain).
- Pre-GPU-fix post-F-mode budget: `build/migration-parity/pipeline-budget-check-after-f-rank.json`
  (294 one-run violations; not a performance closure).
- Post-F-mode strict SIMD: `build/migration-parity/simd-strict-after-float-rank.json`
  (10,952/10,952).
- Post-F-mode pre-GPU-fix all-backend receipt:
  `build/migration-parity/all-backends-after-f-rank.json` (10,952/10,952;
  revision `4329e3e8e`; retained as historical evidence).
- Pre-I-guard regression receipt:
  `build/migration-parity/all-backends-after-gpu-fixes.json` (GPU 10,948/10,952;
  four I-mode resize mismatches; rejected and retained as the regression
  witness).
- Post-I-guard all-backend receipt:
  `build/migration-parity/all-backends-after-i-guard.json` (all six lanes pass;
  revision `907c7a1ef`; SHA-256
  `2866336c830632e6b6e96da6482ce05989ba830d9d97ac9403c331b9a7b05ced`).
- Final standard benchmark:
  `build/migration-parity/final-standard-after-gpu-fixes.json` (744/744;
  parity 202/202; SHA-256
  `2fa139baaa8294aceefa1c359b82da2d8d145f0b5aeac6ac95ae8c73aa0f1325`;
  parity sidecar SHA-256
  `22cd8808fff14e465825b932b9a83fcc002af6ea389b33b6de9cee80f5afa5b2`).
- Final same-tree repeat:
  `build/migration-parity/final-standard-after-gpu-fixes-repeat.json`
  (744/744; parity sidecar 202/202; SHA-256
  `dcbff52f0baaa2517099151c589cf788399a656d3194b5113e902836c24f9259`;
  parity sidecar SHA-256
  `b3d34729b12172c41aaecb1ba6f6556a922aef6c419d5aeb7b18f01d6cbc4e86`).
- Budget reports against the fixed baseline:
  `pipeline-budget-check-after-gpu-fixes.json` (112 violations) and
  `pipeline-budget-check-after-gpu-fixes-repeat.json` (56 violations); hashes
  are `f9bbbf2cf0156dd582837ff11706a837db9fdeb9810b5ea096b8c37f994e2c3d` and
  `261b6a0854613abad75f8751128777266f6556a6fd28aee4eaf01f46ced8d72b`.
  Zero violations are not yet proven.
