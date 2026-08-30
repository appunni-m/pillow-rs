# Active benchmark/backend checklist

This is the short, current checklist for the exhaustive benchmark goal. The
parity gate is green; only the guarded performance comparison remains open.

## Current state

- Standard benchmark: **744/744** workloads measured with complete parity
  receipts and 100% operation coverage.
- Full live-backend parity: CPU, SIMD, GPU, Node WASM, and browser WASM each
  passed **10,952/10,952**; GPU smoke passed **1/1**.
- Latest same-tree budget comparison: **81** violations across 2,976
  comparable rows (41 Pillow, 19 CPU, 16 SIMD, 5 GPU). The repeat is timing
  sensitive, so the gate stays open; no fixture, threshold, denominator, or
  backend-label changes are allowed.
- Goal state: **active** until a stable comparison closes the speed gate.

## Pending, in execution order

- [ ] **Repeat and close the budget gate.** Use paired, actual-backend
  measurements against `final-standard-after-native-geometry.json`. Keep every
  row and the nominal 5% comparator; do not hide timing violations.
- [ ] **Pillow rows (41, latest repeat):**
  `pil-imagefilter.modefilter.standard`,
  `pil-imagefont-freetypefont.font-variant.standard`,
  `pil-imagefont-freetypefont.get-variation-axes.standard`,
  `pil-imagefont-freetypefont.get-variation-names.standard`,
  `pil-imagefont.imagefont.standard`,
  `pil-imagepalette-imagepalette.getdata.standard`,
  `pil-imagesequence-iterator.next.standard`,
  `pil-imagestat-stat.count.standard`, `pipeline-chain.long-point.invert-8`,
  `pipeline-chain.matrix-020`, `pipeline-chain.matrix-085`,
  `pipeline-chain.resize-alpha.la-bicubic-256x256`,
  `pipeline-chain.terminal-read.analysis-scalar-if-1024x768`,
  `pipeline-chain.terminal-read.getcolors.rgb-1024x768`,
  `pipeline-chain.terminal-read.imagestat.i-1024x768`,
  `pipeline-matrix.expanded.reduce.1024x768`,
  `pipeline-matrix.expanded.resize.32x32`,
  `pipeline-op.autocontrast.matrix-32x24`,
  `pipeline-op.blendmodule.matrix-32x24`,
  `pipeline-op.color3dlut.matrix-32x24`,
  `pipeline-op.compositemodule.matrix-32x24`,
  `pipeline-op.contrast.matrix-32x24`,
  `pipeline-op.cover.benchmark-materialized`,
  `pipeline-op.cover.matrix-32x24`,
  `pipeline-op.cropborder.matrix-32x24`,
  `pipeline-op.drawchord.benchmark-materialized`,
  `pipeline-op.drawchord.matrix-32x24`,
  `pipeline-op.drawpieslice.matrix-32x24`,
  `pipeline-op.drawpoint.matrix-32x24`,
  `pipeline-op.equalize.matrix-32x24`, `pipeline-op.eval.matrix-32x24`,
  `pipeline-op.expand.matrix-32x24`,
  `pipeline-op.extractband.matrix-32x24`,
  `pipeline-op.fit.benchmark-materialized`,
  `pipeline-op.grayscale.matrix-32x24`,
  `pipeline-op.maxfilter.benchmark-materialized`,
  `pipeline-op.merge.matrix-32x24`, `pipeline-op.multiply.matrix-32x24`,
  `pipeline-op.pointop.matrix-32x24`,
  `pipeline-op.putalphadata.matrix-32x24`,
  `pipeline-op.scale.benchmark-materialized`.
- [ ] **CPU rows (19, latest repeat):**
  `pil-image-image.save.standard`, `pil-image.open.standard`,
  `pil-imagefont-freetypefont.get-variation-axes.standard`,
  `pipeline-chain.blur-material.box-l-1024x768-radius-4`,
  `pipeline-chain.fused-chops.multiply-screen.l.1024x1024`,
  `pipeline-chain.fused-chops.multiply-screen.rgba.256x256`,
  `pipeline-chain.matrix-022`, `pipeline-chain.matrix-033`,
  `pipeline-chain.matrix-083`,
  `pipeline-chain.resize-alpha.rgba-bilinear-mirror-256x256`,
  `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768`,
  `pipeline-chain.terminal-read.imagestat.cmyk-1024x768`,
  `pipeline-matrix.expanded.crop.256x256`,
  `pipeline-matrix.expanded.pointop.1x1`,
  `pipeline-op.addmodulo.matrix-32x24`,
  `pipeline-op.grayscale.matrix-32x24`,
  `pipeline-op.putalpha.matrix-32x24`, `pipeline-op.reduce.matrix-32x24`,
  `pipeline-op.softlight.matrix-32x24`.
- [ ] **SIMD rows (16, latest repeat):**
  `pil-image-image.save.standard`, `pipeline-chain.color.convert-mode-i`,
  `pipeline-chain.convolution.material.l-5x5-scale.256x256`,
  `pipeline-chain.matrix-010`, `pipeline-chain.matrix-024`,
  `pipeline-chain.quantize.linear-gradient`,
  `pipeline-chain.quantize.radial-gradient`,
  `pipeline-chain.reviewed.draw-filter-invert`,
  `pipeline-chain.reviewed.resize-rotate-crop`,
  `pipeline-chain.terminal-read.analysis-suite.rgb`,
  `pipeline-matrix.expanded.crop.1024x768`,
  `pipeline-op.blend.matrix-32x24`,
  `pipeline-op.contrast.benchmark-materialized`,
  `pipeline-op.cropborder.benchmark-materialized`,
  `pipeline-op.extractband.matrix-32x24`,
  `pipeline-op.solarize.matrix-32x24`.
- [ ] **GPU rows (5, latest repeat):** `pil-image-image.width.standard`,
  `pil-imagechops.add.standard`, `pil-imagefilter.rankfilter.standard`,
  `pil-imagefilter.sharpen.standard`, `pil-imagefilter.smooth.standard`.
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

## Latest receipts

- Benchmark: `build/migration-parity/final-standard-after-next-wave.json`
  (744/744; SHA-256
  `8c6a06999de2dfb101df8057278591c98eb7e4e3482b168b37cc80bd119ade6d`).
- Same-tree repeat: `build/migration-parity/final-standard-after-next-wave-repeat.json`
  (744/744; SHA-256
  `f44a4c1b259f7cf95ae84b7cf9cce6c9d16da811ae85b1f79aee0fd1de380b07`).
- Benchmark parity sidecars: `final-standard-after-next-wave-parity.json`
  SHA-256 `13c6eb0ebd493cd7c318ff112bbf1a8f715399dcc6aefee2c6a631b23c20eb23`;
  repeat SHA-256
  `b297df24c3263b2df9057bb09820db48435a38625be0bf4cffac393dfa1804cc`.
- Budget checks: `pipeline-budget-check-after-next-wave.json` (81;
  SHA-256 `97394ddb6d3d3f8cdc8c997e906aa5e609d06abd90ccee27e64756a539b17f02`)
  and `pipeline-budget-check-after-next-wave-repeat.json` (81;
  SHA-256 `dca1bb65d06c85f03a3ec24ba89e72abaff1efe6b4a89ce131af6967cb1718e4`).
- Reports: `pipeline-performance-report-after-next-wave-repeat.json`,
  `pipeline-benchmark-coverage-after-next-wave-repeat.json`, and
  `pipeline-roadmap-status-after-next-wave-repeat.json` (roadmap: 14 closed,
  50 open, 100% operation coverage).
- Full parity: `build/migration-parity/all-backends-after-next-wave.json`
  (SHA-256
  `fcf35f32347d64d3916000d0f93bb363e981139fb438e75a17f49855defcb307`).

## Required checks before publishing

- `make fmt`
- `RUSTC_WRAPPER= make clippy`
- `make repo-map-check`
- `make migration-parity-fixtures-check`
- `make migration-parity-evidence-check`
- `git diff --check`
