# Exhaustive benchmark/backend row register

Use the focused execution queue in
[`benchmark-backend-pending-now-2026-08-31.md`](benchmark-backend-pending-now-2026-08-31.md)
for the current work order. This file retains the exact row inventory and
receipts for auditability.

The parity gate is green; only the guarded performance comparison remains
open. Use the focused checklist linked above for the execution order.

## Current state

- Standard benchmark: **744/744** workloads measured with complete parity
  receipts and 100% operation coverage.
- Full live-backend parity: CPU, SIMD, GPU, Node WASM, and browser WASM each
  passed **10,952/10,952**; GPU smoke passed **1/1**.
- Latest same-tree budget comparison: **197** timing-sensitive violations
  across 2,976 comparable rows (63 Pillow, 47 CPU, 78 SIMD, 9 GPU). The first
  after-reduce comparison had **56** (27 Pillow, 12 CPU, 15 SIMD, 2 GPU);
  timing-sensitive membership is retained rather than hidden, and no fixture,
  threshold, denominator, or backend-label changes are allowed.
- Goal state: **active** until a stable comparison closes the speed gate.

## Pending, in execution order

- [ ] **Repeat and close the budget gate.** Use paired, actual-backend
  measurements against `final-standard-after-native-geometry.json`. Keep every
  row and the nominal 5% comparator; do not hide timing violations.
- [ ] **Pillow rows (20, latest repeat):**
  `pil-image.open.standard`, `pil-imagefilter.modefilter.standard`,
  `pil-imagefont.imagefont.standard`,
  `pil-imagepalette-imagepalette.copy.standard`,
  `pipeline-chain.long-point.invert-64`, `pipeline-chain.matrix-067`,
  `pipeline-chain.resize-alpha.la-bicubic-256x256`,
  `pipeline-chain.terminal-read.analysis-scalar-if-1024x768`,
  `pipeline-matrix.expanded.add.256x256`,
  `pipeline-matrix.expanded.brightness.256x256`,
  `pipeline-matrix.expanded.darker.256x256`,
  `pipeline-matrix.expanded.gaussianblur.1x1`,
  `pipeline-matrix.expanded.reduce.1024x768`,
  `pipeline-matrix.expanded.resize.1x1`,
  `pipeline-op.composite.benchmark-materialized`,
  `pipeline-op.darker.matrix-32x24`, `pipeline-op.extractband.matrix-32x24`,
  `pipeline-op.filter3x3.matrix-32x24`,
  `pipeline-op.gaussianblur.benchmark-materialized`,
  `pipeline-op.putalphadata.matrix-32x24`.
- [ ] **CPU rows (24, latest repeat):**
  `pil-imagepalette-imagepalette.copy.standard`,
  `pil-imagepalette-imagepalette.getcolor.standard`,
  `pil-imagestat-stat.median.standard`, `pil-imagestat.stat.standard`,
  `pipeline-chain.blur-material.box-rgba-1024x768-radius-4`,
  `pipeline-chain.blur-material.gaussian-rgb-1024x768-radius-4`,
  `pipeline-chain.fused-chops.multiply-screen-distinct-secondary.la.1024x1024`,
  `pipeline-chain.geometry-copy.cropborder-la-1024x768`,
  `pipeline-chain.geometry-material.transverse-rgb-1024x768`,
  `pipeline-chain.loaded-10.rgba-png-512x384`,
  `pipeline-chain.point-fusion.l-002`,
  `pipeline-chain.resize-alpha.rgba-bilinear-mirror-256x256`,
  `pipeline-chain.simd-constant.1024x768`,
  `pipeline-chain.simd-crossover.invert-mirror.32x32`,
  `pipeline-chain.simd-lut.l.1024x768`,
  `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768`,
  `pipeline-chain.terminal-read.analysis-suite.rgb`,
  `pipeline-chain.terminal-read.imagestat.cmyk-1024x768`,
  `pipeline-matrix.expanded.rotate.1x1`, `pipeline-op.constant.matrix-32x24`,
  `pipeline-op.extractband.matrix-32x24`,
  `pipeline-op.fit.benchmark-materialized`,
  `pipeline-op.radialgradient.benchmark-materialized`,
  `pipeline-op.reduce.matrix-32x24`.
- [ ] **SIMD rows (19, latest repeat):**
  `pil-image-image.save.standard`,
  `pil-imagepalette-imagepalette.save.standard`,
  `pipeline-chain.blur-material.box-l-256x256-radius-0.5`,
  `pipeline-chain.color.getchannel-mode-la`,
  `pipeline-chain.geometry-copy.cropborder-la-1024x768`,
  `pipeline-chain.matrix-025`, `pipeline-chain.matrix-070`,
  `pipeline-chain.terminal-read.analysis-suite.rgb`,
  `pipeline-lifecycle.resident.multiply-screen.rgb-1024`,
  `pipeline-matrix.expanded.autocontrast.256x256`,
  `pipeline-matrix.expanded.rotate.256x256`,
  `pipeline-op.crop.matrix-32x24`, `pipeline-op.drawarc.matrix-32x24`,
  `pipeline-op.drawchord.matrix-32x24`,
  `pipeline-op.filter3x3.matrix-32x24`, `pipeline-op.paste.matrix-32x24`,
  `pipeline-op.putalpha.benchmark-materialized`,
  `pipeline-op.remappalette.benchmark-materialized`,
  `pipeline-op.rotate.matrix-32x24`.
- [ ] **GPU rows (8, latest repeat):** `pil-image-image.width.standard`,
  `pil-imagechops.add-modulo.standard`, `pil-imagefilter.rankfilter.standard`,
  `pil-imagefilter.sharpen.standard`, `pil-imagefilter.smooth.standard`,
  `pil-imagestat-stat.stddev.standard`,
  `pipeline-matrix.expanded.convert.1x1`,
  `pipeline-matrix.expanded.convert.32x32`.
- [ ] **Re-run evidence after every accepted change:** standard benchmark,
  budget check, performance report, benchmark coverage, roadmap status, and
  full all-backends parity.
- [ ] **Publish goal state:** commit only verified source/docs changes and push
  `main`; leave this goal active while the performance gate is open.

## Closed in the current wave

- [x] CPU repeated `ImageOps.invert` chains now collapse by parity while
  preserving copy-on-write identity results. Paired actual-CPU median improved
  0.188395→0.160771 ms (backend 0.022688→0.002208 ms); strict parity 24/24
  plus managed CPU 1/1. Commit `56877bdfa`.
- [x] SIMD zero-image Max/Min/Reduce and borderless Expand now use bounded
  exact native copies/fills. Paired actual-SIMD medians improved MaxFilter
  267,354→62,833 ns, Reduce 152,834→54,917 ns, and Expand 3,000→1,417 ns;
  strict parity 32/32. Commit `7bc416892`.
- [x] GPU uniform multiply→screen candidate was rejected: strict byte parity
  passed, but the authoritative resident row was cached and its paired median
  regressed 0.263708→0.267667 ms. No GPU source was integrated.
- [x] SIMD 5×5 convolution identity proof now scans uniform byte images up to
  256×256 for 5×5 filters only; 3×3, rank, and blur guards remain 64×64. The
  exact L=127 material row uses `Filter5x5: native-copy`, passed strict SIMD
  and all-backends parity, and cleared the guarded budget in both repeats.
  Source change is commit `d2e433ba3`.
- [x] CPU Reduce now serializes only sub-512×512 source images and keeps the
  established row-parallel path for larger sources. The 32×24 RGB reduction
  dropped from roughly 64 µs to 3 µs at the adapter boundary, passed strict CPU
  parity 10,952/10,952, and is absent from both after-reduce violation lists.
  The source-pixel guard also fixed the initial output-pixel-threshold
  regression on 1024×768 reductions; source change is commit `09fe72ee8`.

## Latest receipts

- Benchmark: `build/migration-parity/final-standard-after-reduce-source-threshold.json`
  (744/744; SHA-256
  `224b3f80800169327895e4139b4fe411c0d06f02ed86c0c3e4c559ebfffd15a9`).
- Same-tree repeat: `build/migration-parity/final-standard-after-reduce-source-threshold-repeat.json`
  (744/744; SHA-256
  `5512b16940d3cab4a76d18e1aac3998edff8ba3fbfc9271fcc78337c023972c5`).
- Benchmark parity sidecars: `final-standard-after-reduce-source-threshold-parity.json`
  SHA-256 `76b23eb3614991acc36f801536b6a8485964acce6508a2505235e2c1df39157b`;
  repeat SHA-256
  `a815b62ab274f1cb25808e79892a106955875433d0a47eca94221edbc667b2e8`.
- Budget checks: `pipeline-budget-check-after-reduce-source-threshold.json`
  (56; SHA-256
  `601ec408f53887a8b35ca67b0de956913f205101c5d2bbc2e59dd70a1b080fd6`) and
  `pipeline-budget-check-after-reduce-source-threshold-repeat.json` (197;
  SHA-256
  `a68699a9ea3135774b076883362c800daed43134d943f9624c04503fa151ad49`).
- Reports: `pipeline-performance-report-after-reduce-source-threshold-repeat.json`,
  `pipeline-benchmark-coverage-after-reduce-source-threshold-repeat.json`, and
  `pipeline-roadmap-status-after-reduce-source-threshold-repeat.json` (roadmap:
  14 closed, 50 open, 100% operation coverage).
- Full parity: `build/migration-parity/all-backends-after-reduce-wave2.json`
  (CPU/SIMD/GPU/Node/browser 10,952/10,952, GPU smoke 1/1; SHA-256
  `30f0ec37d0aef34256036e6b8ce7eacf307e3cd7ba76e76abc2d018eac260752`).

## Required checks before publishing

- `make fmt`
- `RUSTC_WRAPPER= make clippy`
- `make repo-map-check`
- `make migration-parity-fixtures-check`
- `make migration-parity-evidence-check`
- `git diff --check`
