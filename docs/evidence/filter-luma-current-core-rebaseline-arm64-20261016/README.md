# Filter5x5 L current-core CPU rebaseline — ARM64, 2026-10-16

This refresh checks the retained native-L 5×5 vertical-total path against the
current extension. It makes no source change and is a dirty-tree, host-local
result.

## Frozen scope and parity

- Workload: `pipeline-chain.convolution.native.l-5x5-scale.1024x768`.
- Exact parity case: `convolution-uniform-color-trial-20261014.workflow.native-l-scale-1024`.
- Profile: release, default serial CPU features, warm cache, concurrency 1.
- Boundary: `whole_workflow`; setup, Filter5x5, Scale, and terminal `tobytes`
  materialization are timed together.
- Policy: 1 warmup, 3 iterations × 2 samples. The fixture workload and exact
  parity case have identical steps and observations.
- Exact Pillow parity passed 1/1. The execution proof loaded the same current
  `_core` binary as the timed target run.

The benchmark workflow declares `successful_execution` as its built-in
correctness gate, so its generated parity report selected zero cases. I ran the
one exact parity case separately with the dedicated convolution manifest; do
not treat the empty generated gate as parity evidence.

## Result

| Subject | Median latency | P95 latency | Median throughput |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 5.077771 ms | 5.123917 ms | 196.94 calls/s |
| pillow-rs serial CPU | 1.680147 ms | 2.026542 ms | 595.19 calls/s |

The current source measured 3.022× Pillow by median latency. Its receipt proves
requested and actual CPU execution, terminal completion, two operations
(`Filter5x5` and `Scale`), and no fallback. The whole-workflow receipt reports
3 host buffers / 3,342,336 host-buffer bytes and 2,555,904 peak live bytes.
Separate host-allocation counters report zero and do not capture every
allocation. Concurrency-one throughput is reciprocal latency, not sustained
throughput.

This rebaseline is distinct from the earlier vertical-total A–B–B–A cohort:
that candidate used `filter.rs` SHA
`4f174d73233dbdf74c4a2d6d7e91b0bc2c5e18bd49bddb0dd32cb2218cb95dbf` and
`_core` SHA `88b86723994eb2ac0f15cea0424215bc47e5d5a9f8fe201e677c339e49c1132d`.
The current run uses `filter.rs` SHA
`e224b1e8e3d9ac60df9512a3b011fa64a7fdb056f298651670e4861e2848da68` and
`_core` SHA `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`.
Do not combine the old and current cohorts. This current result clears the
serial-CPU 2× sequencing checkpoint for this single workflow only; it does not
complete Filter5x5 across other modes, kernels, sizes, SIMD, Parallel CPU, GPU,
or runners.

## Identity and artifacts

- Host: macOS 15.7.7 ARM64, CPython 3.12.13; Pillow 12.2.0.
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty).
- Benchmark: `migration-benchmark-9f5d2de89bd8423d9e7c767e2916f1a4`.
- Exact parity: `migration-parity-044e0f3dbfa64bf59d14cf7c421c25f3` (1/1).
- Focused release build: `make build-parity` succeeded; it emitted the existing
  seven missing-documentation warnings in `pillow-rs-py`.

The benchmark report, empty generated correctness-gate report, separate exact
parity report, producer and execution attestations, raw samples, and checksums
are retained in this directory. No code, workload, or parity fixture changed;
no broad benchmark/parity campaign, test suite, coverage, commit, or push ran.
This rebaseline adds no completed item to the 33-item commit batch.
