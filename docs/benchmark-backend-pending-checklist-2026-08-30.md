# Benchmark/backend pending checklist

This is the focused follow-up list for the active audit goal. Historical audit
rows are not pending work. Parity is closed; the remaining work is performance
evidence, integration, and delivery.

## Pending

- [ ] **Integrate verified changes.** Merge the SIMD convolution change and GPU
  RGB staging change into `main`; keep the exact host-control and fallback
  telemetry unchanged.
- [ ] **Rebuild the final evidence.** Run the maintained all-backends gate and
  standard 744-workload benchmark after integration. Regenerate the performance,
  benchmark-coverage, and roadmap reports from those post-integration artifacts.
- [ ] **Recompute the performance gate.** Compare equal-ID, actual-backend
  receipts against the pinned baseline. Current guarded comparison: 577
  violations (510 GPU, 33 CPU, 19 SIMD, 15 Pillow); do not edit thresholds or
  fixtures to reduce this count.
- [ ] **Close or classify the remaining speed gaps.** Measure the GPU crop and
  convolution/chain cohorts with paired actual-GPU receipts, then either land a
  verified regression fix or record the remaining gap as an open performance
  item with a reproducible artifact.
- [ ] **Deliver.** Run `make fmt`, `make clippy`, `make repo-map-check`,
  `git diff --check`, and the relevant parity gates; commit only intentional
  source/docs changes and push `main`.

## Already closed (do not reopen)

- Strict SIMD parity: 10,952/10,952.
- Strict GPU parity: 10,952/10,952.
- Combined CPU/SIMD/GPU/Node/browser parity: 10,952/10,952 per lane.
- Standard benchmark correctness: 744/744 workloads, zero failures and
  not-run records.

## Current verified inputs

- SIMD focused benchmark: `/private/tmp/simd-5x5-direct-vector-benchmark.json`.
- SIMD strict parity: `/private/tmp/simd-strict-conv-direct.json`.
- GPU worker commit: `d5af68d3c66b6134c4277b2d77aa05fb3e20b8ec`.
- SIMD worker commit: `08b1fb4b7` (the final source change is being integrated
  into the root worktree before delivery).
