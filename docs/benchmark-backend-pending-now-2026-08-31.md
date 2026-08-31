# Focused pending checklist — benchmark/backend goal (2026-08-31)

This is the execution list for the current benchmark goal. The exhaustive
row-by-row inventory remains in
[`benchmark-backend-pending-checklist-2026-08-30.md`](benchmark-backend-pending-checklist-2026-08-30.md);
this file is intentionally short enough to run as a work queue.

## Goal

**Close the guarded performance gate without changing behavior or coverage.**
The goal stays **active**: parity is green, but the latest same-tree comparison
still has 71 timing regressions (the immediately preceding repeat had 51).
No public operation, input, denominator,
threshold, or backend result may be removed or relabeled to make the gate pass.

## Verified baseline

- Standard benchmark: 744/744 workloads, exact parity.
- Live backend parity: CPU, SIMD, GPU, Node WASM, and browser WASM each
  10,952/10,952; GPU smoke 1/1.
- Guarded comparison: 71 violations across 2,976 comparable rows in the latest
  repeat (20 Pillow, 24 CPU, 19 SIMD, 8 GPU); the first repeat after the change
  had 51 (20 Pillow, 12 CPU, 13 SIMD, 6 GPU).
- Baseline: `build/migration-parity/final-standard-after-native-geometry.json`.
- Current repeat:
  `build/migration-parity/pipeline-budget-check-after-conv-wave-repeat.json`.

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

### 2. CPU implementation cohort (24 rows in the latest repeat)

Inspect `pillow-rs/src/compute/pool_cpu/` and `pillow-rs/src/pipeline.rs` in
this order: `pipeline-op.reduce.matrix-32x24`,
`pipeline-chain.fused-chops.multiply-screen.rgba.256x256`,
`pipeline-chain.matrix-022`, `pipeline-op.grayscale.matrix-32x24`, then the
remaining rows below.

- [ ] `pil-imagepalette-imagepalette.copy.standard`,
  `pil-imagepalette-imagepalette.getcolor.standard`,
  `pil-imagestat-stat.median.standard`, `pil-imagestat.stat.standard`
- [ ] `pipeline-chain.blur-material.box-rgba-1024x768-radius-4`,
  `pipeline-chain.blur-material.gaussian-rgb-1024x768-radius-4`,
  `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.la.1024x1024`,
  `pipeline-chain.geometry-copy.cropborder-la-1024x768`
- [ ] `pipeline-chain.geometry-material.transverse-rgb-1024x768`,
  `pipeline-chain.loaded-10.rgba-png-512x384`,
  `pipeline-chain.point-fusion.l-002`,
  `pipeline-chain.resize-alpha.rgba-bilinear-mirror-256x256`
- [ ] `pipeline-chain.simd-constant.1024x768`,
  `pipeline-chain.simd-crossover.invert-mirror.32x32`,
  `pipeline-chain.simd-lut.l.1024x768`,
  `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768`
- [ ] `pipeline-chain.terminal-read.analysis-suite.rgb`,
  `pipeline-chain.terminal-read.imagestat.cmyk-1024x768`,
  `pipeline-matrix.expanded.rotate.1x1`, `pipeline-op.constant.matrix-32x24`,
  `pipeline-op.extractband.matrix-32x24`,
  `pipeline-op.fit.benchmark-materialized`,
  `pipeline-op.radialgradient.benchmark-materialized`,
  `pipeline-op.reduce.matrix-32x24`

**Done when:** each accepted CPU change has a first-divergence note, strict
CPU parity, and a paired median improvement with no regression control.

### 3. SIMD implementation cohort (19 rows in the latest repeat)

Inspect `pillow-rs/src/compute/pool_simd/` and the adapter dispatch before
adding a new fast path. Prioritize the two reviewed chains, the 1024×768
crop-border row, and terminal-read analysis.

- [ ] `pil-image-image.save.standard`,
  `pil-imagepalette-imagepalette.save.standard`,
  `pipeline-chain.blur-material.box-l-256x256-radius-0.5`
- [ ] `pipeline-chain.color.getchannel-mode-la`,
  `pipeline-chain.geometry-copy.cropborder-la-1024x768`,
  `pipeline-chain.matrix-025`, `pipeline-chain.matrix-070`
- [ ] `pipeline-chain.terminal-read.analysis-suite.rgb`,
  `pipeline-lifecycle.resident.multiply-screen.rgb-1024`,
  `pipeline-matrix.expanded.autocontrast.256x256`,
  `pipeline-matrix.expanded.rotate.256x256`
- [ ] `pipeline-op.crop.matrix-32x24`, `pipeline-op.drawarc.matrix-32x24`,
  `pipeline-op.drawchord.matrix-32x24`, `pipeline-op.filter3x3.matrix-32x24`,
  `pipeline-op.paste.matrix-32x24`,
  `pipeline-op.putalpha.benchmark-materialized`,
  `pipeline-op.remappalette.benchmark-materialized`,
  `pipeline-op.rotate.matrix-32x24`

**Done when:** each accepted SIMD change has strict full-corpus parity
10,952/10,952 and a paired actual-SIMD improvement; rejected candidates stay
documented with their measurements.

### 4. GPU cohort and receipt accounting (8 rows in the latest repeat)

Inspect the actual-backend and fallback fields before changing a shader. The
rows are:

- [ ] `pil-image-image.width.standard`,
  `pil-imagechops.add-modulo.standard`, `pil-imagefilter.rankfilter.standard`
- [ ] `pil-imagefilter.sharpen.standard`, `pil-imagefilter.smooth.standard`,
  `pil-imagestat-stat.stddev.standard`
- [ ] `pipeline-matrix.expanded.convert.1x1`,
  `pipeline-matrix.expanded.convert.32x32`

**Done when:** each row has a real-device receipt or an explicitly measured
host-control path, plus strict byte parity and a paired GPU performance result;
cached or missing receipts do not count as proof.

### 5. Pillow-side and harness cohort (20 rows in the latest repeat)

- [ ] Reproduce the 20 Pillow regressions from the latest repeat and separate
  oracle timing, binding overhead, and benchmark setup from Rust execution.
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

## Status rule

The goal remains active until the zero-violation evidence gate is met. Parity
success is already recorded; it does not close the performance gate by itself.
