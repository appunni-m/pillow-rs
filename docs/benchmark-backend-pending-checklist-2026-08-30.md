# Active benchmark/backend checklist

This is the short work list for the active audit goal. Parity is complete for
the standard corpus, and every item below is a measured performance
follow-up.

## Current state

- Standard benchmark: **744/744** workloads measured with complete parity
  receipts and 100% operation coverage.
- Performance gate: **open**. The first post-wave comparison found 63
  violations; the same-source repeat found **65** of 2,976 comparable rows.
  The count is timing-sensitive, so both receipts remain evidence until a
  stable budget result is obtained.
- Goal state: **active**, pending the speed gate and final verification. Do
  not change fixtures, thresholds, workload denominators, or backend labels.

## Pending, in execution order

- [ ] **Close the budget gate.** Use paired, actual-backend measurements against
  `final-standard-after-native-geometry.json`; reduce the repeat comparison to
  zero violations without weakening the comparator. Re-run the standard
  benchmark after each accepted optimization.
- [ ] **CPU subject rows (15 in the latest repeat):**
  `pil-image-image.split.standard`,
  `pipeline-chain.blur-material.box-l-1024x768-radius-4`,
  `pipeline-chain.blur-material.box-rgba-256x256-radius-2`,
  `pipeline-chain.long-point.invert-64`,
  `pipeline-chain.point-fusion.l-003`,
  `pipeline-chain.resize-alpha.rgba-bilinear-mirror-256x256`,
  `pipeline-chain.reviewed.draw-batch-rgba-alpha`,
  `pipeline-chain.simd-crossover.invert-mirror.256x256`,
  `pipeline-chain.simd-vector-mirror.l.1024x1024`,
  `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768`,
  `pipeline-chain.terminal-read.analysis-suite.rgb`,
  `pipeline-matrix.expanded.crop.256x256`,
  `pipeline-matrix.expanded.pointop.1x1`,
  `pipeline-op.difference.matrix-32x24`,
  `pipeline-op.transform.matrix-32x24`.
- [ ] **SIMD subject rows (24):**
  `pil-image-image.getcolors.standard`,
  `pipeline-chain.alpha-composite.rgba-256x256`,
  `pipeline-chain.color.getchannel-mode-ycbcr`,
  `pipeline-chain.long-point.invert-64`,
  `pipeline-chain.matrix-002`, `pipeline-chain.matrix-059`,
  `pipeline-chain.rank-filter.large-l-9x9`,
  `pipeline-chain.reviewed.draw-filter-invert`,
  `pipeline-chain.simd-lut.l.1024x768`,
  `pipeline-chain.terminal-read.analysis-suite.rgb`,
  `pipeline-lifecycle.cold.gaussianblur-invert.rgb-1024`,
  `pipeline-matrix.expanded.autocontrast.1024x768`,
  `pipeline-matrix.expanded.autocontrast.256x256`,
  `pipeline-matrix.expanded.autocontrast.32x32`,
  `pipeline-matrix.expanded.convert.1024x768`,
  `pipeline-matrix.expanded.maxfilter.256x256`,
  `pipeline-matrix.expanded.reduce.256x256`,
  `pipeline-matrix.expanded.rotate.256x256`,
  `pipeline-matrix.expanded.screen.256x256`,
  `pipeline-op.cropborder.benchmark-materialized`,
  `pipeline-op.drawpieslice.matrix-32x24`,
  `pipeline-op.expand.matrix-32x24`,
  `pipeline-op.pointop.benchmark-materialized`,
  `pipeline-op.quantize.matrix-32x24`.
- [ ] **GPU subject rows (9):**
  `pil-imagefilter.kernel.standard`, `pil-imagefilter.maxfilter.standard`,
  `pil-imagefilter.medianfilter.standard`,
  `pil-imagefilter.minfilter.standard`, `pil-imagefilter.modefilter.standard`,
  `pil-imagefilter.rankfilter.standard`, `pil-imagefilter.sharpen.standard`,
  `pil-imagefilter.smooth.standard`,
  `pipeline-lifecycle.resident.multiply-screen.rgb-1024`.
- [ ] **Pillow subject rows (17):**
  `pil-imagefont.imagefont.standard`, `pil-imagesequence.iterator.standard`,
  `pipeline-chain.matrix-006`, `pipeline-chain.point-fusion.l-003`,
  `pipeline-chain.terminal-read.analysis-scalar-if-1024x768`,
  `pipeline-chain.terminal-read.getcolors.rgb-1024x768`,
  `pipeline-chain.terminal-read.imagestat.i-1024x768`,
  `pipeline-matrix.expanded.autocontrast.256x256`,
  `pipeline-matrix.expanded.boxblur.256x256`,
  `pipeline-matrix.expanded.brightness.256x256`,
  `pipeline-matrix.expanded.darker.256x256`,
  `pipeline-matrix.expanded.equalize.1024x768`,
  `pipeline-matrix.expanded.equalize.256x256`,
  `pipeline-matrix.expanded.minfilter.32x32`,
  `pipeline-op.blendmodule.benchmark-materialized`,
  `pipeline-op.drawpolygon.matrix-32x24`,
  `pipeline-op.drawrectangle.matrix-32x24`.
- [x] **Full all-backends parity:** CPU, SIMD, GPU, Node WASM, and browser WASM
  each passed 10,952/10,952, with the GPU smoke case at 1/1. The PA resize
  regression found in the first run was fixed before this receipt.
- [x] **Remaining maintained gates:** `make fmt`, `RUSTC_WRAPPER= make clippy`,
  `make repo-map-check`, `make migration-parity-fixtures-check`, and
  `git diff --check` pass. Their results are recorded with the receipts below.
- [ ] **Update and publish the goal state:** keep this checklist current,
  commit only verified source/docs changes, and push `main` after the budget
  and all-backends receipts are recorded.

## Closed in the current wave

- [x] CPU uniform native filters (Min/Max, 3x3, 5x5), strict parity 72/72:
  `8d5c1d9ef`.
- [x] GPU constant packed blur identity dispatch, strict blur parity 17/17;
  paired 1024x768 Gaussian runs improved 36.9–38.2%:
  `a2e97994a`.
- [x] SIMD uniform neighborhoods and zero-image resize, strict filters 11/11
  and resize 7/7: `c831bc0e0`.
- [x] SIMD identity LUT traversal elision, paired invert-chain improvement
  70.39–71.58%, strict parity 1/1: `b73e57442`.
- [x] CPU zero/constant resize fast paths for contain, cover, and finite
  constant-F cases, strict parity 23/23: `26b5f9376`.
- [x] SIMD zero-resize now requires every stored channel to be zero, preserving
  PA/LA alpha bytes; the focused regression and strict full corpus passed:
  `40c28f53d`.

## Latest receipts

- Standard wave: `build/migration-parity/final-standard-after-budget-wave.json`
  (744/744; SHA-256
  `06f14099dd9a94eb32ba825de7c5c69e44fcba9f7ca11b68cab4f2fb4fca44d8`).
- Same-source repeat:
  `build/migration-parity/final-standard-after-budget-wave-repeat.json`
  (744/744; SHA-256
  `555dc71aa3d56b8077c192c9e037df5b3c92c0e5b43832ab4ee8860a8a33b0c5`).
- Repeat parity sidecar:
  `build/migration-parity/final-standard-after-budget-wave-repeat-parity.json`
  (202/202; SHA-256
  `f779fabad94e7ce576c802fb204ddd66bee6ebfd042b0d84b55a337aae2c74a5`).
- Budget wave: `pipeline-budget-check-after-budget-wave.json` (63; SHA-256
  `5ebc07927d88a052ccf0c15add5bc9cffa95c16296773c163db09be28dccda90`).
- Budget repeat: `pipeline-budget-check-after-budget-wave-repeat.json` (65;
  SHA-256 `4e2c1a5f13a1b81b49ece00228f8491849571055b64993ba704888a558cfa94b`).
- Coverage/report/roadmap wave receipts:
  `pipeline-benchmark-coverage-after-budget-wave.json` (SHA-256
  `5f24bcf89e2622850c90935e72d73177c85afb1ae3da7fd74d0929d336476008`),
  `pipeline-performance-report-after-budget-wave.json` (SHA-256
  `59f61923d11c21b5d1d44555676000016d0eb5a091d4b2205aa8a300059fe9a1`), and
  `pipeline-roadmap-status-after-budget-wave.json` (SHA-256
  `a65d4aeed3cdfd5897991f882e2036903eff6249c9dccb5d4de7fe29eaa76acf`).
- All-backends receipt:
  `build/migration-parity/all-backends-after-budget-wave-fixed.json`, CPU,
  SIMD, GPU, Node WASM, and browser WASM 10,952/10,952 plus GPU smoke 1/1
  (SHA-256
  `205d7b7a13af972ebb4fb0aeaf3d0c464e27afb642b3bac1703997ad4acb80f2`).
