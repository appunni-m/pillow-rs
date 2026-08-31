# Focused pending checklist — benchmark/backend goal (2026-08-31)

This is the execution list for the current benchmark goal. The exhaustive
row-by-row inventory remains in
[`benchmark-backend-pending-checklist-2026-08-30.md`](benchmark-backend-pending-checklist-2026-08-30.md);
this file is intentionally short enough to run as a work queue.

## Goal

**Close the guarded performance gate without changing behavior or coverage.**
The goal stays **active**: parity is green, but the latest same-tree comparison
has 197 timing-sensitive regressions (the paired comparison had 56).
No public operation, input, denominator,
threshold, or backend result may be removed or relabeled to make the gate pass.

## Verified baseline

- Standard benchmark: 744/744 workloads, exact parity.
- Live backend parity: CPU, SIMD, GPU, Node WASM, and browser WASM each
  10,952/10,952; GPU smoke 1/1.
- Guarded comparison: the first after-reduce comparison has 56 violations
  across 2,976 comparable rows (27 Pillow, 12 CPU, 15 SIMD, 2 GPU). Its
  same-tree repeat has 197 timing-sensitive violations (63 Pillow, 47 CPU,
  78 SIMD, 9 GPU); both receipts are retained and neither changes the parity
  denominator.
- Baseline: `build/migration-parity/final-standard-after-native-geometry.json`.
- Current pair:
  `build/migration-parity/pipeline-budget-check-after-reduce-source-threshold.json`
  and `build/migration-parity/pipeline-budget-check-after-reduce-source-threshold-repeat.json`.

## Work queue

### 1. Stabilize the measurement gate first

- [ ] Run two paired, same-host standard repeats against the fixed baseline.
- [ ] Preserve every receipt, actual-backend field, fallback field, median, and
  p95; timing-sensitive row membership must be reported, not discarded.
- [ ] For each remaining row, record one disposition: Rust implementation,
  backend routing/receipt accounting, benchmark harness, or Pillow-side
  baseline. A disposition is not a closure.

**Done when:** both repeats are reproducible enough to compare the same cohorts,
and every row has an evidence-backed disposition.

### 2. CPU implementation cohort (12 rows in the primary comparison)

Inspect `pillow-rs/src/compute/pool_cpu/` and `pillow-rs/src/pipeline.rs` in
this order: `pipeline-chain.geometry-material.reduce-rgb-1024x768` (watch row),
`pipeline-chain.fused-chops.multiply-screen.rgba.256x256`,
`pipeline-chain.matrix-022`, `pipeline-op.grayscale.matrix-32x24`, then the
remaining rows below. `pipeline-op.reduce.matrix-32x24` is closed in this wave;
its small-output Rayon overhead is fixed without serializing large sources.

- [ ] `pil-image-image.save.standard`, `pil-image-image.transform.standard`,
  `pil-image.open.standard`, `pipeline-chain.color.convert-mode-f`,
  `pipeline-chain.matrix-054`, `pipeline-chain.reviewed.convert-rgba-cmyk-la`,
  `pipeline-chain.simd-constant.1024x768`,
  `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768`,
  `pipeline-chain.terminal-read.imagestat.cmyk-1024x768`,
  `pipeline-matrix.expanded.autocontrast.32x32`,
  `pipeline-matrix.expanded.convert.256x256`,
  `pipeline-op.lighter.matrix-32x24`

**Done when:** each accepted CPU change has a first-divergence note, strict
CPU parity, and a paired median improvement with no regression control.

### 3. SIMD implementation cohort (15 rows in the primary comparison)

Inspect `pillow-rs/src/compute/pool_simd/` and the adapter dispatch before
adding a new fast path. Prioritize the two reviewed chains, the 1024×768
crop-border row, and terminal-read analysis.

- [ ] `pipeline-chain.geometry-copy.cropborder-la-1024x768`,
  `pipeline-chain.matrix-002`, `pipeline-chain.matrix-020`,
  `pipeline-chain.matrix-022`, `pipeline-chain.matrix-058`,
  `pipeline-chain.metadata-cache.extractband-rgba`,
  `pipeline-chain.rank-filter.large-l-9x9`,
  `pipeline-chain.reviewed.crop-expand-mirror`,
  `pipeline-chain.reviewed.draw-filter-invert`,
  `pipeline-chain.reviewed.resize-rotate-crop`,
  `pipeline-matrix.expanded.brightness.256x256`,
  `pipeline-matrix.expanded.crop.1024x768`,
  `pipeline-op.effectmandelbrot.benchmark-materialized`,
  `pipeline-op.logicalxor.benchmark-materialized`,
  `pipeline-op.putdata.benchmark-materialized`

**Done when:** each accepted SIMD change has strict full-corpus parity
10,952/10,952 and a paired actual-SIMD improvement; rejected candidates stay
documented with their measurements.

### 4. GPU cohort and receipt accounting (2 rows in the primary comparison)

Inspect the actual-backend and fallback fields before changing a shader. The
rows are:

- [ ] `pipeline-chain.rank-filter.material.l-9x9-256x256`,
  `pipeline-lifecycle.resident.multiply-screen.rgb-1024`

**Done when:** each row has a real-device receipt or an explicitly measured
host-control path, plus strict byte parity and a paired GPU performance result;
cached or missing receipts do not count as proof.

### 5. Pillow-side and harness cohort (27 rows in the primary comparison)

- [ ] Reproduce the 27 Pillow regressions from the primary comparison (the
  repeat contains 63 timing-sensitive Pillow rows) and separate
  oracle timing, binding overhead, and benchmark setup from Rust execution.
- [ ] Primary row IDs: `pil-image-image.split.standard`,
  `pil-image-image.tell.standard`, `pil-imagefilter-color3dlut.repr.standard`,
  `pil-imagefilter.kernel.standard`, `pil-imagefilter.maxfilter.standard`,
  `pil-imagefilter.medianfilter.standard`, `pil-imagefilter.rankfilter.standard`,
  `pil-imagefilter.sharpen.standard`, `pil-imagefilter.smooth-more.standard`,
  `pil-imagefilter.smooth.standard`, `pil-imagefilter.unsharpmask.standard`,
  `pil-imagefont.imagefont.standard`, `pil-imagefont.transposedfont.standard`,
  `pil-imagepalette-imagepalette.tobytes.standard`, `pil-imagestat-stat.count.standard`,
  `pipeline-chain.color.convert-mode-la`, `pipeline-chain.long-point.invert-64`,
  `pipeline-chain.long-point.invert-8`, `pipeline-chain.matrix-058`,
  `pipeline-chain.reviewed.draw-batch-rgb-shapes`,
  `pipeline-chain.reviewed.draw-filter-invert`,
  `pipeline-chain.simd-vector-mirror.l.32x32`,
  `pipeline-matrix.expanded.effectspread.32x32`,
  `pipeline-op.alphacomposite.benchmark-materialized`,
  `pipeline-op.cover.benchmark-materialized`,
  `pipeline-op.grayscale.benchmark-materialized`,
  `pipeline-op.transpose.benchmark-materialized`.
- [ ] Keep the exact row IDs in the repeat receipt and the exhaustive register;
  do not “fix” this cohort by changing the denominator or comparator.
- [ ] If a harness defect is confirmed, fix the maintained generator/runner and
  regenerate receipts; if the Pillow timing is authoritative, leave the row
  visible as an open performance item.

**Done when:** every row has a reproducible cause and an approved source-level
fix or an evidence-backed measurement disposition.

### 6. Close and publish

- [ ] Re-run standard benchmark and full all-backends parity after every merged
  source change.
- [ ] Re-run budget, performance, coverage, and roadmap reports.
- [ ] Require two consecutive budget reports with zero violations before
  declaring the performance goal complete.
- [ ] Run `make fmt`, `RUSTC_WRAPPER= make clippy`,
  `make repo-map-check`, `make migration-parity-fixtures-check`,
  `make migration-parity-evidence-check`, and `git diff --check`.
- [ ] Commit only verified source/docs changes and push `main`.

## Closed in this wave

- [x] SIMD 5×5 convolution identity proof now scans uniform byte images up to
  256×256 for 5×5 filters only; 3×3, rank, and blur guards remain 64×64.
  The exact L=127 material row now uses `Filter5x5: native-copy`, passed strict
  SIMD and all-backends parity, and cleared the guarded budget in both repeats.
  Source change is commit `d2e433ba3`.
- [x] CPU Reduce now keeps tiny output workloads serial and uses the existing
  row-parallel path only when the source image is at least 512×512 pixels.
  The exact 32×24 RGB row measured ~3 µs at the adapter backend boundary after
  the change (about 64 µs before), passed strict CPU parity 10,952/10,952,
  remained absent from both guarded violation receipts, and did not change
  large-source routing. The WASM-only compile guard is covered by the passing
  `make build-wasm` run in this wave; source commit `09fe72ee8`.

## Status rule

The goal remains active until the zero-violation evidence gate is met. Parity
success is already recorded; it does not close the performance gate by itself.

## Latest receipts

- Full all-backends parity: `build/migration-parity/all-backends-after-reduce-wave2.json`
  (SHA-256 `30f0ec37d0aef34256036e6b8ce7eacf307e3cd7ba76e76abc2d018eac260752`).
- Primary benchmark/budget: `final-standard-after-reduce-source-threshold.json`
  / `pipeline-budget-check-after-reduce-source-threshold.json` (744/744;
  budget 56; hashes `224b3f80800169327895e4139b4fe411c0d06f02ed86c0c3e4c559ebfffd15a9`
  / `601ec408f53887a8b35ca67b0de956913f205101c5d2bbc2e59dd70a1b080fd6`).
- Same-tree repeat: `final-standard-after-reduce-source-threshold-repeat.json`
  / `pipeline-budget-check-after-reduce-source-threshold-repeat.json` (744/744;
  budget 197; hashes `5512b16940d3cab4a76d18e1aac3998edff8ba3fbfc9271fcc78337c023972c5`
  / `a68699a9ea3135774b076883362c800daed43134d943f9624c04503fa151ad49`).
- Full strict CPU parity: `build/migration-parity/cpu-strict-after-reduce.json`
  (10,952/10,952; SHA-256 `efda569adc6e357b1b65e65ea70e45e8405b13a8f00ecaadce86580a21a35768`).
