# Benchmark/backend pending checklist

This is the focused checklist for the active audit goal. No operation is
excluded; parity is closed. The only open gate is measured performance budget
work.

## Pending

- [ ] **Close the remaining speed budget.** The latest same-source repeat has
  **70** violations across **2,976** comparable rows: 19 CPU, 20 SIMD, 24
  Pillow, and 7 GPU. Work the buckets in that order using paired,
  actual-backend measurements; do not change thresholds, fixtures, or workload
  denominators. The immediately preceding run measured 238, so both receipts
  stay visible as timing-variance evidence.
- [ ] **Recheck the four concrete buckets.** CPU: `minfilter`, `contain`,
  `filter3x3/filter5x5`, and `cover` rows. SIMD: `matrix-096`, median/resize,
  and the reviewed resize/draw chains. GPU: `matrix-058`, the 1024×768
  Gaussian chain, and the remaining standard rows. Pillow: `getcolors`,
  overlay/putdata, and the remaining comparator cohort.
- [ ] **Recompute the budget after each accepted optimization.** Keep the
  latest benchmark, comparison, and hash in this checklist so the count is
  reproducible and historical timing snapshots do not become the work list.
- [x] **Deliver the committed evidence.** Source and documentation commits are
  pushed to `origin/main`; unrelated generated/user files remain unstaged.

## Closed in this pass

- [x] CPU BoxBlur now returns an exact clone for constant images up to 64x64;
  the 32x32 target improved 67.53% whole-workflow and 83.02% in the CPU
  backend. BoxBlur and GaussianBlur parity remained 63/63.
- [x] SIMD Reduce serializes outputs below 1,024 pixels to remove row-task
  overhead. The 16x16, 32x24, and 32x32 targets improved to 0.014229,
  0.014646, and 0.015229 ms; focused strict SIMD parity passed 14/14.
- [x] GPU point fusion now handles explicit native L/LA/RGB/RGBA byte modes;
  the 1,024-dispatch L invert chain became one dispatch and improved 36.6-48.8%
  in paired Metal runs. Focused strict GPU parity passed 1/1.
- [x] SIMD Equalize identity, grouped SIMD convolution, direct RGB upload,
  and bounded readback polling are integrated with exact parity receipts.
- [x] The committed wasm feature gate for the SIMD threshold was verified by
  `make build-wasm-core`; Node and browser lanes then passed the aggregate gate.
- [x] CPU fused point chains apply native L/LA/RGB/RGBA byte LUTs without the
  old RGBA widening round trip.  The LA-002 CPU median improved 27.09%; strict
  point parity passed 6/6.
- [x] SIMD constant Max/Min and Box/Gaussian neighborhoods now use an exact
  native copy after a bounded uniform-image scan.  Paired targets improved
  83.94–97.32%; focused strict parity passed 6/6 and full strict parity
  passed 10,952/10,952.
- [x] GPU readbacks up to 64 KiB now use eight bounded 50 µs polls before the
  existing 1 ms backoff.  Paired actual-Metal crop targets improved 60–86%;
  strict GPU parity passed 2/2.
- [x] The final all-backends receipt passes CPU, SIMD, full GPU, Node WASM, and
  browser WASM at **10,952/10,952** each, plus the GPU smoke case at 1/1.
- [x] Maintained checks pass: `make fmt`, `make clippy`,
  `make repo-map-check`, `make migration-parity-fixtures-check`, and
  `git diff --check`.

## Current receipts

- Standard benchmark: `build/migration-parity/final-standard-after-budget-probes-repeat.json`
  (744/744 measured; SHA-256
  `fbb44f4b5418f7485f733ce250abacd3dac660ce67a583895ded45c8ec4f3ffc`).
- Benchmark parity sidecar: `build/migration-parity/final-standard-after-budget-probes-repeat-parity.json`
  (202/202; SHA-256
  `8d377ca7d332030fa70b450bdbb1711a1d3f1873a60cebe2f56a2e44a62c7e6b`).
- Budget comparison: `build/migration-parity/pipeline-budget-check-after-budget-probes-repeat.json`
  (70 violations; SHA-256
  `21b5d0f80049c7ed8154646d37c009de44eb7d4ca05375b54ec70a61866f6f2b`).
- Variance control: `build/migration-parity/pipeline-budget-check-after-budget-probes.json`
  (238 violations; SHA-256
  `414c99d9ca9076989d2cafc631617e3d55d20248286ddbf9504ad31362beafc7`).
- Performance report: `build/migration-parity/pipeline-performance-report-after-budget-probes-repeat.json`
  (SHA-256
  `e10b0d56e847f7b55149569861767fd8e8da4ee7680ca95bbbec23733634b8ae`).
- Coverage/roadmap receipts:
  `pipeline-benchmark-coverage-after-budget-probes-repeat.json` (SHA-256
  `b466f4fe4387f1fc33e996281fb3947af26c338403e315fc68f89facbceec7f2`) and
  `pipeline-roadmap-status-after-budget-probes-repeat.json` (SHA-256
  `b47be66c44f915171d26c6dd18ffecc844c2f2a61086d054c2ea5cd31e4afd59`).
- All-backends receipt: `build/migration-parity/all-backends-after-budget-probes.json`
  (CPU/SIMD/GPU/Node/browser 10,952/10,952 each, GPU smoke 1/1; SHA-256
  `f3588a552bdffadb1f69b910873ecf9c565c3dfd3911a8fd2a7d4ade4832c6bd`).

The active goal remains open only because the 70-row performance budget is
not yet closed. Do not reopen parity work unless a new exact byte/value/error
divergence is observed.
