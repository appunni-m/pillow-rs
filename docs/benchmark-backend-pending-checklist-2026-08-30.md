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
- Latest same-tree budget comparison: **71** violations across 2,976
  comparable rows (20 Pillow, 24 CPU, 19 SIMD, 8 GPU). The immediately
  preceding repeat had 51; timing-sensitive membership is retained rather than
  hidden, and no fixture, threshold, denominator, or backend-label changes are
  allowed.
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
  Source change is the current working-tree candidate.

## Latest receipts

- Benchmark: `build/migration-parity/final-standard-after-conv-wave.json`
  (744/744; SHA-256
  `4baf4dbb7dfe941948b5b64a83181aa258d04f9e8075889d54ba1bc294a00d6e`).
- Same-tree repeat: `build/migration-parity/final-standard-after-conv-wave-repeat.json`
  (744/744; SHA-256
  `40b413ad0c901815d680f21a311cd9885a591926429d8987c7dfe428d0950dbb`).
- Benchmark parity sidecars: `final-standard-after-conv-wave-parity.json`
  SHA-256 `186fe79f5c516630df2f0db7e8bcac80b4ade60a00cab76b05cc36a142b9c848`;
  repeat SHA-256
  `16f57bfb92a96b2797fb68d52e0e67c66958e63ee003095d43c86e13c087ef32`.
- Budget checks: `pipeline-budget-check-after-conv-wave.json` (51;
  SHA-256 `9ca1295ebd9324996ccd3fe6e2d171b126e00ce4612cb1bbb1ded497c8326b9d`)
  and `pipeline-budget-check-after-conv-wave-repeat.json` (71;
  SHA-256 `f967d5234d43fcb1ec0295e69f8d0b71ec6144ac906081cff1da3ed789d1b980`).
- Reports: `pipeline-performance-report-after-conv-wave-repeat.json`,
  `pipeline-benchmark-coverage-after-conv-wave-repeat.json`, and
  `pipeline-roadmap-status-after-conv-wave-repeat.json` (roadmap: 14 closed,
  50 open, 100% operation coverage).
- Full parity: `build/migration-parity/all-backends-after-conv-wave.json`
  (SHA-256
  `bf9c3008510c3265c70449671dd435407a65e594ccdf401efe5bc0aeeeb7a077`).

## Required checks before publishing

- `make fmt`
- `RUSTC_WRAPPER= make clippy`
- `make repo-map-check`
- `make migration-parity-fixtures-check`
- `make migration-parity-evidence-check`
- `git diff --check`
